// Keyboard & Shortcuts logic, free of QML so it can be tested
// (shell/tests/run.mjs).
//
// Shortcuts are strings the compositor parses: modifiers then one key,
// "SUPER+SHIFT+S", "PRTSC", "XF86AudioRaiseVolume", "SUPER+1-9" for the
// workspace shortcuts (one per number key) and "SUPER+ARROWS" for the
// direction shortcuts (one per arrow key). Settings store only changes:
// "shortcuts.keys" maps an id to its keys (none = turned off), and
// "shortcuts.custom" holds your own [{ id, name, command, keys }].
// Scratchpads' keys show up as shortcuts "scratchpad:<id>" and are kept in
// "windows.scratchpads" (state.pads), with the rest of each scratchpad.
.pragma library

const MODS = ["SUPER", "CTRL", "ALT", "SHIFT"];
const MOD_ALIASES = { SUPER: "SUPER", LOGO: "SUPER", WIN: "SUPER", MOD4: "SUPER", CTRL: "CTRL", CONTROL: "CTRL", ALT: "ALT", MOD1: "ALT", SHIFT: "SHIFT" };

// Keys on the main block by position (evdev code + 8, which Qt reports as
// the native scan code on Wayland), named as on a US keyboard: shortcuts
// follow the key, whatever layout is active, as the compositor matches them.
const SCAN = {
    10: "1", 11: "2", 12: "3", 13: "4", 14: "5", 15: "6", 16: "7", 17: "8", 18: "9", 19: "0",
    20: "minus", 21: "equal", 24: "Q", 25: "W", 26: "E", 27: "R", 28: "T", 29: "Y", 30: "U", 31: "I",
    32: "O", 33: "P", 34: "bracketleft", 35: "bracketright", 38: "A", 39: "S", 40: "D", 41: "F",
    42: "G", 43: "H", 44: "J", 45: "K", 46: "L", 47: "semicolon", 48: "apostrophe", 49: "grave",
    51: "backslash", 52: "Z", 53: "X", 54: "C", 55: "V", 56: "B", 57: "N", 58: "M", 59: "comma",
    60: "period", 61: "slash",
};

// Qt key codes (stable Qt ABI values) for keys named by what they do.
const QT = {
    0x01000001: "TAB", 0x01000002: "TAB", 0x01000003: "BACKSPACE", 0x01000004: "RETURN", 0x01000005: "RETURN",
    0x01000006: "Insert", 0x01000007: "Delete", 0x01000009: "PRTSC", 0x01000010: "Home", 0x01000011: "End",
    0x01000012: "Left", 0x01000013: "Up", 0x01000014: "Right", 0x01000015: "Down", 0x01000016: "Prior",
    0x01000017: "Next", 0x20: "SPACE",
    0x01000070: "XF86AudioLowerVolume", 0x01000071: "XF86AudioMute", 0x01000072: "XF86AudioRaiseVolume",
    0x01000080: "XF86AudioPlay", 0x01000082: "XF86AudioPrev", 0x01000083: "XF86AudioNext",
    0x01000085: "XF86AudioPlay", 0x010000b2: "XF86MonBrightnessUp", 0x010000b3: "XF86MonBrightnessDown",
    0x01000113: "XF86AudioMicMute",
};
const QT_F1 = 0x01000030, QT_F35 = 0x01000052;
const QT_MODIFIERS = [0x01000020, 0x01000021, 0x01000022, 0x01000023, 0x01000024, 0x01000025, 0x01000053, 0x01000054, 0x01001103];
const QT_ESCAPE = 0x01000000;

function isModifier(key) { return QT_MODIFIERS.includes(key) }
function isEscape(key) { return key === QT_ESCAPE }
function isSuper(key) { return key === 0x01000022 || key === 0x01000053 || key === 0x01000054 }

// The key's name for a shortcut, or "" if it can't be one.
function keyName(key, scanCode) {
    if (key >= QT_F1 && key <= QT_F35) return "F" + (key - QT_F1 + 1);
    if (QT[key]) return QT[key];
    return SCAN[scanCode] ?? "";
}

// "SUPER+CTRL+ALT+SHIFT+KEY" from held modifiers and a key name.
function combo(mods, key) {
    const parts = [];
    if (mods.logo) parts.push("SUPER");
    if (mods.ctrl) parts.push("CTRL");
    if (mods.alt) parts.push("ALT");
    if (mods.shift) parts.push("SHIFT");
    if (key) parts.push(key);
    return parts.join("+");
}

// Modifiers in one order and the key upper-cased, so equal shortcuts compare equal.
function normalize(c) {
    const toks = String(c).split("+").map(t => t.trim()).filter(t => t);
    if (toks.length === 0) return "";
    const key = toks[toks.length - 1].toUpperCase();
    const mods = new Set(toks.slice(0, -1).map(t => MOD_ALIASES[t.toUpperCase()] ?? t.toUpperCase()));
    if (toks.length === 1 && MOD_ALIASES[key] === "SUPER") return "SUPER";
    return MODS.filter(m => mods.has(m)).concat([key]).join("+");
}

// Every concrete combo: "SUPER+1-9" is nine, "SUPER+ARROWS" four.
function expand(c) {
    const n = normalize(c);
    if (n.endsWith("+1-9")) return [1, 2, 3, 4, 5, 6, 7, 8, 9].map(d => n.slice(0, -3) + d);
    if (n.endsWith("+ARROWS")) return ["LEFT", "RIGHT", "UP", "DOWN"].map(k => n.slice(0, -6) + k);
    return [n];
}

