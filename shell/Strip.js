// The bar's workspace strip (Figma "Top bar - workspace strip";
// "bar.workspaceStyle" strip): one cell per workspace with the icons of
// the apps open on it, and the live preview under a cell. From the
// compositor's `windows` event (each window's workspace, and its `place`
// on it whether shown or not).
.pragma library

// Icons a cell shows before "+N".
const MAX_ICONS = 4;

// How many workspaces the bar shows: at least 5, more if a later one is
// current or in use (as the dots do).
function shown(active, occupied) {
    return Math.max(5, active, ...(occupied && occupied.length ? occupied : [0]));
}

// Workspace `n`'s windows (1-based), oldest first so icons don't jump
// around as focus changes, and how many more than fit.
function cell(windows, n) {
    const on = (windows || []).filter(w => w.workspace === n).sort((a, b) => a.id - b.id);
    return { windows: on.slice(0, MAX_ICONS), more: Math.max(0, on.length - MAX_ICONS) };
}

// Which of `rects` (each {x, width}, in one coordinate space, in order)
// `x` is over, or -1.
function cellAt(rects, x) {
    return (rects || []).findIndex(r => x >= r.x && x < r.x + r.width);
}

// Workspace `n` on the screen `screen` ({x, y, width, height}) drawn
// `width` wide: each of its windows on that screen (by its middle) where
// it is on it, scaled. Bottom first, as the windows event lists the most
// recently used first.
function preview(windows, n, screen, width) {
    if (!screen || !(screen.width > 0)) return [];
    const k = width / screen.width;
    return (windows || [])
        .filter(w => w.workspace === n && w.place)
        .filter(w => {
            const mx = w.place[0] + w.place[2] / 2, my = w.place[1] + w.place[3] / 2;
            return mx >= screen.x && mx < screen.x + screen.width && my >= screen.y && my < screen.y + screen.height;
        })
        .reverse()
        .map(w => ({
            id: w.id,
            x: Math.round((w.place[0] - screen.x) * k),
            y: Math.round((w.place[1] - screen.y) * k),
            width: Math.round(w.place[2] * k),
            height: Math.round(w.place[3] * k),
        }));
}

// The line under a preview: its windows' titles, or that it's empty.
function caption(windows, n) {
    const titles = (windows || []).filter(w => w.workspace === n).map(w => w.title || w.app_id || "").filter(t => t);
    return titles.length ? titles.join(", ") : "No windows";
}
