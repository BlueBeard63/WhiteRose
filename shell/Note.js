// The note (the Note module, SUPER+SHIFT+N): one text, its first line the title.
// A line starting "[ ]" is a task to do, "[x]" one done; with any tasks
// the note shows as a checklist with how many are done.
.pragma library

const TASK = /^\s*\[( |x|X)\]\s?(.*)$/;

// { title, lines: [{ text, task, done, at }], tasks, done }: `at` is the
// line's number in the text (the title is line 0).
function parse(text) {
    const all = String(text ?? "").split("\n");
    const lines = [];
    for (let i = 1; i < all.length; i++) {
        const m = TASK.exec(all[i]);
        if (m) lines.push({ text: m[2], task: true, done: m[1] !== " ", at: i });
        else if (all[i].trim() !== "") lines.push({ text: all[i], task: false, done: false, at: i });
    }
    const tasks = lines.filter(l => l.task);
    return { title: all[0].trim(), lines, tasks: tasks.length, done: tasks.filter(l => l.done).length };
}

// `text` with the task on line `at` ticked or unticked.
function toggle(text, at) {
    const all = String(text ?? "").split("\n");
    const m = TASK.exec(all[at] ?? "");
    if (!m) return text;
    all[at] = all[at].replace(/\[( |x|X)\]/, m[1] === " " ? "[x]" : "[ ]");
    return all.join("\n");
}

// `text` without the tasks that are done (Settings › Note).
function withoutDone(text) {
    const all = String(text ?? "").split("\n");
    return all.filter((l, i) => i === 0 || !(TASK.exec(l) && TASK.exec(l)[1] !== " ")).join("\n");
}
