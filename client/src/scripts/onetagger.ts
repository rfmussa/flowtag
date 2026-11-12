import { notify } from '@/lib/notify';
import { alertDialog, choiceDialog, confirmDialog } from '@/lib/dialogs';
import { ref, Ref } from 'vue';
import { AutotaggerConfig, AutotaggerPlatform, ConfigCallbackResponse, TaggerStatus } from './autotagger';
import { Player } from './player';
import { Settings } from './settings';
import { Playlist, Spotify, wsUrl } from './utils';
import { ManualTag } from './manualtag';
import router from './router';
import { getTagStore } from './tagstore';
import { handleFlowKeys } from './flowkeys';
import { isCommitChord } from './commitsheet';
import { runQuitFlow } from '@/lib/quit';
import { TangleStatus } from '@/lib/tangle';

class OneTagger {
    // Singleton
    private static instance: OneTagger;

    info: Ref<AppInfo> = ref({}) as Ref<AppInfo>;
    config: Ref<AutotaggerConfig> = ref(new AutotaggerConfig());
    lock: Ref<{ locked: boolean }> = ref({ locked: false });
    player: Ref<Player> = ref(new Player(this));
    settings: Ref<Settings> = ref(new Settings());
    spotify: Ref<Spotify> = ref(new Spotify());
    helpDialog: Ref<{ open: boolean, route?: string }> = ref({ open: false });
    folderBrowser: Ref<FolderBrowser> = ref(new FolderBrowser());
    taggerStatus: Ref<TaggerStatus> = ref(new TaggerStatus());
    autoTaggerPlaylist: Ref<Playlist> = ref({});
    manualTag: Ref<ManualTag> = ref(new ManualTag());
    // Tangle sync (backend state; the token is never kept client-side)
    tangleStatus: Ref<TangleStatus | null> = ref(null);
    /** Last connect/sync error code from the backend, cleared on success */
    tangleError: Ref<string | null> = ref(null);
    /** User name from the last `tangleConnected` reply; cleared on disconnect */
    tangleUser: Ref<string | null> = ref(null);
    /** Latest enable-preview counts; consumed by the enable confirmation */
    tanglePreview: Ref<{ tags: number, entries: number } | null> = ref(null);

    // Websocket
    private ws!: WebSocket;
    private wsPromiseResolve?: (_: any) => void;
    private wsPromise?;

    // Startup: platforms still loading
    /** True until the backend has sent the platform list (App.vue shows a blocking notice) */
    platformsLoading: Ref<boolean> = ref(false);

    constructor() {
        // Singleton
        if (OneTagger.instance) {
            return OneTagger.instance;
        }
        OneTagger.instance = this;


        // WS connection promise
        this.wsPromise = new Promise((res) => this.wsPromiseResolve = res);
        // Setup WS connection
        this.ws = new WebSocket(wsUrl());
        this.ws.addEventListener('error', (e) => this.onError(e ?? 'Websocket error!'));
        this.ws.addEventListener('close', (_) => this.onError('WebSocket closed!'));
        this.ws.addEventListener('open', (_) => {
            // Resolve connection promise
            if (this.wsPromiseResolve) {
                this.wsPromiseResolve(null);
                this.wsPromiseResolve = undefined;
            }

            // Load initial data
            this.send('loadSettings');
            setTimeout(() => {
                this.send('init');
                this.send('spotifyAuthorized');
                // Load platforms
                setTimeout(() => this.loadPlatforms(), 25);
            }, 100);
        });
        this.ws.addEventListener('message', (event) => {
            // Parse incoming message
            let json = JSON.parse(event.data);
            if (!json.action) return;
            this.incomingEvent(json);
        });

        // Keybinds
        document.addEventListener('keydown', (e) => {
            // Never steal keys from editable elements
            const target = e.target as HTMLElement | null;
            // (the commit chord is the exception: it works from the tag input)
            const editable = target && (target.nodeName == "INPUT" || target.nodeName == "TEXTAREA" || target.isContentEditable);
            if (editable && !isCommitChord(e)) return true;

            // Dev tools
            if (e.key == 'F3') {
                window.dispatchEvent(new CustomEvent('flowtag-open-devtools'));
                e.preventDefault();
                e.stopPropagation();
                return true;
            }

            if (this.handleKeyDown(e)) {
                e.preventDefault();
                return false;
            }
        });
    }

