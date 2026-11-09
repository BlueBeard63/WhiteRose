// Tests for the shell's pure JS (Calc.js, Search.js, DragData.js, QuickSettings.js, Notifications.js, CalendarGrid.js). Run: node shell/tests/run.mjs
// The files are QML JS libraries; strip the `.pragma library` line and load them.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import assert from "node:assert/strict";

const here = dirname(fileURLToPath(import.meta.url));
function load(file, names) {
    const src = readFileSync(join(here, "..", file), "utf8").replace(/^\.pragma library\s*$/m, "");
    return new Function(`${src}\nreturn { ${names.join(", ")} };`)();
}
const Calc = load("Calc.js", ["evaluate"]);
const Search = load("Search.js", ["score", "rankApps", "frequent", "parseRecent", "matchFiles", "tildify"]);
const DragData = load("DragData.js", ["fileUri", "forFiles", "forEntry"]);
const QS = load("QuickSettings.js", ["uptime", "duration", "batteryLine", "nightLightUntil", "nextProfile", "profileName", "initials", "displayName"]);
const N = load("Notifications.js", ["ago", "shouldPopup", "popupTimeout", "buttons", "hasDefault", "iconKind", "themedImageName"]);
const Cal = load("CalendarGrid.js", ["key", "monthGrid", "gridRange", "busyDays", "onDay", "dayLabel", "timeRange"]);

let passed = 0, failed = 0;
function test(name, fn) {
    try { fn(); passed++; } catch (e) { failed++; console.error(`FAIL ${name}\n  ${e.message}`); }
}

// ---- Calc --------------------------------------------------------------
test("arithmetic and precedence", () => {
    assert.equal(Calc.evaluate("2+3*4"), "14");
    assert.equal(Calc.evaluate("(2+3)*4"), "20");
    assert.equal(Calc.evaluate("10/4"), "2.5");
    assert.equal(Calc.evaluate("7 % 3"), "1");
    assert.equal(Calc.evaluate("2^3^2"), "512", "^ is right-associative");
    assert.equal(Calc.evaluate("-2^2"), "-4", "unary minus binds looser than ^");
    assert.equal(Calc.evaluate("3--2"), "5");
    assert.equal(Calc.evaluate("2^-1"), "0.5", "unary minus in an exponent");
    assert.equal(Calc.evaluate("-3*2"), "-6");
    assert.equal(Calc.evaluate("-(2+3)"), "-5");
    assert.equal(Calc.evaluate("+5*2"), "10");
    assert.equal(Calc.evaluate("1.5e3 + 1"), "1501");
    assert.equal(Calc.evaluate("6×7"), "42");
    assert.equal(Calc.evaluate("9÷3="), "3");
});
test("functions and constants", () => {
    assert.equal(Calc.evaluate("sqrt(16)"), "4");
    assert.equal(Calc.evaluate("2*pi"), "6.28318530718");
    assert.equal(Calc.evaluate("log(1000)"), "3");
    assert.equal(Calc.evaluate("round(2.6)+abs(-1)"), "4");
    assert.equal(Calc.evaluate("pi"), "3.14159265359", "a constant alone is a calculation");
});
test("floating point noise is trimmed", () => {
    assert.equal(Calc.evaluate("0.1+0.2"), "0.3");
});
test("not calculations", () => {
    for (const s of ["", "firefox", "12", "  42 ", "1/0", "2+", "(2+3", "2+3)", "alert(1)", "2+foo", "process.exit()", "sqrt"])
        assert.equal(Calc.evaluate(s), null, JSON.stringify(s));
});

