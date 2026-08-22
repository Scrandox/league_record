import type { MetadataFile } from "../bindings";

export function toVideoName(videoId: string): string {
    return videoId.slice(0, videoId.lastIndexOf("."));
}

export function toVideoId(videoName: string): string {
    return videoName + ".mp4";
}

export function splitRight(string: string, separator: string): string {
    return string.slice(string.lastIndexOf(separator) + 1);
}

export function isFavorite(metadataFile: MetadataFile | null): boolean {
    if (!metadataFile) return false;
    if ("Metadata" in metadataFile) return metadataFile.Metadata.favorite;
    if ("Deferred" in metadataFile) return metadataFile.Deferred.favorite;
    return false;
}

export function isRanked(metadataFile: MetadataFile | null): boolean {
    if (metadataFile && "Metadata" in metadataFile) return metadataFile.Metadata.queue.isRanked;
    return false;
}

// single letter for the result column: W / L / R(emake), "-" when no data yet
export function resultOf(metadataFile: MetadataFile | null): "W" | "L" | "R" | "-" {
    if (!metadataFile || !("Metadata" in metadataFile)) return "-";
    const stats = metadataFile.Metadata.stats;
    if (stats.gameEndedInEarlySurrender) return "R";
    return stats.win ? "W" : "L";
}

export function kdaOf(metadataFile: MetadataFile | null): string | null {
    if (!metadataFile || !("Metadata" in metadataFile)) return null;
    const stats = metadataFile.Metadata.stats;
    return `${stats.kills}/${stats.deaths}/${stats.assists}`;
}

export function championOf(metadataFile: MetadataFile | null): string | null {
    if (!metadataFile || !("Metadata" in metadataFile)) return null;
    return metadataFile.Metadata.championName;
}

// return this error in 'default' switch branches to make the switch statement exhaustive
export class UnreachableError extends Error {
    constructor(val: never) {
        super(`unreachable case: ${JSON.stringify(val)}`);
    }
}
