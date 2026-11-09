/** Tag-frame names typed by the user, per container format */
export type FrameFormat = 'id3' | 'vorbis' | 'mp4';

// Frames the tagger writes itself: overriding them can corrupt files
const RESERVED: Record<FrameFormat, readonly string[]> = {
    id3: ['APIC', 'TXXX', 'POPM', 'TYER', 'TDAT', 'TORY', 'TRDA', 'TRCK', 'TDTG', 'TSOT', 'TIT2',
        'TDOR', 'TKEY', 'TSOC', 'TCMP', 'TBPM', 'TSOP', 'TSO2', 'TSOA', 'SYLT', 'TSRC'],
    vorbis: ['METADATA_BLOCK_PICTURE', 'DATE', 'ORIGINALDATE', 'TRACKNUMBER', 'TAGGINGTIME',
        'TRACK', 'TITLESORT', 'ORIGYEAR', 'INITIALKEY', 'KEY', 'COMPOSERSORT', 'COMPILATION',
        'BPM', 'ARTISTSORT', 'ALBUMARTISTSORT', 'ALBUMSORT', 'POPULARIMETER', 'RATING', 'ISRC'],
    mp4: [],
};

// The tag editor writes fewer frames itself, so fewer are off limits there
const RESERVED_TAG_EDITOR: Record<FrameFormat, readonly string[]> = {
    id3: ['APIC', 'TXXX', 'POPM', 'COMM', 'USLT'],
    vorbis: [],
    mp4: [],
};

/** An ID3 name that isn't a 4-char frame id is a custom (TXXX) field: free-form */
function isId3Custom(format: FrameFormat, name: string): boolean {
    return format === 'id3' && name.length !== 4;
}

/** Why a frame name is unsafe, or undefined when it's fine (most serious problem wins) */
export function validateFrameName(format: FrameFormat, raw: string, tagEditor = false): string | undefined {
    const name = raw.trim();
    if (!name) return "Shouldn't be empty";
    const upper = name.toUpperCase();
    if ((tagEditor ? RESERVED_TAG_EDITOR : RESERVED)[format].includes(upper)) {
        return `${upper} is reserved and might corrupt the file`;
    }
    if (name.includes(' ') && !isId3Custom(format, name)) return "Tag names shouldn't contain spaces";
    if (format !== 'mp4' && upper !== name && !isId3Custom(format, name)) return 'Tag names should be uppercase';
    return undefined;
}

/** Autocomplete suggestions read 'TCON (Genre)': keep only the frame id */
export function stripFrameHelper(format: FrameFormat, value: string): string {
    if ((format === 'id3' || format === 'mp4') && /[©a-zA-Z0-9]{4} \(.*\)/.test(value)) {
        return value.replace(/ \(.*\)/, '');
    }
    return value;
}
