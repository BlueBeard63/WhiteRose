.pragma library
// Picking a view from what's in a folder, and laying out the disk usage map.

// The view for a folder that has none of its own: mostly pictures or
// videos → gallery; a code project (or mostly code) → list; else `fallback`.
function adaptive(entries, fallback) {
    const files = entries.filter(e => !e.dir);
    if (files.length >= 4) {
        const media = files.filter(e => /^(image|video)\//.test(e.mime)).length;
        if (media / files.length >= 0.6) return "gallery";
    }
    if (entries.some(e => e.dir && e.name === ".git") || entries.some(e => ["Cargo.toml", "package.json", "go.mod", "pyproject.toml", "Makefile", "CMakeLists.txt"].includes(e.name)))
        return "list";
    if (files.length >= 4) {
        const code = files.filter(e => /^text\/x-|javascript|json|x-shellscript|toml|yaml/.test(e.mime)).length;
        if (code / files.length >= 0.6) return "list";
    }
    return fallback;
}

// Squarified treemap: `items` ({value} > 0, largest first is best) laid in
// the rectangle; returns [{item, x, y, w, h}].
function treemap(items, x, y, w, h) {
    const list = items.filter(i => i.value > 0).sort((a, b) => b.value - a.value);
    const out = [];
    let total = list.reduce((a, i) => a + i.value, 0);
    if (total <= 0 || w <= 0 || h <= 0) return out;
    let rest = list.slice();
    while (rest.length > 0) {
        const short = Math.min(w, h);
        const scale = (w * h) / total;
        // Grow the row while the worst aspect ratio improves.
        let row = [], rowSum = 0, worst = Infinity;
        for (const it of rest) {
            const sum = rowSum + it.value;
            const side = (sum * scale) / short;
            const vals = row.concat([it]).map(r => r.value * scale);
            const ratio = Math.max(...vals.map(a => Math.max(side * side / a, a / (side * side))));
            if (row.length > 0 && ratio > worst) break;
            row.push(it); rowSum = sum; worst = ratio;
        }
        const thick = (rowSum * scale) / short;
        let at = 0;
        for (const it of row) {
            const len = (it.value * scale) / thick;
            if (w >= h) out.push({ item: it, x: x, y: y + at, w: thick, h: len });
            else out.push({ item: it, x: x + at, y: y, w: len, h: thick });
            at += len;
        }
        if (w >= h) { x += thick; w -= thick } else { y += thick; h -= thick }
        total -= rowSum;
        rest = rest.slice(row.length);
    }
    return out;
}
