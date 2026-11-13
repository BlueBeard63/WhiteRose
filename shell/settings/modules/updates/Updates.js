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