    // SHOULD BE OVERWRITTEN
    quickTagUnfocus() {}
    onTaggingDone(_: any) {}
    onTagEditorEvent(_: any) {}
    onAudioFeaturesEvent(_: any) {}
    onRenamerEvent(_: any) {}
    onFolderBrowserEvent(_: any) {}
    // =======================

    // Display error to the user
    onError(msg: any) {
        // Show error dialog
        alertDialog({ title: 'Error', message: `${msg}` });

        console.error(msg);
    }

    // Send action to WebSocket
    async send(action: string, params = {}) {
        // Wait for WS connection
        if (this.wsPromise) {
            await this.wsPromise;
            this.wsPromise = undefined;
        }
        let data = { action };
        Object.assign(data, params);
        this.ws.send(JSON.stringify(data));
    }

    // Process incoming event
    private async incomingEvent(json: any) {
        // FlowTag (library/tagging) messages are handled by the tag store
        if (getTagStore().handleMessage(json)) return;

        /*eslint-disable no-case-declarations*/
        switch (json.action) {
            // Initial info
            case 'init':
                // Fill AppInfo
                Object.assign(this.info.value, json);
                this.info.value.platforms = [];
                // Path from args
                if (json.startContext.startPath) {
                    this.settings.value.path = json.startContext.startPath;
                    this.config.value.path = json.startContext.startPath;
                }

                this.info.value.ready = true;
                break;
        
            // Load log
            case 'log':
                this.info.value.log = json.log;
                break;

            // Settings loaded
            case 'loadSettings':
                this.loadSettings(json.settings);
                break;
            // Load custom platform settings
            case 'defaultCustomPlatformSettings':
                // Update custom platform settings to V2
                if (!this.config.value.custom['beatport'] || typeof this.config.value.custom['beatport']['art_resolution'] === 'object') {
                    this.config.value.custom = json.custom;
                }
                // Merge platform custom
                for (let platform of Object.keys(json.custom)) {
                    // All keys
                    if (!this.config.value.custom[platform]) {
                        this.config.value.custom[platform] = json.custom[platform];
                        continue;
                    }
                    // Per key
                    for (let key of Object.keys(json.custom[platform])) {
                        if (!this.config.value.custom[platform][key]) {
                            this.config.value.custom[platform][key] = json.custom[platform][key];
                        }
                    }
                }
                
                break;
            // Path selected
            case 'browse':
                this.onBrowse(json);
                break;
            // Error
            case 'error':
                // Unlock, callback
                this.lock.value.locked = false;
                import('./diag').then((d) => d.diagError('serverError', { message: String(json.message).slice(0, 400) }));
                this.onError(json.message);
                break;
            case 'startTagging':
                this.lock.value.locked = true;
                this.taggerStatus.value.reset();
                this.taggerStatus.value.total = json.files;
                this.taggerStatus.value.type = json.type;
                break;
            // Status
            case 'taggingProgress':
                this.taggerStatus.value.progress = json.status.progress;
                // De duplicate failed
                var i = this.taggerStatus.value.statuses.findIndex((s) => s[0].status.path == json.status.status.path)
                if (i != -1) {
                    this.taggerStatus.value.statuses[i].push(json.status);
                    return;
                }
                this.taggerStatus.value.statuses.unshift([json.status]);
                
                break;
            // Tagging done
            case 'taggingDone':
                this.lock.value.locked = false;
                this.taggerStatus.value.done = true;
                this.taggerStatus.value.progress = 1.0;
                this.taggerStatus.value.data = json.data;
                this.onTaggingDone(json.path);
                break;
            // Player load track
            case 'playerLoad':
                this.player.value.duration = json.duration;
                this.player.value.position = 0;
                this.player.value.playing = false;
                this.player.value.title = json.title;
                this.player.value.artists = json.artists;
                // Resolve promise
                if (this.player.value._playerLoadResolve) {
                    this.player.value._playerLoadResolve();
                    this.player.value._playerLoadResolve = undefined;
                }
                break;
            case 'playerSync':
                this.player.value.playing = json.playing;
                // Play after seeking
                if (this.settings.value.playOnSeek && !json.playing)
                    this.player.value.play();
                break;
            // Spotify
            case 'spotifyAuthorized':
                this.spotify.value.authorized = json.value;
                break;
            // Folder browser
            case 'folderBrowser':
                this.onFolderBrowserEvent(json);
                break;
            // Manual tag applied tags
            case 'manualTagApplied':
                this.manualTag.value._resolveSaving!(json);
                break;

            // Callback from config
            case 'configCallback':
                let response = json.response as ConfigCallbackResponse;
                switch (response.type) {
                    case 'empty':
                        break;
                    case 'error':
                        this.onError(response.error);
                        break;
                    case 'updateConfig':
                        if (['__proto__', 'constructor', 'prototype'].includes(json.platform)) {
                            console.error(`Invalid platform key: ${json.platform}`);
                            break;
                        }
                        Object.assign(this.config.value.custom[json.platform], response.config);
                        break;
                }
                break;
            // Platforms loaded
            case 'loadPlatforms':
                this.info.value.platforms = json.platforms;
                this.send('defaultCustomPlatformSettings');
                this.platformsLoading.value = false;
                break;

            // Tangle sync
            case 'tangleStatus':
                this.tangleStatus.value = json.status;
                break;
            case 'tanglePreview':
                this.tanglePreview.value = { tags: json.tags, entries: json.entries };
                break;
            case 'tangleConnected':
                this.tangleError.value = null;
                this.tangleUser.value = json.user ?? null;
                this.send('tangleStatus');
                break;
            case 'tangleError':
                this.tangleError.value = json.code;
                break;

            // Show notification
            case 'notify':
                notify({
                    position: 'top-right',
                    timeout: 5000,
                    progress: true,
                    message: json.message
                });
                break

            // Debug
            default:
                // Tag editor
                if (json.action.startsWith('tagEditor')) {
                    this.onTagEditorEvent(json);
                    break;
                }
                // Renamer
                if (json.action.startsWith('renamer')) {
                    this.onRenamerEvent(json);
                    break;
                }

                console.log(`Unknown action: ${json.action}`);
                console.log(json);
                break;
        }
    }

