// A file shown as itself: pictures, the start of text and code (in
// colour), Markdown drawn (tables, code; Mermaid and maths when their tools
// are installed), JSON / JSONC / JSON5 formatted, video and audio with a
// play button, PDF pages; anything else as its icon. `big` (quick look)
// shows it all: full size, playing, every page, and can show the source of
// Markdown and JSON. `facts` is what it turned out to be ("4032 × 3024",
// "3:05", "12 pages").
//
// Video, audio and PDF come from MediaPreview.qml and PdfPreview.qml,
// loaded only when needed (and only if Qt's multimedia and PDF modules are
// there: without them it's the icon).
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Window
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as Fmt

Item {
    id: root
    property var entry: null
    property bool big: false
    // Inside an archive: the file taken out into a cache (only what can be
    // shown, and not too big: it's read out of the archive to show it).
    readonly property bool fromArchive: (root.entry?.path ?? "").startsWith("archive:")
    // On a network drive: shown from a local copy, made in the background
    // (remotecopy.rs); the readers below would otherwise read through the
    // drive on the window's thread (a PDF on a Nextcloud froze Files).
    readonly property bool remote: !!root.entry && !root.fromArchive && !root.entry.dir && Fs.isRemote(root.entry.path)
    property string localCopy: ""
    property bool fetching: false
    property bool tooBig: false
    // (A moment later: in the change itself `remote` and `kind` can still be
    // the last entry's.)
    onEntryChanged: {
        root.localCopy = "";
        root.tooBig = false;
        root.fetching = false;
        const asked = root.entry?.path ?? "";
        Qt.callLater(() => {
            if (!root.entry || root.entry.path !== asked) return;
            root.fetching = root.remote && root.kind !== "other";
            if (root.fetching) Ops.fetchCopy(asked);
        });
    }
    Connections {
        target: Ops
        function onCopyFetched(path, result) {
            if (!root.entry || path !== root.entry.path) return;
            const r = JSON.parse(result);
            root.fetching = false;
            root.localCopy = r.local ?? "";
            root.tooBig = r.error === "big";
        }
    }
    readonly property string path: !root.entry ? ""
        : root.remote ? root.localCopy
        : !root.fromArchive ? root.entry.path
        : root.entry.dir || root.kind === "other" || (root.entry.size ?? 0) > 30e6 ? ""
        : Fs.archiveFile(root.entry.path)
    readonly property string mime: root.entry?.mime ?? ""
    readonly property string kind: {
        const m = root.mime;
        if (!root.entry) return "";
        if (root.entry.dir) return "folder";
        if (m.startsWith("image/")) return "image";
        if (m.startsWith("video/")) return "video";
        if (m.startsWith("audio/")) return "audio";
        if (m === "application/pdf") return "pdf";
        const name = (root.entry.name ?? "").toLowerCase();
        if (m === "text/markdown" || /\.(md|markdown|mdown|mkd)$/.test(name)) return "markdown";
        if (m === "application/json" || /\.(json|jsonc|json5)$/.test(name)) return "json";
        if (m.startsWith("text/") || ["application/json", "application/x-shellscript", "application/javascript", "application/xml",
             "application/toml", "application/x-yaml", "application/sql", "application/x-desktop", "application/x-php",
             "application/x-perl", "application/x-ruby", "application/x-python", "application/x-trash"].includes(m)) return "text";
        return "other";
    }
    readonly property string facts: root.kind === "image" ? Fs.imageSize(root.path)
        : root.kind === "text" ? (textView.more ? "First part shown" : "")
        : root.kind === "json" ? (root.showSource || !jsonView.ok ? "" : "Formatted")
        : root.kind === "markdown" ? (mdView.read.hint ?? "")
        : (special.item?.facts ?? "")
    // Shown as its icon (nothing better, or it couldn't be shown).
    readonly property bool iconOnly: root.fetching || root.tooBig || root.kind === "folder" || root.kind === "other"
        || (root.kind === "image" && picture.status === Image.Error)
        || ((root.kind === "text" || root.kind === "json" || (root.kind === "markdown" && root.showSource)) && !textView.ok)
        || (root.kind === "markdown" && !root.showSource && !mdView.ok)
        || ((root.kind === "video" || root.kind === "audio" || root.kind === "pdf") && special.status !== Loader.Ready)

    // ---- pictures ------------------------------------------------------------------------
    // Its own size ("W × H"): small pictures are shown at most twice it,
    // not stretched to a blur.
    readonly property var natural: {
        const m = /^(\d+) × (\d+)$/.exec(root.kind === "image" ? Fs.imageSize(root.path) : "");
        return m ? Qt.size(+m[1], +m[2]) : Qt.size(0, 0);
    }
    readonly property real fit: root.natural.width > 0
        ? Math.min(2, root.width / root.natural.width, root.height / root.natural.height) : 1
    Image {
        id: picture
        anchors.centerIn: parent
        width: root.natural.width > 0 ? root.natural.width * root.fit : root.width
        height: root.natural.height > 0 ? root.natural.height * root.fit : root.height
        visible: root.kind === "image" && status === Image.Ready
        source: root.kind === "image" ? Fmt.fileUrl(root.path) : ""
        // Decoded at the size it's shown, not the file's (memory).
        sourceSize: Qt.size(Math.max(16, width) * Screen.devicePixelRatio, Math.max(16, height) * Screen.devicePixelRatio)
        fillMode: Image.PreserveAspectFit
        asynchronous: true
        autoTransform: true
        cache: false
    }

    // ---- text and code -------------------------------------------------------------------------
    // Markdown and JSON: their source instead (quick look's "Source").
    property bool showSource: false
    onPathChanged: root.showSource = false
    // JSON formatted (when it parses).
    readonly property var jsonView: root.kind === "json" && !root.showSource ? JSON.parse(Fs.prettyJson(root.path)) : ({ ok: false })
    readonly property bool asText: root.kind === "text" || (root.kind === "json" && (root.showSource || !root.jsonView.ok)) || (root.kind === "markdown" && root.showSource)
    Flickable {
        id: textView
        readonly property var read: root.kind === "json" && !root.showSource && root.jsonView.ok
            ? { ok: true, text: root.big ? root.jsonView.text : root.jsonView.text.slice(0, 16384), more: !root.big && root.jsonView.text.length > 16384 }
            : root.asText ? JSON.parse(Fs.readText(root.path, root.big ? 1048576 : 16384)) : ({ ok: false })
        readonly property bool ok: textView.read.ok === true
        readonly property bool more: textView.read.more === true
        visible: root.asText || (root.kind === "json" && textView.ok)
        anchors.fill: parent
        clip: true
        interactive: root.big
        contentWidth: width
        contentHeight: body.implicitHeight + 24
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar { policy: root.big ? ScrollBar.AsNeeded : ScrollBar.AlwaysOff }
        TextEdit {
            id: body
            x: 12; y: 12
            width: parent.width - 24
            readOnly: true
            selectByMouse: root.big
            wrapMode: TextEdit.WrapAnywhere
            text: (textView.read.text ?? "") + (textView.more ? "\n…" : "")
            color: Theme.text
            selectionColor: Theme.tint(0.4)
            font { family: Theme.mono; pointSize: Theme.pt(root.big ? 11.5 : 9.5) }
        }
        // In colour (formatted JSON as JSON, whatever the file's called).
        Highlight {
            textEdit: body
            fileName: textView.visible ? (root.kind === "json" && !root.showSource ? "formatted.json" : root.entry?.name ?? "") : ""
            dark: !Theme.light
        }
    }
    // Doesn't parse: shown as it is, with where it went wrong.
    Rectangle {
        visible: root.kind === "json" && !root.showSource && !root.jsonView.ok && (root.jsonView.error ?? "") !== ""
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom; margins: 8 }
        height: jsonErr.implicitHeight + 12; radius: 8
        color: Theme.surface2
        Text {
            id: jsonErr
            anchors { fill: parent; margins: 6 }
            wrapMode: Text.Wrap
            text: "Not valid JSON: " + (root.jsonView.error ?? "")
            color: Theme.rose
            font { family: Theme.font; pointSize: Theme.pt(10.5) }
        }
    }

    // ---- Markdown, drawn ------------------------------------------------------------------------
    Flickable {
        id: mdView
        property var read: ({ ok: false })
        readonly property bool ok: mdView.read.ok === true
        function load() {
            Ops.accent = Theme.rose.toString();
            mdView.read = root.kind === "markdown" && !root.showSource
                ? JSON.parse(Ops.markdown(root.path, !Theme.light, Theme.text.toString())) : { ok: false };
        }
        Connections { target: root; function onPathChanged() { mdView.load() } function onShowSourceChanged() { mdView.load() } function onKindChanged() { mdView.load() } }
        // Diagrams or maths finished drawing: shown.
        Connections { target: Ops; function onRendered(path) { if (path === root.path) mdView.load() } }
        Component.onCompleted: load()
        visible: root.kind === "markdown" && !root.showSource && mdView.ok
        anchors.fill: parent
        clip: true
        interactive: root.big
        contentWidth: width
        contentHeight: mdText.implicitHeight + 32
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar { policy: root.big ? ScrollBar.AsNeeded : ScrollBar.AlwaysOff }
        Text {
            id: mdText
            x: root.big ? 24 : 12; y: root.big ? 20 : 12
            width: parent.width - 2 * x
            textFormat: Text.MarkdownText
            text: mdView.read.text ?? ""
            baseUrl: mdView.read.base ?? ""
            wrapMode: Text.Wrap
            color: Theme.text
            linkColor: Theme.rose
            font { family: Theme.font; pointSize: Theme.pt(root.big ? 12.5 : 9.5) }
            onLinkActivated: link => Qt.openUrlExternally(link)
            HoverHandler { cursorShape: mdText.hoveredLink ? Qt.PointingHandCursor : Qt.ArrowCursor }
        }
    }
    // Quick look: the source of Markdown or JSON, and back.
    Rectangle {
        visible: root.big && (root.kind === "markdown" || (root.kind === "json" && root.jsonView.ok !== undefined))
        anchors { top: parent.top; right: parent.right; margins: 10 }
        width: srcText.implicitWidth + 20; height: 26; radius: 13
        color: srcArea.containsMouse ? Theme.surface2 : Theme.surface1
        Text {
            id: srcText
            anchors.centerIn: parent
            text: root.showSource ? (root.kind === "markdown" ? "Drawn" : "Formatted") : "Source"
            color: Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(11) }
        }
        MouseArea { id: srcArea; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: root.showSource = !root.showSource }
    }

    // ---- video, audio, PDF ------------------------------------------------------------------------
    Loader {
        id: special
        anchors.fill: parent
        readonly property string file: (root.kind === "video" || root.kind === "audio") && !Session.noMedia ? "MediaPreview.qml"
            : root.kind === "pdf" && !Session.noPdf ? "PdfPreview.qml" : ""
        // Qt's module isn't there: not tried again this time round.
        onStatusChanged: if (special.status === Loader.Error) {
            if (special.file === "PdfPreview.qml") Session.noPdf = true; else Session.noMedia = true;
        }
        // Loaded again for each file (a new player or document each time).
        onFileChanged: special.load()
        Connections { target: root; function onPathChanged() { special.load() } }
        function load() {
            if (special.file === "") { special.source = ""; return }
            special.setSource(Qt.resolvedUrl(special.file), { path: root.path, big: root.big });
        }
        Component.onCompleted: special.load()
    }

    // ---- anything else: its icon --------------------------------------------------------------
    Image {
        anchors.centerIn: parent
        visible: root.iconOnly && root.entry !== null
        width: Math.min(root.big ? 160 : 96, root.width * 0.6); height: width
        sourceSize: Qt.size(width * 2, height * 2)
        // The theme's large one (the listing's is 48 px: blurry blown up).
        readonly property string large: root.entry ? Fs.typeIcon(root.mime, root.entry.dir, 256) : ""
        source: large ? Fmt.fileUrl(large) : root.entry?.icon ? Fmt.fileUrl(root.entry.icon) : ""
        asynchronous: true
    }

    // A network drive's file being copied to show it, or too big to.
    Text {
        visible: root.fetching || root.tooBig
        anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom; bottomMargin: 16 }
        text: root.fetching ? "Loading preview…" : "Too big to preview from the network"
        color: Theme.muted
        font { family: Theme.font; pointSize: Theme.pt(11) }
    }
}
