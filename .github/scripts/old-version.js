// Closes a bug report made from a WhiteRose version older than the newest
// release on its channel: a pre-release is compared with the newest release
// of any kind, a release with the newest full release (someone on 0.2.1 isn't
// told to move to an alpha). Reports without a readable version, or from a
// version newer than any release (a build being worked on), are left open.
// Run by .github/workflows/old-version.yml (actions/github-script).

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

module.exports = async ({ github, context }) => {
    const issue = context.payload.issue;
    if (!issue.labels.some(l => l.name === "bug")) return;
    const version = reportedVersion(issue.body);
    if (!version) return;

    const { owner, repo } = context.repo;
    const releases = (await github.rest.repos.listReleases({ owner, repo, per_page: 100 })).data
        .filter(r => !r.draft && (isPre(version) || !r.prerelease))
        .map(r => r.tag_name.replace(/^v/, ""));
    const newest = releases.reduce((a, b) => (a === "" || compare(b, a) > 0 ? b : a), "");
    if (newest === "" || compare(version, newest) >= 0) return;

    await github.rest.issues.createComment({
        owner, repo, issue_number: issue.number,
        body: `This was reported from WhiteRose ${version}, and the newest release is ${newest}, so it's been closed automatically.\n\n` +
            `Update in Settings › Updates and check whether it still happens. If it does, open a new report from ${newest}.`,
    });
    await github.rest.issues.addLabels({ owner, repo, issue_number: issue.number, labels: ["old version"] });
    await github.rest.issues.update({ owner, repo, issue_number: issue.number, state: "closed", state_reason: "not_planned" });
};

module.exports.compare = compare;
module.exports.reportedVersion = reportedVersion;
