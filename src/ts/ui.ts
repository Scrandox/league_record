import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import * as clipboard from "@tauri-apps/plugin-clipboard-manager";

import { commands, type GameMetadata, type MarkerFlags, type Recording, type RecordingState } from "../bindings";
import { toVideoId, toVideoName, isFavorite, isRanked, resultOf, kdaOf, championOf } from "./util";

const appWindow = getCurrentWebviewWindow();

// text glyphs (escaped so the source stays ASCII)
const STAR = "\u2605";
const STAR_OUTLINE = "\u2606";
const PENCIL = "\u270E";
const CROSS = "\u2715";
const DOT = " \u00B7 ";
const DASH = "\u2013";

type FilterTab = "all" | "favorites" | "ranked";
type ViewMode = "list" | "grid";

type TimelineEvent = { timestamp: number; name: string; markerClass: string };

// maps a marker class to its --m-* token for the timeline dialog color bars
const MARKER_TOKEN: Record<string, string> = {
    kill: "--m-kill",
    death: "--m-death",
    assist: "--m-assist",
    turret: "--m-structure",
    inhibitor: "--m-structure",
    voidgrub: "--m-herald",
    herald: "--m-herald",
    atakhan: "--m-atakhan",
    baron: "--m-baron",
    "infernal-dragon": "--m-dragon",
    "ocean-dragon": "--m-dragon",
    "mountain-dragon": "--m-dragon",
    "cloud-dragon": "--m-dragon",
    "hextech-dragon": "--m-dragon",
    "chemtech-dragon": "--m-dragon",
    "elder-dragon": "--m-dragon",
    dragon: "--m-dragon",
    highlight: "--m-highlight",
};

function el<K extends keyof HTMLElementTagNameMap>(
    tag: K,
    attrs: Record<string, string> = {},
    children: Array<Node | string> = [],
): HTMLElementTagNameMap[K] {
    const element = document.createElement(tag);
    for (const [key, value] of Object.entries(attrs)) {
        element.setAttribute(key, value);
    }
    element.append(...children);
    return element;
}

export default class UI {
    private readonly modal;
    private readonly modalContent;
    private readonly sidebar;
    private readonly gridView;
    private readonly playerView;
    private readonly videoFolderBtn;
    private readonly searchInput;
    private readonly filterTabs;
    private readonly viewListBtn;
    private readonly viewGridBtn;
    private readonly statePill;
    private readonly statePillText;
    private readonly statusMeta;
    private readonly statusTotals;
    private readonly markerCount;
    private readonly markerLists;

    private readonly checkboxKill;
    private readonly checkboxDeath;
    private readonly checkboxAssist;
    private readonly checkboxStructure;
    private readonly checkboxDragon;
    private readonly checkboxHerald;
    private readonly checkboxBaron;

    private readonly showTimestampsButton;

    // view-local state (see DESIGN.md: filtering/search/view never leave the frontend)
    private recordings: ReadonlyArray<Recording> = [];
    private recordingsSizeGb = 0;
    private searchQuery = "";
    private filterTab: FilterTab = "all";
    private viewMode: ViewMode = "list";
    private activeVideoId: string | null = null;

    // resolves a videoId to a playable URL for grid thumbnails (set once main.ts knows the recordings path)
    private thumbnailSrc: ((videoId: string) => string) | null = null;

    private onVideo: (videoId: string) => void = () => {};
    private onFavorite: (videoId: string) => Promise<boolean | null> = () => Promise.resolve(null);
    private onRename: (videoId: string) => void = () => {};
    private onDelete: (videoId: string) => void = () => {};

