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
