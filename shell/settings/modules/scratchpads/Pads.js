.pragma library
.import "../rules/Rules.js" as R
// Scratchpads (windows.scratchpads) for the Scratchpads page: making,
// describing and checking them. A pad is { id, name, command, keys,
// size?, place?, hideOnBlur? }; see compositor/src/scratchpad.rs.

const DEFAULT_SIZE = ["60%", "50%"];

// Apps that hand a new window to the copy already running unless asked
// not to, and how to ask: the key must start the process that opens it.
const OWN_INSTANCE = { ghostty: "--gtk-single-instance=false", ptyxis: "--standalone" };

// `cmd` with what its app needs to open its own window, if it's one of those.
function ownInstance(cmd) {
    const parts = String(cmd ?? "").trim().split(/\s+/).filter(p => p);
    if (parts.length === 0) return "";
    const flag = OWN_INSTANCE[parts[0].split("/").pop()];
    if (flag && !parts.some(p => p === flag || p.startsWith(flag.split("=")[0] + "="))) parts.splice(1, 0, flag);
    return parts.join(" ");
}

// Whether a pad can be saved: an id and something to run.
function complete(p) {
    return !!(p && p.id && String(p.command ?? "").trim());
}

// An id not used yet ("pad-1", "pad-2", …).
function newId(pads) {
    const taken = (pads ?? []).map(p => p.id);
    let n = 1;
    while (taken.includes("pad-" + n)) n++;
    return "pad-" + n;
}

// A new pad: the terminal the first time, else a blank one; with `key` if
// it's free (`free(key)` says).
function make(pads, terminal, free) {
    const list = pads ?? [];
    const term = String(terminal || "ptyxis").trim();
    const hasTerminal = list.some(p => (p.command ?? "").split(/\s+/)[0] === term.split(/\s+/)[0]);
    const key = "SUPER+GRAVE";
    return {
        id: newId(list),
        name: hasTerminal ? "Scratchpad " + (list.length + 1) : "Terminal",
        command: hasTerminal ? "" : ownInstance(term),
        keys: !hasTerminal && free(key) ? [key] : [],
    };
}

function name(p) {
    return p.name || (p.command ?? "").split(/\s+/)[0] || "New scratchpad";
}

function place(p) {
    return p.place === "centre" || p.place === "center" ? "centre" : "top";
}

function size(p) {
    return R.isPair(p.size) ? [p.size[0], p.size[1]] : DEFAULT_SIZE.slice();
}

// "SUPER + ` · ghostty · Under the island · 60% × 50%": its card's line.
function summary(p, spoken) {
    const keys = (p.keys ?? []).length > 0 ? p.keys.map(spoken).join(", ") : "No key";
    const app = (p.command ?? "").split(/\s+/)[0].split("/").pop() || "No command";
    const s = size(p);
    return [keys, app, place(p) === "centre" ? "Centred" : "Under the island", s[0] + " × " + s[1]]
        .concat(p.hideOnBlur ? ["Hides when it loses focus"] : []).join(" · ");
}

// A copy of `pads` with pad `i` changed by `patch` (null removes a key).
function set(pads, i, patch) {
    return pads.map((p, j) => {
        if (j !== i) return p;
        const q = Object.assign({}, p);
        for (const k in patch) {
            if (patch[k] === null) delete q[k];
            else q[k] = patch[k];
        }
        return q;
    });
}