    constructor() {
        this.modal = document.querySelector<HTMLDivElement>("#modal")!;
        this.modalContent = document.querySelector<HTMLDivElement>("#modal-content")!;
        this.sidebar = document.querySelector<HTMLUListElement>("#sidebar-content")!;
        this.gridView = document.querySelector<HTMLDivElement>("#grid-view")!;
        this.playerView = document.querySelector<HTMLDivElement>("#player-view")!;
        this.videoFolderBtn = document.querySelector<HTMLButtonElement>("#vid-folder-btn")!;
        this.searchInput = document.querySelector<HTMLInputElement>("#search-input")!;
        this.filterTabs = Array.from(document.querySelectorAll<HTMLButtonElement>("#filter-tabs .lr-tab"));
        this.viewListBtn = document.querySelector<HTMLButtonElement>("#view-list")!;
        this.viewGridBtn = document.querySelector<HTMLButtonElement>("#view-grid")!;
        this.statePill = document.querySelector<HTMLDivElement>("#state-pill")!;
        this.statePillText = document.querySelector<HTMLSpanElement>("#state-pill-text")!;
        this.statusMeta = document.querySelector<HTMLDivElement>("#status-meta")!;
        this.statusTotals = document.querySelector<HTMLSpanElement>("#status-totals")!;
        this.markerCount = document.querySelector<HTMLSpanElement>("#marker-count")!;
        this.markerLists = document.querySelector<HTMLDivElement>("#marker-lists")!;

        this.checkboxKill = document.querySelector<HTMLInputElement>("#kill")!;
        this.checkboxDeath = document.querySelector<HTMLInputElement>("#death")!;
        this.checkboxAssist = document.querySelector<HTMLInputElement>("#assist")!;
        this.checkboxStructure = document.querySelector<HTMLInputElement>("#structure")!;
        this.checkboxDragon = document.querySelector<HTMLInputElement>("#dragon")!;
        this.checkboxHerald = document.querySelector<HTMLInputElement>("#herald")!;
        this.checkboxBaron = document.querySelector<HTMLInputElement>("#baron")!;

        this.showTimestampsButton = document.querySelector<HTMLButtonElement>("#copy-timestamps-btn")!;

        this.searchInput.addEventListener("input", () => {
            this.searchQuery = this.searchInput.value.trim().toLowerCase();
            this.render();
        });

        for (const tab of this.filterTabs) {
            tab.addEventListener("click", () => {
                this.filterTab = (tab.dataset.tab as FilterTab) ?? "all";
                this.render();
            });
        }

        this.viewListBtn.addEventListener("click", () => this.setViewMode("list"));
        this.viewGridBtn.addEventListener("click", () => this.setViewMode("grid"));
    }

    public showWindow = () => {
        void appWindow.show();
    };

    public setFullscreen = (fullscreen: boolean) => {
        void appWindow.setFullscreen(fullscreen);
    };

    public setRecordingsFolderBtnOnClickHandler = (handler: (e: MouseEvent) => void) => {
        this.videoFolderBtn.addEventListener("click", handler);
    };

    public setCheckboxOnClickHandler = (handler: (e: MouseEvent) => void) => {
        this.checkboxKill.addEventListener("click", handler);
        this.checkboxDeath.addEventListener("click", handler);
        this.checkboxAssist.addEventListener("click", handler);
        this.checkboxStructure.addEventListener("click", handler);
        this.checkboxDragon.addEventListener("click", handler);
        this.checkboxHerald.addEventListener("click", handler);
        this.checkboxBaron.addEventListener("click", handler);
    };

    public setShowTimestampsOnClickHandler = (handler: (e: MouseEvent) => void) => {
        this.showTimestampsButton.addEventListener("click", handler);
    };

    public setThumbnailSrcResolver = (resolver: (videoId: string) => string) => {
        this.thumbnailSrc = resolver;
        this.render();
    };

    // --- recordings list / grid ---

    public updateSideBar = (
        recordingsSizeGb: number,
        recordings: ReadonlyArray<Recording>,
        onVideo: (videoId: string) => void,
        onFavorite: (videoId: string) => Promise<boolean | null>,
        onRename: (videoId: string) => void,
        onDelete: (videoId: string) => void,
    ) => {
        this.recordingsSizeGb = recordingsSizeGb;
        this.recordings = recordings;
        this.onVideo = onVideo;
        this.onFavorite = onFavorite;
        this.onRename = onRename;
        this.onDelete = onDelete;
        this.render();
    };

