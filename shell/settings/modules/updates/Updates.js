// Updates page logic, free of QML so it can be tested (shell/tests/run.mjs).
.pragma library

const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

// "just now", "5 minutes ago", "2 hours ago", "yesterday", "on 9 Nov" (unix seconds).
function ago(unix, nowMs) {
    if (!unix) return "never";
    const s = Math.max(0, Math.round(nowMs / 1000 - unix));
    if (s < 60) return "just now";
    if (s < 3600) { const m = Math.round(s / 60); return m + (m === 1 ? " minute ago" : " minutes ago"); }
    if (s < 86400) { const h = Math.round(s / 3600); return h + (h === 1 ? " hour ago" : " hours ago"); }
    if (s < 2 * 86400) return "yesterday";
    return "on " + day(unix, nowMs);
}

// "9 Nov" (with the year when it isn't this one).
function day(unix, nowMs) {
    const d = new Date(unix * 1000), now = new Date(nowMs);
    return d.getDate() + " " + months[d.getMonth()] + (d.getFullYear() !== now.getFullYear() ? " " + d.getFullYear() : "");
}

// The header's line: "WhiteRose 0.4.2 · checked 5 minutes ago".
function headline(st, nowMs) {
    if (!st) return "";
    const v = st.running ? "WhiteRose " + st.running : "A build being worked on";
    if (st.busy === "checking") return v + " · checking…";
    if (!st.supported) return v;
    return v + " · checked " + ago(st.checked, nowMs);
}

// The release's badge: [text, kind] (kind: "ready" | "busy" | "plain").
function badge(r) {
    switch (r?.state) {
    case "ready": return r.pending ? ["Ready to apply", "ready"] : ["Installed", "ready"];
    case "installing": return ["Installing…", "busy"];
    case "downloading": return ["Downloading…", "busy"];
    case "downloaded": return ["Downloaded", "plain"];
    case "available": return ["New", "plain"];
    }
    return ["", "plain"];
}

// The note beside the release's buttons.
function note(r, st) {
    switch (r?.state) {
    case "available": return "Downloads in the background. Your apps keep running.";
    case "downloading": return "Checking each file against the signed release as it lands.";
    case "downloaded": return "Installs beside " + st.running + ", which stays to go back to. Asks for your password.";
    case "installing": return "Installing beside " + st.running + "…";
    case "ready": return st.launcher
        ? "No restart needed: the session restarts and your apps close."
        : "No restart needed. Log out and back in to switch.";
    }
    return "";
}

// 0..1 of the download.
function fraction(st) {
    const p = st?.progress ?? [0, 0];
    return p[1] > 0 ? Math.min(1, p[0] / p[1]) : 0;
}

// ---- the Releases tab ---------------------------------------------------------------------

// The series (from whiterosed's update_releases) narrowed to what matches
// `query`: a version ("0.3", "alpha.2") or words from a release's changes.
// A series whose name matches keeps all its releases.
function filterSeries(series, query) {
    const q = (query ?? "").trim().toLowerCase();
    if (q === "") return series ?? [];
    const out = [];
    for (const s of series ?? []) {
        if (("v" + s.name).startsWith(q.startsWith("v") ? q : "v" + q)) { out.push(s); continue; }
        const rs = s.releases.filter(r => r.version.toLowerCase().includes(q.replace(/^v/, ""))
            || (r.changes ?? []).some(c => c.toLowerCase().includes(q)));
        if (rs.length) out.push({ name: s.name, releases: rs });
    }
    return out;
}

// The series' line under its name: "3 releases · 0.3.0-alpha.3 running".
function seriesNote(s) {
    const n = s.releases.length;
    let t = n + (n === 1 ? " release" : " releases");
    const run = s.releases.find(r => r.state === "running");
    const next = s.releases.find(r => r.state === "next");
    if (run) t += " · " + run.version + " running";
    else if (next) t += " · " + next.version + " starts next";
    return t;
}

// A release's faint facts: "alpha · 13 Nov".
function releaseFacts(r, nowMs) {
    const parts = [];
    const m = /-(alpha|beta|rc)/i.exec(r.version);
    if (r.pre) parts.push(m ? m[1].toLowerCase() : "pre-release");
    if (r.published > 0) parts.push(day(r.published, nowMs));
    return parts.join(" · ");
}

// What a release row says on the right: [text, button] (button "" for none).
function releaseAction(r) {
    switch (r.state) {
    case "running": return ["Running", ""];
    case "next": return ["Starts at next login", ""];
    case "installed": return ["", "Switch to it"];
    }
    return ["", "Install"];
}

// The newest release listed ("" for none): the series and their releases
// come newest first.
function newestRelease(series) {
    return series?.[0]?.releases?.[0]?.version ?? "";
}
