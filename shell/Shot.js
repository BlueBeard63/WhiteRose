// The screenshot tool's selection maths, free of QML so it can be tested.
// Rectangles are { x, y, w, h } in screen pixels (logical).
.pragma library

// The rectangle dragged from `a` to `b`; square (the shorter side wins) with Shift.
function fromDrag(a, b, square) {
    let w = b.x - a.x, h = b.y - a.y;
    if (square) {
        const s = Math.min(Math.abs(w), Math.abs(h));
        w = Math.sign(w || 1) * s;
        h = Math.sign(h || 1) * s;
    }
    return { x: Math.min(a.x, a.x + w), y: Math.min(a.y, a.y + h), w: Math.abs(w), h: Math.abs(h) };
}

// Keeps `r` inside `bounds`, sliding it rather than shrinking it.
function clampMove(r, bounds) {
    const w = Math.min(r.w, bounds.w), h = Math.min(r.h, bounds.h);
    return {
        x: Math.max(bounds.x, Math.min(r.x, bounds.x + bounds.w - w)),
        y: Math.max(bounds.y, Math.min(r.y, bounds.y + bounds.h - h)),
        w, h,
    };
}

// Drags handle `edge` ("n", "ne", "e", "se", "s", "sw", "w", "nw") of `r`
// by (dx, dy). The opposite side stays put; dragging past it flips.
function resize(r, edge, dx, dy, square) {
    let x0 = r.x, y0 = r.y, x1 = r.x + r.w, y1 = r.y + r.h;
    if (edge.includes("w")) x0 += dx;
    if (edge.includes("e")) x1 += dx;
    if (edge.includes("n")) y0 += dy;
    if (edge.includes("s")) y1 += dy;
    let out = { x: Math.min(x0, x1), y: Math.min(y0, y1), w: Math.abs(x1 - x0), h: Math.abs(y1 - y0) };
    if (square && edge.length === 2) {
        const s = Math.min(out.w, out.h);
        if (edge.includes("w")) out.x = out.x + out.w - s;
        if (edge.includes("n")) out.y = out.y + out.h - s;
        out.w = s; out.h = s;
    }
    return out;
}

// Rounded to whole pixels, for the compositor: [x, y, w, h].
function toRegion(r) {
    const x = Math.round(r.x), y = Math.round(r.y);
    return [x, y, Math.round(r.x + r.w) - x, Math.round(r.y + r.h) - y];
}

// The topmost window under (x, y) on the current workspace (windows come
// most recently used first, which is also their stacking order).
function windowAt(windows, x, y, workspace) {
    return (windows ?? []).find(w => w.workspace === workspace && w.geometry
        && x >= w.geometry[0] && x < w.geometry[0] + w.geometry[2]
        && y >= w.geometry[1] && y < w.geometry[1] + w.geometry[3]) ?? null;
}

// "700 × 413" in real pixels at `scale`.
function sizeLabel(r, scale) { return Math.round(r.w * scale) + " × " + Math.round(r.h * scale) }
