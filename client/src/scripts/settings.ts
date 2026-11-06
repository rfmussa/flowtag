import { AutotaggerConfig, AutotaggerProfile } from "./autotagger";
import { FrameName, Keybind, Separators } from "./utils";

class Settings {
    path?: string;
    autoTaggerConfig: any = {};
    autoTaggerSinglePage: boolean = false;
    autoTaggerProfiles: AutotaggerProfile[] = [];
    autoTaggerProfile: string = 'Default';
    showAutoTaggerProfiles: boolean = false;
    primaryColor: string = '#00D2BF';
    volume: number = 0.05;
    helpButton: boolean = true;
    continuePlayback: boolean = false;
    clientSidePlayer: boolean = false;
    nonNativeBrowser: boolean = false;
    playOnSeek: boolean = false;
    autoPlayNext: boolean = false;
    externalAudioPlayer: boolean = false;
    devtools: boolean = false;

    renamer: RenamerSettings = new RenamerSettings();
    audioFeatures: AudioFeaturesSettings = new AudioFeaturesSettings();

    tagEditorDouble: boolean = false;
    tagEditorCustom: string[] = [];
    tagEditorAutosave: boolean =  false;
    tagEditorPlayer: boolean =  false;

    // Read from json
    // Tolerant of missing / legacy keys: anything absent keeps its default
    static fromJson(data: any): Settings {
        const safe = (data && typeof data === 'object') ? data : {};
        let settings: Settings = Object.assign(new Settings(), safe);
        settings.renamer = Object.assign(new RenamerSettings(), safe.renamer);
        settings.audioFeatures = AudioFeaturesSettings.fromJson(safe.audioFeatures);
        if (!Array.isArray(settings.autoTaggerProfiles)) settings.autoTaggerProfiles = [];
        if (!Array.isArray(settings.tagEditorCustom)) settings.tagEditorCustom = [];
        if (!settings.autoTaggerConfig || typeof settings.autoTaggerConfig !== 'object') settings.autoTaggerConfig = {};
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

class RenamerSettings {
    path?: string;
    outDir?: string;
    template?: string;
    copy: boolean = false;
    subfolders: boolean = true;
    overwrite: boolean = false;
}

class AudioFeaturesSettings {
    spotifyClientId?: string;
    spotifyClientSecret?: string;
    config?: AudioFeaturesConfig;

    static fromJson(data: any): AudioFeaturesSettings {
        let a: AudioFeaturesSettings = Object.assign(new AudioFeaturesSettings(), data);
        if (data?.config)
            a.config = AudioFeaturesConfig.fromJson(data.config);
        return a;
    }
}


class AudioFeaturesConfig {
    path?: string;
    metaTag = true;
    skipTagged = false;
    includeSubfolders = true;
    mainTag = FrameName.same('AUDIO_FEATURES');
    separators = new Separators();
    properties: Record<string, AudioFeaturesProperty> = {
        acousticness: new AudioFeaturesProperty(0, 90, '1T_ACOUSTICNESS'),
        danceability: new AudioFeaturesProperty(20, 80, '1T_DANCEABILITY'),
        energy: new AudioFeaturesProperty(20, 90, '1T_ENERGY'),
        instrumentalness: new AudioFeaturesProperty(50, 90, '1T_INSTRUMENTALNESS'),
        liveness: new AudioFeaturesProperty(0, 80, '1T_LIVENESS'), 
        speechiness: new AudioFeaturesProperty(0, 70, '1T_SPEECHINESS'),
        valence: new AudioFeaturesProperty(15, 85, '1T_VALENCE'), 
        popularity: new AudioFeaturesProperty(0, 80, '1T_POPULARITY'), 
    }
    type?: string;

    static fromJson(data: any): AudioFeaturesConfig {
        let c: AudioFeaturesConfig = Object.assign(new AudioFeaturesConfig(), data);
        if (data?.mainTag) c.mainTag = FrameName.fromJson(data.mainTag);
        // Start from defaults so properties missing in older settings files still exist
        c.properties = { ...new AudioFeaturesConfig().properties };
        for (const [k, v] of Object.entries(data?.properties ?? {})) {
            c.properties[k] = AudioFeaturesProperty.fromJson(v);
        }
        return c;
    }
}

class AudioFeaturesProperty {
    tag: FrameName;
    enabled: boolean;
    range: { min: number, max: number }

    constructor(min: number, max: number, frameName: string) {
        this.tag = FrameName.same(frameName);
        this.range = { min, max };
        this.enabled = true;
    }

    static fromJson(data: any): AudioFeaturesProperty {
        let p = new AudioFeaturesProperty(data?.range?.min ?? 0, data?.range?.max ?? 100, '');
        p.enabled = data?.enabled ?? true;
        p.tag = FrameName.fromJson(data?.tag);
        return p;
    }
}

export { Settings, AudioFeaturesConfig, AudioFeaturesProperty };