// ---- Search ------------------------------------------------------------
const apps = [
    { id: "org.gnome.Nautilus", name: "Files", genericName: "File Manager", keywords: ["folder", "explorer"] },
    { id: "firefox_firefox", name: "Firefox", genericName: "Web Browser", keywords: ["internet"] },
    { id: "org.gnome.Ptyxis", name: "Ptyxis", genericName: "Terminal", keywords: ["shell", "command"] },
    { id: "org.gnome.TextEditor", name: "Text Editor", genericName: "Text Editor", keywords: [] },
    { id: "spotify", name: "Spotify", genericName: "Music Player", keywords: [] },
];
test("score tiers", () => {
    assert.ok(Search.score("fire", "Firefox") > Search.score("fox", "Firefox"));
    assert.equal(Search.score("ed", "Text Editor"), 80, "word prefix");
    assert.equal(Search.score("xt", "Text Editor"), 60, "substring");
    assert.ok(Search.score("tte", "Text Editor") > 0, "subsequence");
    assert.equal(Search.score("zzz", "Firefox"), -1);
    assert.equal(Search.score("", "Firefox"), -1);
});
test("apps match by name, generic name and keywords", () => {
    assert.equal(Search.rankApps("fire", apps, {})[0].app.id, "firefox_firefox");
    assert.equal(Search.rankApps("terminal", apps, {})[0].app.id, "org.gnome.Ptyxis", "generic name");
    assert.equal(Search.rankApps("folder", apps, {})[0].app.id, "org.gnome.Nautilus", "keyword");
    assert.equal(Search.rankApps("browser", apps, {})[0].app.id, "firefox_firefox");
    assert.equal(Search.rankApps("qqq", apps, {}).length, 0);
});
test("descriptions don't produce scattered-letter matches", () => {
    // Real entries that matched "term" before the fix.
    const real = apps.concat([
        { id: "thunderbird", name: "Thunderbird Mail", genericName: "Mail Client", comment: "Send and receive mail with Thunderbird", keywords: ["Email", "Newsgroup"] },
        { id: "discord_discord", name: "Discord", genericName: "Internet Messenger", comment: "All-in-one voice and text chat for gamers that's free, secure, and works on both your desktop and phone." },
        { id: "guymager", name: "Guymager", genericName: "Imager", comment: "Forensic imager for media acquisition" },
    ]);
    const ids = Search.rankApps("term", real, {}).map(r => r.app.id);
    assert.deepEqual(ids, ["org.gnome.Ptyxis"], `only the terminal should match, got ${ids}`);
    // Scattered letters still work on the app's own name ("ttx" -> Text Editor).
    assert.equal(Search.rankApps("txed", real, {})[0]?.app.id, "org.gnome.TextEditor");
    // A description word prefix still counts, just weakly.
    assert.equal(Search.rankApps("forensic", real, {})[0]?.app.id, "guymager");
});

test("usage breaks ties toward what you launch", () => {
    // "t" prefixes Text Editor (name) and Terminal (generic); heavy use flips it.
    const plain = Search.rankApps("t", apps, {});
    const used = Search.rankApps("t", apps, { "org.gnome.Ptyxis": 50 });
    assert.equal(plain[0].app.id, "org.gnome.TextEditor");
    assert.equal(used[0].app.id, "org.gnome.Ptyxis");
});
test("frequent: pinned first, then most launched, skipping unknown ids", () => {
    const f = Search.frequent(apps, { spotify: 9, "org.gnome.TextEditor": 3 }, ["firefox_firefox", "gone.app"], 12);
    assert.deepEqual(f.map(a => a.id), ["firefox_firefox", "spotify", "org.gnome.TextEditor"]);
});
test("recent files: local only, decoded, newest first, deduped", () => {
    const xml = `
      <bookmark href="davs://host/remote.php/x.png" added="a" modified="2026-10-05T21:38:38Z" visited="v">
      <bookmark href="file:///home/u/Projects/shell.qml" added="a" modified="2026-11-09T10:00:00Z" visited="v">
      <bookmark href="file:///home/u/Pictures/top%20bar%20flows.png" added="a" modified="2026-11-09T11:00:00Z" visited="v">
      <bookmark href="file:///home/u/Projects/shell.qml" added="a" modified="2026-11-01T10:00:00Z" visited="v">
      <bookmark href="file:///home/u/R%26D.txt" added="a" modified="2026-11-08T10:00:00Z" visited="v">`;
    const r = Search.parseRecent(xml);
    assert.deepEqual(r.map(f => f.name), ["top bar flows.png", "shell.qml", "R&D.txt"]);
    assert.equal(r[0].dir, "/home/u/Pictures");
    assert.deepEqual(Search.parseRecent(""), []);
});
test("file matching and display paths", () => {
    const files = [{ name: "shell.qml" }, { name: "compositor.go" }, { name: "notes.txt" }];
    assert.equal(Search.matchFiles("comp", files)[0].file.name, "compositor.go");
    assert.equal(Search.matchFiles("xyz", files).length, 0);
    assert.equal(Search.matchFiles("term", [{ name: "01-start-here.md" }]).length, 0, "no scattered-letter file matches");
    assert.equal(Search.matchFiles("here", [{ name: "01-start-here.md" }]).length, 1);
    assert.equal(Search.tildify("/home/u/Projects", "/home/u"), "~/Projects");
    assert.equal(Search.tildify("/etc/x", "/home/u"), "/etc/x");
});

