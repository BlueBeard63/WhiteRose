// Closes a bug report that didn't come from an official WhiteRose build
// (no build ID, or one the release's SHA256SUMS doesn't list: rebuilt,
// patched, forks), and one made from a version older than the newest
// release on its channel: a pre-release is compared with the newest release
// of any kind, a release with the newest full release (someone on 0.2.1 isn't
// told to move to an alpha). Only issues labelled bug (the bug form) are
// looked at; ideas and blank issues never are. If the release's checksums
// can't be read, the report is left open. Run by .github/workflows/old-version.yml (actions/github-script).

// Orders versions like daemon/internal/update.compare: numbers first, then
// a release beats its pre-releases, pre-release parts compared one by one
// (numbers before words, "alpha.12" > "alpha.9").
function compare(a, b) {
    const split = v => {
        v = v.trim().replace(/^v/, "");
        let pre = [];
        const i = v.search(/[-~]/);
        if (i >= 0) {
            pre = v.slice(i + 1).toLowerCase().split(/[.-]/).filter(p => p !== "");
            v = v.slice(0, i);
        }
        return [v.split(".").map(n => parseInt(n, 10) || 0), pre];
    };
    const [an, ap] = split(a), [bn, bp] = split(b);
    for (let i = 0; i < Math.max(an.length, bn.length); i++) {
        const x = an[i] ?? 0, y = bn[i] ?? 0;
        if (x !== y) return x > y ? 1 : -1;
    }
    if (ap.length === 0 || bp.length === 0) return (ap.length === 0) - (bp.length === 0);
    for (let i = 0; i < Math.max(ap.length, bp.length); i++) {
        if (i >= ap.length) return -1;
        if (i >= bp.length) return 1;
        const x = ap[i], y = bp[i];
        const xn = /^\d+$/.test(x), yn = /^\d+$/.test(y);
        if (xn && yn) { if (+x !== +y) return +x > +y ? 1 : -1; continue; }
        if (xn !== yn) return xn ? -1 : 1;
        if (x !== y) return x > y ? 1 : -1;
    }
    return 0;
}

// The version from the form's "WhiteRose version" answer ("" if none).
function reportedVersion(body) {
    const m = /###\s*WhiteRose version\s*\n+\s*(.+)/i.exec(body ?? "");
    if (!m) return "";
    const v = m[1].trim().replace(/^whiterose\s+/i, "").replace(/^v/, "");
    return /^\d+\.\d+(\.\d+)?([-~][0-9A-Za-z.-]+)?$/.test(v) ? v : "";
}

const isPre = v => /[-~]/.test(v);

// The form's "Build" answer: a 64-character build ID ("" if none).
function reportedBuild(body) {
    const m = /###\s*Build\s*\n+\s*(.+)/i.exec(body ?? "");
    const id = m ? m[1].trim().toLowerCase() : "";
    return /^[0-9a-f]{64}$/.test(id) ? id : "";
}

// Whether SHA256SUMS lists `id` as a build of `version` (deb or rpm).
function listsBuild(sums, version, id) {
    return (sums ?? "").split("\n").some(l => {
        const [h, name] = l.trim().split(/\s+/);
        return h?.toLowerCase() === id && (name === `build-${version}-deb` || name === `build-${version}-rpm`);
    });
}

const notes = {
    noBuild: () =>
        "This report doesn't have a build ID from WhiteRose, so it's been closed automatically.\n\n" +
        "Report problems from WhiteRose itself (SUPER+SHIFT+B, or Settings › About › Report a problem): it fills the build in. " +
        "If you use a modified build or a fork, report it to whoever made it.",
    otherBuild: version =>
        `This report came from a build of WhiteRose ${version} that doesn't match the release, so it's been closed automatically.\n\n` +
        "Modified builds and forks are looked after by whoever made them: report it there. " +
        `If you installed WhiteRose from this repository's releases, reinstall or update it and report again.`,
    old: (version, newest) =>
        `This was reported from WhiteRose ${version}, and the newest release is ${newest}, so it's been closed automatically.\n\n` +
        `Update in Settings › Updates and check whether it still happens. If it does, open a new report from ${newest}.`,
};

// `fetchText(url)`: a release file's text (tests pass their own).
module.exports = async ({ github, context, fetchText }) => {
    fetchText ??= async url => {
        const res = await fetch(url);
        if (!res.ok) throw new Error(`${url}: ${res.status}`);
        return res.text();
    };
    const issue = context.payload.issue;
    if (!issue.labels.some(l => l.name === "bug")) return;
    const { owner, repo } = context.repo;
    const close = async (text, label) => {
        await github.rest.issues.createComment({ owner, repo, issue_number: issue.number, body: text });
        await github.rest.issues.addLabels({ owner, repo, issue_number: issue.number, labels: [label] });
        await github.rest.issues.update({ owner, repo, issue_number: issue.number, state: "closed", state_reason: "not_planned" });
    };

    // From WhiteRose's report flow, an official build of a release: else closed.
    const version = reportedVersion(issue.body);
    const build = reportedBuild(issue.body);
    if (!version || !build) return close(notes.noBuild(), "unverified build");
    const all = (await github.rest.repos.listReleases({ owner, repo, per_page: 100 })).data.filter(r => !r.draft);
    const release = all.find(r => r.tag_name.replace(/^v/, "") === version);
    const sumsAsset = release?.assets?.find(a => a.name === "SHA256SUMS");
    let sums = "";
    if (sumsAsset) {
        try { sums = await fetchText(sumsAsset.browser_download_url); } catch (e) {
            // GitHub hiccup: leave it open rather than close a real report.
            console.log(`couldn't read SHA256SUMS: ${e.message}`);
            return;
        }
    }
    if (!listsBuild(sums, version, build)) return close(notes.otherBuild(version), "unverified build");

    // An official build, but an old one.
    const newest = all
        .filter(r => isPre(version) || !r.prerelease)
        .map(r => r.tag_name.replace(/^v/, ""))
        .reduce((a, b) => (a === "" || compare(b, a) > 0 ? b : a), "");
    if (newest !== "" && compare(version, newest) < 0) return close(notes.old(version, newest), "old version");
};

module.exports.compare = compare;
module.exports.reportedVersion = reportedVersion;
module.exports.reportedBuild = reportedBuild;
module.exports.listsBuild = listsBuild;
