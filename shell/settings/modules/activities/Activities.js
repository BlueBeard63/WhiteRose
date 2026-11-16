// Settings › Activities: editing "activities.list" ([{ id, name }],
// "default" first and always there; keys.go checkActivities).
.pragma library

const MAX_NAME = 40;

// The list with activity `id` renamed, or null when `name` won't do
// (empty, too long, or no such activity).
function rename(list, id, name) {
    const n = String(name ?? "").trim();
    if (n === "" || n.length > MAX_NAME || !(list ?? []).some(a => a.id === id)) return null;
    return list.map(a => a.id === id ? { id: a.id, name: n } : { id: a.id, name: a.name });
}

// The list without activity `id` (never the default one).
function remove(list, id) {
    if (id === "default") return list;
    return (list ?? []).filter(a => a.id !== id);
}

// The first "Activity N" not taken.
function newName(list) {
    const names = (list ?? []).map(a => a.name);
    for (let n = 2; ; n++) if (!names.includes("Activity " + n)) return "Activity " + n;
}

// The list with a new activity on the end (`now`: ms, for its id).
function add(list, now) {
    const ids = (list ?? []).map(a => a.id);
    let id = "a" + Number(now).toString(16);
    while (ids.includes(id)) id += "x";
    return (list ?? []).concat([{ id: id, name: newName(list) }]);
}

// "Current · 5 windows", "1 window", "No windows".
function subtitle(windows, current) {
    const n = windows ?? 0;
    const count = n === 0 ? "No windows" : n === 1 ? "1 window" : n + " windows";
    return current ? "Current · " + count.charAt(0).toLowerCase() + count.slice(1) : count;
}
