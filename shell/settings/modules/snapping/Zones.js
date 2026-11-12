// Zone grids for snapping (windows.snap.zones): each zone is [x, y, w, h]
// as fractions of the area below the bar. The editor on the Windows page
// splits zones, merges two into one and moves the line between them, and
// only ever saves grids the compositor accepts (layout/snap.rs): 1-16
// zones, each inside the area and at least a tenth of it each way, none
// overlapping.
.pragma library

const EPS = 1e-6;
const MIN = 0.1;
const MAX_ZONES = 16;

function same(a, b) { return Math.abs(a - b) < EPS; }
// Edges are kept to 4 places, edge by edge, so zones that meet share the
// exact same number (and the JSON stays short).
function r(v) { return Math.round(v * 1e4) / 1e4; }

// The grid used where a monitor has none: two thirds and a third.
function fallback() { return [[0, 0, 0.6667, 1], [0.6667, 0, 0.3333, 1]]; }

// Each zone's edges rounded, and its size taken from them (rounded too:
// it's then within 1e-15 of the far edge, and the compositor allows 1e-6).
function normalize(zones) {
    return zones.map(([x, y, w, h]) => {
        const x0 = r(x), y0 = r(y), x1 = r(x + w), y1 = r(y + h);
        return [x0, y0, r(x1 - x0), r(y1 - y0)];
    });
}

// As the compositor and the daemon check it.
function valid(zones) {
    if (!Array.isArray(zones) || zones.length === 0 || zones.length > MAX_ZONES) return false;
    for (const z of zones) {
        if (!Array.isArray(z) || z.length !== 4 || !z.every(Number.isFinite)) return false;
        const [x, y, w, h] = z;
        if (x < -EPS || y < -EPS || w < MIN - EPS || h < MIN - EPS || x + w > 1 + EPS || y + h > 1 + EPS) return false;
    }
    for (let i = 0; i < zones.length; i++) {
        for (let j = 0; j < i; j++) {
            const [ax, ay, aw, ah] = zones[i], [bx, by, bw, bh] = zones[j];
            const ow = Math.min(ax + aw, bx + bw) - Math.max(ax, bx);
            const oh = Math.min(ay + ah, by + bh) - Math.max(ay, by);
            if (ow > EPS && oh > EPS) return false;
        }
    }
    return true;
}

// Zone `i` cut in half, side by side ("side") or one above the other
// ("stacked"). null if the halves would be too small, or too many zones.
function split(zones, i, how) {
    const z = zones[i];
    if (!z) return null;
    const [x, y, w, h] = z;
    let a, b;
    if (how === "side") {
        const at = r(x + w / 2);
        a = [x, y, at - x, h]; b = [at, y, x + w - at, h];
    } else {
        const at = r(y + h / 2);
        a = [x, y, w, at - y]; b = [x, at, w, y + h - at];
    }
    const out = normalize(zones.slice(0, i).concat([a, b], zones.slice(i + 1)));
    return valid(out) ? out : null;
}

// Whether zones `i` and `j` together make a rectangle (side by side with
// the same top and height, or stacked with the same left and width).
function canMerge(zones, i, j) {
    const a = zones[i], b = zones[j];
    if (!a || !b || i === j) return false;
    const sideBySide = same(a[1], b[1]) && same(a[3], b[3]) && (same(a[0] + a[2], b[0]) || same(b[0] + b[2], a[0]));
    const stacked = same(a[0], b[0]) && same(a[2], b[2]) && (same(a[1] + a[3], b[1]) || same(b[1] + b[3], a[1]));
    return sideBySide || stacked;
}

// Zones `i` and `j` as one, in the first one's place. null if they don't
// make a rectangle.
function merge(zones, i, j) {
    if (!canMerge(zones, i, j)) return null;
    const a = zones[i], b = zones[j];
    const x = Math.min(a[0], b[0]), y = Math.min(a[1], b[1]);
    const u = [x, y, Math.max(a[0] + a[2], b[0] + b[2]) - x, Math.max(a[1] + a[3], b[1] + b[3]) - y];
    const first = Math.min(i, j), second = Math.max(i, j);
    const out = zones.slice();
    out[first] = u;
    out.splice(second, 1);
    return normalize(out);
}