    private matchesSearch = (recording: Recording): boolean => {
        if (this.searchQuery === "") return true;
        if (recording.videoId.toLowerCase().includes(this.searchQuery)) return true;

        const metadata = recording.metadata;
        if (metadata && "Metadata" in metadata) {
            if (metadata.Metadata.championName.toLowerCase().includes(this.searchQuery)) return true;
            if (metadata.Metadata.enemyChampionName?.toLowerCase().includes(this.searchQuery)) return true;
        }
        return false;
    };

    private matchesTab = (recording: Recording, tab: FilterTab): boolean => {
        if (tab === "favorites") return isFavorite(recording.metadata);
        if (tab === "ranked") return isRanked(recording.metadata);
        return true;
    };

    private render = () => {
        const searched = this.recordings.filter(this.matchesSearch);

        // counts are part of the tab label (DESIGN.md "Filter tabs")
        for (const tab of this.filterTabs) {
            const key = (tab.dataset.tab as FilterTab) ?? "all";
            const count = searched.filter((r) => this.matchesTab(r, key)).length;
            tab.textContent = `${key.toUpperCase()} ${count}`;
            tab.setAttribute("aria-selected", String(key === this.filterTab));
        }

        const visible = searched.filter((r) => this.matchesTab(r, this.filterTab));

        this.sidebar.replaceChildren(...(visible.length > 0 ? visible.map(this.createRow) : [this.emptyState()]));
        this.gridView.replaceChildren(...(visible.length > 0 ? visible.map(this.createCard) : [this.emptyState()]));

        this.statusTotals.textContent =
            `${this.recordings.length} RECORDINGS${DOT}${this.recordingsSizeGb.toFixed(2)} GB`;
    };

    private emptyState = () => {
        const noneAtAll = this.recordings.length === 0;
        return el("div", { class: "lr-empty" }, [
            el("div", { class: "lr-empty__fact" }, [noneAtAll ? "NO RECORDINGS" : "NO MATCHES"]),
            el("div", { class: "lr-empty__hint" }, [
                noneAtAll
                    ? "Recordings appear here when a game of League is played."
                    : "No recording matches the current search and filter.",
            ]),
        ]);
    };

    private createActions = (recording: Recording): HTMLDivElement => {
        const favorite = isFavorite(recording.metadata);

        const favoriteBtn = el("button", { class: `favorite${favorite ? " active" : ""}`, title: "Favorite" }, [
            favorite ? STAR : STAR_OUTLINE,
        ]);
        favoriteBtn.addEventListener("click", (e) => {
            e.stopPropagation();
            // eslint-disable-next-line always-return
            this.onFavorite(recording.videoId).then((fav) => {
                if (fav !== null) {
                    favoriteBtn.textContent = fav ? STAR : STAR_OUTLINE;
                    favoriteBtn.classList.toggle("active", fav);
                }
            });
        });

        const renameBtn = el("button", { class: "rename", title: "Rename" }, [PENCIL]);
        renameBtn.addEventListener("click", (e) => {
            e.stopPropagation();
            this.onRename(recording.videoId);
        });

        const deleteBtn = el("button", { class: "delete", title: "Delete" }, [CROSS]);
        deleteBtn.addEventListener("click", (e) => {
            e.stopPropagation();
            this.onDelete(recording.videoId);
        });

        return el("div", { class: "lr-row__actions" }, [favoriteBtn, renameBtn, deleteBtn]);
    };

    private createRow = (recording: Recording): HTMLLIElement => {
        const result = resultOf(recording.metadata);
        const kda = kdaOf(recording.metadata);
        const favorite = isFavorite(recording.metadata);

        const name = el("span", { class: "video-name" }, [
            ...(favorite ? [el("span", { class: "fav-mark" }, [STAR])] : []),
            el("span", { class: "name-text", title: toVideoName(recording.videoId) }, [toVideoName(recording.videoId)]),
        ]);

        const row = el(
            "li",
            {
                class: "lr-row",
                "data-vid": recording.videoId,
                "aria-selected": String(recording.videoId === this.activeVideoId),
            },
            [
                name,
                el("span", { class: "result", "data-result": result }, [result === "-" ? DASH : result]),
                el("div", { class: "lr-row__end" }, [
                    el("span", { class: "kda" }, [kda ?? ""]),
                    this.createActions(recording),
                ]),
            ],
        );
        row.addEventListener("click", () => this.onVideo(recording.videoId));
        return row;
    };

