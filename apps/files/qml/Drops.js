.pragma library
// Drag and drop: what's carried, and what a drop does.

// Local paths as file:// URIs for text/uri-list.
// (Things inside an archive go as archive: places: Files takes them out
// where they're dropped; other apps don't know them and ignore them.)
function uris(paths) {
    return paths.map(p => p.startsWith("archive:") ? "archive:" + encodeURI(p.slice(8))
                                                    : "file://" + p.split("/").map(encodeURIComponent).join("/")).join("\r\n") + "\r\n";
}

// The local files among dropped URLs (others, like web links, are ignored).
function paths(urls) {
    const out = [];
    for (const u of urls) {
        const s = String(u);
        if (s.startsWith("archive:")) { out.push("archive:" + decodeURI(s.slice(8))); continue }
        if (!s.startsWith("file://")) continue;
        // file:///p or file://localhost/p
        const p = decodeURIComponent(s.replace(/^file:\/\/(localhost)?/, ""));
        if (p.startsWith("/")) out.push(p.replace(/\/+$/, "") || "/");
    }
    return out;
}

function parent(p) {
    const i = p.lastIndexOf("/");
    return i <= 0 ? "/" : p.substring(0, i);
}

// What dropping `paths` on folder `dest` does: {kind, paths}, or null for
// nothing. Ctrl copies, Shift moves; otherwise the same drive moves and
// another copies. The Trash ("trash:") trashes. Never onto or into itself,
// and moving into the folder it's already in does nothing. Things from inside
// an archive are always copied (taken out), and nothing goes into one.
function plan(paths, dest, ctrl, shift, sameDrive) {
    if (!dest || paths.length === 0 || dest.startsWith("archive:")) return null;
    const fromArchive = paths.some(p => p.startsWith("archive:"));
    if (fromArchive) return dest.startsWith("/") ? { kind: "copy", paths: paths.filter(p => p.startsWith("archive:")) } : null;
    if (dest === "trash:") return { kind: "trash", paths: paths };
    const ok = paths.filter(p => p !== dest && !dest.startsWith(p + "/"));
    const copy = ctrl || (!shift && !sameDrive);
    const real = copy ? ok : ok.filter(p => parent(p) !== dest);
    return real.length > 0 ? { kind: copy ? "copy" : "move", paths: real } : null;
}

// Whether a drag over `dest` could do anything there (for the highlight).
function accepts(paths, dest) {
    if (!dest || paths.length === 0 || dest.startsWith("archive:")) return false;
    if (paths.some(p => p.startsWith("archive:"))) return dest.startsWith("/");
    // The shelf gathers anything (it only keeps where each is).
    if (dest === "shelf:") return true;
    return dest === "trash:" || paths.some(p => p !== dest && !dest.startsWith(p + "/"));
}
