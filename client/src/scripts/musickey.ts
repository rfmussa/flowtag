// Musical key helpers (Camelot / OpenKey colors)

const KEY_COLORS: Record<string, string> = {
    "12A": "#00e5e5",
    "12B": "#00e5e5",
    "01A": "#00d58f",
    "01B": "#00d58f",
    "02A": "#3ffb3f",
    "02B": "#3ffb3f",
    "03A": "#97fb00",
    "03B": "#97fb00",
    "04A": "#fed600",
    "04B": "#fed600",
    "05A": "#f98c28",
    "05B": "#f98c28",
    "06A": "#fe642d",
    "06B": "#fe642d",
    "07A": "#f94949",
    "07B": "#f94949",
    "08A": "#fe3fea",
    "08B": "#fe3fea",
    "09A": "#ac64fe",
    "09B": "#ac64fe",
    "10A": "#3e89fa",
    "10B": "#3e89fa",
    "11A": "#00c9fe",
    "11B": "#00c9fe"
};

const CAMELOT_KEYS: Record<string, string> = {
    "ABM" :"01A",
    "G#M" :"01A",
    "B"   :"01B",
    "D#M" :"02A",
    "EBM" :"02A",
    "GB"  :"02B",
    "F#"  :"02B",
    "A#M" :"03A",
    "BBM" :"03A",
    "C#"  :"03B",
    "DB"  :"03B",
    "DD"  :"03B",
    "FM"  :"04A",
    "G#"  :"04B",
    "AB"  :"04B",
    "CM"  :"05A",
    "D#"  :"05B",
    "EB"  :"05B",
    "GM"  :"06A",
    "A#"  :"06B",
    "BB"  :"06B",
    "DM"  :"07A",
    "F"   :"07B",
    "AM"  :"08A",
    "C"   :"08B",
    "EM"  :"09A",
    "G"   :"09B",
    "BM"  :"10A",
    "D"   :"10B",
    "GBM" :"11A",
    "F#M" :"11A",
    "A"   :"11B",
    "C#M" :"12A",
    "DBM" :"12A",
    "E"   :"12B",
}

const OPENKEY_KEYS: Record<string, string> = {
    "5m":   "12A",
    "5d":   "12B",
    "6m":   "01A",
    "6d":   "01B",
    "7m":   "02A",
    "7d":   "02B",
    "8m":   "03A",
    "8d":   "03B",
    "9m":   "04A",
    "9d":   "04B",
    "10m":  "05A",
    "10d":  "05B",
    "11m":  "06A",
    "11d":  "06B",
    "12m":  "07A",
    "12d":  "07B",
    "1m":   "08A",
    "1d":   "08B",
    "2m":   "09A",
    "2d":   "09B",
    "3m":   "10A",
    "3d":   "10B",
    "4m":   "11A",
    "4d":   "11B"
}

// Get color (hex value) for a musical key, or undefined when unknown
function keyColor(key?: string): string | undefined {
    if (!key) return;
    const raw = key.trim();
    // Camelot or OpenKey notation mapped to a wheel position
    let color = KEY_COLORS[CAMELOT_KEYS[raw.toUpperCase()]] || KEY_COLORS[OPENKEY_KEYS[raw.toLowerCase()]];
    // Wheel position given directly ("8A", zero-padded or not)
    if (!color) {
        let padded = raw.toUpperCase();
        if (padded.length < 3) padded = `0${padded}`;
        color = KEY_COLORS[padded];
    }
    return color;
}

/** keyColor as a full CSS declaration, for :style string bindings */
function keyColorStyle(key?: string): string | undefined {
    const color = keyColor(key);
    return color ? `color: ${color};` : undefined;
}

export { CAMELOT_KEYS, KEY_COLORS, OPENKEY_KEYS, keyColor, keyColorStyle };