    // Handle message from Webview/OS
    onOSMessage(json: any) {
        let route = router.currentRoute.value.path.substring(1).split('/')[0].toLowerCase();

        switch (json.action) {
            // Drag and drop path
            case 'browse':
                // Callback by route
                switch (route) {
                    case 'autotagger':
                        this.config.value.path = json.path;
                        break;
                    case 'audiofeatures':
                        this.onAudioFeaturesEvent(json);
                        break;
                    case 'tageditor':
                        this.onTagEditorEvent(json);
                        break;
                    case 'renamer':
                        this.onRenamerEvent(json);
                        break;
                    default:
                        this.settings.value.path = json.path;
                        break;
                }
                break;

            // Exit app
            case 'exit':
                this.exitWithStagePrompt();
                break;
            default:
                console.log(`Unknown OS action: ${json}`);
                break;
        }
    }

    // Open URL in external browser
    url(url: string) {
        this.send('browser', {url});
    }

    // Open native folder browser
    browse(context: string, path?: string) {
        if (this.settings.value.nonNativeBrowser) {
            this.folderBrowser.value.context = context;
            if (path)
                this.folderBrowser.value.basePath = path;
            this.folderBrowser.value.open = true;
            return;
        }
        this.send('browse', { context, path });        
    }

    // onBrowse event
    onBrowse(json: BrowseEvent) {
        // FlowTag library folder
        if (json.context == 'tagFolderAdd' && json.path)
            this.send('tagFolderAdd', { path: json.path });
        // Autotagger path
        if (json.context == 'at')
            this.config.value.path = json.path;
        // Audio features path
        if (json.context == 'af')
            this.onAudioFeaturesEvent(json);
        // Tag editor
        if (json.context == 'te')
            this.onTagEditorEvent(json);
        // Renamer
        if (json.context == 'rn' || json.context == 'rnOutput')
            this.onRenamerEvent(json)
    }

    // Save settings to file
    saveSettings(notif = true) {
        this.settings.value.saveATProfile(this.settings.value.autoTaggerProfile, this.config.value);
        // Very dirty way to clone a dict, but eh
        this.settings.value.autoTaggerConfig = JSON.parse(JSON.stringify(this.config.value));
        this.settings.value.volume = this.player.value.volume;
        // Save
        this.send("saveSettings", {settings: JSON.parse(JSON.stringify(this.settings.value))});
        // Notification
        if (notif)
            notify({
                message: "Settings saved",
                timeout: 500,
                position: 'top-right'
            });
    }

