// The bug report flow's guesses (ReportView.qml), free of QML so they can be
// tested (shell/tests/run.mjs): which problem, what kind, where, and the
// title. Every guess is only a starting point; each step can change it.
.pragma library

// The kinds of problem (step 2), in the order shown.
const kinds = [
    { value: "crashed", label: "Crashed", phrase: "crashed" },
    { value: "froze", label: "Froze", phrase: "froze" },
    { value: "looks", label: "Looks wrong", phrase: "looks wrong" },
    { value: "wrong", label: "Doesn't work", phrase: "doesn't do what it should" },
    { value: "slow", label: "Slow", phrase: "is slow" },
];

// The parts of WhiteRose (step 3), then the apps that were open.
const areas = [
    "The whole desktop", "The bar", "The dock", "The island", "Notifications",
    "Quick settings", "The lock screen", "Windows and tiling", "Displays",
    "Settings", "Files", "Notepad",
];

// WhiteRose's own apps by app ID.
const ownApps = { "whiterose-settings": "Settings", "whiterose-files": "Files", "whiterose-notepad": "Notepad" };

function appLabel(id) {
    return ownApps[id] ?? id;
}

// Step 1: the snapshot to start on. The one it was opened from, else the
// newest ready one from the last 10 minutes, else "" (one is taken now).
function pickSnapshot(list, fromId, nowMs) {
    if (fromId && (list ?? []).some(s => s.id === fromId)) return fromId;
    const recent = (list ?? []).find(s => s.ready && nowMs - s.time < 10 * 60 * 1000);
    return recent ? recent.id : "";
}

// Step 2: what kind. A crash record → crashed; a memory close, or memory
// nearly full at the time → slow; else "doesn't work".
function kindFor(rec, snap) {
    const what = rec?.what ?? "";
    if (what === "compositor" || what === "shell" || what === "daemon") return "crashed";
    if (what === "memory") return "slow";
    const l = snap?.load;
    if (l && l.mem_total_mb > 0 && l.mem_used_mb / l.mem_total_mb > 0.9) return "slow";
    return "wrong";
}

// Step 3: where. What crashed, else the focused app, else the whole desktop.
function whereFor(rec, snap) {
    switch (rec?.what) {
    case "compositor": return "The whole desktop";
    case "shell": return "The bar";
    case "daemon": return "Settings";
    case "memory": return rec.app ? appLabel(rec.app) : "The whole desktop";
    }
    const f = snap?.windows?.focused ?? "";
    if (f && f !== "(no app ID)") return appLabel(f);
    return "The whole desktop";
}

// Step 3's dropdown: WhiteRose's parts, then the apps that were open.
function areaOptions(snap, current) {
    const out = areas.slice();
    for (const id of Object.keys(snap?.windows?.apps ?? {}).sort()) {
        const l = appLabel(id);
        if (id !== "(no app ID)" && !out.includes(l)) out.push(l);
    }
    if (current && !out.includes(current)) out.push(current);
    return out;
}

// The report's title, made from the kind and where: "Files froze". A
// crash says what it was ("The shell crashed (signal 11, SIGSEGV)").
function titleFor(kind, where, recTitle) {
    if (kind === "crashed" && recTitle && /crashed|exited/.test(recTitle)) return recTitle;
    const k = kinds.find(x => x.value === kind);
    return where + " " + (k ? k.phrase : "has a problem");
}

// A new draft from the snapshot it starts on.
function newDraft(list, fromId, nowMs) {
    return { step: 1, snapshot: pickSnapshot(list, fromId, nowMs), kind: "", where: "", title: "", what: "", off: [], edited: false };
}

// Fills in what hasn't been chosen, from the record and snapshot. A title
// you've edited is kept.
function fill(draft, rec, snap, recTitle) {
    const d = Object.assign({}, draft);
    if (!d.kind) d.kind = kindFor(rec, snap);
    if (!d.where) d.where = whereFor(rec, snap);
    if (!d.edited) d.title = titleFor(d.kind, d.where, recTitle);
    return d;
}

// The sections step 5 lists, by key.
const sectionNames = {
    whiterose: "WhiteRose", system: "System and hardware", load: "Load at the time",
    windows: "Open windows (app names only)", output: "What it printed last", log: "Session log (last lines)",
};

// "The shell crashed · 14:02" for step 1's list.
function snapshotLabel(s, nowMs) {
    const d = new Date(s.time), now = new Date(nowMs);
    const hm = String(d.getHours()).padStart(2, "0") + ":" + String(d.getMinutes()).padStart(2, "0");
    const same = d.toDateString() === now.toDateString();
    return s.title + " · " + (same ? hm : d.getDate() + "/" + (d.getMonth() + 1) + " " + hm);
}

// Whether Next is allowed on `step`.
function canGoOn(d) {
    if (d.step === 3) return (d.title ?? "").trim() !== "";
    if (d.step === 4) return (d.what ?? "").trim().length >= 10;
    return true;
}
