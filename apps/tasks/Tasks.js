// Pure presentation logic, shared by QML and the JS tests.
function size(n) {
    if (n === null || n === undefined || !Number.isFinite(n)) return "Unavailable";
    const units = ["B", "KiB", "MiB", "GiB", "TiB"];
    let i = 0;
    while (n >= 1024 && i < units.length - 1) { n /= 1024; i++; }
    return (i ? n.toFixed(n < 10 ? 1 : 0) : Math.round(n)) + " " + units[i];
}
function rate(n) { return n === null || n === undefined ? "Unavailable" : size(n) + "/s"; }
function percent(n) { return n === null || n === undefined ? "Unavailable" : n.toFixed(1) + "%"; }
function name(id) {
    return (id || "").replace(/^org\.|^com\.|^io\./, "").split(".").pop().replace(/^whiterose-/, "") || "Unknown app";
}
function rows(apps, query, sort, onlyApps) {
    query = query.toLowerCase().trim();
    return apps.filter(a => (!onlyApps || !a.unit.startsWith("pid:")) &&
        (!query || (a.id + " " + a.unit + " " + a.processes.map(p => p.name + " " + p.pid).join(" ")).toLowerCase().includes(query)))
        .slice().sort((a,b) => sort === "name" ? name(a.id).localeCompare(name(b.id)) :
            (b[sort] ?? -1) - (a[sort] ?? -1) || a.unit.localeCompare(b.unit));
}
