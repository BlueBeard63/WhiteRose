// Tests for the shell's pure JS (Calc.js, Search.js). Run: node shell/tests/run.mjs
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

console.log(`${passed} passed, ${failed} failed`);
process.exit(failed ? 1 : 0);
