.pragma library
// Groups: which group each item is in, and the items in group order (each
// group keeping the order they came in), with headings.
.import "Format.js" as F

// The group of an item: {key, label}; the key sorts the groups.
function groupOf(e, by, now) {
    if (by === "kind") {
        const k = e.dir ? "Folder" : F.kind(e.mime);
        return { key: (e.dir ? "0" : "1") + k, label: e.dir ? "Folders" : plural(k) };
    }
    if (by === "modified") {
        const day = 86400000;
        const d = new Date(now);
        const today = new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
        const t = e.modified || 0;
        if (t >= today) return { key: "0", label: "Today" };
        if (t >= today - day) return { key: "1", label: "Yesterday" };
        if (t >= today - 6 * day) return { key: "2", label: "Earlier this week" };
        if (t >= new Date(d.getFullYear(), d.getMonth(), 1).getTime()) return { key: "3", label: "Earlier this month" };
        if (t >= new Date(d.getFullYear(), 0, 1).getTime()) return { key: "4", label: "Earlier this year" };
        if (t <= 0) return { key: "9", label: "Unknown" };
        const y = new Date(t).getFullYear();
        return { key: "5" + String(9999 - y).padStart(4, "0"), label: String(y) };
    }
    if (by === "size") {
        if (e.dir) return { key: "0", label: "Folders" };
        const s = e.size;
        return s >= 1e9 ? { key: "1", label: "Huge (over 1 GB)" }
            : s >= 1e8 ? { key: "2", label: "Large (100 MB – 1 GB)" }
            : s >= 1e6 ? { key: "3", label: "Medium (1 – 100 MB)" }
            : s >= 1e5 ? { key: "4", label: "Small (100 kB – 1 MB)" }
            : s > 0 ? { key: "5", label: "Tiny (under 100 kB)" }
            : { key: "6", label: "Empty" };
    }
    if (by === "name") {
        const c = (e.name.replace(/^\./, "")[0] ?? "#").toUpperCase();
        return /[A-Z]/.test(c) ? { key: "1" + c, label: c } : /[0-9]/.test(c) ? { key: "0", label: "0–9" } : { key: "2", label: "#" };
    }
    return { key: "", label: "" };
}

function plural(k) {
    return { Image: "Images", Video: "Videos", Document: "Documents", Spreadsheet: "Spreadsheets", Presentation: "Presentations",
             Archive: "Archives", Script: "Scripts", File: "Other files" }[k] ?? k;
}

// The items grouped: a new list in group order with `group` (label) on each
// (copies; the originals are left alone). `descending` turns the groups round.
function arrange(entries, by, descending, now) {
    if (!by) return entries;
    const tagged = entries.map((e, i) => { const g = groupOf(e, by, now); return { e: e, g: g, i: i } });
    tagged.sort((a, b) => (a.g.key < b.g.key ? -1 : a.g.key > b.g.key ? 1 : a.i - b.i) * (descending && a.g.key !== b.g.key ? -1 : 1));
    return tagged.map(t => Object.assign({}, t.e, { group: t.g.label }));
}

// Grouped items (from arrange) as rows for a grid `cols` across: a heading
// row per group ({header, count, folded}), then rows of up to `cols` items
// ({items: [{e, i}]}, i its place in `entries`), none for folded groups.
function rows(entries, cols, folded) {
    const out = [];
    let group = null, row = null;
    entries.forEach((e, i) => {
        if (e.group !== group) {
            group = e.group;
            const count = entries.filter(x => x.group === group).length;
            out.push({ header: group, count: count, folded: !!folded[group] });
            row = null;
        }
        if (folded[group]) return;
        if (!row || row.items.length >= cols) { row = { items: [] }; out.push(row) }
        row.items.push({ e: e, i: i });
    });
    return out;
}

// Where the arrow keys go in those rows from item `at`: Left / Right one
// along, Up / Down the same column (or the nearest) in the row above or
// below. -1: nowhere.
function step(rowList, at, key) {
    const places = [];
    rowList.forEach((r, ri) => { if (r.items) r.items.forEach((it, c) => places.push({ i: it.i, ri: ri, c: c })) });
    const k = places.findIndex(p => p.i === at);
    if (k < 0) return places.length ? places[0].i : -1;
    if (key === "left") return k > 0 ? places[k - 1].i : -1;
    if (key === "right") return k < places.length - 1 ? places[k + 1].i : -1;
    const here = places[k];
    const rowsWith = [...new Set(places.map(p => p.ri))];
    const ri = rowsWith.indexOf(here.ri) + (key === "up" ? -1 : 1);
    if (ri < 0 || ri >= rowsWith.length) return -1;
    const there = places.filter(p => p.ri === rowsWith[ri]);
    return there[Math.min(here.c, there.length - 1)].i;
}