    private createCard = (recording: Recording): HTMLElement => {
        const result = resultOf(recording.metadata);
        const champion = championOf(recording.metadata);
        const kda = kdaOf(recording.metadata);

        const thumb = el("div", { class: "lr-card__thumb" }, [
            // a muted video with a media fragment renders the frame at that time - no
            // separate thumbnail files needed; 90s in is past the loading screen
            ...(this.thumbnailSrc !== null
                ? [el("video", { src: `${this.thumbnailSrc(recording.videoId)}#t=90`, preload: "metadata", muted: "", tabindex: "-1" })]
                : []),
            el("span", { class: "lr-card__badge", "data-result": result }, [result === "-" ? DASH : result]),
            ...(isFavorite(recording.metadata) ? [el("span", { class: "lr-card__fav" }, [STAR])] : []),
        ]);

        const card = el(
            "button",
            {
                class: "lr-card",
                "data-vid": recording.videoId,
                "aria-selected": String(recording.videoId === this.activeVideoId),
            },
            [
                thumb,
                el("div", { class: "lr-card__body" }, [
                    el("div", { class: "lr-card__name", title: toVideoName(recording.videoId) }, [
                        toVideoName(recording.videoId),
                    ]),
                    el("div", { class: "lr-card__meta" }, [
                        champion !== null ? `${champion.toUpperCase()}${DOT}${kda}` : "NO MATCH DATA",
                    ]),
                ]),
            ],
        );
        card.addEventListener("click", () => {
            // the player only lives in list view, so picking a card jumps back to it
            this.setViewMode("list");
            this.onVideo(recording.videoId);
        });
        return card;
    };

    public setViewMode = (mode: ViewMode) => {
        this.viewMode = mode;
        this.playerView.classList.toggle("hidden", mode !== "list");
        this.gridView.classList.toggle("hidden", mode !== "grid");
        this.viewListBtn.setAttribute("aria-selected", String(mode === "list"));
        this.viewGridBtn.setAttribute("aria-selected", String(mode === "grid"));
    };

    public getActiveVideoId = (): string | null => {
        return this.activeVideoId;
    };

    public setActiveVideoId = (videoId: string | null): boolean => {
        this.activeVideoId = videoId;

        let found = videoId === null;
        for (const item of document.querySelectorAll<HTMLElement>("[data-vid]")) {
            const selected = item.dataset.vid === videoId;
            item.setAttribute("aria-selected", String(selected));
            found ||= selected;
        }
        return found;
    };

    // --- status bar ---

    public setStatusMessage = (message: string) => {
        this.statusMeta.replaceChildren(el("span", {}, [message]));
    };

    public setStatusMetadata = (data: GameMetadata) => {
        const stats = data.stats;
        const parts: Array<Node | string> = [];
        const push = (node: Node | string) => {
            if (parts.length > 0) parts.push(DOT);
            parts.push(node);
        };

        push(`${data.player.gameName}#${data.player.tagLine}`);
        push(data.championName.toUpperCase());
        if (data.summonerSpells && data.summonerSpells.length > 0) {
            push(data.summonerSpells.map((s) => s.toUpperCase()).join("+"));
        }
        if (data.runes) {
            // keystone inline, full rune page in the tooltip
            const keystone = data.runes.perks[0];
            const runesText =
                keystone !== undefined
                    ? `${keystone.toUpperCase()} (${data.runes.primaryStyle.toUpperCase()}/${data.runes.subStyle.toUpperCase()})`
                    : `${data.runes.primaryStyle.toUpperCase()}/${data.runes.subStyle.toUpperCase()}`;
            const runesTitle = `Runes: ${data.runes.perks.join(", ")} (${data.runes.primaryStyle}/${data.runes.subStyle})`;
            push(el("span", { title: runesTitle }, [runesText]));
        }
        push(el("span", { class: "em" }, [`${stats.kills}/${stats.deaths}/${stats.assists}`]));
        push(`${stats.totalMinionsKilled} CS`);
        push(`${stats.visionScore} WS`);
        push(data.queue.name.toUpperCase());
        if (stats.gameEndedInEarlySurrender) {
            push(el("span", { class: "remake" }, ["REMAKE"]));
        } else if (stats.win) {
            push(el("span", { class: "win" }, ["VICTORY"]));
        } else {
            push(el("span", { class: "loss" }, ["DEFEAT"]));
        }

        this.statusMeta.replaceChildren(el("span", {}, parts));
    };

