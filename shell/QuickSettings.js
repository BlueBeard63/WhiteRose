.pragma library
// Pure logic for the quick settings panel (tested in tests/run.mjs).

// "Up 3h 12m", "Up 2d 4h", "Up 7m" from /proc/uptime's first number.
function uptime(seconds) {
    const s = Math.max(0, Math.floor(seconds));
    const d = Math.floor(s / 86400), h = Math.floor(s % 86400 / 3600), m = Math.floor(s % 3600 / 60);
    if (d > 0) return `Up ${d}d ${h}h`;
    if (h > 0) return `Up ${h}h ${m}m`;
    return `Up ${m}m`;
}

// "4h 10m", "45m" (rounded to the minute), "" for unknown (0).
function duration(seconds) {
    const m = Math.round(seconds / 60);
    if (m <= 0) return "";
    const h = Math.floor(m / 60);
    return h > 0 ? `${h}h ${m % 60}m` : `${m}m`;
}

// Footer line: "82% · 4h 10m left", "64% · 1h 5m to full", "Fully charged".
// state: "charging" | "discharging" | "full" | other.
function batteryLine(percent, state, timeToEmpty, timeToFull) {
    if (state === "full") return "Fully charged";
    if (state === "charging") {
        const t = duration(timeToFull);
        return t ? `${percent}% · ${t} to full` : `${percent}% · Charging`;
    }
    const t = duration(timeToEmpty);
    return t ? `${percent}% · ${t} left` : `${percent}%`;
}

// When Night Light switched on at `now` (ms) turns itself off: the next 07:00.
function nightLightUntil(now) {
    const d = new Date(now);
    const until = new Date(d.getFullYear(), d.getMonth(), d.getDate(), 7, 0, 0, 0);
    if (until.getTime() <= now) until.setDate(until.getDate() + 1);
    return until.getTime();
}

// Power profiles cycle saver -> balanced -> performance (when the hardware has
// one) -> saver. Profiles are QuickShell's PowerProfile enum values: 0 saver,
// 1 balanced, 2 performance.
function nextProfile(current, hasPerformance) {
    if (current === 0) return 1;
    if (current === 1) return hasPerformance ? 2 : 0;
    return 0;
}

function profileName(p) { return p === 0 ? "Power Saver" : p === 2 ? "Performance" : "Balanced" }

// Initials for the avatar fallback: "bluebeard" -> "b", "Jack Morrison" -> "JM".
function initials(name) {
    const words = String(name).trim().split(/\s+/).filter(Boolean);
    if (words.length === 0) return "?";
    if (words.length === 1) return words[0][0];
    return (words[0][0] + words[words.length - 1][0]).toUpperCase();
}

// Full name from an /etc/passwd line's GECOS field, else the login name.
function displayName(passwd, user) {
    for (const line of String(passwd).split("\n")) {
        const f = line.split(":");
        if (f[0] === user) {
            const gecos = (f[4] ?? "").split(",")[0].trim();
            return gecos || user;
        }
    }
    return user;
}
