// Touchpad gestures (Settings › Mouse & touchpad › Gestures; the
// compositor's gestures.rs): "gestures.bindings" maps "<fingers>-<gesture>"
// to "app", "workspaces" or a shortcut's id. A gesture not in it is the
// app's. Shared by the Settings section and the SUPER+K sheet.
.pragma library

// What the compositor starts with (keys.go has the same).
const DEFAULT = { "3-swipe-horizontal": "workspaces" };

// The gestures, in the order the page lists them; `cap`: how the sheet
// draws it.
const KINDS = [
    { id: "swipe-horizontal", label: "Swipe left or right", cap: "SWIPE ←→", swipe: true },
    { id: "swipe-up", label: "Swipe up", cap: "SWIPE ↑", swipe: true },
    { id: "swipe-down", label: "Swipe down", cap: "SWIPE ↓", swipe: true },
    { id: "pinch-in", label: "Pinch in", cap: "PINCH IN", swipe: false },
    { id: "pinch-out", label: "Pinch out", cap: "PINCH OUT", swipe: false },
];

function bindings(v) {
    return v && typeof v === "object" ? v : DEFAULT;
}

// What `fingers` doing `kind` is set to ("app" if nothing).
function actionOf(b, fingers, kind) {
    return bindings(b)[fingers + "-" + kind] ?? "app";
}

// The setting with one gesture changed ("app" takes it out).
function set(b, fingers, kind, action) {
    const out = Object.assign({}, bindings(b));
    const key = fingers + "-" + kind;
    if (action === "app") delete out[key];
    else out[key] = action;
    return out;
}

// A shortcut a gesture can run: one action, not a range of keys (workspace
// 1–9, arrows), and not a scratchpad (those have their own keys).
function usable(s) {
    if (!s || String(s.id).includes(":")) return false;
    return !(s.defaults ?? []).some(k => /1-9|ARROWS/i.test(k));
}

// The dropdown for `kind`: the app, what follows the fingers, then every
// shortcut it can run (its group shown faintly).
function options(kind, shortcuts) {
    const out = [{ label: "Pass to app", value: "app" }];
    if (kind === "swipe-horizontal") out.push({ label: "Switch workspace", value: "workspaces" });
    for (const s of (shortcuts ?? []).filter(usable))
        out.push({ label: s.name, value: s.id, hint: s.command != null ? "Yours" : s.group });
    return out;
}

// The rows for the sheet: [{ label, keys }] for every gesture that isn't
// the app's (with drag on, the three-finger swipes are on four fingers),
// and the drag itself.
function sheet(b, drag, shortcuts) {
    const names = {};
    for (const s of shortcuts ?? []) names[s.id] = s.name;
    const rows = [];
    if (drag) rows.push({ label: "Move a window", keys: ["3 FINGERS", "DRAG"] });
    for (const fingers of [3, 4, 2]) {
        for (const k of KINDS) {
            if (fingers === 2 && k.swipe) continue;
            // With drag on, four-finger swipes do what three-finger ones were set to.
            if (drag && k.swipe && fingers === 4) continue;
            const a = actionOf(b, fingers, k.id);
            if (a === "app") continue;
            const shown = drag && k.swipe && fingers === 3 ? 4 : fingers;
            const label = a === "workspaces" ? "Switch workspace" : names[a];
            if (!label) continue;
            rows.push({ label: label, keys: [shown + " FINGERS", k.cap] });
        }
    }
    return rows;
}
