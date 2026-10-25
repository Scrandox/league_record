use std::time::Duration;

use anyhow::{bail, Context, Result};
use riot_datatypes::lcu::{Game, Player, Stats};
use riot_datatypes::{Champion, MatchId, ParticipantId, Queue, Timeline};
use shaco::rest::LcuRestClient;
use tokio::{time::sleep, try_join};
use tokio_util::sync::CancellationToken;

use super::{GameEvent, GameMetadata, MetadataFile};
use crate::cancellable;
use crate::library::RecordingLibrary;

/// How often to retry fetching game data from the LCU before giving up.
#[derive(Debug, Clone, Copy)]
pub struct Retry {
    pub attempts: u32,
    pub delay: Duration,
}

impl Retry {
    /// A single attempt, for data that should already be there.
    pub const NONE: Retry = Retry {
        attempts: 1,
        delay: Duration::ZERO,
    };
    /// Right after a game the client needs up to a minute until match history is available.
    pub const GAME_END: Retry = Retry {
        attempts: 60,
        delay: Duration::from_secs(1),
    };
}

// manually fill data for swarm champions because the client somehow doesn't have info on them
// https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/v1/champion-summary.json
const SWARM_CHAMPIONS: &[(i64, &str)] = &[
    (3147, "Riven"),
    (3151, "Jinx"),
    (3152, "Leona"),
    (3153, "Seraphine"),
    (3156, "Briar"),
    (3157, "Yasuo"),
    (3159, "Aurora"),
    (3678, "Illaoi"),
    (3947, "Xayah"),
];

/// Completes a `Deferred` metadata file by fetching the game data and upgrading the file in
/// place. Files that aren't deferred are returned unchanged.
pub async fn resolve_deferred(
    library: &RecordingLibrary,
    video_id: &str,
    client: &LcuRestClient,
    retry: Retry,
    cancel_token: &CancellationToken,
) -> Result<MetadataFile> {
    let metadata_file = library.read_metadata(video_id)?;
    let MetadataFile::Deferred(deferred) = &metadata_file else {
        return Ok(metadata_file);
    };

    let game_metadata = fetch_game_metadata(
        client,
        deferred.ingame_time_rec_start_offset,
        deferred.match_id.clone(),
        retry,
        cancel_token,
    )
    .await?;

    // re-read right before writing so a favorite toggled while we were fetching isn't lost
    let upgraded = library.read_metadata(video_id)?.upgrade(game_metadata);
    library.write_metadata(video_id, &upgraded)?;
    Ok(upgraded)
}

pub async fn fetch_game_metadata(
    client: &LcuRestClient,
    ingame_time_rec_start_offset: f64,
    match_id: MatchId,
    retry: Retry,
    cancel_token: &CancellationToken,
) -> Result<GameMetadata> {
    let (player, game, timeline) = fetch_match_history(client, &match_id, retry, cancel_token).await?;

    let participant_id = game
        .participant_identities
        .iter()
        .find(|pi| pi.player == player)
        .map(|pi| pi.participant_id)
        .context("player not found in game info")?;

    let participant = game
        .participants
        .into_iter()
        .find(|p| p.participant_id == participant_id)
        .context("player participant_id not found in game info")?;

    let queue = fetch_queue(client, game.queue_id).await?;

    let champion_name = match SWARM_CHAMPIONS.iter().find(|(id, _)| *id == participant.champion_id) {
        Some((_, name)) => (*name).to_owned(),
        None => {
            client
                .get::<Champion>(format!(
                    "/lol-champions/v1/inventories/{}/champions/{}",
                    player.summoner_id.context("player has no summoner_id")?,
                    participant.champion_id
                ))
                .await?
                .name
        }
    };

    Ok(shape_metadata(
        ingame_time_rec_start_offset,
        match_id,
        player,
        queue,
        champion_name,
        participant_id,
        participant.stats,
        timeline,
    ))
}

async fn fetch_match_history(
    client: &LcuRestClient,
    match_id: &MatchId,
    retry: Retry,
    cancel_token: &CancellationToken,
) -> Result<(Player, Game, Timeline)> {
    let mut player_game = None;
    let mut timeline = None;

    let attempts = retry.attempts.max(1);
    for attempt in 1..=attempts {
        player_game = try_join!(
            client.get::<Player>("/lol-summoner/v1/current-summoner"),
            client.get::<Game>(format!("/lol-match-history/v1/games/{}", match_id.game_id)),
        )
        .ok();

        timeline = client
            .get::<Timeline>(format!("/lol-match-history/v1/game-timelines/{}", match_id.game_id))
            .await
            .ok();

        if player_game.is_some() && timeline.is_some() {
            break;
        }

        if attempt < attempts {
            let cancelled = cancellable!(sleep(retry.delay), cancel_token, ());
            if cancelled {
                bail!("task cancelled (fetch_game_metadata)");
            }
        }
    }

    let Some((player, game)) = player_game else { bail!("unable to collect game data") };
    // the timeline is optional: without it there are just no event markers
    Ok((player, game, timeline.unwrap_or_default()))
}

async fn fetch_queue(client: &LcuRestClient, queue_id: i64) -> Result<Queue> {
    Ok(match queue_id {
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
        id => client.get::<Queue>(format!("/lol-game-queues/v1/queues/{id}")).await?,
    })
}

#[allow(clippy::too_many_arguments)]
fn shape_metadata(
    ingame_time_rec_start_offset: f64,
    match_id: MatchId,
    player: Player,
    queue: Queue,
    champion_name: String,
    participant_id: ParticipantId,
    stats: Stats,
    timeline: Timeline,
) -> GameMetadata {
    let events: Vec<GameEvent> = timeline
        .frames
        .into_iter()
        .flat_map(|frame| frame.events.into_iter().filter_map(|event| event.try_into().ok()))
        .collect();

    GameMetadata {
        favorite: false,
        match_id,
        ingame_time_rec_start_offset,
        highlights: vec![],
        queue,
        player,
        champion_name,
        stats,
        participant_id,
        events,
    }
}
