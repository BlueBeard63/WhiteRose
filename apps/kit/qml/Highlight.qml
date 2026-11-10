// Syntax colours from KDE's syntax-highlighting library (about 300
// formats), when it's installed (the package depends on it). Without it,
// text is plain; nothing breaks. `syntax` names the format ("Rust"), or
// `fileName` picks it as the library would for that name ("main.rs").
import QtQuick

Item {
    id: root
    property var textEdit: null
    property string syntax: ""
    property string fileName: ""
    property bool dark: true
    property var highlighter: null
    visible: false

    function rebuild() {
        if (highlighter) { highlighter.destroy(); root.highlighter = null }
        if (!textEdit || (syntax === "" && fileName === "")) return;
        const theme = dark ? "Repository.DarkTheme" : "Repository.LightTheme";
        const definition = syntax !== "" ? JSON.stringify(syntax) : "Repository.definitionForFileName(" + JSON.stringify(fileName) + ")";
        try {
            root.highlighter = Qt.createQmlObject(`import org.kde.syntaxhighlighting
                SyntaxHighlighter { theme: Repository.defaultTheme(${theme}); definition: ${definition} }`, root, "highlighter");
            highlighter.textEdit = textEdit;
        } catch (e) {
            root.highlighter = null; // the library isn't installed: plain text
        }
    }
    onSyntaxChanged: rebuild()
    onFileNameChanged: rebuild()
    onDarkChanged: rebuild()
    onTextEditChanged: rebuild()
}