    // --- state pill ---

    public setRecordingState = (state: RecordingState, detail: string) => {
        this.statePill.dataset.state = state.toLowerCase();
        this.statePillText.textContent = `${state.toUpperCase()}${DOT}${detail}`;
    };

    // --- modal / dialogs ---

    public showModal = (content: HTMLElement) => {
        this.modalContent.replaceChildren(content);
        this.modal.classList.add("open");
    };

    public hideModal = () => {
        this.modalContent.replaceChildren();
        this.modal.classList.remove("open");
    };

    public modalIsOpen = () => {
        return this.modal.classList.contains("open");
    };

    private dialog = (title: string, body: Array<Node | string>, danger = false): HTMLDivElement => {
        const closeBtn = el("button", { class: "lr-dialog__close", title: "Close" }, [CROSS]);
        closeBtn.addEventListener("click", this.hideModal);

        return el("div", { class: `lr-dialog${danger ? " lr-dialog--danger" : ""}` }, [
            el("div", { class: "lr-dialog__head" }, [el("span", {}, [title]), closeBtn]),
            el("div", { class: "lr-dialog__body" }, body),
        ]);
    };

    public showErrorModal = (text: string) => {
        const closeBtn = el("button", { class: "lr-btn lr-btn--primary" }, ["CLOSE"]);
        closeBtn.addEventListener("click", this.hideModal);

        this.showModal(
            this.dialog("ERROR", [
                el("p", { class: "lr-dialog__prose" }, [text]),
                el("div", { class: "lr-dialog__actions" }, [closeBtn]),
            ]),
        );
    };

    public showRenameModal = (
        videoId: string,
        videoIds: ReadonlyArray<string>,
        rename: (videoId: string, newVideoId: string) => void,
    ) => {
        const videoName = toVideoName(videoId);

        const input = el("input", {
            type: "text",
            id: "new-name",
            value: videoName,
            placeholder: "new name",
            spellcheck: "false",
            autocomplete: "off",
        });
        input.value = videoName;

        const validityChecker = (_e: Event) => {
            if (videoIds.includes(toVideoId(input.value))) {
                input.setCustomValidity("there is already a file with this name");
                saveButton.setAttribute("disabled", "true");
            } else {
                input.setCustomValidity("");
                saveButton.removeAttribute("disabled");
            }

            input.reportValidity();
        };
        input.addEventListener("input", validityChecker);
        input.setCustomValidity("there is already a file with this name");

        const renameHandler = (e: KeyboardEvent | MouseEvent) => {
            // if the event is a KeyboardEvent also check if the key pressed was 'enter'
            const keyboardEvent = "key" in e;
            if (input.checkValidity() && (!keyboardEvent || e.key === "Enter")) {
                e.preventDefault();
                this.hideModal();
                rename(videoId, toVideoId(input.value));

                input.removeEventListener("keydown", renameHandler);
                input.removeEventListener("input", validityChecker);
            }
        };
        input.addEventListener("keydown", renameHandler);

        const saveButton = el("button", { class: "lr-btn lr-btn--primary", disabled: "true" }, ["SAVE"]);
        saveButton.addEventListener("click", renameHandler);
        const cancelButton = el("button", { class: "lr-btn" }, ["CANCEL"]);
        cancelButton.addEventListener("click", this.hideModal);

        this.showModal(
            this.dialog("RENAME RECORDING", [
                el("p", { class: "lr-dialog__prose" }, ["Change name of ", el("span", { class: "em" }, [videoName])]),
                el("label", { class: "lr-input" }, [input]),
                el("div", { class: "lr-dialog__actions" }, [cancelButton, saveButton]),
            ]),
        );

        input.setSelectionRange(input.value.length, input.value.length);
        input.focus();
    };

