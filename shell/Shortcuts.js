// The keyboard shortcut sheet (SUPER+K; Figma: "Island v2 — Shortcuts
// (SUPER+K)"): the shortcuts worth knowing, by group, with the keys they're
// on now (the compositor's list, so a changed one shows its new keys), and
// your own (custom shortcuts and scratchpads) in a group after them.
.pragma library
.import "settings/modules/keyboard/Keys.js" as K

// Which rows show, in order. `id`: a shortcut in the compositor's list;
// `mouse`: a mouse move, drawn as these caps (not a shortcut you can change).
const SHEET = [
    { name: "Windows", rows: [
        { label: "Move", mouse: ["SUPER", "DRAG"] },
        { label: "Resize", mouse: ["SUPER", "RIGHT-DRAG"] },
        { label: "Resize with keys", id: "resize-mode" },
        { label: "Snap to a side", id: "move-direction" },
        { label: "Snap layouts", id: "snap-picker" },
        { label: "Float or tile", id: "toggle-floating" },
        { label: "Fullscreen", id: "fullscreen" },
        { label: "Close", id: "close-window" },
    ] },
    { name: "Workspaces", rows: [
        { label: "Go to workspace", id: "workspace" },
        { label: "Move window there", id: "move-to-workspace" },
        { label: "Move to next or previous", id: "move-to-workspace-step" },
        { label: "Move to a monitor", id: "move-to-monitor" },
        { label: "Focus a window", id: "focus-direction" },
        { label: "Switch windows", id: "app-switcher" },
        { label: "Next window layout", id: "window-layout" },
    ] },
    { name: "Desktop", rows: [
        { label: "Launcher", id: "launcher" },
        { label: "Terminal", id: "terminal" },
        { label: "Quick settings", id: "quick-settings" },
        { label: "Notifications", id: "notifications" },
        { label: "Clipboard", id: "clipboard" },
        { label: "Screenshot an area", id: "screenshot-region" },
        { label: "Lock", id: "lock" },
    ] },
];

// The groups to draw: [{ name, rows: [{ label, keys: ["SUPER", "Q"] }] }].
// A shortcut that's turned off (no keys) is left out; one on several keys
// shows the first.
function groups(shortcuts) {
    const byId = {};
    for (const s of shortcuts ?? []) byId[s.id] = s;
    const out = [];
    for (const g of SHEET) {
        const rows = [];
        for (const r of g.rows) {
            if (r.mouse) { rows.push({ label: r.label, keys: r.mouse }); continue; }
            const s = byId[r.id];
            if (s && s.keys && s.keys.length > 0) rows.push({ label: r.label, keys: K.caps(s.keys[0]) });
        }
        if (rows.length > 0) out.push({ name: g.name, rows: rows });
    }
    const yours = (shortcuts ?? [])
        .filter(s => (s.command !== undefined && s.command !== null || String(s.id).startsWith("scratchpad:")) && s.keys && s.keys.length > 0)
        .map(s => ({ label: s.name, keys: K.caps(s.keys[0]) }));
    if (yours.length > 0) out.push({ name: "Yours", rows: yours });
    return out;
}
