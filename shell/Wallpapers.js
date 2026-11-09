// Wallpapers: built-in gradients (no files, nothing to load) and images from
// your folders. "wallpaper.current" is "builtin:<name>" or an image path.
.pragma library

// Diagonal gradients, top-left to bottom-right (Figma: Wallpaper 1-4).
const BUILTIN = [
    { id: "builtin:rose", name: "Rose", from: "#E3A0B0", via: "#8E6A9E", to: "#2B2453" },
    { id: "builtin:dusk", name: "Dusk", from: "#8FA6E0", via: "#3B4566", to: "#131318" },
    { id: "builtin:sand", name: "Sand", from: "#E7C49A", via: "#B07F59", to: "#5A3826" },
    { id: "builtin:mint", name: "Mint", from: "#93D9BD", via: "#3F6F5D", to: "#13241E" },
];

function builtin(id) { return BUILTIN.find(b => b.id === id) ?? null }
function isBuiltin(id) { return String(id).startsWith("builtin:") }

// "~/Pictures" -> "/home/me/Pictures".
function expand(path, home) { return String(path).startsWith("~/") ? home + String(path).slice(1) : String(path) }
function tildify(path, home) { return home && String(path).startsWith(home + "/") ? "~" + String(path).slice(home.length) : String(path) }

const IMAGE = /\.(png|jpe?g|webp|avif|bmp)$/i;

// The images `find` printed (one per line), sorted, duplicates dropped.
function parseList(text) {
    return [...new Set(String(text).split("\n").map(s => s.trim()).filter(s => s && IMAGE.test(s)))].sort();
}

// Everything offered: the gradients, then your images and added files.
function all(images, files) {
    return BUILTIN.map(b => b.id).concat([...new Set((images ?? []).concat(files ?? []))]);
}

// The next wallpaper when rotating: through your images if you have any,
// otherwise through the gradients.
function next(images, files, current) {
    const own = [...new Set((images ?? []).concat(files ?? []))];
    const list = own.length > 0 ? own : BUILTIN.map(b => b.id);
    const i = list.indexOf(current);
    return list[(i + 1) % list.length];
}

const ROTATIONS = [
    { label: "Never", value: 0 }, { label: "Every 15 min", value: 15 }, { label: "Every 30 min", value: 30 },
    { label: "Every hour", value: 60 }, { label: "Every day", value: 1440 },
];
function rotationText(minutes) {
    return minutes === 0 ? "" : minutes === 1440 ? "changes daily" : minutes === 60 ? "changes every hour" : "changes every " + minutes + " min";
}
