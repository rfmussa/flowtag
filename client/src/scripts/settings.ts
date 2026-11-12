import { AutotaggerConfig, AutotaggerProfile } from "./autotagger";
import { Keybind } from "./utils";
import type { Scope, SortField } from "./tagstore";
import type { QuickFilter } from "@/lib/status";

/** Remappable FlowTag actions (letter keys; Space/arrows/digits/Esc/⌘Z stay fixed) */
const FLOWTAG_ACTIONS = ['prev', 'next', 'focusTag', 'focusFilter', 'done', 'edit', 'lookup', 'sidebar', 'stage', 'skip'] as const;
type FlowTagAction = typeof FLOWTAG_ACTIONS[number];

/** Optional track-list columns (title/artist/chips are always on) */
interface FlowTagColumns {
    year: boolean;
    bpm: boolean;
    key: boolean;
    path: boolean;
    added: boolean;
}
const DEFAULT_COLUMNS: FlowTagColumns = { year: true, bpm: true, key: true, path: false, added: false };

/** Where the user left the QuickTag view; restored on the next launch */
interface ViewState {
    scope: Scope;
    filterText: string;
    quickFilter: QuickFilter;
    sort: { by: SortField; desc: boolean };
    selectedTrackId: number | null;
    firstVisibleIndex: number;
}

const SORT_FIELDS: SortField[] = ['path', 'title', 'artist', 'bpm', 'key', 'year', 'added'];
const QUICK_FILTERS: QuickFilter[] = ['all', 'notdone', 'untagged'];

/** Tolerant parse: anything malformed yields undefined, individual bad fields fall back to defaults */
function viewStateFromJson(data: any): ViewState | undefined {
    if (!data || typeof data !== 'object') return undefined;
    const scope: Scope = (data.scope === 'stage' || typeof data.scope === 'number') ? data.scope : 'all';
    const sort = (data.sort && typeof data.sort === 'object') ? data.sort : {};
    return {
        scope,
        filterText: typeof data.filterText === 'string' ? data.filterText : '',
        quickFilter: QUICK_FILTERS.includes(data.quickFilter) ? data.quickFilter : 'all',
        sort: {
            by: SORT_FIELDS.includes(sort.by) ? sort.by : 'path',
            desc: sort.desc === true,
        },
        selectedTrackId: typeof data.selectedTrackId === 'number' ? data.selectedTrackId : null,
        firstVisibleIndex: typeof data.firstVisibleIndex === 'number' && data.firstVisibleIndex >= 0
            ? Math.floor(data.firstVisibleIndex) : 0,
    };
}

/** FlowTag-specific settings */
class FlowTagSettings {
    /** Custom keybind per action; a missing entry means the built-in default key */
    keybinds: Partial<Record<FlowTagAction, Keybind>> = {};
    columns: FlowTagColumns = { ...DEFAULT_COLUMNS };
    /** Experimental audition analysis: loudness auto-gain, intro/drop/outro
     *  markers, shift+Space plays from the drop. Default off. */
    analysisEnabled: boolean = false;
    /** One-way tag sync to Tangle. The backend doesn't persist it; the client resends it. */
    tangleSync: boolean = false;
    /** The view as the user left it */
    viewState?: ViewState;
    /** Key of the welcome-back card the user already dismissed */
    welcomeSeen?: string;

    static fromJson(data: any): FlowTagSettings {
        const s = new FlowTagSettings();
        const safe = (data && typeof data === 'object') ? data : {};
        const raw = (safe.keybinds && typeof safe.keybinds === 'object') ? safe.keybinds : {};
        const keybinds: Partial<Record<FlowTagAction, Keybind>> = {};
        for (const action of FLOWTAG_ACTIONS) {
            const kb = (raw as any)[action];
            if (kb && typeof kb.key === 'string' && kb.key) keybinds[action] = Keybind.fromJson(kb);
        }
        s.keybinds = keybinds;
        if (typeof safe.analysisEnabled === 'boolean') s.analysisEnabled = safe.analysisEnabled;
        if (typeof safe.tangleSync === 'boolean') s.tangleSync = safe.tangleSync;
        s.viewState = viewStateFromJson(safe.viewState);
        if (typeof safe.welcomeSeen === 'string') s.welcomeSeen = safe.welcomeSeen;
        const cols = (safe.columns && typeof safe.columns === 'object') ? safe.columns : {};
        const columns = { ...DEFAULT_COLUMNS };
        for (const key of Object.keys(DEFAULT_COLUMNS) as (keyof FlowTagColumns)[]) {
            if (typeof (cols as any)[key] === 'boolean') columns[key] = (cols as any)[key];
        }
        s.columns = columns;
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
    /** Selecting a row starts/switches playback (audition cruise). Off:
     *  selection is pure; Space, double-click, D and media keys play. */
    playOnSelect: boolean = false;
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
export type { FlowTagAction, ViewState };