// ---- DragData --------------------------------------------------------------
test("dragged files: encoded uri list plus plain paths", () => {
    assert.equal(DragData.fileUri("/home/me/Pictures/Screenshot 2026-11-09 #1?.png"),
        "file:///home/me/Pictures/Screenshot%202026-11-09%20%231%3F.png");
    assert.equal(DragData.fileUri("/tmp/café.txt"), "file:///tmp/caf%C3%A9.txt");
    assert.deepEqual(DragData.forFiles(["/a b", "/c"]), {
        "text/uri-list": "file:///a%20b\r\nfile:///c\r\n",
        "text/plain": "/a b\n/c",
    });
    assert.equal(DragData.forFiles([]), null);
});
test("clipboard entries drag as what they are", () => {
    assert.deepEqual(DragData.forEntry({ kind: "text", available: true, text: "hi" }), { "text/plain": "hi" });
    assert.equal(DragData.forEntry({ kind: "text", available: true, text: null }), null, "too long to drag");
    assert.deepEqual(DragData.forEntry({ kind: "image", available: true, drag_path: "/run/d/Clipboard image.png" }),
        { "text/uri-list": "file:///run/d/Clipboard%20image.png\r\n", "text/plain": "/run/d/Clipboard image.png" });
    const files = [{ path: "/gone", exists: false }, { path: "/here", exists: true }];
    assert.deepEqual(DragData.forEntry({ kind: "files", available: true, files }),
        { "text/uri-list": "file:///here\r\n", "text/plain": "/here" }, "only files still on disk");
    assert.equal(DragData.forEntry({ kind: "files", available: false, files: [{ path: "/gone", exists: false }] }), null);
});

