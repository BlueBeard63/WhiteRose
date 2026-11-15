// What an empty document says before anything is typed (Editor.qml), free
// of QML so it can be tested (shell/tests/run.mjs). `d`: { scratch, path,
// syntax, language }.
.pragma library

function text(d) {
    if (d.scratch) return "Notes, lists, anything. It saves itself.";
    if (!d.path) return "Start typing. Ctrl+S saves it, and the name you give it sets the language.";
    const name = d.path.substring(d.path.lastIndexOf("/") + 1);
    if (!d.syntax) return name + " is empty.";
    if (d.syntax === "Markdown") return name + " is empty. # starts a heading, - starts a list.";
    return name + " is empty. It's highlighted as " + d.language + ".";
}