// The zones whose edges lie on the line along zone `i`'s `dir` edge
// ("right" or "down"), as long as they line up with it or with each other
// along the line (so the middle of four quarters moves all four). Each as
// [index, far]: far when it's the zone's right or bottom edge on the line.
// null for the area's own edge.
function onLine(zones, i, dir) {
    const z = zones[i];
    if (!z) return null;
    const ax = dir === "right" ? 0 : 1, across = 1 - ax;
    const line = z[ax] + z[ax + 2];
    if (line <= EPS || line >= 1 - EPS) return null;
    const cand = [];
    zones.forEach((o, k) => {
        if (same(o[ax] + o[ax + 2], line)) cand.push([k, true]);
        else if (same(o[ax], line)) cand.push([k, false]);
    });
    const members = [[i, true]];
    let lo = z[across], hi = z[across] + z[across + 2];
    for (let grew = true; grew;) {
        grew = false;
        for (const [k, far] of cand) {
            if (members.some(m => m[0] === k)) continue;
            const s = zones[k][across], e = s + zones[k][across + 2];
            if (s <= hi + EPS && e >= lo - EPS) {
                members.push([k, far]);
                lo = Math.min(lo, s); hi = Math.max(hi, e);
                grew = true;
            }
        }
    }
    return { axis: ax === 0 ? "x" : "y", at: line, lo, hi, members };
}

// Every line between zones that can be dragged, once each: where it is,
// how far it runs, and a zone and edge that name it for moveLine.
function lines(zones) {
    const out = [];
    zones.forEach((z, i) => {
        for (const dir of ["right", "down"]) {
            const l = onLine(zones, i, dir);
            if (!l) continue;
            if (out.some(o => o.axis === l.axis && same(o.at, l.at) && same(o.lo, l.lo) && same(o.hi, l.hi))) continue;
            out.push({ axis: l.axis, at: l.at, lo: l.lo, hi: l.hi, zone: i, dir });
        }
    });
    return out;
}

// The line along zone `i`'s `dir` edge moved to `to` (a fraction), as far
// as every zone on it keeps a tenth. The zones as they are if it can't move.
function moveLine(zones, i, dir, to) {
    const l = onLine(zones, i, dir);
    if (!l) return zones;
    const ax = l.axis === "x" ? 0 : 1;
    let lo = MIN, hi = 1 - MIN;
    for (const [k, far] of l.members) {
        const z = zones[k];
        if (far) lo = Math.max(lo, z[ax] + MIN);
        else hi = Math.min(hi, z[ax] + z[ax + 2] - MIN);
    }
    if (lo > hi) return zones;
    const at = r(Math.min(hi, Math.max(lo, to)));
    const out = zones.map(z => z.slice());
    for (const [k, far] of l.members) {
        const z = out[k];
        if (far) z[ax + 2] = at - z[ax];
        else { z[ax + 2] = z[ax] + z[ax + 2] - at; z[ax] = at; }
    }
    return normalize(out);
}

// The editor's tools: Resize drags the lines between zones; Draw drags out
// a new zone over empty space; the others act on the zone clicked. The tool
// stays picked until another is.
const TOOLS = ["resize", "side", "stacked", "merge", "draw", "delete"];

// The tools as Settings shows them (icons, named), in Settings and on screen.
function toolOptions() {
    return [
        { label: "Resize", value: "resize", icon: "move" },
        { label: "Side by side", value: "side", icon: "columns2" },
        { label: "Stacked", value: "stacked", icon: "rows2" },
        { label: "Merge", value: "merge", icon: "merge" },
        { label: "Draw", value: "draw", icon: "squarePlus" },
        { label: "Delete", value: "delete", icon: "trash" },
    ];
}

// The grid less zone `i`, leaving empty space where it was (a window
// dropped there with SHIFT just floats). null for the last zone.
function remove(zones, i) {
    if (!zones[i] || zones.length <= 1) return null;
    return zones.filter((_, k) => k !== i);
}

