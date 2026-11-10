// What Files' toolbar and right-click menu can be made of (Settings ›
// Files › Toolbar & menus, and Files itself: the same list for both).
//   files.toolbar   extra toolbar buttons, by id, in order
//   files.menuHide  built-in right-click entries left out, by id
//   files.actions   your own actions: [{name, command, on, types, terminal}]
.pragma library

// Buttons that can go in the toolbar.
var toolbar = [
    { id: "new-folder", label: "New folder", icon: "plus" },
    { id: "refresh", label: "Look again", icon: "restart" },
    { id: "terminal", label: "Open in terminal", icon: "terminal" },
    { id: "editor", label: "Open in code editor", icon: "code" },
    { id: "split", label: "Split", icon: "viewColumns" },
    { id: "hidden", label: "Hidden files", icon: "eye" },
    { id: "details", label: "Details panel", icon: "info" },
    { id: "shelf", label: "Put on the shelf", icon: "briefcase" },
    { id: "compress", label: "Compress", icon: "package" },
    { id: "compare", label: "Compare & sync", icon: "copy" },
    { id: "new-tab", label: "New tab", icon: "square" },
];

// Right-click entries that can be left out: each matched by how its label starts.
var menu = [
    { id: "quick-look", label: "Quick look" },
    { id: "open-tab", label: "Open in new tab" },
    { id: "open-with", label: "Open with" },
    { id: "open-in", label: "Open in" },
    { id: "duplicate", label: "Duplicate" },
    { id: "copy-path", label: "Copy path" },
    { id: "compress", label: "Compress…" },
    { id: "other-side", label: "Copy to other side", also: "Move to other side" },
    { id: "group", label: "New folder with selection…" },
    { id: "looks", label: "Colour and icon…" },
    { id: "shelf", label: "Put on the shelf" },
    { id: "tags", label: "Tags" },
    { id: "delete", label: "Delete permanently…" },
];

// The menu without what's left out (and without separators left doubled,
// first or last).
function trim(items, hidden) {
    if (!hidden || hidden.length === 0) return items;
    const out = [];
    for (const it of items) {
        if (it.separator) { if (out.length > 0 && !out[out.length - 1].separator) out.push(it); continue }
        const m = menu.find(x => hidden.includes(x.id) && (it.label.startsWith(x.label) || (x.also && it.label.startsWith(x.also))));
        if (!m) out.push(it);
    }
    while (out.length > 0 && out[out.length - 1].separator) out.pop();
    return out;
}

// Does action `a` apply to what's chosen (all of it)?
function applies(a, entries) {
    if (!entries || entries.length === 0) return false;
    const types = (a.types || "").split(/[\s,]+/).map(t => t.replace(/^\./, "").toLowerCase()).filter(t => t);
    return entries.every(e => {
        if (a.on === "files" && e.dir) return false;
        if (a.on === "folders" && !e.dir) return false;
        if (types.length === 0) return true;
        const ext = e.name.includes(".") ? e.name.substring(e.name.lastIndexOf(".") + 1).toLowerCase() : "";
        return !e.dir && types.includes(ext);
    });
}

function quote(s) { return "'" + String(s).replace(/'/g, "'\\''") + "'" }

// The shell command for action `command` on `paths` in folder `dir`:
// %f the paths, %n their names, %d the folder (each quoted); with none of
// them, the paths go on the end.
function command(cmd, paths, dir) {
    const f = paths.map(quote).join(" ");
    const n = paths.map(p => quote(p.substring(p.lastIndexOf("/") + 1))).join(" ");
    if (!/%[fnd]/.test(cmd)) return cmd + " " + f;
    return cmd.replace(/%([fnd%])/g, (m, c) => c === "f" ? f : c === "n" ? n : c === "d" ? quote(dir) : "%");
}