// The first other shortcut already using any of `c`'s combos.
function conflict(shortcuts, c, exceptId) {
    const mine = expand(c);
    return (shortcuts ?? []).find(s => s.id !== exceptId && (s.keys ?? []).some(k => expand(k).some(x => mine.includes(x)))) ?? null;
}

// Workspace shortcuts take a number key and stand for all nine
// (range "digits"); direction shortcuts take an arrow key and stand for all
// four ("arrows"). Null if the key doesn't fit the range.
function forRange(c, range) {
    const n = normalize(c);
    if (range === "arrows") {
        const m = n.match(/^(.*\+)?(LEFT|RIGHT|UP|DOWN)$/);
        return m ? (m[1] ?? "") + "ARROWS" : null;
    }
    const m = n.match(/^(.*\+)?([0-9])$/);
    return m ? (m[1] ?? "") + "1-9" : null;
}

const LABELS = {
    XF86AUDIORAISEVOLUME: "VOL +", XF86AUDIOLOWERVOLUME: "VOL −", XF86AUDIOMUTE: "MUTE", XF86AUDIOMICMUTE: "MIC MUTE",
    XF86MONBRIGHTNESSUP: "BRIGHT +", XF86MONBRIGHTNESSDOWN: "BRIGHT −", XF86AUDIOPLAY: "PLAY", XF86AUDIONEXT: "NEXT",
    XF86AUDIOPREV: "PREV", "1-9": "1–9", PRIOR: "PG UP", NEXT: "PG DN", MINUS: "-", EQUAL: "=",
    BRACKETLEFT: "[", BRACKETRIGHT: "]", SEMICOLON: ";", APOSTROPHE: "'", GRAVE: "`", BACKSLASH: "\\",
    COMMA: ",", PERIOD: ".", SLASH: "/", LEFT: "←", RIGHT: "→", UP: "↑", DOWN: "↓", ARROWS: "←→↑↓",
};

// The key caps to draw for a shortcut.
function caps(c) {
    return normalize(c).split("+").filter(t => t).map(t => LABELS[t] ?? t);
}

// "SUPER + SPACE" for sentences.
function spoken(c) { return caps(c).join(" + ") }

function sameList(a, b) {
    const x = (a ?? []).map(normalize), y = (b ?? []).map(normalize);
    return x.length === y.length && x.every((k, i) => k === y[i]);
}

// The id a scratchpad's key goes by among the shortcuts.
const PAD = "scratchpad:";
function padShortcut(padId) { return PAD + padId }

// Gives shortcut `id` the keys `keys`, returning new { overrides, custom,
// pads }. Built-in shortcuts back on their defaults are dropped from the
// overrides.
function setKeys(state, id, keys) {
    const overrides = Object.assign({}, state.overrides ?? {});
    const custom = (state.custom ?? []).map(c => Object.assign({}, c));
    const pads = (state.pads ?? []).map(p => Object.assign({}, p));
    const c = custom.find(x => x.id === id);
    const pad = String(id).startsWith(PAD) ? pads.find(p => PAD + p.id === id) : null;
    if (c) c.keys = keys.slice();
    else if (String(id).startsWith(PAD)) {
        if (pad) pad.keys = keys.slice();
    } else {
        const view = (state.shortcuts ?? []).find(s => s.id === id);
        if (view && sameList(keys, view.defaults)) delete overrides[id];
        else overrides[id] = keys.slice();
    }
    return { overrides, custom, pads };
}

// Records `c` for shortcut `id`: replacing its keys (or adding to them),
// and taking the combo away from whichever shortcut had it.
function assign(state, id, c, add) {
    const n = normalize(c);
    const view = (state.shortcuts ?? []).find(s => s.id === id);
    let next = { overrides: state.overrides, custom: state.custom, pads: state.pads };
    const other = conflict(state.shortcuts, n, id);
    if (other) {
        const mine = expand(n);
        const left = other.keys.filter(k => !expand(k).some(x => mine.includes(x)));
        next = setKeys(Object.assign({}, state, next), other.id, left);
    }
    const keys = add && view ? view.keys.filter(k => normalize(k) !== n).concat([n]) : [n];
    return setKeys(Object.assign({}, state, next), id, keys);
}

// A new custom shortcut's id, unused so far.
function newId(name, custom) {
    const base = "custom-" + (String(name).toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "").slice(0, 40) || "shortcut");
    const taken = (custom ?? []).map(c => c.id);
    let id = base, n = 2;
    while (taken.includes(id)) id = base + "-" + n++;
    return id;
}

// Layouts and variants from XKB's evdev.lst.
function parseLayouts(text) {
    const out = { layouts: [], variants: [] };
    let section = "";
    for (const line of String(text).split("\n")) {
        if (line.startsWith("! ")) { section = line.slice(2).trim(); continue; }
        const m = line.match(/^\s+(\S+)\s+(.*)$/);
        if (!m) continue;
        if (section === "layout") out.layouts.push({ layout: m[1], variant: "", name: m[2].trim() });
        else if (section === "variant") {
            const v = m[2].match(/^([^:]+):\s*(.*)$/);
            if (v) out.variants.push({ layout: v[1], variant: m[1], name: v[2].trim() });
        }
    }
    return out;
}

// Layouts matching a search, by name or code: plain layouts first.
function searchLayouts(db, q) {
    const s = String(q).trim().toLowerCase();
    const all = db.layouts.concat(db.variants);
    if (s === "") return db.layouts;
    return all.filter(l => l.name.toLowerCase().includes(s) || l.layout === s);
}
