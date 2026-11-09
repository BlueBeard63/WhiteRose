// The Displays page's logic, kept free of QML so it can be tested
// (shell/tests/run.mjs).
//
// The page edits a *draft*: one entry per connected monitor, built from what
// the compositor reports. Applying turns the draft back into the "displays"
// setting (the shape of the daemon's config.Displays):
//   { monitor: [{ id, label, mode: "2560x1440@165", scale, transform, vrr }],
//     profile: [{ name, output: [{ id, position: [x, y], disabled }] }] }
// Per-monitor preferences follow the monitor by ID; placement belongs to the
// profile for this exact set of monitors.
.pragma library

const SWAPS = ["90", "270", "flipped-90", "flipped-270"];

// "2560x1440@165", or "2560x1440@59.951" (refresh is in mHz).
function modeString(m) {
    if (!m) return "";
    const hz = Math.round(m.refresh) / 1000;
    return m.width + "x" + m.height + "@" + hz;
}

function hzLabel(refresh) {
    const hz = refresh / 1000;
    return (Math.abs(hz - Math.round(hz)) < 0.03 ? Math.round(hz) : hz.toFixed(2)) + " Hz";
}

// Distinct resolutions, largest first: [{ width, height, preferred }].
function resolutions(modes) {
    const seen = {};
    const out = [];
    for (const m of modes ?? []) {
        const k = m.width + "x" + m.height;
        if (seen[k]) { seen[k].preferred = seen[k].preferred || m.preferred; continue; }
        seen[k] = { width: m.width, height: m.height, preferred: !!m.preferred };
        out.push(seen[k]);
    }
    return out.sort((a, b) => b.width * b.height - a.width * a.height || b.width - a.width);
}

// Refresh rates (mHz) offered at a resolution, highest first.
function refreshes(modes, width, height) {
    const r = (modes ?? []).filter(m => m.width === width && m.height === height).map(m => m.refresh);
    return [...new Set(r)].sort((a, b) => b - a);
}

// The same resolution at `refresh`, else the nearest rate it has.
function withResolution(modes, width, height, refresh) {
    const rates = refreshes(modes, width, height);
    if (rates.length === 0) return null;
    const best = rates.reduce((a, b) => Math.abs(b - refresh) < Math.abs(a - refresh) ? b : a);
    return { width, height, refresh: best };
}

// Size in the shared (logical) space.
function logicalSize(d) {
    if (!d.mode) return { w: 0, h: 0 };
    const s = d.scale > 0 ? d.scale : 1;
    const w = Math.round(d.mode.width / s), h = Math.round(d.mode.height / s);
    return SWAPS.includes(d.transform) ? { w: h, h: w } : { w, h };
}

function defaultLabel(o) {
    if (o.builtin) return "Built-in";
    return o.model || o.connector;
}

// The draft for what's connected now (outputs as the compositor reports them).
function draftFrom(outputs, saved) {
    const monitors = (saved && saved.monitor) || [];
    return (outputs ?? []).map(o => {
        const m = monitors.find(x => x.id === o.id);
        return {
            id: o.id,
            strong: !!o.strong,
            builtin: !!o.builtin,
            connector: o.connector,
            make: o.make,
            model: o.model,
            modes: o.modes ?? [],
            label: (m && m.label) || defaultLabel(o),
            mode: o.mode ? { width: o.mode.width, height: o.mode.height, refresh: o.mode.refresh } : null,
            scale: o.scale,
            transform: o.transform || "normal",
            position: [o.position[0], o.position[1]],
            enabled: o.enabled !== false,
            vrr: !!o.vrr,
        };
    });
}

function rect(d) {
    const s = logicalSize(d);
    return { x: d.position[0], y: d.position[1], w: s.w, h: s.h };
}

function overlaps(a, b) {
    return a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
}

// Snaps a dragged monitor `r` ({x, y, w, h}, logical) to the edges of
// `others` within `threshold`: side by side (left to right, right to left)
// and aligned (tops, bottoms, lefts, rights). Returns { x, y, to } where `to`
// is the index of the monitor it snapped to (or -1).
function snap(r, others, threshold) {
    let best = { dx: threshold + 1, dy: threshold + 1, x: r.x, y: r.y, toX: -1, toY: -1 };
    others.forEach((o, i) => {
        for (const [from, to] of [[r.x, o.x + o.w], [r.x + r.w, o.x], [r.x, o.x], [r.x + r.w, o.x + o.w]]) {
            const d = Math.abs(to - from);
            if (d < best.dx) { best.dx = d; best.x = r.x + to - from; best.toX = i; }
        }
        for (const [from, to] of [[r.y, o.y + o.h], [r.y + r.h, o.y], [r.y, o.y], [r.y + r.h, o.y + o.h]]) {
            const d = Math.abs(to - from);
            if (d < best.dy) { best.dy = d; best.y = r.y + to - from; best.toY = i; }
        }
    });
    return {
        x: best.dx <= threshold ? best.x : r.x,
        y: best.dy <= threshold ? best.y : r.y,
        to: best.dx <= threshold ? best.toX : best.dy <= threshold ? best.toY : -1,
    };
}