// ---- QuickSettings -----------------------------------------------------------
test("uptime and durations", () => {
    assert.equal(QS.uptime(11520.4), "Up 3h 12m");
    assert.equal(QS.uptime(420), "Up 7m");
    assert.equal(QS.uptime(2 * 86400 + 4 * 3600 + 59), "Up 2d 4h");
    assert.equal(QS.duration(15000), "4h 10m");
    assert.equal(QS.duration(2700), "45m");
    assert.equal(QS.duration(0), "");
});
test("battery footer", () => {
    assert.equal(QS.batteryLine(82, "discharging", 15000, 0), "82% · 4h 10m left");
    assert.equal(QS.batteryLine(82, "discharging", 0, 0), "82%", "UPower still estimating");
    assert.equal(QS.batteryLine(64, "charging", 0, 3900), "64% · 1h 5m to full");
    assert.equal(QS.batteryLine(64, "charging", 0, 0), "64% · Charging");
    assert.equal(QS.batteryLine(100, "full", 0, 0), "Fully charged");
});
test("night light runs until the next 07:00", () => {
    const at = (h, m) => new Date(2026, 10, 9, h, m).getTime();
    assert.equal(QS.nightLightUntil(at(21, 30)), new Date(2026, 10, 10, 7, 0).getTime());
    assert.equal(QS.nightLightUntil(at(2, 0)), new Date(2026, 10, 9, 7, 0).getTime(), "after midnight: this morning");
    assert.equal(QS.nightLightUntil(at(7, 0)), new Date(2026, 10, 10, 7, 0).getTime(), "exactly 07:00: tomorrow");
});
test("power profiles cycle and skip performance when missing", () => {
    assert.deepEqual([0, 1, 2].map(p => QS.nextProfile(p, true)), [1, 2, 0]);
    assert.deepEqual([0, 1].map(p => QS.nextProfile(p, false)), [1, 0]);
    assert.equal(QS.profileName(1), "Balanced");
});
test("user name and initials", () => {
    const passwd = "root:x:0:0:root:/root:/bin/bash\njack:x:1000:1000:Jack Morrison,,,:/home/jack:/bin/zsh\nbluebeard:x:1001:1001:,,,:/home/b:/bin/zsh";
    assert.equal(QS.displayName(passwd, "jack"), "Jack Morrison");
    assert.equal(QS.displayName(passwd, "bluebeard"), "bluebeard", "empty GECOS: login name");
    assert.equal(QS.displayName(passwd, "nobody"), "nobody");
    assert.equal(QS.initials("Jack Morrison"), "JM");
    assert.equal(QS.initials("bluebeard"), "b");
    assert.equal(QS.initials("  "), "?");
});

// ---- Notifications -----------------------------------------------------------
test("notification age", () => {
    assert.deepEqual([0, 59, 120, 3599, 3600, 86399, 86400 * 3].map(N.ago), ["now", "now", "2m", "59m", "1h", "23h", "3d"]);
});
test("popups respect the panel and Do Not Disturb", () => {
    assert.equal(N.shouldPopup(1, false, false), true);
    assert.equal(N.shouldPopup(1, false, true), false, "already visible in the open panel");
    assert.equal(N.shouldPopup(1, true, false), false, "Do Not Disturb");
    assert.equal(N.shouldPopup(0, true, false), false);
    assert.equal(N.shouldPopup(2, true, false), true, "critical gets through Do Not Disturb");
});
test("popup timeouts", () => {
    assert.equal(N.popupTimeout(2, 5000), 0, "critical stays until dismissed");
    assert.equal(N.popupTimeout(1, -1), 5000, "server default");
    assert.equal(N.popupTimeout(1, 0), 5000, "'never expire' still leaves the screen; it stays in the panel");
    assert.equal(N.popupTimeout(1, 500), 3000, "not too short to read");
    assert.equal(N.popupTimeout(1, 120000), 30000, "not forever");
    assert.equal(N.popupTimeout(1, 8000), 8000);
});
test("action buttons leave out the default action", () => {
    const acts = [{ identifier: "default", text: "View" }, { identifier: "open", text: "Open" }, { identifier: "folder", text: "Show in folder" }];
    assert.deepEqual(N.buttons(acts).map(a => a.text), ["Open", "Show in folder"]);
    assert.equal(N.hasDefault(acts), true);
    assert.equal(N.hasDefault(N.buttons(acts)), false);
    assert.deepEqual(N.buttons(null), []);
});
test("theme icons hidden in image URLs", () => {
    assert.equal(N.themedImageName("image://icon/battery-caution"), "battery-caution");
    assert.equal(N.themedImageName("image://icon/folder-download?fallback=x"), "folder-download");
    assert.equal(N.themedImageName("image://qsimage/12/34"), "", "real pixel data");
    assert.equal(N.themedImageName("file:///tmp/a.png"), "");
    assert.equal(N.themedImageName(""), "");
});
test("icon strings", () => {
    assert.deepEqual(N.iconKind("/usr/share/icons/x.png"), { kind: "url", value: "file:///usr/share/icons/x.png" });
    assert.deepEqual(N.iconKind("file:///tmp/a.png"), { kind: "url", value: "file:///tmp/a.png" });
    assert.deepEqual(N.iconKind("image://qsimage/12"), { kind: "url", value: "image://qsimage/12" });
    assert.deepEqual(N.iconKind("thunderbird"), { kind: "name", value: "thunderbird" });
    assert.deepEqual(N.iconKind(""), { kind: "none", value: "" });
    assert.deepEqual(N.iconKind(undefined), { kind: "none", value: "" });
});

