import { AutotaggerConfig, AutotaggerProfile } from "./autotagger";
import { Keybind } from "./utils";

/** Remappable FlowTag actions (letter keys; Space/arrows/digits/Esc/⌘Z stay fixed) */
const FLOWTAG_ACTIONS = ['prev', 'next', 'focusTag', 'focusFilter', 'done', 'edit', 'lookup'] as const;
type FlowTagAction = typeof FLOWTAG_ACTIONS[number];

/** FlowTag-specific settings */
class FlowTagSettings {
    /** Custom keybind per action; a missing entry means the built-in default key */
    keybinds: Partial<Record<FlowTagAction, Keybind>> = {};

    static fromJson(data: any): FlowTagSettings {
        const s = new FlowTagSettings();
        const raw = (data && typeof data === 'object' && data.keybinds && typeof data.keybinds === 'object')
            ? data.keybinds : {};
        const keybinds: Partial<Record<FlowTagAction, Keybind>> = {};
        for (const action of FLOWTAG_ACTIONS) {
            const kb = (raw as any)[action];
            if (kb && typeof kb.key === 'string' && kb.key) keybinds[action] = Keybind.fromJson(kb);
        }
        s.keybinds = keybinds;
        return s;
    }
}

/** Spotify credentials for the lookup platforms.
 *  The JSON key stays `audioFeatures` so existing settings files keep their login. */
class SpotifyCredentials {
    spotifyClientId?: string;
    spotifyClientSecret?: string;

    static fromJson(data: any): SpotifyCredentials {
        const c = new SpotifyCredentials();
        if (data && typeof data === 'object') {
            if (typeof data.spotifyClientId === 'string') c.spotifyClientId = data.spotifyClientId;
            if (typeof data.spotifyClientSecret === 'string') c.spotifyClientSecret = data.spotifyClientSecret;
        }
        return c;
    }
}

/** Settings of removed OneTagger features; dropped on load so they stop round-tripping */
const LEGACY_SETTINGS_KEYS = [
    'primaryColor', 'helpButton', 'renamer',
    'tagEditorDouble', 'tagEditorCustom', 'tagEditorAutosave', 'tagEditorPlayer',
] as const;

class Settings {
    path?: string;
    autoTaggerConfig: any = {};
    autoTaggerSinglePage: boolean = false;
    autoTaggerProfiles: AutotaggerProfile[] = [];
    autoTaggerProfile: string = 'Default';
    showAutoTaggerProfiles: boolean = false;
    volume: number = 0.05;
    continuePlayback: boolean = false;
    clientSidePlayer: boolean = false;
    nonNativeBrowser: boolean = false;
    playOnSeek: boolean = false;
    autoPlayNext: boolean = false;
    externalAudioPlayer: boolean = false;
    devtools: boolean = false;

    flowtag: FlowTagSettings = new FlowTagSettings();
    audioFeatures: SpotifyCredentials = new SpotifyCredentials();

    // Read from json
    // Tolerant of missing / legacy keys: anything absent keeps its default
    static fromJson(data: any): Settings {
        const safe = (data && typeof data === 'object') ? data : {};
        let settings: Settings = Object.assign(new Settings(), safe);
        settings.flowtag = FlowTagSettings.fromJson(safe.flowtag);
        settings.audioFeatures = SpotifyCredentials.fromJson(safe.audioFeatures);
        if (!Array.isArray(settings.autoTaggerProfiles)) settings.autoTaggerProfiles = [];
        if (!settings.autoTaggerConfig || typeof settings.autoTaggerConfig !== 'object') settings.autoTaggerConfig = {};
        for (const key of LEGACY_SETTINGS_KEYS) delete (settings as any)[key];
        return settings;
    }

    /// Save new AT profile
    saveATProfile(name: string, config: AutotaggerConfig) {
        let i = this.autoTaggerProfiles.findIndex(p => p.name == name);
        if (i == -1) {
            this.autoTaggerProfiles.push({ name, config: JSON.parse(JSON.stringify(config)) });
            return;
        }
        this.autoTaggerProfiles[i] = { name, config: JSON.parse(JSON.stringify(config)) };
    }
}

export { Settings, FlowTagSettings, FLOWTAG_ACTIONS };
export type { FlowTagAction };
