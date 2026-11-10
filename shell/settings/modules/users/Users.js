// Users page logic, free of QML so it can be tested (shell/tests/run.mjs).
.pragma library

// A username suggested from a full name: "Anna-Maria Smith" -> "annamaria".
function suggestName(full) {
    const first = String(full).trim().split(/\s+/)[0] ?? "";
    let n = first.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase().replace(/[^a-z0-9]/g, "");
    if (n !== "" && !/^[a-z_]/.test(n)) n = "u" + n;
    return n.slice(0, 32);
}

// Why a username can't be used, or "" (the daemon checks again).
function nameProblem(name, taken) {
    if (name === "") return "Needs a username";
    if (!/^[a-z_][a-z0-9_-]{0,31}$/.test(name)) return "Lowercase letters, digits, - and _, starting with a letter";
    if (["root", "daemon", "bin", "sys", "nobody", "admin", "sudo", "wheel"].includes(name)) return "That name is reserved";
    if ((taken ?? []).includes(name)) return "Someone already has that username";
    return "";
}

// Why a new password can't be used yet, or "".
function passwordProblem(pw, again) {
    if (pw === "") return "Enter a password";
    if (pw.length < 8) return "At least 8 characters";
    if (again !== pw) return "The passwords don't match";
    return "";
}

// "Signed in today", "… yesterday", "… 3 days ago", "… on 2 Oct", "Never signed in".
function lastSeen(unix, nowMs) {
    if (!unix) return "Never signed in";
    const then = new Date(unix * 1000), now = new Date(nowMs);
    const day = d => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
    const days = Math.round((day(now) - day(then)) / 86400000);
    if (days <= 0) return "Signed in today";
    if (days === 1) return "Signed in yesterday";
    if (days < 7) return "Signed in " + days + " days ago";
    const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    return "Signed in on " + then.getDate() + " " + months[then.getMonth()] + (then.getFullYear() !== now.getFullYear() ? " " + then.getFullYear() : "");
}

// ---- fingerprints ----------------------------------------------------------------------

// fprintd's finger names, in the order they're offered.
const fingers = [
    "right-index-finger", "left-index-finger", "right-thumb", "left-thumb",
    "right-middle-finger", "left-middle-finger", "right-ring-finger", "left-ring-finger",
    "right-little-finger", "left-little-finger",
];

// "right-index-finger" -> "Right index finger"; "left-thumb" -> "Left thumb".
function fingerName(f) {
    const s = String(f).replace(/-/g, " ");
    return s.charAt(0).toUpperCase() + s.slice(1);
}

// The first finger not added yet ("" when all ten are).
function nextFinger(enrolled) {
    return fingers.find(f => !(enrolled ?? []).includes(f)) ?? "";
}

// What to say while adding one (`e` is the latest "fingerprint" event).
function enrollText(e, scan) {
    const verb = scan === "swipe" ? "Swipe your finger across" : "Touch";
    if (!e) return verb + " the reader";
    if (e.state === "retry" || e.state === "failed" || e.state === "done") return e.message;
    if (!e.stage) return verb + " the reader";
    return scan === "swipe" ? "Again, a few more times" : "Lift and touch again, moving it a little";
}

// 0..1 of the touches done.
function enrollFraction(e) {
    if (!e || !e.stages) return 0;
    return Math.max(0, Math.min(1, e.stage / e.stages));
}