    public showDeleteModal = (videoId: string, deleteVideo: (videoId: string) => void) => {
        const videoName = toVideoName(videoId);

        let confirmDelete = true;
        const checkbox = el("input", { type: "checkbox", id: "dont-ask-again" });
        checkbox.addEventListener("change", () => {
            confirmDelete = !confirmDelete;
        });

        const deleteBtn = el("button", { class: "lr-btn lr-btn--danger" }, ["DELETE"]);
        deleteBtn.addEventListener("click", () => {
            this.hideModal();
            deleteVideo(videoId);

            if (!confirmDelete) {
                commands.disableConfirmDelete();
            }
        });
        const cancelButton = el("button", { class: "lr-btn" }, ["CANCEL"]);
        cancelButton.addEventListener("click", this.hideModal);

        this.showModal(
            this.dialog(
                "DELETE RECORDING",
                [
                    el("p", { class: "lr-dialog__prose" }, [
                        "Delete recording ",
                        el("span", { class: "em" }, [videoName]),
                        "? This cannot be undone.",
                    ]),
                    el("label", { class: "legend-item", style: "margin-top: 10px" }, [checkbox, "don't ask again"]),
                    el("div", { class: "lr-dialog__actions" }, [cancelButton, deleteBtn]),
                ],
                true,
            ),
        );
    };

    public showTimelineModal = (timelineEvents: Array<TimelineEvent>, setTime: (millis: number) => void) => {
        const items = timelineEvents.map((event) => {
            const item = el("li", { style: `color: var(${MARKER_TOKEN[event.markerClass] ?? "--text-2"})` }, [
                el("span", { class: "event-bar" }),
                el("span", { class: "event-time" }, [formatTimestamp(event.timestamp)]),
                el("span", {}, [event.name]),
            ]);
            item.addEventListener("click", () => {
                setTime(event.timestamp);
                this.hideModal();
            });
            return item;
        });

        const copyBtn = el("button", { class: "lr-btn lr-btn--primary" }, ["COPY TO CLIPBOARD"]);
        copyBtn.addEventListener("click", () =>
            clipboard.writeText(timelineEvents.map((e) => `${formatTimestamp(e.timestamp)} ${e.name}`).join("\n")),
        );

        this.showModal(
            this.dialog("TIMESTAMPS", [
                el("ul", { class: "timeline-event-list" }, items),
                el("div", { class: "lr-dialog__actions" }, [copyBtn]),
            ]),
        );
    };

    // --- player / markers ---

    public showBigPlayButton = (show: boolean) => {
        const bpb = document.querySelector<HTMLButtonElement>(".vjs-big-play-button");
        if (bpb !== null) {
            bpb.style.display = show ? "block !important" : "none !important";
        }
    };

    public setMarkerCount = (count: number) => {
        this.markerCount.textContent = `${count} SHOWN`;
    };

    public setMarkerFlags = (settings: MarkerFlags) => {
        this.checkboxKill.checked = settings.kill;
        this.checkboxDeath.checked = settings.death;
        this.checkboxAssist.checked = settings.assist;
        this.checkboxStructure.checked = settings.structure;
        this.checkboxDragon.checked = settings.dragon;
        this.checkboxHerald.checked = settings.herald;
        this.checkboxBaron.checked = settings.baron;
    };

    public getMarkerFlags = (): MarkerFlags => {
        return {
            kill: this.checkboxKill.checked,
            death: this.checkboxDeath.checked,
            assist: this.checkboxAssist.checked,
            structure: this.checkboxStructure.checked,
            dragon: this.checkboxDragon.checked,
            herald: this.checkboxHerald.checked,
            baron: this.checkboxBaron.checked,
        };
    };

    public showMarkerFlags = (show: boolean) => {
        this.markerLists.classList.toggle("disabled", !show);
    };
}

export function formatTimestamp(timestamp: number): string {
    let secs = timestamp / 1000;

    let minutes = Math.floor(secs / 60);
    secs -= minutes * 60;

    const hours = Math.floor(minutes / 60);
    minutes -= hours * 60;

    return `${hours.toString().padStart(2, "0")}:${minutes.toString().padStart(2, "0")}:${Math.floor(secs)
        .toString()
        .padStart(2, "0")}`;
}