// ---- CalendarGrid --------------------------------------------------------------
test("month grid starts on Monday and covers six weeks", () => {
    const g = Cal.monthGrid(2026, 10, new Date(2026, 10, 9));   // November 2026
    assert.equal(g.length, 42);
    assert.equal(Cal.key(g[0].date), "2026-10-26", "Monday before the 1st (a Sunday)");
    assert.deepEqual(g.slice(0, 7).map(c => c.day), [26, 27, 28, 29, 30, 31, 1]);
    assert.equal(g[6].inMonth, true);
    assert.equal(g.filter(c => c.today).map(c => c.day)[0], 9);
    assert.equal(g[5].weekend && g[6].weekend && !g[4].weekend, true);
    assert.equal(Cal.key(g[41].date), "2026-12-06");
    assert.deepEqual(Cal.gridRange(2026, 10), { from: "2026-10-26", to: "2026-12-07" });
    const jun = Cal.monthGrid(2026, 5, new Date(2026, 0, 1));    // June 2026 starts on a Monday
    assert.equal(Cal.key(jun[0].date), "2026-06-01");
});
test("days with events, including multi-day and midnight ends", () => {
    const iso = (y, m, d, h = 0, mi = 0) => new Date(y, m, d, h, mi).toISOString();
    const events = [
        { start: iso(2026, 10, 12, 15), end: iso(2026, 10, 12, 16), all_day: false },
        { start: iso(2026, 10, 18), end: iso(2026, 10, 20), all_day: true },        // 18th and 19th
        { start: iso(2026, 10, 24, 22), end: iso(2026, 10, 25, 0), all_day: false }, // ends at midnight
    ];
    assert.deepEqual(Object.keys(Cal.busyDays(events)).sort(), ["2026-11-12", "2026-11-18", "2026-11-19", "2026-11-24"]);
    assert.equal(Cal.onDay(events, new Date(2026, 10, 19)).length, 1);
    assert.equal(Cal.onDay(events, new Date(2026, 10, 25)).length, 0);
});
test("day labels and times", () => {
    const today = new Date(2026, 10, 9, 14, 32);
    assert.equal(Cal.dayLabel(new Date(2026, 10, 9), today), "TODAY");
    assert.equal(Cal.dayLabel(new Date(2026, 10, 10), today), "TOMORROW");
    assert.equal(Cal.dayLabel(new Date(2026, 10, 8), today), "YESTERDAY");
    assert.equal(Cal.dayLabel(new Date(2026, 10, 12), today), "THU 12 NOV");
    const iso = (d, h, m = 0) => new Date(2026, 10, d, h, m).toISOString();
    const day = new Date(2026, 10, 9);
    assert.equal(Cal.timeRange({ start: iso(9, 15), end: iso(9, 16), all_day: false }, day), "15:00 – 16:00");
    assert.equal(Cal.timeRange({ start: iso(9, 18, 30), end: iso(9, 18, 30), all_day: false }, day), "18:30");
    assert.equal(Cal.timeRange({ start: iso(8, 22), end: iso(9, 2), all_day: false }, day), "until 02:00");
    assert.equal(Cal.timeRange({ start: iso(9, 0), end: iso(10, 0), all_day: true }, day), "All day");
});

console.log(`${passed} passed, ${failed} failed`);
process.exit(failed ? 1 : 0);
