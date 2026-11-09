// Tests for the shell's pure JS (Calc.js, Search.js, DragData.js, QuickSettings.js, Notifications.js, CalendarGrid.js, Displays.js, Keys.js). Run: node shell/tests/run.mjs
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
const Search = load("Search.js", ["score", "rankApps", "frequent", "parseRecent", "matchFiles", "tildify", "matchCommands", "pushHistory", "terminalCommand", "results"]);
const DragData = load("DragData.js", ["fileUri", "forFiles", "forEntry"]);
const QS = load("QuickSettings.js", ["uptime", "duration", "batteryLine", "nightLightUntil", "nextProfile", "profileName", "initials", "displayName"]);
const N = load("Notifications.js", ["ago", "shouldPopup", "popupTimeout", "buttons", "hasDefault", "iconKind", "themedImageName", "rememberApp"]);
const D = load("settings/modules/displays/Displays.js", ["modeString", "hzLabel", "resolutions", "refreshes", "withResolution", "logicalSize", "draftFrom", "snap", "resolveOverlap", "connected", "normalize", "place", "profilesFor", "profileName", "toConfig", "differs"]);
const K = load("settings/modules/keyboard/Keys.js", ["isModifier", "keyName", "combo", "normalize", "expand", "conflict", "forRange", "caps", "spoken", "setKeys", "assign", "newId", "parseLayouts", "searchLayouts"]);
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

console.log(`${passed} passed, ${failed} failed`);
process.exit(failed ? 1 : 0);
