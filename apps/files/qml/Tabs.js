.pragma library
// Tab order: moving, where a dragged tab lands, which tab shows after a close.

// `list` with the item at `from` moved to `to` (a new array).
function move(list, from, to) {
    const out = list.slice();
    if (from < 0 || from >= out.length) return out;
    to = Math.max(0, Math.min(out.length - 1, to));
    const [it] = out.splice(from, 1);
    out.splice(to, 0, it);
    return out;
}

// Where a tab dragged to `x` belongs, given every tab's [left, width]:
// past the middle of a neighbour, it takes that neighbour's place.
function slot(tabs, from, x) {
    let to = from;
    for (let i = 0; i < tabs.length; i++) {
        const mid = tabs[i][0] + tabs[i][1] / 2;
        if (i < from && x < mid) { to = i; break }
        if (i > from && x > mid) to = i;
    }
    return to;
}

// The tab to show once tab `closed` of `count` has gone, `current` showing.
function afterClose(count, closed, current) {
    if (count <= 1) return -1;
    if (closed < current) return current - 1;
    if (closed > current) return current;
    return Math.min(closed, count - 2);
}

// Dragged this far below the tab bar, a tab leaves the bar: a drag other
// windows' bars can take, or (dropped anywhere else) a window of its own.
const tearOff = 48;

// Where a tab dropped at `x` goes, given every tab's [left, width]: before
// the first tab whose middle is past x.
function insertAt(tabs, x) {
    for (let i = 0; i < tabs.length; i++) if (x < tabs[i][0] + tabs[i][1] / 2) return i;
    return tabs.length;
}

// The drag's payload, and back.
const mime = "application/x-whiterose-tab";
// `id`: letters and digits, for the window that takes it to say so (Fs.takeTab).
function pack(token, win, index, tab, id) { return JSON.stringify({ token: token, win: win, index: index, tab: tab, id: id }) }
function unpack(text) {
    try { const d = JSON.parse(text); return d && d.tab && typeof d.tab === "object" ? d : null } catch (e) { return null }
}
