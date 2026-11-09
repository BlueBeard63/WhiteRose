.pragma library
// Launcher ranking and recent-files parsing. Pure functions, tested by
// shell/tests/run.mjs.

// How well `text` matches `query` (both any case): 100 prefix, 80 word
// prefix, 60 substring, up to 30 for an in-order subsequence; -1 = no match.
function score(query, text) {
    if (!query || !text) return -1;
    var q = query.toLowerCase(), t = text.toLowerCase();
    if (t.indexOf(q) === 0) return 100 - Math.min(20, (t.length - q.length) * 0.2);
    var words = t.split(/[\s\-_.]+/);
    for (var i = 1; i < words.length; i++) if (words[i].indexOf(q) === 0) return 80;
    if (t.indexOf(q) >= 0) return 60;
    // Subsequence ("gte" -> "gnome text editor"), penalised by gaps.
    var gaps = 0, j = 0, last = -1;
    for (var k = 0; k < t.length && j < q.length; k++) {
        if (t[k] === q[j]) { if (last >= 0) gaps += k - last - 1; last = k; j++; }
    }
    if (j === q.length) return Math.max(1, 30 - gaps);
    return -1;
}

// apps: [{ id, name, genericName, keywords: [], comment }]; usage: { id: count }.
// Returns [{ app, score }] best first, at most `limit`.
//
// Only the app's name may match by scattered letters. The generic name must
// contain the query; keywords and the description only count when a word
// starts with it. Otherwise long descriptions match nearly any query (all
// the letters of "term" appear, in order, in "send and receive mail with
// Thunderbird"). Results far weaker than the best are dropped.
function rankApps(query, apps, usage, limit) {
    var out = [];
    for (var i = 0; i < apps.length; i++) {
        var a = apps[i];
        var best = score(query, a.name);
        var g = score(query, a.genericName);
        if (g >= 60) best = Math.max(best, g * 0.8);
        var kw = a.keywords || [];
        for (var k = 0; k < kw.length; k++) {
            var ks = score(query, kw[k]);
            if (ks >= 80) best = Math.max(best, ks * 0.7);
        }
        var c = score(query, a.comment);
        if (c >= 80) best = Math.max(best, c * 0.4);
        if (best <= 0) continue;
        var uses = (usage && usage[a.id]) || 0;
        out.push({ app: a, score: best + Math.log2(1 + uses) * 5 });
    }
    out.sort(function (x, y) { return y.score - x.score || (x.app.name < y.app.name ? -1 : 1); });
    var floor = out.length ? out[0].score * 0.35 : 0;
    return out.filter(function (r) { return r.score >= floor; }).slice(0, limit || 8);
}

// Most-used apps for the empty launcher: pinned first, then by launch count.
function frequent(apps, usage, pinned, limit) {
    var byId = {};
    apps.forEach(function (a) { byId[a.id] = a; });
    var out = [], seen = {};
    (pinned || []).forEach(function (id) { if (byId[id] && !seen[id]) { out.push(byId[id]); seen[id] = true; } });
    var rest = apps.filter(function (a) { return !seen[a.id] && (usage[a.id] || 0) > 0; });
    rest.sort(function (x, y) { return (usage[y.id] || 0) - (usage[x.id] || 0); });
    return out.concat(rest).slice(0, limit || 12);
}

