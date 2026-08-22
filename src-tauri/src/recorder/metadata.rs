use std::time::Duration;

use anyhow::{bail, Context, Result};
use riot_datatypes::lcu::{Game, Participant, Player};
use riot_datatypes::{Champion, MatchId, Queue, SummonerId, Timeline};
use riot_local_auth::Credentials;
use serde::Deserialize;
use shaco::rest::LcuRestClient;
use tokio::{time::sleep, try_join};
use tokio_util::sync::CancellationToken;

use super::{GameEvent, GameMetadata, Runes};
use crate::cancellable;

pub async fn process_data(ingame_time_rec_start_offset: f64, match_id: MatchId) -> Result<GameMetadata> {
    let lcu_rest_client = LcuRestClient::new()?;

    let (player, game) = try_join!(
        lcu_rest_client.get::<Player>("/lol-summoner/v1/current-summoner"),
        lcu_rest_client.get::<Game>(format!("/lol-match-history/v1/games/{}", match_id.game_id)),
    )?;
    let timeline = lcu_rest_client
        .get::<Timeline>(format!("/lol-match-history/v1/game-timelines/{}", match_id.game_id))
        .await
        .unwrap_or_default();

    build_metadata(
        &lcu_rest_client,
        ingame_time_rec_start_offset,
        match_id,
        player,
        game,
        timeline,
    )
    .await
}

pub async fn process_data_with_retry(
    ingame_time_rec_start_offset: f64,
    match_id: MatchId,
    credentials: &Credentials,
    cancel_token: &CancellationToken,
) -> Result<GameMetadata> {
    let lcu_rest_client = LcuRestClient::from(credentials);

    let mut player_info = None;
    let mut timeline_data = None;
    for _ in 0..60 {
        player_info = try_join!(
            lcu_rest_client.get::<Player>("/lol-summoner/v1/current-summoner"),
            lcu_rest_client.get::<Game>(format!("/lol-match-history/v1/games/{}", match_id.game_id)),
        )
        .ok();

        timeline_data = lcu_rest_client
            .get::<Timeline>(format!("/lol-match-history/v1/game-timelines/{}", match_id.game_id))
            .await
            .ok();

        if player_info.is_some() && timeline_data.is_some() {
            break;
        }

        let cancelled = cancellable!(sleep(Duration::from_secs(1)), cancel_token, ());
        if cancelled {
            bail!("task cancelled (process_data)");
        }
    }

    let Some((player, game)) = player_info else { bail!("unable to collect game data") };
    let timeline = timeline_data.unwrap_or_default();

    build_metadata(
        &lcu_rest_client,
        ingame_time_rec_start_offset,
        match_id,
        player,
        game,
        timeline,
    )
    .await
}

async fn build_metadata(
    lcu_rest_client: &LcuRestClient,
    ingame_time_rec_start_offset: f64,
    match_id: MatchId,
    player: Player,
    game: Game,
    timeline: Timeline,
) -> Result<GameMetadata> {
    let queue = match game.queue_id {
        -1 => Queue {
            id: -1,
            name: "Practicetool".into(),
            is_ranked: false,
        },
        0 => Queue {
            id: 0,
            name: "Custom Game".into(),
            is_ranked: false,
        },
        id => {
            lcu_rest_client
                .get::<Queue>(format!("/lol-game-queues/v1/queues/{id}"))
                .await?
        }
    };

    let participant_id = game
        .participant_identities
        .iter()
        .find(|pi| pi.player == player)
        .map(|pi| pi.participant_id)
        .context("player not found in game info")?;

    let participant = game
        .participants
        .iter()
        .find(|p| p.participant_id == participant_id)
        .context("player participant_id not found in game info")?;

    let summoner_id = player.summoner_id.context("current summoner has no summoner_id")?;
    let champion_name = resolve_champion_name(lcu_rest_client, summoner_id, participant.champion_id).await?;

    // best-effort extras - never fail metadata collection because of them
    let enemy_champion_name = match find_lane_opponent(&game.participants, participant) {
        Some(enemy) => match resolve_champion_name(lcu_rest_client, summoner_id, enemy.champion_id).await {
            Ok(name) => Some(name),
            Err(e) => {
                log::warn!("failed to resolve enemy champion name: {e}");
                None
            }
        },
        None => None,
    };
    let summoner_spells = summoner_spell_names(lcu_rest_client, participant)
        .await
        .unwrap_or_else(|e| {
            log::warn!("failed to resolve summoner spell names: {e}");
            vec![]
        });
    let runes = rune_names(lcu_rest_client, participant).await.unwrap_or_else(|e| {
        log::warn!("failed to resolve rune names: {e}");
        Runes::default()
    });

    let stats = participant.stats.clone();

    let events: Vec<GameEvent> = timeline
        .frames
        .into_iter()
        .flat_map(|frame| frame.events.into_iter().filter_map(|event| event.try_into().ok()))
        .collect();

    Ok(GameMetadata {
        favorite: false,
        match_id,
        ingame_time_rec_start_offset,
        highlights: vec![],
        queue,
        player,
        champion_name,
        enemy_champion_name,
        summoner_spells,
        runes,
        stats,
        participant_id,
        events,
    })
}