    // Load settings from JSON
    loadSettings(data: any) {
        this.settings.value = Settings.fromJson(data);
        
        // AT config (nested)
        this.config.value.loadSettings(this.settings.value.autoTaggerConfig);
 
        // Restore specific
        this.player.value.volume = this.settings.value.volume??0.5;
        this.player.value.audio.volume = this.player.value.volume;
        this.player.value.setVolume(this.player.value.volume);
        this.spotify.value.clientId = this.settings.value.audioFeatures.spotifyClientId;
        this.spotify.value.clientSecret = this.settings.value.audioFeatures.spotifyClientSecret;

        // The backend doesn't persist the Tangle switch: resend it, then ask for state
        if (this.settings.value.flowtag.tangleSync) this.send('tangleSetEnabled', { enabled: true });
        this.send('tangleStatus');
    }

    // Load autotagger profile
    loadATProfile(name: string) {
        let profile = this.settings.value.autoTaggerProfiles.find(p => p.name == name);
        if (profile == null) throw Error('Invalid profile');

        this.config.value.loadSettings(profile.config);
        this.settings.value.autoTaggerProfile = name;
        // Update custom values in case
        this.send('defaultCustomPlatformSettings');
    }

    /// Load AT platforms
    loadPlatforms() {
        this.send('loadPlatforms');
        this.platformsLoading.value = true;
    }

    /// The pre-existing exit paths: confirm if tagging runs, otherwise quit
    private exitNormally() {
        // AT
        if (this.lock.value.locked && !this.taggerStatus.value.done) {
            confirmDialog({
                title: 'Tagging in progress',
                message: 'Tagging is still running. Exit anyway?',
                okLabel: 'Exit anyway',
                destructive: true,
            }).then((ok) => { if (ok) this.forceExit(); });
        }
        // Exit anyway
        else {
            try {
                // @ts-ignore
                window.ipc.postMessage('exit');
            } catch (e) {
                this.send('exit');
            }
        }
    }

    /// Quit flow: offer to commit ready staged tracks first, then the normal exit paths
    private exitWithStagePrompt() {
        const store = getTagStore();
        runQuitFlow({
            stageReady: store.stageReady.value,
            readyIds: store.stageRows.value.filter((r) => r.ready).map((r) => r.trackId),
            taggingRunning: this.lock.value.locked && !this.taggerStatus.value.done,
            confirm: () => choiceDialog({
                title: `Commit ${store.stageReady.value} ready staged tracks first?`,
                okLabel: 'Commit & quit',
                cancelLabel: 'Quit anyway',
            }),
            commitAndWait: (ids) => store.commitStageAndWait(ids, 5000),
            forceExit: () => this.forceExit(),
            exitNormally: () => this.exitNormally(),
        }).catch((e) => console.error(e));
    }

    /// Exit while the tagger holds the main connection: use a fresh socket
    forceExit() {
        const ws = new WebSocket(wsUrl());
        ws.onopen = () => ws.send(JSON.stringify({ action: 'exit' }));
    }

    /// Stop the tagging process
    stopTagging() {
        let ws = new WebSocket(wsUrl());
        ws.onopen = () => {
            ws.send(JSON.stringify({action: 'stopTagging'}));
        };
    }

    // Handle keydown event for keyboard bindings
    handleKeyDown(event: KeyboardEvent) {
        // FlowTag view has its own keyboard map
        if (router.currentRoute.value.path.startsWith('/quicktag')) {
            return handleFlowKeys(event);
        }

        // Tag editor save
        if (event.code == "KeyS" && (event.ctrlKey || event.metaKey) && this.onTagEditorEvent) {
            this.onTagEditorEvent({action: '_tagEditorSave'});
            return true;
        }

        return false;
    }
    
}

interface AppInfo {
    version: string;
    os: string;
    arch: string;
    customPlatformCompat: number;
    ready: boolean;
    platforms: AutotaggerPlatform[];
    renamerDocs: any,
    commit: string,
    workDir: string,
    dataDir: string,
    startContext: { serverMode: boolean, expose: boolean, startPath?: string, browser: boolean },
    log?: string,
}

class FolderBrowser {
    open: boolean = false;
    basePath: string = '/';
    context?: string;
}

interface BrowseEvent {
    path: string;
    context?: string;
    action?: string;
}


/// Get OneTagger instance
function get1t(): OneTagger {
    return new OneTagger();
}

export { OneTagger, get1t };