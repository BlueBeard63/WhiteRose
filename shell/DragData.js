.pragma library
// What a dragged island item hands to the app it's dropped on (the mime map
// for Qt's Drag.mimeData). Files go as a file list, so mail clients attach
// them and web pages / file pickers upload them; the plain-text path is what
// a text field inserts. Text entries drag as text.

// file:///home/me/My%20File.png: each path segment percent-encoded.
function fileUri(path) {
    return "file://" + path.split("/").map(encodeURIComponent).join("/");
}

function forFiles(paths) {
    if (!paths || paths.length === 0) return null;
    return {
        "text/uri-list": paths.map(fileUri).join("\r\n") + "\r\n",
        "text/plain": paths.join("\n"),
    };
}

// A clipboard history entry (as the compositor sends it), or null when it
// can't be dragged: every file gone, or a text too long to have been sent.
function forEntry(e) {
    if (!e || !e.available) return null;
    switch (e.kind) {
    case "text": return typeof e.text === "string" ? { "text/plain": e.text } : null;
    case "image": return e.drag_path ? forFiles([e.drag_path]) : null;
    case "files": return forFiles(e.files.filter(f => f.exists).map(f => f.path));
    }
    return null;
}