// Moves `r` the least distance needed so it overlaps none of `others`.
function resolveOverlap(r, others) {
    for (let guard = 0; guard < others.length * 2 + 1; guard++) {
        const hit = others.find(o => overlaps(r, o));
        if (!hit) return r;
        const moves = [
            { x: hit.x - r.w, y: r.y }, { x: hit.x + hit.w, y: r.y },
            { x: r.x, y: hit.y - r.h }, { x: r.x, y: hit.y + hit.h },
        ];
        moves.sort((a, b) => (Math.abs(a.x - r.x) + Math.abs(a.y - r.y)) - (Math.abs(b.x - r.x) + Math.abs(b.y - r.y)));
        r = Object.assign({}, r, moves.find(m => !others.some(o => overlaps(Object.assign({}, r, m), o))) ?? moves[0]);
    }
    return r;
}

// True if every enabled monitor touches another (no islands left floating
// apart, which the pointer couldn't cross to).
function connected(draft) {
    const on = draft.filter(d => d.enabled).map(rect);
    if (on.length < 2) return true;
    const touch = (a, b) => a.x <= b.x + b.w && b.x <= a.x + a.w && a.y <= b.y + b.h && b.y <= a.y + a.h;
    const seen = new Set([0]);
    const queue = [0];
    while (queue.length) {
        const i = queue.shift();
        on.forEach((r, j) => { if (!seen.has(j) && touch(on[i], r)) { seen.add(j); queue.push(j); } });
    }
    return seen.size === on.length;
}

// Shifts everything so the layout starts at (0, 0).
function normalize(draft) {
    const on = draft.filter(d => d.enabled);
    if (on.length === 0) return draft;
    const minX = Math.min(...on.map(d => d.position[0])), minY = Math.min(...on.map(d => d.position[1]));
    return draft.map(d => Object.assign({}, d, { position: [d.position[0] - minX, d.position[1] - minY] }));
}

// Drops monitor `index` at (x, y): snapped, never overlapping, normalized.
function place(draft, index, x, y, threshold) {
    const others = draft.filter((d, i) => i !== index && d.enabled).map(rect);
    const r = Object.assign(rect(draft[index]), { x: Math.round(x), y: Math.round(y) });
    const s = snap(r, others, threshold);
    const free = resolveOverlap(Object.assign({}, r, { x: s.x, y: s.y }), others);
    const next = draft.map((d, i) => i === index ? Object.assign({}, d, { position: [free.x, free.y] }) : d);
    return normalize(next);
}

function sameSet(a, b) {
    return a.length === b.length && a.every(x => b.includes(x));
}

// Profiles made for exactly these monitors, in priority order.
function profilesFor(saved, ids) {
    return ((saved && saved.profile) || []).filter(p => sameSet(p.output.map(o => o.id), ids));
}

// A name for a new profile: the monitors' labels.
function profileName(draft, saved) {
    const base = draft.map(d => d.label).join(" + ");
    const taken = ((saved && saved.profile) || []).map(p => p.name);
    let name = base, n = 2;
    while (taken.includes(name)) name = base + " " + n++;
    return name;
}

// The "displays" setting with the draft applied. `profile` names the profile
// to store the layout in (created if new, moved first so it wins); with one
// monitor there's no layout to keep.
function toConfig(saved, draft, profile) {
    const cfg = {
        monitor: ((saved && saved.monitor) || []).map(m => Object.assign({}, m)),
        profile: ((saved && saved.profile) || []).map(p => ({ name: p.name, output: p.output.map(o => Object.assign({}, o)) })),
    };
    for (const d of draft) {
        const m = {
            id: d.id,
            label: d.label === defaultLabel(d) ? "" : d.label,
            mode: modeString(d.mode),
            scale: d.scale === 1 ? 0 : d.scale,
            transform: d.transform === "normal" ? "" : d.transform,
            vrr: d.vrr ? "on" : "",
        };
        // Drop empty fields, as the daemon writes them.
        for (const k of ["label", "mode", "scale", "transform", "vrr"]) if (!m[k]) delete m[k];
        const i = cfg.monitor.findIndex(x => x.id === d.id);
        if (i >= 0) cfg.monitor[i] = m; else cfg.monitor.push(m);
    }
    if (draft.length > 1) {
        const name = profile || profileName(draft, saved);
        const p = {
            name,
            output: draft.map(d => {
                const o = { id: d.id, position: [d.position[0], d.position[1]] };
                if (!d.enabled) o.disabled = true;
                return o;
            }),
        };
        cfg.profile = [p].concat(cfg.profile.filter(x => x.name !== name));
    }
    return cfg;
}

// Has the draft changed anything that applying would change?
function differs(a, b) {
    const key = d => JSON.stringify([d.id, d.label, d.mode && modeString(d.mode), d.scale, d.transform, d.position, d.enabled, d.vrr]);
    return a.length !== b.length || a.some((d, i) => key(d) !== key(b[i]));
}