function decodePath(href) {
    try { return decodeURIComponent(href.replace(/^file:\/\//, "")); } catch (e) { return null; }
}

// recently-used.xbel -> [{ path, name, dir, modified }], newest first, local files only.
function parseRecent(xml) {
    if (!xml) return [];
    var re = /<bookmark\s+href="([^"]+)"[^>]*?modified="([^"]+)"/g, m, out = [], seen = {};
    while ((m = re.exec(xml)) !== null) {
        if (m[1].indexOf("file://") !== 0) continue;   // skip davs://, sftp:// ...
        var path = decodePath(m[1].replace(/&amp;/g, "&"));
        if (!path || seen[path]) continue;
        seen[path] = true;
        var slash = path.lastIndexOf("/");
        out.push({ path: path, name: path.slice(slash + 1), dir: path.slice(0, slash), modified: m[2] });
    }
    out.sort(function (a, b) { return a.modified < b.modified ? 1 : -1; });
    return out;
}

function matchFiles(query, files, limit) {
    var out = [];
    for (var i = 0; i < files.length; i++) {
        // File names must contain the query; scattered letters match too much
        // ("term" is in order in "start-here.md").
        var s = score(query, files[i].name);
        if (s >= 60) out.push({ file: files[i], score: s });
    }
    out.sort(function (a, b) { return b.score - a.score; });
    return out.slice(0, limit || 4);
}

// "~/Projects/x" for display.
function tildify(path, home) {
    return home && path.indexOf(home) === 0 ? "~" + path.slice(home.length) : path;
}

// Programs on $PATH whose name starts with the query (or has a part after a
// "-" or "." that does), best first. A match in the middle ("ht" in
// "ethtool") is noise among thousands of programs. `commands` is the list of
// executable names.
function matchCommands(query, commands, limit) {
    var q = query.trim().split(/\s+/)[0];
    if (!q) return [];
    var out = [];
    for (var i = 0; i < commands.length; i++) {
        var s = score(q, commands[i]);
        if (s >= 80) out.push({ cmd: commands[i], score: s - commands[i].length * 0.01 });
    }
    out.sort(function (a, b) { return b.score - a.score || (a.cmd < b.cmd ? -1 : 1); });
    return out.slice(0, limit || 6).map(function (r) { return r.cmd; });
}

// Newest first, no repeats, at most `max`.
function pushHistory(list, entry, max, same) {
    same = same || function (a, b) { return a === b; };
    var out = [entry];
    for (var i = 0; i < (list || []).length && out.length < max; i++)
        if (!same(list[i], entry)) out.push(list[i]);
    return out;
}

// argv that runs `cmd` in `terminal` and leaves a shell open afterwards, so
// the output stays readable. Terminals disagree on how to take a command.
function terminalCommand(terminal, cmd) {
    var parts = (terminal || "ptyxis").trim().split(/\s+/);
    var name = parts[0].split("/").pop();
    var script = ["sh", "-c", cmd + '; exec "${SHELL:-sh}"'];
    var sep = {
        "ptyxis": ["--"], "gnome-terminal": ["--"], "kgx": ["--"], "gnome-console": ["--"],
        "alacritty": ["-e"], "konsole": ["-e"], "xterm": ["-e"], "terminator": ["-x"],
        "wezterm": ["start", "--"], "foot": [], "kitty": [], "ghostty": ["-e"],
    }[name];
    if (sep === undefined) sep = ["-e"];
    if (name === "ptyxis" && parts.indexOf("--standalone") < 0) parts.push("--standalone");
    return parts.concat(sep, script);
}

// Everything the launcher lists, in display order, for the current query and
// category chip. Items: { kind: "app" | "file" | "calc" | "cmd", ... };
// `grid` is how many leading items are shown as the app grid.
//
// s = { query, category, apps, usage, pinned, files, commands, history,
//       calcHistory, calc (the query's result or null) }
function results(s) {
    var q = (s.query || "").trim();
    var cat = s.category || "all";
    var apps = function (list) { return list.map(function (a) { return { kind: "app", app: a }; }); };
    var files = function (list) { return list.map(function (f) { return { kind: "file", file: f }; }); };
    var cmds = function (cmd) {
        return [{ kind: "cmd", cmd: cmd, terminal: false }, { kind: "cmd", cmd: cmd, terminal: true }];
    };

    // ">" at the start means a command, whichever chip is on.
    if (q.charAt(0) === ">") { cat = "commands"; q = q.replace(/^>\s*/, ""); }

    if (q === "") {
        switch (cat) {
        case "apps": {
            var all = s.apps.slice().sort(function (a, b) { return a.name.toLowerCase() < b.name.toLowerCase() ? -1 : 1; });
            return { items: apps(all), grid: all.length };
        }
        case "files": return { items: files(s.files.slice(0, 8)), grid: 0 };
        case "commands":
            return { items: (s.history || []).slice(0, 8).map(function (c) { return { kind: "cmd", cmd: c, terminal: false, recent: true }; }), grid: 0 };
        case "calculator":
            return { items: (s.calcHistory || []).slice(0, 6).map(function (c) { return { kind: "calc", expr: c.expr, result: c.result }; }), grid: 0 };
        }
        var grid = frequent(s.apps, s.usage || {}, s.pinned, 12);
        return { items: apps(grid).concat(files(s.files.slice(0, 3))), grid: grid.length };
    }

    var out = [];
    if (cat === "commands") {
        out = cmds(q);
        var first = q.split(/\s+/)[0];
        // Programs, only while typing the program name (not its arguments).
        if (q.indexOf(" ") < 0)
            matchCommands(q, s.commands || [], 6).forEach(function (c) { if (c !== first) out.push({ kind: "cmd", cmd: c, terminal: false }); });
        (s.history || []).forEach(function (h) {
            var listed = out.some(function (o) { return o.cmd === h; });
            if (!listed && h.toLowerCase().indexOf(q.toLowerCase()) >= 0 && out.length < 10)
                out.push({ kind: "cmd", cmd: h, terminal: false, recent: true });
        });
        return { items: out, grid: 0 };
    }
    if (s.calc !== null && s.calc !== undefined && (cat === "all" || cat === "calculator"))
        out.push({ kind: "calc", expr: q, result: s.calc });
    if (cat === "calculator") return { items: out, grid: 0 };
    if (cat === "all" || cat === "apps")
        rankApps(q, s.apps, s.usage, cat === "apps" ? 10 : 5).forEach(function (r) { out.push({ kind: "app", app: r.app }); });
    if (cat === "all" || cat === "files")
        matchFiles(q, s.files, cat === "files" ? 10 : 3).forEach(function (r) { out.push({ kind: "file", file: r.file }); });
    // In All, a program whose name matches exactly can be run directly.
    if (cat === "all" && (s.commands || []).indexOf(q.split(/\s+/)[0]) >= 0)
        out.push({ kind: "cmd", cmd: q, terminal: false });
    return { items: out, grid: 0 };
}
