// The memory warning's words (MemoryView.qml), free of QML so they can be
// tested (shell/tests/run.mjs).
.pragma library

// "6.2 GB", "850 MB".
function size(bytes) {
    const gb = bytes / (1 << 30);
    if (gb >= 1) return (gb >= 10 ? Math.round(gb) : Math.round(gb * 10) / 10) + " GB";
    return Math.max(1, Math.round(bytes / (1 << 20))) + " MB";
}

// A readable name for an app id with no desktop entry: its last part,
// capitalised ("org.mozilla.firefox" → "Firefox", "code" → "Code").
function appName(id) {
    const last = String(id ?? "").split(".").pop() || "This app";
    return last.charAt(0).toUpperCase() + last.slice(1);
}

// [title, text] for a memory_warning ({ level, app: { id, bytes, growing,
// reason }, available, total }); `name` is the app's name.
function warningText(w, name) {
    const a = w?.app ?? {};
    const used = size(a.bytes ?? 0);
    const close = "Closing it frees the memory. Anything unsaved in it is lost.";
    if (a.reason === "growing" || a.growing)
        return [name + " is using " + used + " and still growing", "It has grown steadily for 10 minutes, which looks like a memory leak. " + close];
    if (a.reason === "half")
        return [name + " is using " + used + ", more than half of memory", close];
    return ["Memory is nearly full", name + " is using the most (" + used + "). " + close];
}
