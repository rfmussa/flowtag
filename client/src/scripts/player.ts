import { get1t, OneTagger } from "./onetagger";
import { diag } from "./diag";
import { httpUrl, wsUrl } from "./utils";
import router from "./router";
import { getTagStore } from "./tagstore";
import { endOfTrackAction } from "../lib/player";

const WAVES = 180;

class Player {
    waveform: number[] = [];
    playing: boolean = false;
    position: number = 0;
    duration: number = 1;
    volume: number = 0.5;
    wasPlaying: boolean = false;
    /** Track id of the FlowTag-started file (markers match against it) */
    trackId?: number;
    /** Auto-gain applied by the server for this track (dB, 0 when off) */
    gainDb: number = 0;
    title?: string;
    artists: string[] = [];
    audio: HTMLAudioElement = new Audio();
    path?: string;

    // Using _ instead of private because of reactivity proxy
    _waveformLock: boolean[] = [];
    _waveformPath?: string;
    _playerLoadPromise?: Promise<any>;
    _playerLoadResolve?: any;

    constructor($1t: OneTagger) {
        // Setup
        this.generateDefaultWaveform();
        setInterval(() => {
            const store = getTagStore();
            const action = endOfTrackAction({
                playing: this.playing,
                position: this.position,
                duration: this.duration,
                autoPlayNext: $1t.settings.value.autoPlayNext,
                onQuickTag: router.currentRoute.value.path.startsWith('/quicktag'),
                multiSelect: store.selectedIds.value.length > 0,
            });
            if (action === 'advance') {
                // Same pattern as the media "next" key: wasPlaying makes the
                // selection watcher start the newly selected track. Without it
                // the selection moved but nothing played, so this check refired
                // every tick and the cursor ran away to the end of the list.
                const before = store.selectedId.value;
                this.wasPlaying = true;
                store.selectOffset(1);
                if (store.selectedId.value === before) {
                    // Last row: nothing to advance to
                    this.wasPlaying = false;
                    this.pause();
                } else {
                    // The watcher's startTrack resets the clock on Vue's flush;
                    // clamp now so the check cannot refire before that lands
                    this.position = 0;
                }
            } else if (action === 'pause') {
                this.pause();
            }

            if (this.playing)
                this.position += 150;
        }, 150);
    }

    // Stop & reset player
    stop() {
        this.pause();
        const $1t = get1t();
        $1t.send('playerStop');
        this.position = 0;
        this.duration = 1;
        this.title = undefined;
        this.artists = [];
        this.generateDefaultWaveform();
    }

    // Fill waveform array with empty data
    generateDefaultWaveform() {
        for (let i=0; i < WAVES; i++) {
            this.waveform[i] = 0;
        }
    }

    // Start generating waveform in background
    async generateWaveform(path: string) {
        this._waveformPath = path;
        // Resolve locks
        this._waveformLock.push(true);
        while (this._waveformLock.length > 1) {
            await new Promise((res) => setTimeout(() => res(null), 50));
            if (path != this._waveformPath) {
                this._waveformLock.pop();
                return;
            }
        }
        await this._generateWaveformInner(path);
    }

    // Actually generate the waveform
    async _generateWaveformInner(path: string) {
        const $1t = get1t();
        this.generateDefaultWaveform();
        let waveformIndex = 0;
        // Separate socket = separate thread
        let ws = new WebSocket(wsUrl());
        ws.addEventListener('message', (event) => {
            // Lock
            if (this._waveformLock.length > 1) {
                ws.close();
                this._waveformLock.pop();
                return;
            }

            let json = JSON.parse(event.data);
            // New wave
            if (json.action == 'waveformWave') {
                this.waveform[waveformIndex] = json.wave;
                waveformIndex++;
            }
            // Finish
            if (json.action == 'waveformDone' || json.action == 'error') {
                ws.close();
                this._waveformLock.pop();

                // Autoplay, delay just in case for windows
                setTimeout(() => {
                    if ($1t.settings.value.continuePlayback && this.wasPlaying) {
                        this.play();
                        this.wasPlaying = false;
                    }
                }, 100);
            }
            // NOTE: bars are now pushed by the server without per-bar acks
        });
        // Start loading on connection
        ws.addEventListener('open', (_) => {
            ws.send(JSON.stringify({
                action: 'waveform',
                path
            }));
        });

        // Auto play on unix systems (on Windows it causes errors, because file is read twice)
        if ($1t.info.value.os != 'windows' && $1t.settings.value.continuePlayback && this.wasPlaying) {
            this.play();
            this.wasPlaying = false;
        }
    }

    // Start playback
    async play() {
        const $1t = get1t();

        // External
        if ($1t.settings.value.externalAudioPlayer) {
            $1t.send('openFile', { path: this.path });
            return;
        }
            
        // Client side player
        if ($1t.settings.value.clientSidePlayer) {
            this.playing = false;
            this.audio.play();
            return;
        }

        // Wait for load
        if (this._playerLoadPromise) {
            await this._playerLoadPromise;
            this._playerLoadPromise = undefined;
        }
        // Start playing
        $1t.send("playerPlay");
        this.playing = true;
    }

    // Pause playback
    pause() {
        const $1t = get1t();
        $1t.settings.value.clientSidePlayer ? this.audio.pause() : $1t.send("playerPause");
        this.playing = false;
        this.wasPlaying = false;
    }

    // Seek to position (ms)
    seek(pos: number) {
        const $1t = get1t();
        this.playing = false;

        if ($1t.settings.value.clientSidePlayer) {
            this.audio.currentTime = pos / 1000.0;
        } else {
            $1t.send("playerSeek", { pos })
        }
        this.position = pos;
    }

    // Set volume
    setVolume(volume: number | undefined | null) {
        if (!volume) return;
        this.audio.volume = volume;
        get1t().send("playerVolume", {volume});
    }

    // FlowTag path: one round-trip start + DB-cached waveform by track id
    startTrack(path: string, trackId: number, title: string, artist: string, autoplay: boolean) {
        const $1t = get1t();
        diag('playStart', { trackId, autoplay, file: path.split('/').pop() });

        // Server-mode / external player setups use the legacy load path, which
        // knows how to drive the <audio> element or external app
        if ($1t.settings.value.clientSidePlayer || $1t.settings.value.externalAudioPlayer) {
            this.title = title;
            this.artists = artist ? [artist] : [];
            this.wasPlaying = autoplay;
            this.loadTrack(path);
            return;
        }

        this.path = path;
        this.title = title;
        this.artists = artist ? [artist] : [];
        this.position = 0;
        this.playing = autoplay;
        this.wasPlaying = false;
        this.trackId = trackId;
        this.gainDb = 0;
        this.generateDefaultWaveform();
        $1t.send('playerStart', { path, autoplay, autoGain: $1t.settings.value.flowtag.analysisEnabled });
        $1t.send('waveformGet', { trackId });
    }

    // Load track to play
    loadTrack(path: string) {
        const $1t = get1t();

        // Setup client-side audio player
        if ($1t.settings.value.clientSidePlayer) {
            this.audio.pause();
            this.playing = false;
            this.audio = new Audio(`${httpUrl()}/audio?path=${encodeURIComponent(path)}`);
            this.audio.volume = this.volume;
            const cb = () => {
                this.playing = !this.audio.paused;
                this.position = Math.round(this.audio.currentTime * 1000);
            }
            this.audio.addEventListener('playing', cb);
        }

        // Server side
        this._playerLoadPromise = new Promise((res) => this._playerLoadResolve = res);
        $1t.send("playerLoad", { path });
        this.generateWaveform(path);

        this.path = path;
    }
}

export { Player, WAVES };