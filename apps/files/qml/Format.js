// Sizes, dates and kinds the way the file manager shows them.
.pragma library

function size(bytes) {
    if (bytes < 1000) return bytes + " B";
    const units = ["kB", "MB", "GB", "TB"];
    let v = bytes, i = -1;
    do { v /= 1000; i++; } while (v >= 1000 && i < units.length - 1);
    return (v < 10 ? v.toFixed(1) : Math.round(v)) + " " + units[i];
}

function date(ms, nowMs) {
    if (!ms) return "";
    const d = new Date(ms), now = new Date(nowMs);
    const day = x => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
    const days = Math.round((day(now) - day(d)) / 86400000);
    const hm = String(d.getHours()).padStart(2, "0") + ":" + String(d.getMinutes()).padStart(2, "0");
    if (days === 0) return "Today " + hm;
    if (days === 1) return "Yesterday " + hm;
    const m = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"][d.getMonth()];
    return d.getDate() + " " + m + (d.getFullYear() !== now.getFullYear() ? " " + d.getFullYear() : "");
}

const KINDS = {
    "inode/directory": "Folder", "application/pdf": "PDF", "text/plain": "Text", "text/markdown": "Markdown",
    "application/zip": "Archive", "application/x-compressed-tar": "Archive", "application/x-7z-compressed": "Archive",
    "application/octet-stream": "File", "application/json": "JSON", "application/x-shellscript": "Script",
};
function kind(mime) {
    if (KINDS[mime]) return KINDS[mime];
    const [media, sub] = String(mime).split("/");
    if (media === "image") return "Image";
    if (media === "audio") return "Audio";
    if (media === "video") return "Video";
    if (media === "text") return "Text";
    if (/document|word|opendocument.text/.test(sub ?? "")) return "Document";
    if (/sheet|excel/.test(sub ?? "")) return "Spreadsheet";
    if (/presentation|powerpoint/.test(sub ?? "")) return "Presentation";
    return "File";
}

// The selection after a click on index `i` (plain, Ctrl to toggle, Shift
// for a range from `anchor`), as { selected: {path: true}, anchor }.
function click(entries, selected, anchor, i, ctrl, shift) {
    const path = entries[i]?.path;
    if (path === undefined) return { selected: {}, anchor: -1 };
    if (shift && anchor >= 0) {
        const out = ctrl ? Object.assign({}, selected) : {};
        for (let k = Math.min(anchor, i); k <= Math.max(anchor, i); k++) out[entries[k].path] = true;
        return { selected: out, anchor: anchor };
    }
    if (ctrl) {
        const out = Object.assign({}, selected);
        if (out[path]) delete out[path]; else out[path] = true;
        return { selected: out, anchor: i };
    }
    const out = {};
    out[path] = true;
    return { selected: out, anchor: i };
}

// Time left at this rate: "5 s left", "3 min left", "1 h 20 min left".
function eta(bytesLeft, rate) {
    if (!(rate > 0) || bytesLeft <= 0) return "";
    const s = Math.ceil(bytesLeft / rate);
    if (s < 60) return s + " s left";
    const m = Math.round(s / 60);
    if (m < 60) return m + " min left";
    return Math.floor(m / 60) + " h " + (m % 60) + " min left";
}

// The stem of a file name: what renaming selects ("notes" of "notes.txt";
// all of a folder's or a dotfile's name).
function stemLength(name, dir) {
    if (dir) return name.length;
    const tar = name.indexOf(".tar.");
    if (tar > 0) return tar;
    const dot = name.lastIndexOf(".");
    return dot > 0 ? dot : name.length;
}

// "9 Nov 2026, 20:43" (Properties).
function fullDate(ms) {
    if (!ms) return "";
    const d = new Date(ms);
    const m = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"][d.getMonth()];
    return d.getDate() + " " + m + " " + d.getFullYear() + ", "
        + String(d.getHours()).padStart(2, "0") + ":" + String(d.getMinutes()).padStart(2, "0");
}

// "6.6 kB (6,600 bytes)"; small sizes just in bytes.
function exactSize(bytes) {
    if (bytes < 1000) return bytes + (bytes === 1 ? " byte" : " bytes");
    // (Qt's JS has no locale grouping; done by hand.)
    return size(bytes) + " (" + String(bytes).replace(/\B(?=(\d{3})+(?!\d))/g, ",") + " bytes)";
}

// Permission bits: `who` 0 owner, 1 group, 2 others; `what` 0 read, 1 write, 2 run.
function hasBit(mode, who, what) { return (mode & (0o400 >> (who * 3 + what))) !== 0 }
function withBit(mode, who, what, on) {
    const bit = 0o400 >> (who * 3 + what);
    return on ? (mode | bit) : (mode & ~bit);
}


// Which checksum a pasted one matches ("SHA-256", "SHA-1", "MD5"), or "":
// any case, spaces and a "sha256:"-style prefix ignored.
function sumMatch(sums, pasted) {
    const parts = String(pasted ?? "").trim().split(":");
    const p = parts[parts.length - 1].replace(/\s+/g, "").toLowerCase();
    if (p === "" || !sums) return "";
    if (p === sums.sha256) return "SHA-256";
    if (p === sums.sha1) return "SHA-1";
    if (p === sums.md5) return "MD5";
    return "";
}

// Paths as text, one per line: "plain", "quoted" (for a shell), "uri"
// (file://, percent-encoded) or "name".
function pathsAs(paths, how) {
    const one = p => {
        switch (how) {
        case "quoted": return "'" + p.replace(/'/g, "'\\''") + "'";
        case "uri": return "file://" + p.split("/").map(encodeURIComponent).join("/");
        case "name": return p.slice(p.lastIndexOf("/") + 1);
        default: return p;
        }
    };
    return paths.map(one).join(how === "quoted" ? " " : "\n");
}

// A name for a folder made from a selection: the shared start of the names
// (at least 3 letters, trimmed of separators), else "New folder".
function groupName(names) {
    if (names.length === 0) return "New folder";
    const stems = names.map(n => n.replace(/\.[^./]+$/, ""));
    let p = stems[0];
    for (const s of stems) while (!s.startsWith(p)) p = p.slice(0, -1);
    // Ending partway through a word or number: back to the whole word.
    const word = /[\p{L}\p{N}]/u;
    if (p !== "" && word.test(p[p.length - 1]) && stems.some(s => s.length > p.length && word.test(s[p.length])))
        p = p.replace(/[\p{L}\p{N}]+$/u, "");
    p = p.replace(/[\s._\-()[\]]+$/, "");
    return p.length >= 3 ? p : "New folder";
}