// The zone holding the point (x, y) (fractions), or -1 in empty space.
function zoneAt(zones, x, y) {
    return zones.findIndex(z => x > z[0] + EPS && x < z[0] + z[2] - EPS && y > z[1] + EPS && y < z[1] + z[3] - EPS);
}

// Drawn edges land on 24ths of the area, or on another zone's edge within
// SNAP of them (so zones line up and share lines Resize can move).
const GRID = 24;
const SNAP = 0.02;
function snapTo(v, edges) {
    let best = Math.round(v * GRID) / GRID, near = SNAP;
    for (const e of edges) {
        if (Math.abs(e - v) <= near) { best = e; near = Math.abs(e - v); }
    }
    return Math.min(1, Math.max(0, best));
}

// The zone drawn by dragging from (x0, y0), which must be in empty space, to
// (x1, y1): edges snapped, and stopped at the zones around it instead of
// overlapping them. { zone: [x, y, w, h], ok }: ok when it's at least a
// tenth each way and there's room for another zone. null if the drag
// started inside a zone.
function draw(zones, x0, y0, x1, y1) {
    if (zoneAt(zones, x0, y0) >= 0) return null;
    const xs = [0, 1], ys = [0, 1];
    for (const z of zones) { xs.push(z[0], z[0] + z[2]); ys.push(z[1], z[1] + z[3]); }
    let l = snapTo(Math.min(x0, x1), xs), rt = snapTo(Math.max(x0, x1), xs);
    let t = snapTo(Math.min(y0, y1), ys), b = snapTo(Math.max(y0, y1), ys);
    // Each zone it runs into cuts it back to that zone's near edge, on the
    // side away from where the drag started.
    for (let changed = true; changed;) {
        changed = false;
        for (const z of zones) {
            const zr = z[0] + z[2], zb = z[1] + z[3];
            if (Math.min(rt, zr) - Math.max(l, z[0]) <= EPS || Math.min(b, zb) - Math.max(t, z[1]) <= EPS) continue;
            if (z[0] >= x0 - EPS) rt = z[0];
            else if (zr <= x0 + EPS) l = zr;
            else if (z[1] >= y0 - EPS) b = z[1];
            else t = zb;
            changed = true;
        }
    }
    const zone = [r(l), r(t), r(rt - l), r(b - t)];
    const ok = zone[2] >= MIN - EPS && zone[3] >= MIN - EPS && zones.length < MAX_ZONES;
    return { zone, ok };
}

// The grid with `zone` added, or null if the compositor wouldn't take it.
function add(zones, zone) {
    const out = normalize(zones.concat([zone]));
    return valid(out) ? out : null;
}

// What clicking zone `i` with `tool` does, given the zone Merge has picked
// first (`picked`, or -1): { zones: the new grid, or null for no change,
// picked }. Side by side and Stacked split it (null if too small); Merge
// picks a first zone, then merges it with a second that makes a rectangle
// with it (another zone is picked instead; the same one again unpicks it).
function click(zones, tool, picked, i) {
    if (tool === "side" || tool === "stacked") return { zones: split(zones, i, tool), picked: -1 };
    if (tool === "delete") return { zones: remove(zones, i), picked: -1 };
    if (tool !== "merge") return { zones: null, picked: -1 };
    if (picked === i) return { zones: null, picked: -1 };
    if (picked >= 0 && canMerge(zones, picked, i)) return { zones: merge(zones, picked, i), picked: -1 };
    return { zones: null, picked: i };
}

// width / height of the area below a bar `bar` px tall on `output` (as
// the compositor reports it: its mode, scale and rotation); 16:9 if it
// isn't known.
function aspect(output, bar) {
    const m = output?.mode;
    if (!m || !(m.width > 0) || !(m.height > 0)) return 16 / 9;
    const s = output.scale > 0 ? output.scale : 1;
    const turned = ["90", "270", "flipped-90", "flipped-270"].includes(output.transform);
    const w = (turned ? m.height : m.width) / s, h = (turned ? m.width : m.height) / s;
    return h - bar > 0 ? w / (h - bar) : 16 / 9;
}
