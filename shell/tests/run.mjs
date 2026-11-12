// Tests for the shell's pure JS (Calc.js, Search.js, DragData.js, QuickSettings.js, Notifications.js, CalendarGrid.js, Displays.js, Keys.js, Sun.js, Wallpapers.js, Shot.js, Users.js, Updates.js, Files Format.js, Tabs.js, Reorder.js, FilesActions.js and the Snapping page's Zones.js). Run: node shell/tests/run.mjs
// The files are QML JS libraries; strip the `.pragma library` line and load them.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import assert from "node:assert/strict";

const here = dirname(fileURLToPath(import.meta.url));
// `deps`: what its `.import "…" as Name` lines bring in, by Name.
function load(file, names, deps = {}) {
    const src = readFileSync(join(here, "..", file), "utf8").replace(/^\.pragma library\s*$/m, "").replace(/^\.import .*$/gm, "");
    return new Function(...Object.keys(deps), `${src}\nreturn { ${names.join(", ")} };`)(...Object.values(deps));
}
const Calc = load("Calc.js", ["evaluate"]);
const Search = load("Search.js", ["score", "rankApps", "frequent", "parseRecent", "matchFiles", "tildify", "matchCommands", "pushHistory", "terminalCommand", "results"]);
const DragData = load("DragData.js", ["fileUri", "forFiles", "forEntry"]);
const QS = load("QuickSettings.js", ["uptime", "duration", "batteryLine", "nightLightUntil", "nextProfile", "profileName", "initials", "displayName"]);
const N = load("Notifications.js", ["ago", "shouldPopup", "popupTimeout", "buttons", "hasDefault", "iconKind", "themedImageName", "rememberApp"]);
const D = load("settings/modules/displays/Displays.js", ["modeString", "hzLabel", "resolutions", "refreshes", "withResolution", "logicalSize", "draftFrom", "snap", "dragTo", "resolveOverlap", "connected", "normalize", "place", "profilesFor", "profileName", "toConfig", "differs"]);
const K = load("settings/modules/keyboard/Keys.js", ["isModifier", "keyName", "combo", "normalize", "expand", "conflict", "forRange", "caps", "spoken", "setKeys", "assign", "newId", "parseLayouts", "searchLayouts"]);
const Sun = load("Sun.js", ["times", "isDay", "untilChange"]);
const W = load("Wallpapers.js", ["builtin", "isBuiltin", "expand", "tildify", "parseList", "all", "next", "rotationText"]);
const Shot = load("Shot.js", ["fromDrag", "clampMove", "resize", "toRegion", "windowAt", "sizeLabel"]);
const U = load("settings/modules/users/Users.js", ["suggestName", "nameProblem", "passwordProblem", "lastSeen", "fingers", "fingerName", "nextFinger", "enrollText", "enrollFraction"]);
const Up = load("settings/modules/updates/Updates.js", ["ago", "day", "headline", "badge", "note", "fraction"]);
const F = load("../apps/files/qml/Format.js", ["size", "date", "kind", "click", "eta", "stemLength", "fullDate", "exactSize", "hasBit", "withBit", "sumMatch", "pathsAs", "groupName"]);
const Drops = load("../apps/files/qml/Drops.js", ["uris", "paths", "parent", "plan", "accepts"]);
const Lay = load("../apps/files/qml/Layout.js", ["adaptive", "treemap"]);
const Reorder = load("../apps/files/qml/Reorder.js", ["slot"]);
const Act = load("settings/modules/files/FilesActions.js", ["trim", "applies", "command", "quote", "menu", "toolbar"]);
const Arr = load("../apps/files/qml/Arrange.js", ["groupOf", "arrange", "rows", "step"], { F });
const Tabs = load("../apps/files/qml/Tabs.js", ["move", "slot", "afterClose", "tearOff", "insertAt", "mime", "pack", "unpack"]);
const Z = load("settings/modules/snapping/Zones.js", ["fallback", "normalize", "valid", "split", "canMerge", "merge", "lines", "moveLine", "select", "aspect"]);
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
    assert.equal(N.shouldPopup(1, false, false, true), false, "a muted app doesn't pop up");
    assert.equal(N.shouldPopup(2, false, false, true), true, "unless it's critical");
});
test("popup timeouts", () => {
    assert.equal(N.popupTimeout(2, 5000), 0, "critical stays until dismissed");
    assert.equal(N.popupTimeout(1, -1), 5000, "server default");
    assert.equal(N.popupTimeout(1, 0), 5000, "'never expire' still leaves the screen; it stays in the panel");
    assert.equal(N.popupTimeout(1, 500), 3000, "not too short to read");
    assert.equal(N.popupTimeout(1, 120000), 30000, "not forever");
    assert.equal(N.popupTimeout(1, 8000), 8000);
    assert.equal(N.popupTimeout(1, -1, 12000), 12000, "the user's popup duration");
    assert.equal(N.popupTimeout(1, 8000, 12000), 8000, "an app's own timeout wins");
    assert.deepEqual(N.rememberApp(["Mail", "Chat"], "Chat"), ["Chat", "Mail"]);
    assert.deepEqual(N.rememberApp(["Mail"], ""), ["Mail"]);
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

// ---- Launcher results per chip ------------------------------------------------
{
    const app = (id, name, extra = {}) => ({ id, name, genericName: "", keywords: [], comment: "", ...extra });
    const apps = [app("firefox", "Firefox"), app("ptyxis", "Terminal", { genericName: "Terminal" }), app("files", "Files"), app("calc", "Calculator")];
    const files = [{ name: "notes.md", path: "/h/notes.md", dir: "/h" }, { name: "terminal-tips.txt", path: "/h/t.txt", dir: "/h" }];
    const commands = ["htop", "htpasswd", "firefox", "fish", "ls", "lsblk", "ethtool", "gp-display-html"];
    const base = { apps, files, commands, usage: { firefox: 3 }, pinned: ["files"], history: ["sudo apt update", "htop"], calcHistory: [{ expr: "2+2", result: "4" }] };
    const kinds = r => r.items.map(i => i.kind + ":" + (i.app?.id ?? i.file?.name ?? (i.cmd !== undefined ? i.cmd + (i.terminal ? "@term" : "") : i.result)));
    test("each chip has its own empty state", () => {
        const all = Search.results({ ...base, query: "", category: "all" });
        assert.deepEqual(kinds(all), ["app:files", "app:firefox", "file:notes.md", "file:terminal-tips.txt"]);
        assert.equal(all.grid, 2, "pinned then used apps form the grid");
        const a = Search.results({ ...base, query: "", category: "apps" });
        assert.deepEqual(kinds(a), ["app:calc", "app:files", "app:firefox", "app:ptyxis"], "every app, A-Z");
        assert.equal(a.grid, 4);
        assert.deepEqual(kinds(Search.results({ ...base, query: "", category: "files" })), ["file:notes.md", "file:terminal-tips.txt"]);
        assert.deepEqual(kinds(Search.results({ ...base, query: "", category: "commands" })), ["cmd:sudo apt update", "cmd:htop"], "recent commands");
        assert.deepEqual(kinds(Search.results({ ...base, query: "", category: "calculator" })), ["calc:4"]);
    });
    test("commands chip lists programs and offers a terminal", () => {
        const r = Search.results({ ...base, query: "ht", category: "commands" });
        // Run as typed (in the background or a terminal), then the programs
        // that match; a recent command already listed isn't repeated.
        assert.deepEqual(kinds(r), ["cmd:ht", "cmd:ht@term", "cmd:htop", "cmd:htpasswd", "cmd:gp-display-html"]);
        assert.deepEqual(kinds(Search.results({ ...base, query: "apt", category: "commands" })).slice(2), ["cmd:sudo apt update"], "recent commands match anywhere");
        const args = Search.results({ ...base, query: "ls -la", category: "commands" });
        assert.deepEqual(kinds(args), ["cmd:ls -la", "cmd:ls -la@term"], "no program list once arguments are typed");
        assert.deepEqual(Search.matchCommands("ht", ["ethtool", "htop", "gp-display-html", "pod2html"], 6), ["htop", "gp-display-html"], "prefixes only, not the middle of a name");
        const gt = Search.results({ ...base, query: "> htop", category: "all" });
        assert.deepEqual(kinds(gt).slice(0, 2), ["cmd:htop", "cmd:htop@term"], "> works from any chip");
    });
    test("other chips filter while typing", () => {
        assert.deepEqual(kinds(Search.results({ ...base, query: "term", category: "apps" })), ["app:ptyxis"]);
        assert.deepEqual(kinds(Search.results({ ...base, query: "term", category: "files" })), ["file:terminal-tips.txt"]);
        assert.deepEqual(kinds(Search.results({ ...base, query: "2*3", category: "calculator", calc: "6" })), ["calc:6"]);
        assert.deepEqual(kinds(Search.results({ ...base, query: "fire", category: "all" })), ["app:firefox"]);
        assert.deepEqual(kinds(Search.results({ ...base, query: "htop", category: "all" })), ["cmd:htop"], "a known program can be run from All");
    });
    test("history and terminals", () => {
        assert.deepEqual(Search.pushHistory(["a", "b", "c"], "b", 3), ["b", "a", "c"]);
        assert.deepEqual(Search.pushHistory(["a", "b", "c"], "d", 3), ["d", "a", "b"]);
        assert.deepEqual(Search.terminalCommand("ptyxis", "htop"), ["ptyxis", "--standalone", "--", "sh", "-c", 'htop; exec "${SHELL:-sh}"']);
        assert.deepEqual(Search.terminalCommand("ptyxis --standalone", "x").slice(0, 3), ["ptyxis", "--standalone", "--"]);
        assert.deepEqual(Search.terminalCommand("foot", "x").slice(0, 2), ["foot", "sh"]);
        assert.deepEqual(Search.terminalCommand("/usr/bin/alacritty", "x").slice(0, 2), ["/usr/bin/alacritty", "-e"]);
        assert.deepEqual(Search.terminalCommand("weird-term", "x").slice(0, 2), ["weird-term", "-e"]);
    });
}

// ---- Displays ------------------------------------------------------------
const laptop = {
    id: "builtin", strong: true, builtin: true, connector: "eDP-1", make: "BOE", model: "NE160QDM",
    modes: [{ width: 2560, height: 1600, refresh: 165000, preferred: true }, { width: 2560, height: 1600, refresh: 60000, preferred: false },
            { width: 1920, height: 1200, refresh: 60000, preferred: false }],
    mode: { width: 2560, height: 1600, refresh: 165000, preferred: true }, scale: 1.6, transform: "normal",
    position: [0, 0], enabled: true, vrr: false,
};
const dell = {
    id: "edid:3f9a1c2b7e44", strong: true, builtin: false, connector: "DP-5", make: "DEL", model: "DELL U2723QE",
    modes: [{ width: 2560, height: 1440, refresh: 59951, preferred: true }], mode: { width: 2560, height: 1440, refresh: 59951 },
    scale: 1, transform: "normal", position: [1600, 0], enabled: true, vrr: false,
};

test("modes: strings the daemon accepts, labels, choices", () => {
    assert.equal(D.modeString({ width: 2560, height: 1440, refresh: 165000 }), "2560x1440@165");
    assert.equal(D.modeString({ width: 1920, height: 1080, refresh: 59951 }), "1920x1080@59.951");
    assert.match(D.modeString({ width: 1920, height: 1080, refresh: 59951 }), /^([1-9][0-9]{2,4})x([1-9][0-9]{2,4})(?:@([0-9]{1,3}(?:\.[0-9]{1,3})?))?$/);
    assert.equal(D.hzLabel(165000), "165 Hz");
    assert.equal(D.hzLabel(59951), "59.95 Hz");
    assert.equal(D.hzLabel(60020), "60 Hz");
    assert.deepEqual(D.resolutions(laptop.modes).map(r => r.width), [2560, 1920]);
    assert.deepEqual(D.refreshes(laptop.modes, 2560, 1600), [165000, 60000]);
    assert.deepEqual(D.withResolution(laptop.modes, 1920, 1200, 165000), { width: 1920, height: 1200, refresh: 60000 });
});

test("logical size follows scale and rotation", () => {
    const d = D.draftFrom([laptop])[0];
    assert.deepEqual(D.logicalSize(d), { w: 1600, h: 1000 });
    assert.deepEqual(D.logicalSize(Object.assign({}, d, { transform: "90" })), { w: 1000, h: 1600 });
});

test("labels: saved, else Built-in or the model", () => {
    const draft = D.draftFrom([laptop, dell], { monitor: [{ id: dell.id, label: "Desk left" }] });
    assert.deepEqual(draft.map(d => d.label), ["Built-in", "Desk left"]);
});

test("snapping: side by side and aligned, only within reach", () => {
    const other = { x: 0, y: 0, w: 1600, h: 1000 };
    assert.deepEqual(D.snap({ x: 1612, y: 9, w: 2560, h: 1440 }, [other], 20), { x: 1600, y: 0, to: 0 });
    assert.deepEqual(D.snap({ x: 1700, y: 300, w: 2560, h: 1440 }, [other], 20), { x: 1700, y: 300, to: -1 });
    // Bottoms line up.
    assert.equal(D.snap({ x: 1600, y: -446, w: 2560, h: 1440 }, [other], 20).y, -440);
});

test("dropping never overlaps and the layout starts at 0,0", () => {
    let draft = D.draftFrom([laptop, dell]);
    draft = D.place(draft, 1, 800, 100, 20);
    const [a, b] = draft.map(d => ({ x: d.position[0], y: d.position[1], w: D.logicalSize(d).w, h: D.logicalSize(d).h }));
    assert.ok(!(a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h), JSON.stringify([a, b]));
    assert.equal(Math.min(a.x, b.x), 0);
    assert.equal(Math.min(a.y, b.y), 0);
    assert.ok(D.connected(draft));
    // Dragged far away: still placed, but no longer touching.
    const apart = D.place(D.draftFrom([laptop, dell]), 1, 5000, 3000, 20);
    assert.ok(!D.connected(apart));
});

test("toConfig keeps other monitors, writes a profile first, and round-trips", () => {
    const saved = {
        monitor: [{ id: "edid:aaaaaaaaaaaa", scale: 2 }, { id: "builtin", scale: 1.25 }],
        profile: [{ name: "Old", output: [{ id: "builtin", position: [0, 0] }] }],
    };
    let draft = D.draftFrom([laptop, dell], saved);
    draft[1].label = "Desk left";
    draft[1].vrr = true;
    draft[1].enabled = false;
    const cfg = D.toConfig(saved, draft, "");
    assert.deepEqual(cfg.monitor[0], { id: "edid:aaaaaaaaaaaa", scale: 2 }, "other monitors untouched");
    assert.deepEqual(cfg.monitor[1], { id: "builtin", mode: "2560x1600@165", scale: 1.6 });
    assert.deepEqual(cfg.monitor[2], { id: dell.id, label: "Desk left", mode: "2560x1440@59.951", vrr: "on" });
    assert.equal(cfg.profile[0].name, "Built-in + Desk left");
    assert.deepEqual(cfg.profile[0].output[1], { id: dell.id, position: [1600, 0], disabled: true });
    assert.equal(cfg.profile[1].name, "Old");
    assert.deepEqual(D.profilesFor(cfg, ["builtin", dell.id]).map(p => p.name), ["Built-in + Desk left"]);
    // Saving again under the same name replaces, not duplicates.
    const again = D.toConfig(cfg, draft, "Built-in + Desk left");
    assert.equal(again.profile.length, 2);
    // One monitor: no profile.
    assert.equal(D.toConfig({}, D.draftFrom([laptop]), "").profile.length, 0);
    assert.equal(D.profileName(draft, { profile: [{ name: "Built-in + Desk left", output: [] }] }), "Built-in + Desk left 2");
});

test("differs notices changes and only changes", () => {
    const a = D.draftFrom([laptop, dell]);
    const b = D.draftFrom([laptop, dell]);
    assert.ok(!D.differs(a, b));
    b[0].scale = 2;
    assert.ok(D.differs(a, b));
});

test("a dragged tile stays inside the card; fine moves go a quarter as far", () => {
    const eq = (a, b, m) => assert.deepEqual(a, b, m);
    eq(D.dragTo(100, 50, 40, -20, false, 500, 200), { x: 140, y: 30 }, "drag: follows the pointer");
    eq(D.dragTo(100, 50, 40, -20, true, 500, 200), { x: 110, y: 45 }, "drag: fine moves a quarter as far");
    eq(D.dragTo(100, 50, -400, 900, false, 500, 200), { x: 0, y: 200 }, "drag: can't leave the card");
    eq(D.dragTo(100, 50, 900, -400, false, 500, 200), { x: 500, y: 0 }, "drag: nor off the other sides");
    eq(D.dragTo(10, 10, 5, 5, false, -20, -20), { x: 0, y: 0 }, "drag: a tile bigger than the card stays at its corner");
});

test("a dragged bookmark lands before the first row whose middle is below the pointer", () => {
    // Three other bookmarks, 32 tall, 2 apart; the second has a folder open under it (rows 102-134).
    const others = [{ y: 0, height: 32 }, { y: 34, height: 32 }, { y: 136, height: 32 }];
    assert.equal(Reorder.slot(others, -10), 0, "above everything: first");
    assert.equal(Reorder.slot(others, 20), 1, "past the first one's middle");
    assert.equal(Reorder.slot(others, 110), 2, "over the open folder: after its bookmark");
    assert.equal(Reorder.slot(others, 400), 3, "below everything: last");
});

test("Files' menus: entries left out, separators tidied, your actions applied", () => {
    const items = [{ label: "Open" }, { label: "Quick look" }, { separator: true }, { label: "Duplicate" }, { label: "Copy path", items: [] }, { separator: true },
                   { label: "Tags", items: [] }, { label: "Move to other side" }, { separator: true }, { label: "Delete permanently…" }];
    assert.deepEqual(Act.trim(items, ["quick-look", "duplicate", "copy-path", "tags", "other-side", "delete"]).map(i => i.label ?? "—"), ["Open"]);
    assert.deepEqual(Act.trim(items, ["duplicate"]).map(i => i.label ?? "—"), ["Open", "Quick look", "—", "Copy path", "—", "Tags", "Move to other side", "—", "Delete permanently…"]);
    assert.equal(Act.trim(items, []), items);
    // Applies to: files / folders / types.
    const a = { on: "files", types: "jpg, .PNG" };
    assert.ok(Act.applies(a, [{ name: "a.jpg", dir: false }, { name: "b.png", dir: false }]));
    assert.ok(!Act.applies(a, [{ name: "a.jpg", dir: false }, { name: "c.txt", dir: false }]));
    assert.ok(!Act.applies({ on: "folders", types: "" }, [{ name: "x", dir: false }]));
    assert.ok(Act.applies({ on: "any", types: "" }, [{ name: "x", dir: true }]));
    // Commands: quoted paths in place, or on the end.
    assert.equal(Act.command("convert %f -resize 50% out.png", ["/a b/it's.jpg"], "/a b"), "convert '/a b/it'\\''s.jpg' -resize 50% out.png");
    assert.equal(Act.command("gimp", ["/x/1.png", "/x/2.png"], "/x"), "gimp '/x/1.png' '/x/2.png'");
    assert.equal(Act.command("cd %d && zip out.zip %n", ["/p/a.txt"], "/p"), "cd '/p' && zip out.zip 'a.txt'");
    // Every id is unique.
    for (const list of [Act.menu, Act.toolbar]) assert.equal(new Set(list.map(x => x.id)).size, list.length);
});

// ---- Keys (Keyboard & Shortcuts) -------------------------------------------
const views = [
    { id: "launcher", keys: ["SUPER"], defaults: ["SUPER"] },
    { id: "fullscreen", keys: ["SUPER+F"], defaults: ["SUPER+F"] },
    { id: "close-window", keys: ["SUPER+Q"], defaults: ["SUPER+Q"] },
    { id: "workspace", keys: ["SUPER+1-9"], defaults: ["SUPER+1-9"] },
    { id: "custom-files", keys: ["SUPER+E"], defaults: [], command: "nautilus" },
];

test("keys: names by position and by function", () => {
    assert.equal(K.keyName(0x46, 41), "F", "letters by position");
    assert.equal(K.keyName(0x21, 10), "1", "SHIFT+1 is still 1");
    assert.equal(K.keyName(0x01000031, 0), "F2");
    assert.equal(K.keyName(0x01000072, 0), "XF86AudioRaiseVolume");
    assert.equal(K.keyName(0x01000004, 36), "RETURN");
    assert.equal(K.keyName(0x12345, 999), "", "unknown keys can't be shortcuts");
    assert.ok(K.isModifier(0x01000053) && K.isModifier(0x01000020) && !K.isModifier(0x46));
});

test("keys: combos are written one way", () => {
    assert.equal(K.combo({ logo: true, shift: true }, "S"), "SUPER+SHIFT+S");
    assert.equal(K.combo({ logo: true }, ""), "SUPER");
    assert.equal(K.normalize("shift+super+s"), "SUPER+SHIFT+S");
    assert.equal(K.normalize("ctrl+alt+BackSpace"), "CTRL+ALT+BACKSPACE");
    assert.equal(K.normalize("logo"), "SUPER");
    assert.deepEqual(K.expand("SUPER+1-9").length, 9);
    assert.equal(K.expand("SUPER+1-9")[2], "SUPER+3");
    assert.equal(K.forRange("SUPER+SHIFT+4"), "SUPER+SHIFT+1-9");
    assert.equal(K.forRange("SUPER+W"), null);
    assert.equal(K.forRange("SUPER+SHIFT+Left", "arrows"), "SUPER+SHIFT+ARROWS");
    assert.equal(K.forRange("SUPER+4", "arrows"), null);
    assert.deepEqual(K.expand("super+arrows"), ["SUPER+LEFT", "SUPER+RIGHT", "SUPER+UP", "SUPER+DOWN"]);
    assert.deepEqual(K.caps("SUPER+ARROWS"), ["SUPER", "←→↑↓"]);
    // A single arrow shortcut clashes with the range that includes it.
    assert.equal(K.conflict([{ id: "focus-direction", keys: ["SUPER+ARROWS"] }], "SUPER+Up", "x")?.id, "focus-direction");
    assert.deepEqual(K.caps("SUPER+1-9"), ["SUPER", "1–9"]);
    assert.deepEqual(K.caps("XF86AudioRaiseVolume"), ["VOL +"]);
    assert.equal(K.spoken("SUPER+SPACE"), "SUPER + SPACE");
});

test("keys: conflicts, including inside 1-9", () => {
    assert.equal(K.conflict(views, "super+f", "close-window").id, "fullscreen");
    assert.equal(K.conflict(views, "SUPER+F", "fullscreen"), null, "not with itself");
    assert.equal(K.conflict(views, "SUPER+3", "fullscreen").id, "workspace");
    assert.equal(K.conflict(views, "SUPER+SHIFT+F", "x"), null);
});

test("keys: assigning replaces, takes over, and drops back to defaults", () => {
    const st = { shortcuts: views, overrides: {}, custom: [{ id: "custom-files", name: "Files", command: "nautilus", keys: ["SUPER+E"] }] };
    // SUPER+E for fullscreen: taken from the custom shortcut.
    let r = K.assign(st, "fullscreen", "SUPER+E", false);
    assert.deepEqual(r.overrides, { fullscreen: ["SUPER+E"] });
    assert.deepEqual(r.custom[0].keys, []);
    // Close window gets SUPER+F (taken from fullscreen's default).
    r = K.assign(st, "close-window", "SUPER+F", true);
    assert.deepEqual(r.overrides, { fullscreen: [], "close-window": ["SUPER+Q", "SUPER+F"] });
    // Setting a shortcut back to its default removes the override.
    const st2 = Object.assign({}, st, { overrides: { "close-window": ["ALT+F4"] }, shortcuts: views.map(v => v.id === "close-window" ? Object.assign({}, v, { keys: ["ALT+F4"] }) : v) });
    assert.deepEqual(K.assign(st2, "close-window", "SUPER+Q", false).overrides, {});
    // A plain number key taken by the workspaces leaves the other eight.
    r = K.assign(st, "fullscreen", "SUPER+3", false);
    assert.deepEqual(r.overrides.workspace, [], "the range can't lose one key, so it's turned off");
});

test("keys: new ids and the XKB layout list", () => {
    assert.equal(K.newId("Open Files!", []), "custom-open-files");
    assert.equal(K.newId("Open Files", [{ id: "custom-open-files" }]), "custom-open-files-2");
    const db = K.parseLayouts("! model\n  pc105           Generic 105-key PC\n\n! layout\n  gb              English (UK)\n  es              Spanish\n\n! variant\n  extd            gb: English (UK, extended, Windows)\n  nodeadkeys      es: Spanish (no dead keys)\n\n! option\n  grp             Switching\n");
    assert.deepEqual(db.layouts.map(l => l.layout), ["gb", "es"]);
    assert.deepEqual(db.variants[1], { layout: "es", variant: "nodeadkeys", name: "Spanish (no dead keys)" });
    assert.deepEqual(K.searchLayouts(db, "spanish").map(l => l.variant), ["", "nodeadkeys"]);
    assert.equal(K.searchLayouts(db, "").length, 2, "browsing shows plain layouts");
});

// ---- Sun (Auto theme) ------------------------------------------------------
const near = (a, b, mins) => Math.abs(a - b) <= mins * 60000;
test("sun: London's longest day and a winter day", () => {
    const june = Sun.times(Date.UTC(2026, 5, 21, 12), 51.5074, -0.1278);
    assert.ok(near(june.rise, Date.UTC(2026, 5, 21, 3, 43), 3), new Date(june.rise).toISOString());
    assert.ok(near(june.set, Date.UTC(2026, 5, 21, 20, 21), 3), new Date(june.set).toISOString());
    const dec = Sun.times(Date.UTC(2026, 11, 21, 12), 51.5074, -0.1278);
    assert.ok(near(dec.rise, Date.UTC(2026, 11, 21, 8, 4), 3), new Date(dec.rise).toISOString());
    assert.ok(near(dec.set, Date.UTC(2026, 11, 21, 15, 53), 3), new Date(dec.set).toISOString());
});
test("sun: day and night, poles, and no location", () => {
    assert.equal(Sun.isDay(Date.UTC(2026, 5, 21, 12), 51.5, -0.13), true);
    assert.equal(Sun.isDay(Date.UTC(2026, 5, 21, 23), 51.5, -0.13), false);
    assert.equal(Sun.times(Date.UTC(2026, 5, 21, 12), 78.2, 15.6).polar, "day", "Svalbard in June");
    assert.equal(Sun.isDay(Date.UTC(2026, 11, 21, 12), 78.2, 15.6), false, "Svalbard in December");
    const noon = new Date(2026, 10, 9, 12).getTime();
    assert.equal(Sun.isDay(noon, null, null), true);
    assert.equal(Sun.isDay(new Date(2026, 10, 9, 22).getTime(), null, null), false);
    assert.equal(Sun.untilChange(noon, null, null), 7 * 3600000 + 1000 > 6 * 3600000 ? 6 * 3600000 : 7 * 3600000 + 1000);
    const evening = new Date(2026, 10, 9, 18, 30).getTime();
    assert.equal(Sun.untilChange(evening, null, null), 30 * 60000 + 1000);
});

// ---- Wallpapers --------------------------------------------------------------
test("wallpapers: paths, listing and rotation", () => {
    assert.equal(W.expand("~/Pictures/Wallpapers", "/home/me"), "/home/me/Pictures/Wallpapers");
    assert.equal(W.expand("/srv/bg", "/home/me"), "/srv/bg");
    assert.equal(W.tildify("/home/me/Pictures/a.jpg", "/home/me"), "~/Pictures/a.jpg");
    assert.deepEqual(W.parseList("/w/b.JPG\n/w/a.png\n/w/notes.txt\n\n/w/a.png\n"), ["/w/a.png", "/w/b.JPG"]);
    assert.ok(W.isBuiltin("builtin:rose") && W.builtin("builtin:rose").name === "Rose");
    assert.equal(W.all(["/w/a.png"], ["/x/c.jpg", "/w/a.png"]).length, 4 + 2);
    assert.equal(W.next(["/w/a.png", "/w/b.png"], [], "/w/a.png"), "/w/b.png");
    assert.equal(W.next(["/w/a.png", "/w/b.png"], [], "/w/b.png"), "/w/a.png", "wraps");
    assert.equal(W.next(["/w/a.png"], [], "builtin:rose"), "/w/a.png", "from a gradient to your images");
    assert.equal(W.next([], [], "builtin:rose"), "builtin:dusk", "no images: through the gradients");
    assert.equal(W.rotationText(30), "changes every 30 min");
    assert.equal(W.rotationText(0), "");
});

// ---- Shot (screenshot tool) --------------------------------------------------
test("shot: drawing a selection, any direction, square with shift", () => {
    assert.deepEqual(Shot.fromDrag({ x: 100, y: 100 }, { x: 300, y: 250 }), { x: 100, y: 100, w: 200, h: 150 });
    assert.deepEqual(Shot.fromDrag({ x: 300, y: 250 }, { x: 100, y: 100 }), { x: 100, y: 100, w: 200, h: 150 }, "dragged up-left");
    assert.deepEqual(Shot.fromDrag({ x: 100, y: 100 }, { x: 300, y: 250 }, true), { x: 100, y: 100, w: 150, h: 150 });
    assert.deepEqual(Shot.fromDrag({ x: 300, y: 300 }, { x: 100, y: 250 }, true), { x: 250, y: 250, w: 50, h: 50 });
});
test("shot: moving stays on screen, resizing keeps the other side", () => {
    const screen = { x: 0, y: 0, w: 1440, h: 808 };
    assert.deepEqual(Shot.clampMove({ x: 1400, y: -20, w: 200, h: 100 }, screen), { x: 1240, y: 0, w: 200, h: 100 });
    const r = { x: 100, y: 100, w: 200, h: 100 };
    assert.deepEqual(Shot.resize(r, "se", 50, 20), { x: 100, y: 100, w: 250, h: 120 });
    assert.deepEqual(Shot.resize(r, "w", -30, 999), { x: 70, y: 100, w: 230, h: 100 }, "an edge only moves its own side");
    assert.deepEqual(Shot.resize(r, "e", -260, 0), { x: 40, y: 100, w: 60, h: 100 }, "dragged past the other side flips");
    assert.deepEqual(Shot.resize(r, "nw", -10, -10, true), { x: 190, y: 90, w: 110, h: 110 }, "square from a corner");
});
test("shot: regions, windows under the pointer, size label", () => {
    assert.deepEqual(Shot.toRegion({ x: 10.4, y: 20.6, w: 100.2, h: 50 }), [10, 21, 101, 50]);
    const wins = [
        { id: 2, workspace: 1, geometry: [100, 100, 400, 300] },
        { id: 1, workspace: 1, geometry: [0, 0, 1000, 800] },
        { id: 3, workspace: 2, geometry: [0, 0, 1000, 800] },
    ];
    assert.equal(Shot.windowAt(wins, 150, 150, 1).id, 2, "the topmost");
    assert.equal(Shot.windowAt(wins, 700, 700, 1).id, 1);
    assert.equal(Shot.windowAt(wins, 1200, 10, 1), null);
    assert.equal(Shot.sizeLabel({ w: 560, h: 330 }, 1.25), "700 × 413");
});

// ---- Users -------------------------------------------------------------------
test("users: suggested usernames, checks, last seen", () => {
    assert.equal(U.suggestName("Anna-Maria Smith"), "annamaria");
    assert.equal(U.suggestName("Zoë Ångström"), "zoe");
    assert.equal(U.suggestName("  "), "");
    assert.equal(U.suggestName("2pac"), "u2pac");
    assert.equal(U.nameProblem("jack", []), "");
    assert.equal(U.nameProblem("Jack", []), "Lowercase letters, digits, - and _, starting with a letter");
    assert.equal(U.nameProblem("root", []), "That name is reserved");
    assert.equal(U.nameProblem("jack", ["jack"]), "Someone already has that username");
    assert.equal(U.passwordProblem("short", "short"), "At least 8 characters");
    assert.equal(U.passwordProblem("longenough", "longenougH"), "The passwords don't match");
    assert.equal(U.passwordProblem("longenough", "longenough"), "");
    const now = new Date(2026, 10, 9, 15).getTime();
    assert.equal(U.lastSeen(0, now), "Never signed in");
    assert.equal(U.lastSeen(new Date(2026, 10, 9, 8).getTime() / 1000, now), "Signed in today");
    assert.equal(U.lastSeen(new Date(2026, 10, 8, 23).getTime() / 1000, now), "Signed in yesterday");
    assert.equal(U.lastSeen(new Date(2026, 10, 5).getTime() / 1000, now), "Signed in 4 days ago");
    assert.equal(U.lastSeen(new Date(2026, 9, 2).getTime() / 1000, now), "Signed in on 2 Oct");
    assert.equal(U.lastSeen(new Date(2025, 9, 2).getTime() / 1000, now), "Signed in on 2 Oct 2025");
});

test("updates page text", () => {
    const now = Date.UTC(2026, 10, 10, 12, 0, 0);
    const t = now / 1000;
    assert.equal(Up.ago(t - 20, now), "just now");
    assert.equal(Up.ago(t - 300, now), "5 minutes ago");
    assert.equal(Up.ago(t - 60, now), "1 minute ago");
    assert.equal(Up.ago(t - 7200, now), "2 hours ago");
    assert.equal(Up.ago(t - 86400 * 1.5, now), "yesterday");
    assert.equal(Up.ago(Date.UTC(2026, 10, 2, 12) / 1000, now), "on 2 Nov");
    assert.equal(Up.ago(Date.UTC(2025, 10, 2, 12) / 1000, now), "on 2 Nov 2025");
    assert.equal(Up.ago(0, now), "never");
    assert.equal(Up.headline({ running: "0.4.2", supported: true, checked: t - 300 }, now), "WhiteRose 0.4.2 · checked 5 minutes ago");
    assert.equal(Up.headline({ running: "", supported: false }, now), "A build being worked on");
    assert.equal(Up.headline({ running: "0.4.2", supported: true, busy: "checking" }, now), "WhiteRose 0.4.2 · checking…");
    assert.deepEqual(Up.badge({ state: "ready", pending: true }), ["Ready to apply", "ready"]);
    assert.deepEqual(Up.badge(null), ["", "plain"]);
    assert.match(Up.note({ state: "ready" }, { running: "0.4.2", launcher: true }), /session restarts/);
    assert.match(Up.note({ state: "ready" }, { running: "0.4.2", launcher: false }), /log out/);
    assert.match(Up.note({ state: "downloaded" }, { running: "0.4.2" }), /beside 0.4.2/);
    assert.equal(Up.fraction({ progress: [25, 100] }), 0.25);
    assert.equal(Up.fraction({ progress: [0, 0] }), 0);
});

test("files: dragging out of archives", () => {
    const a = "archive:/home/me/My Trip.zip!/photos/a b.jpg";
    const u = Drops.uris([a, "/home/me/x y.txt"]);
    assert.equal(u, "archive:/home/me/My%20Trip.zip!/photos/a%20b.jpg\r\nfile:///home/me/x%20y.txt\r\n");
    assert.deepEqual(Drops.paths(u.trim().split("\r\n")), [a, "/home/me/x y.txt"]);
    // Out of an archive: always copied, never into an archive or the Trash.
    assert.deepEqual(Drops.plan([a], "/home/me/Desktop", false, true, true), { kind: "copy", paths: [a] });
    assert.equal(Drops.plan([a], "trash:", false, false, false), null);
    assert.equal(Drops.plan(["/home/me/x"], "archive:/t.zip!/", false, false, false), null);
    assert.equal(Drops.accepts([a], "/tmp"), true);
    assert.equal(Drops.accepts(["/home/me/x"], "archive:/t.zip!/"), false);
});

test("files: checksums, paths as text, group names", () => {
    const sums = { md5: "900150983cd24fb0d6963f7d28e17f72", sha1: "a9993e364706816aba3e25717850c26c9cd0d89d", sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad" };
    assert.equal(F.sumMatch(sums, " SHA256:BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD "), "SHA-256");
    assert.equal(F.sumMatch(sums, "900150983cd24fb0d6963f7d28e17f72"), "MD5");
    assert.equal(F.sumMatch(sums, "a999 3e36 4706 816a ba3e 2571 7850 c26c 9cd0 d89d"), "SHA-1");
    assert.equal(F.sumMatch(sums, "nope"), "");
    assert.equal(F.sumMatch(sums, ""), "");
    const ps = ["/home/me/My Notes/it's.md", "/tmp/a b"];
    assert.equal(F.pathsAs(ps, "plain"), "/home/me/My Notes/it's.md\n/tmp/a b");
    assert.equal(F.pathsAs(ps, "quoted"), "'/home/me/My Notes/it'\\''s.md' '/tmp/a b'");
    assert.equal(F.pathsAs(["/home/me/My Notes/ü#1.md"], "uri"), "file:///home/me/My%20Notes/%C3%BC%231.md");
    assert.equal(F.pathsAs(ps, "name"), "it's.md\na b");
    assert.equal(F.groupName(["Holiday 2024 - 01.jpg", "Holiday 2024 - 02.jpg"]), "Holiday 2024");
    assert.equal(F.groupName(["a.txt", "b.txt"]), "New folder");
    assert.equal(F.groupName(["report.pdf"]), "report");
    assert.equal(F.groupName([]), "New folder");
});

test("fingerprint names and progress", () => {
    assert.equal(U.fingerName("right-index-finger"), "Right index finger");
    assert.equal(U.fingerName("left-thumb"), "Left thumb");
    assert.equal(U.nextFinger([]), "right-index-finger");
    assert.equal(U.nextFinger(["right-index-finger"]), "left-index-finger");
    assert.equal(U.nextFinger(U.fingers), "");
    assert.equal(U.enrollText(null, "press"), "Touch the reader");
    assert.equal(U.enrollText({ state: "stage", stage: 0, stages: 16 }, "swipe"), "Swipe your finger across the reader");
    assert.match(U.enrollText({ state: "stage", stage: 3, stages: 16 }, "press"), /Lift and touch again/);
    assert.equal(U.enrollText({ state: "retry", message: "Put your finger in the middle" }, "press"), "Put your finger in the middle");
    assert.equal(U.enrollFraction({ stage: 4, stages: 16 }), 0.25);
    assert.equal(U.enrollFraction({ stage: 20, stages: 16 }), 1);
    assert.equal(U.enrollFraction(null), 0);
});

// ---- Files (Format.js) ----------------------------------------------------------
test("files: sizes, dates, kinds", () => {
    assert.equal(F.size(512), "512 B");
    assert.equal(F.size(1234), "1.2 kB");
    assert.equal(F.size(52_400_000), "52 MB");
    assert.equal(F.size(3_100_000_000), "3.1 GB");
    const now = new Date(2026, 10, 9, 15).getTime();
    assert.equal(F.date(new Date(2026, 10, 9, 8, 5).getTime(), now), "Today 08:05");
    assert.equal(F.date(new Date(2026, 10, 8, 23, 0).getTime(), now), "Yesterday 23:00");
    assert.equal(F.date(new Date(2026, 2, 4).getTime(), now), "4 Mar");
    assert.equal(F.date(new Date(2024, 2, 4).getTime(), now), "4 Mar 2024");
    assert.equal(F.kind("image/png"), "Image");
    assert.equal(F.kind("application/vnd.oasis.opendocument.text"), "Document");
    assert.equal(F.kind("inode/directory"), "Folder");
});
test("files: selection by click, ctrl and shift", () => {
    const e = ["a", "b", "c", "d", "e"].map(n => ({ path: "/" + n }));
    let s = F.click(e, {}, -1, 1, false, false);
    assert.deepEqual(Object.keys(s.selected), ["/b"]);
    s = F.click(e, s.selected, s.anchor, 3, false, true);
    assert.deepEqual(Object.keys(s.selected), ["/b", "/c", "/d"], "shift: a range from the anchor");
    s = F.click(e, s.selected, s.anchor, 2, true, false);
    assert.deepEqual(Object.keys(s.selected), ["/b", "/d"], "ctrl: toggle one");
    s = F.click(e, s.selected, s.anchor, 0, false, false);
    assert.deepEqual(Object.keys(s.selected), ["/a"], "plain: just that one");
});

test("files: time left and the part of a name renaming selects", () => {
    assert.equal(F.eta(0, 100), "");
    assert.equal(F.eta(500, 0), "", "no rate yet: nothing");
    assert.equal(F.eta(4500, 1000), "5 s left");
    assert.equal(F.eta(180_000, 1000), "3 min left");
    assert.equal(F.eta(4_800_000, 1000), "1 h 20 min left");
    assert.equal(F.stemLength("notes.txt", false), 5);
    assert.equal(F.stemLength("backup.tar.gz", false), 6);
    assert.equal(F.stemLength(".bashrc", false), 7);
    assert.equal(F.stemLength("my.folder", true), 9);
});
test("files: properties formatting and permission bits", () => {
    assert.equal(F.fullDate(new Date(2026, 10, 9, 20, 43).getTime()), "9 Nov 2026, 20:43");
    assert.equal(F.fullDate(0), "");
    assert.equal(F.exactSize(1), "1 byte");
    assert.equal(F.exactSize(6600), "6.6 kB (6,600 bytes)");
    assert.equal(F.hasBit(0o640, 0, 1), true, "owner write");
    assert.equal(F.hasBit(0o640, 1, 1), false, "group write");
    assert.equal(F.hasBit(0o640, 1, 0), true, "group read");
    assert.equal(F.withBit(0o644, 0, 2, true), 0o744, "owner run on");
    assert.equal(F.withBit(0o755, 2, 2, false), 0o754, "others run off");
    assert.equal(F.withBit(0o4755, 0, 2, false), 0o4655, "other bits kept");
});
test("files: what's dragged and what a drop does", () => {
    assert.equal(Drops.uris(["/home/me/a b#1.txt", "/x"]), "file:///home/me/a%20b%231.txt\r\nfile:///x\r\n");
    assert.deepEqual(Drops.paths(["file:///home/me/a%20b%231.txt", "https://example.com/x", "file://localhost/tmp/y/"]), ["/home/me/a b#1.txt", "/tmp/y"]);
    const a = ["/home/me/a.txt", "/home/me/photos"];
    assert.deepEqual(Drops.plan(a, "/home/me/Documents", false, false, true), { kind: "move", paths: a }, "same drive: move");
    assert.deepEqual(Drops.plan(a, "/media/usb", false, false, false), { kind: "copy", paths: a }, "other drive: copy");
    assert.equal(Drops.plan(a, "/home/me/Documents", true, false, true).kind, "copy", "Ctrl copies");
    assert.equal(Drops.plan(a, "/media/usb", false, true, false).kind, "move", "Shift moves");
    assert.deepEqual(Drops.plan(a, "/home/me/photos", false, false, true), { kind: "move", paths: ["/home/me/a.txt"] }, "not onto itself");
    assert.equal(Drops.plan(["/home/me/photos"], "/home/me/photos/2024", false, false, true), null, "not into itself");
    assert.equal(Drops.plan(a, "/home/me", false, false, true), null, "moving where it already is: nothing");
    assert.equal(Drops.plan(a, "/home/me", true, false, true).kind, "copy", "copying there: numbered copies");
    assert.deepEqual(Drops.plan(a, "trash:", false, false, false), { kind: "trash", paths: a });
    assert.equal(Drops.accepts(["/home/me/photos"], "/home/me/photos"), false);
    assert.equal(Drops.accepts([], "/x"), false);
});
test("files: a view to suit the folder", () => {
    const f = (name, mime) => ({ name, mime, dir: false });
    const photos = ["a", "b", "c", "d"].map(n => f(n + ".jpg", "image/jpeg")).concat([f("notes.txt", "text/plain")]);
    assert.equal(Lay.adaptive(photos, "list"), "grid", "pictures: thumbnails");
    assert.equal(Lay.adaptive(photos.slice(0, 2), "list"), "list", "too few to say");
    assert.equal(Lay.adaptive([{ name: ".git", dir: true, mime: "inode/directory" }, f("x.png", "image/png")], "grid"), "list", "a repository");
    assert.equal(Lay.adaptive([f("Cargo.toml", "application/toml")], "cards"), "list");
    assert.equal(Lay.adaptive([f("a.pdf", "application/pdf"), f("b.odt", "x")], "cards"), "cards", "else what it was");
});
test("files: disk usage map", () => {
    const items = [{ value: 6 }, { value: 6 }, { value: 4 }, { value: 3 }, { value: 2 }, { value: 2 }, { value: 1 }, { value: 0 }];
    const r = Lay.treemap(items, 0, 0, 600, 400);
    assert.equal(r.length, 7, "nothing for empty ones");
    const area = r.reduce((a, b) => a + b.w * b.h, 0);
    assert.ok(Math.abs(area - 240000) < 1, "fills the space");
    for (const b of r) {
        assert.ok(b.x >= -1e-6 && b.y >= -1e-6 && b.x + b.w <= 600 + 1e-6 && b.y + b.h <= 400 + 1e-6, "inside");
        assert.ok(Math.abs(b.w * b.h - b.item.value * 10000) < 1, "area for size");
    }
    // No overlaps.
    for (let i = 0; i < r.length; i++) for (let j = i + 1; j < r.length; j++) {
        const a = r[i], b = r[j];
        const ox = Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x), oy = Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y);
        assert.ok(ox <= 1e-6 || oy <= 1e-6, "overlap " + i + " " + j);
    }
    assert.deepEqual(Lay.treemap([], 0, 0, 10, 10), []);
});
test("files: grouping", () => {
    const now = new Date(2026, 10, 9, 15).getTime();
    const day = 86400000;
    const f = (name, extra) => Object.assign({ name, dir: false, mime: "text/plain", size: 10, modified: now }, extra);
    const items = [
        f("old.txt", { modified: new Date(2023, 4, 1).getTime() }),
        f("today.txt"),
        f("Photos", { dir: true, mime: "inode/directory", modified: now - day }),
        f("pic.png", { mime: "image/png", modified: new Date(2026, 1, 3).getTime() }),
    ];
    const by = (k, d) => Arr.arrange(items, k, d, now).map(e => e.group + ":" + e.name);
    assert.deepEqual(by("modified"), ["Today:today.txt", "Yesterday:Photos", "Earlier this year:pic.png", "2023:old.txt"]);
    assert.deepEqual(by("modified", true), ["2023:old.txt", "Earlier this year:pic.png", "Yesterday:Photos", "Today:today.txt"]);
    assert.deepEqual(by("kind"), ["Folders:Photos", "Images:pic.png", "Text:old.txt", "Text:today.txt"], "folders first, then kinds; order kept inside");
    assert.equal(Arr.groupOf(f("x", { size: 2e9 }), "size", now).label, "Huge (over 1 GB)");
    assert.equal(Arr.groupOf(f("x", { size: 0 }), "size", now).label, "Empty");
    assert.equal(Arr.groupOf(f(".bashrc"), "name", now).label, "B");
    assert.equal(Arr.groupOf(f("2024 taxes"), "name", now).label, "0–9");
    assert.equal(Arr.groupOf(f("a", { tags: ["Work", "Urgent"] }), "tag", now).label, "Work", "its first tag");
    assert.equal(Arr.groupOf(f("a"), "tag", now).label, "No tags");
    assert.ok(Arr.groupOf(f("a"), "tag", now).key > Arr.groupOf(f("b", { tags: ["Zed"] }), "tag", now).key, "untagged last");
    assert.equal(Arr.arrange(items, "", false, now), items, "no grouping: as it was");
    assert.equal(items[0].group, undefined, "the originals untouched");
});
test("files: grouped grid rows and moving through them", () => {
    const e = (name, group) => ({ name, group });
    const list = [e("a", "A"), e("b", "A"), e("c", "A"), e("d", "B"), e("f", "C"), e("g", "C")];
    const r = Arr.rows(list, 2, {});
    assert.deepEqual(r.map(x => x.header ?? x.items.map(it => it.e.name).join("")), ["A", "ab", "c", "B", "d", "C", "fg"]);
    assert.equal(r[0].count, 3);
    const folded = Arr.rows(list, 2, { B: true });
    assert.deepEqual(folded.map(x => x.header ?? x.items.map(it => it.e.name).join("")), ["A", "ab", "c", "B", "C", "fg"]);
    assert.equal(folded[3].folded, true);
    // a b / c / d / f g
    assert.equal(Arr.step(r, 1, "down"), 2, "b down: c (nearest column)");
    assert.equal(Arr.step(r, 2, "down"), 3);
    assert.equal(Arr.step(r, 3, "down"), 4);
    assert.equal(Arr.step(r, 5, "up"), 3, "g up: d");
    assert.equal(Arr.step(r, 2, "right"), 3, "on to the next group");
    assert.equal(Arr.step(r, 0, "left"), -1);
    assert.equal(Arr.step(r, 5, "down"), -1);
    assert.equal(Arr.step(folded, 2, "down"), 4, "over a folded group");
});
test("files: moving tabs", () => {
    assert.deepEqual(Tabs.move(["a", "b", "c", "d"], 0, 2), ["b", "c", "a", "d"]);
    assert.deepEqual(Tabs.move(["a", "b", "c", "d"], 3, 0), ["d", "a", "b", "c"]);
    assert.deepEqual(Tabs.move(["a", "b"], 0, 9), ["b", "a"], "past the end: last");
    const list = ["a", "b"];
    Tabs.move(list, 0, 1);
    assert.deepEqual(list, ["a", "b"], "the original stays as it was");
});
test("files: where a dragged tab lands", () => {
    const geo = [[0, 100], [104, 100], [208, 100]];       // left, width
    assert.equal(Tabs.slot(geo, 0, 140), 0, "not yet past the neighbour's middle");
    assert.equal(Tabs.slot(geo, 0, 160), 1);
    assert.equal(Tabs.slot(geo, 0, 290), 2, "past two middles");
    assert.equal(Tabs.slot(geo, 2, 120), 1);
    assert.equal(Tabs.slot(geo, 2, 10), 0);
    assert.equal(Tabs.slot(geo, 1, 150), 1, "over itself: stays");
});
test("files: where a tab from another window lands, and what it carries", () => {
    const geo = [[0, 100], [104, 100], [208, 100]];
    assert.equal(Tabs.insertAt(geo, 10), 0);
    assert.equal(Tabs.insertAt(geo, 60), 1, "past the first one's middle");
    assert.equal(Tabs.insertAt(geo, 290), 3, "past the last: at the end");
    assert.equal(Tabs.insertAt([], 50), 0);
    const d = Tabs.unpack(Tabs.pack("abc", 2, 1, { path: "/x", second: { path: "/y" } }, "abc2x1"));
    assert.deepEqual(d, { token: "abc", win: 2, index: 1, tab: { path: "/x", second: { path: "/y" } }, id: "abc2x1" });
    assert.equal(Tabs.unpack("not json"), null);
    assert.equal(Tabs.unpack('{"tab": 5}'), null, "no tab in it");
});
test("files: the tab shown after a close", () => {
    assert.equal(Tabs.afterClose(1, 0, 0), -1, "the last one: none");
    assert.equal(Tabs.afterClose(4, 1, 1), 1, "the shown one: its right neighbour takes its place");
    assert.equal(Tabs.afterClose(4, 3, 3), 2, "the shown last one: the one before");
    assert.equal(Tabs.afterClose(4, 0, 2), 1, "one before the shown: shown shifts left");
    assert.equal(Tabs.afterClose(4, 3, 1), 1, "one after: shown stays");
});

// ---- Zones (Settings > Snapping) ------------------------------------
test("zone grids are checked as the compositor does", () => {
    assert.ok(Z.valid(Z.fallback()));
    assert.ok(Z.valid([[0, 0, 0.4, 1], [0.6, 0, 0.4, 1]]), "gaps are fine");
    assert.ok(!Z.valid([]));
    assert.ok(!Z.valid([[0, 0, 0.6, 1], [0.5, 0, 0.5, 1]]), "overlap");
    assert.ok(!Z.valid([[0.6, 0, 0.5, 1]]), "outside");
    assert.ok(!Z.valid([[0, 0, 0.05, 1]]), "too small");
    assert.ok(!Z.valid([[0, 0, 1]]), "not four numbers");
    assert.ok(!Z.valid(Array.from({ length: 17 }, () => [0, 0, 0.1, 0.1])), "too many");
});
test("splitting a zone", () => {
    const halves = Z.split([[0, 0, 1, 1]], 0, "side");
    assert.deepEqual(halves, [[0, 0, 0.5, 1], [0.5, 0, 0.5, 1]]);
    assert.deepEqual(Z.split(halves, 1, "stacked"), [[0, 0, 0.5, 1], [0.5, 0, 0.5, 0.5], [0.5, 0.5, 0.5, 0.5]]);
    // A third split in two: the halves still meet (well within the
    // compositor's 1e-6).
    const t = Z.split(Z.fallback(), 1, "side");
    assert.ok(Math.abs(t[1][0] + t[1][2] - t[2][0]) < 1e-9);
    assert.ok(Z.valid(t));
    assert.equal(Z.split([[0, 0, 0.15, 1]], 0, "side"), null, "halves too small");
    assert.equal(Z.split([[0, 0, 1, 1]], 3, "side"), null, "no such zone");
});
test("merging two zones that make a rectangle", () => {
    const three = [[0, 0, 0.5, 1], [0.5, 0, 0.5, 0.6], [0.5, 0.6, 0.5, 0.4]];
    assert.ok(Z.canMerge(three, 1, 2));
    assert.ok(!Z.canMerge(three, 0, 1), "an L shape");
    assert.deepEqual(Z.merge(three, 2, 1), [[0, 0, 0.5, 1], [0.5, 0, 0.5, 1]]);
    assert.deepEqual(Z.merge([[0, 0, 0.5, 1], [0.5, 0, 0.5, 1]], 0, 1), [[0, 0, 1, 1]]);
    assert.equal(Z.merge(three, 0, 1), null);
});
test("the lines between zones, once each", () => {
    const quarters = [[0, 0, 0.5, 0.5], [0.5, 0, 0.5, 0.5], [0, 0.5, 0.5, 0.5], [0.5, 0.5, 0.5, 0.5]];
    const ls = Z.lines(quarters);
    assert.equal(ls.length, 2, "one upright, one across");
    assert.deepEqual(ls.map(l => [l.axis, l.at, l.lo, l.hi]), [["x", 0.5, 0, 1], ["y", 0.5, 0, 1]]);
    assert.deepEqual(Z.lines([[0, 0, 1, 1]]), []);
});
test("moving a line moves every zone on it", () => {
    const quarters = [[0, 0, 0.5, 0.5], [0.5, 0, 0.5, 0.5], [0, 0.5, 0.5, 0.5], [0.5, 0.5, 0.5, 0.5]];
    const moved = Z.moveLine(quarters, 0, "right", 0.3);
    assert.deepEqual(moved.map(z => [z[0], z[2]]), [[0, 0.3], [0.3, 0.7], [0, 0.3], [0.3, 0.7]]);
    // Never under a tenth.
    assert.deepEqual(Z.moveLine([[0, 0, 0.5, 1], [0.5, 0, 0.5, 1]], 0, "right", 0.99), [[0, 0, 0.9, 1], [0.9, 0, 0.1, 1]]);
    // Only zones lined up along the line: the bottom-right one ends before it.
    const three = [[0, 0, 0.5, 1], [0.5, 0, 0.5, 0.6], [0.5, 0.6, 0.5, 0.4]];
    assert.deepEqual(Z.moveLine(three, 1, "down", 0.5), [[0, 0, 0.5, 1], [0.5, 0, 0.5, 0.5], [0.5, 0.5, 0.5, 0.5]]);
    assert.equal(Z.moveLine(three, 0, "down", 0.5), three, "the area's own edge doesn't move");
    assert.ok(Z.valid(Z.moveLine(Z.fallback(), 0, "right", 0.123456)));
});
test("selecting zones", () => {
    const three = [[0, 0, 0.5, 1], [0.5, 0, 0.5, 0.6], [0.5, 0.6, 0.5, 0.4]];
    assert.deepEqual(Z.select(three, [], 1), [1]);
    assert.deepEqual(Z.select(three, [1], 2), [1, 2], "a zone it merges with joins");
    assert.deepEqual(Z.select(three, [1], 0), [0], "one it doesn't is chosen instead");
    assert.deepEqual(Z.select(three, [1, 2], 2), [1]);
    assert.deepEqual(Z.select(three, [1, 2], 0), [0]);
});
test("a monitor's shape below the bar", () => {
    assert.equal(Z.aspect({ mode: { width: 2880, height: 1612 }, scale: 2, transform: "normal" }, 26), 1440 / 780);
    assert.equal(Z.aspect({ mode: { width: 1920, height: 1080 }, scale: 1, transform: "90" }, 26), 1080 / 1894, "turned");
    assert.equal(Z.aspect(null, 26), 16 / 9);
    assert.equal(Z.aspect({ mode: null }, 26), 16 / 9);
});

console.log(`${passed} passed, ${failed} failed`);
process.exit(failed ? 1 : 0);
