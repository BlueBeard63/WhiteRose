// Syntax colours from KDE's syntax-highlighting library (about 300
// formats), when it's installed (the package depends on it). Without it,
// text is plain; nothing breaks.
import QtQuick

Item {
    id: root
    property var textEdit: null
    property string syntax: ""
    property bool dark: true
    property var highlighter: null
    visible: false

    function rebuild() {
        if (highlighter) { highlighter.destroy(); root.highlighter = null }
        if (!textEdit || syntax === "") return;
        const theme = dark ? "Repository.DarkTheme" : "Repository.LightTheme";
        try {
            root.highlighter = Qt.createQmlObject(`import org.kde.syntaxhighlighting
                SyntaxHighlighter { theme: Repository.defaultTheme(${theme}) }`, root, "highlighter");
            highlighter.textEdit = textEdit;
            highlighter.definition = syntax;
        } catch (e) {
            root.highlighter = null; // the library isn't installed: plain text
        }
    }
    onSyntaxChanged: rebuild()
    onDarkChanged: rebuild()
    onTextEditChanged: rebuild()
}
