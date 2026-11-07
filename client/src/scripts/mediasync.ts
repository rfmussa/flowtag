import { watch } from 'vue';
import { get1t } from './onetagger';

/**
 * Mirrors the client player into the OS Now Playing panel: the server relays
 * state to the GUI shell (souvlaki), which owns the macOS media controls.
 * Sends on play/pause/track changes and on seeks (position jumping away from
 * its wall-clock expectation); steady playback needs no updates, the OS
 * interpolates the position itself.
 */
export function initMediaSync() {
    const $1t = get1t();
    let lastPositionMs = 0;
    let lastSentAt = 0;
    let lastPlaying = false;

    const send = () => {
        const p = $1t.player.value;
        lastPositionMs = p.position ?? 0;
        lastSentAt = Date.now();
        lastPlaying = p.playing;
        $1t.send('nowPlaying', {
            playing: p.playing,
            title: p.title ?? '',
            artist: (p.artists ?? []).join(', '),
            positionMs: Math.max(Math.round(p.position ?? 0), 0),
            durationMs: Math.max(Math.round(p.duration ?? 0), 0),
            path: p.path ?? '',
        });
    };

    // Play/pause and track switches
    watch(
        () => [$1t.player.value.playing, $1t.player.value.path, $1t.player.value.title, $1t.player.value.duration],
        () => send(),
    );

    // Seeks: the position jumped away from where wall-clock playback would be
    watch(() => $1t.player.value.position, (pos) => {
        const expected = lastPositionMs + (lastPlaying ? Date.now() - lastSentAt : 0);
        if (Math.abs((pos ?? 0) - expected) > 2000) {
            send();
        } else if (!lastPlaying) {
            // Keep the paused baseline fresh so a scrub is measured per step
            lastPositionMs = pos ?? 0;
            lastSentAt = Date.now();
        }
    });
}