async fn resolve_champion_name(
    lcu_rest_client: &LcuRestClient,
    summoner_id: SummonerId,
    champion_id: riot_datatypes::ChampionId,
) -> Result<String> {
    // manually fill data for swarm champions because the client somehow doesn't have info on them
    // https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/v1/champion-summary.json
    Ok(match champion_id {
        3147 => "Riven".into(),
        3151 => "Jinx".into(),
        3152 => "Leona".into(),
        3153 => "Seraphine".into(),
        3156 => "Briar".into(),
        3157 => "Yasuo".into(),
        3159 => "Aurora".into(),
        3678 => "Illaoi".into(),
        3947 => "Xayah".into(),
        _ => {
            lcu_rest_client
                .get::<Champion>(format!(
                    "/lol-champions/v1/inventories/{summoner_id}/champions/{champion_id}"
                ))
                .await?
                .name
        }
    })
}

/// find the enemy participant in the same lane
/// on botlane prefer matching the role too (carry vs carry, support vs support)
fn find_lane_opponent<'a>(participants: &'a [Participant], me: &Participant) -> Option<&'a Participant> {
    let my_team = me.team_id.clone()?;
    if me.timeline.lane.is_empty() || me.timeline.lane == "NONE" {
        return None;
    }

    let is_enemy_in_lane = |p: &&Participant| {
        p.team_id.as_ref().is_some_and(|t| *t != my_team) && p.timeline.lane == me.timeline.lane
    };

    participants
        .iter()
        .find(|p| is_enemy_in_lane(p) && p.timeline.role == me.timeline.role)
        .or_else(|| participants.iter().find(is_enemy_in_lane))
}

#[derive(Deserialize)]
struct IdName {
    id: i64,
    name: String,
}

async fn summoner_spell_names(lcu_rest_client: &LcuRestClient, participant: &Participant) -> Result<Vec<String>> {
    let spells = lcu_rest_client
        .get::<Vec<IdName>>("/lol-game-data/assets/v1/summoner-spells.json")
        .await?;

    let name_of = |id: i64| {
        spells
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.name.clone())
            .unwrap_or_else(|| format!("Spell {id}"))
    };

    Ok(vec![name_of(participant.spell1_id), name_of(participant.spell2_id)])
}

async fn rune_names(lcu_rest_client: &LcuRestClient, participant: &Participant) -> Result<Runes> {
    let (perks, styles) = try_join!(
        lcu_rest_client.get::<Vec<IdName>>("/lol-perks/v1/perks"),
        lcu_rest_client.get::<Vec<IdName>>("/lol-perks/v1/styles"),
    )?;

    let stats = &participant.stats;
    let style_name = |id: i64| {
        styles
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.name.clone())
            .unwrap_or_else(|| format!("Style {id}"))
    };
    let perk_name = |id: i64| {
        perks
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| format!("Perk {id}"))
    };

    Ok(Runes {
        primary_style: style_name(stats.perk_primary_style),
        sub_style: style_name(stats.perk_sub_style),
        perks: [stats.perk0, stats.perk1, stats.perk2, stats.perk3, stats.perk4, stats.perk5]
            .into_iter()
            .map(perk_name)
            .collect(),
    })
}
