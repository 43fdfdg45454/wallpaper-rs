import { media } from "wprs";

let [line, accent, playing] = ["", [1, 1, 1, 1], false];

export function mediaStatus({ enabled }) {
    if (!enabled) [line, playing] = ["", false];
}

export function mediaPlayback({ state }) {
    playing = state === "playing";
}

export function mediaProperties({ title, artist }) {
    line = artist ? `${title} · ${artist}` : title;
}

export function mediaThumbnail({ colors }) {
    accent = colors ? [colors.primary.r, colors.primary.g, colors.primary.b, 1] : [1, 1, 1, 1];
}

export function mediaTimeline({ position, duration }) {
    if (duration > 0 && position >= duration) playing = false;
}

export const nowPlaying = () => line;
export const accentColor = () => accent;
export const shown = () => (playing ? 1 : 0.4);
export const progress = () => media.progress;
