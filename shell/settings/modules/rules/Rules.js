.pragma library
// Window rules (windows.rules) for the Window rules page: reading, editing
// and describing them. A rule is { app?, title?, ...actions }; see
// compositor/src/rules.rs for what each action does.

// `s` as a regular expression that matches only itself.
function escape(s) {
    return String(s).replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

// The pattern that matches exactly this app id.
function exactly(appId) {
    return "^" + escape(appId) + "$";
}

// The app id an exact pattern stands for, or "" if it's another pattern.
function exactApp(pattern) {
    const m = /^\^((?:[^.*+?^${}()|[\]\\]|\\[.*+?^${}()|[\]\\])+)\$$/.exec(pattern ?? "");
    return m ? m[1].replace(/\\(.)/g, "$1") : "";
}

// Why `p` isn't a pattern the compositor takes, or "" if it is. JavaScript
// checks the syntax; what Rust's regex doesn't have (lookaround and
// backreferences) is refused here too.
function patternError(p) {
    if (!p) return "";
    if (/\(\?<?[=!]/.test(p)) return "Lookahead and lookbehind aren't supported";
    if (/\\[1-9]/.test(p)) return "Backreferences aren't supported";
    try {
        new RegExp(p);
    } catch (e) {
        return String(e.message).replace(/^Invalid regular expression: /, "");
    }
    return "";
}

// Whether a rule can be saved: it matches something, with patterns that work.
function complete(r) {
    return !!(r && (r.app || r.title)) && !patternError(r.app) && !patternError(r.title);
}

// Whether `v` is a [a, b] pair. (Not Array.isArray: a rule that went
// through a Repeater's modelData has Qt lists inside, not JS arrays.)
function isPair(v) {
    return v !== null && typeof v === "object" && v.length === 2;
}

// "900" → 900, "60%" / "60 %" → "60%"; anything else → null. `min`: the
// smallest pixel count (1 for a size, 0 for a position).
function parseLength(s, min) {
    const t = String(s ?? "").trim();
    const pct = /^(\d+(?:\.\d+)?)\s*%$/.exec(t);
    if (pct) return Number(pct[1]) <= 100 ? Number(pct[1]) + "%" : null;
    if (!/^\d+$/.test(t)) return null;
    const n = Number(t);
    return n >= (min ?? 0) && n <= 100000 ? n : null;
}

function lengthText(v) {
    return v === undefined || v === null ? "" : String(v);
}

// A copy of rule `i` of `rules` with `key` set to `value` (removed for
// undefined, null or "").
function set(rules, i, key, value) {
    return rules.map((r, j) => {
        if (j !== i) return r;
        const out = Object.assign({}, r);
        if (value === undefined || value === null || value === "") delete out[key];
        else out[key] = value;
        return out;
    });
}

// Rule `i` moved one place up (-1) or down (+1).
function move(rules, i, by) {
    const j = i + by;
    if (i < 0 || j < 0 || i >= rules.length || j >= rules.length) return rules.slice();
    const out = rules.slice();
    const r = out[i];
    out[i] = out[j];
    out[j] = r;
    return out;
}

function remove(rules, i) {
    return rules.filter((_, j) => j !== i);
}

// What a rule is called: its app id, else its pattern, else its title's.
function name(r) {
    return exactApp(r?.app) || r?.app || r?.title || "New rule";
}

// Its "tiling", "position" and "opens" choices, as the page shows them.
function tiling(r) {
    return r?.float === true ? "float" : r?.float === false ? "tile" : "";
}
function position(r) {
    return r?.position === "centre" || r?.position === "center" ? "centre" : isPair(r?.position) ? "at" : "";
}
function opens(r) {
    return r?.fullscreen ? "fullscreen" : r?.maximized ? "maximized" : "";
}

// One line saying what a rule does, after its number (1-based `n`).
// `monitorName`: a monitor id to the name shown.
function summary(r, n, monitorName) {
    if (!r?.app && !r?.title) return n + " · Not saved: it needs an app id or a title";
    const does = [];
    if (r.workspace) does.push("Workspace " + r.workspace);
    if (r.monitor) does.push("On " + (monitorName ? monitorName(r.monitor) : r.monitor));
    const t = tiling(r);
    if (t) does.push(t === "float" ? "Floats" : "Tiles");
    if (isPair(r.size)) does.push(r.size[0] + " × " + r.size[1]);
    const p = position(r);
    if (p === "centre") does.push("Centred");
    if (p === "at") does.push("At " + r.position[0] + ", " + r.position[1]);
    const o = opens(r);
    if (o) does.push(o === "fullscreen" ? "Full screen" : "Maximised");
    if (does.length === 0) does.push("Does nothing yet");
    return [String(n)].concat(r.app && r.title ? ["Title " + r.title] : [], does).join(" · ");
}

// The open windows to offer a rule for: one per app id (those with one),
// in the order given. `rules`: apps that already have an exact rule are
// marked.
function offers(windows, rules) {
    const seen = {};
    const out = [];
    for (const w of windows ?? []) {
        const app = w.app_id ?? "";
        if (!app || seen[app]) continue;
        seen[app] = true;
        out.push({ app: app, title: w.title ?? "", workspace: w.workspace, has: (rules ?? []).some(r => exactApp(r.app) === app) });
    }
    return out;
}
