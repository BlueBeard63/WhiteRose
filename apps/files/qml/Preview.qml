// A file shown as itself: pictures, the start of text and code, video and
// audio with a play button, PDF pages; anything else as its icon. `big`
// (quick look) shows it all: full size, playing, every page. `facts` is
// what it turned out to be ("4032 × 3024", "3:05", "12 pages").
//
// Video, audio and PDF come from MediaPreview.qml and PdfPreview.qml,
// loaded only when needed (and only if Qt's multimedia and PDF modules are
// there: without them it's the icon).
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Window
import WhiteRose.Kit
import WhiteRose.Files

Item {
    id: root
    property var entry: null
    property bool big: false
    readonly property string path: root.entry?.path ?? ""
    readonly property string mime: root.entry?.mime ?? ""
    readonly property string kind: {
        const m = root.mime;
        if (!root.entry) return "";
        if (root.entry.dir) return "folder";
        if (m.startsWith("image/")) return "image";
        if (m.startsWith("video/")) return "video";
        if (m.startsWith("audio/")) return "audio";
        if (m === "application/pdf") return "pdf";
        if (m.startsWith("text/") || ["application/json", "application/x-shellscript", "application/javascript", "application/xml",
             "application/toml", "application/x-yaml", "application/sql", "application/x-desktop", "application/x-php",
             "application/x-perl", "application/x-ruby", "application/x-python", "application/x-trash"].includes(m)) return "text";
        return "other";
    }
    readonly property string facts: root.kind === "image" ? Fs.imageSize(root.path)
        : root.kind === "text" ? (textView.more ? "First part shown" : "")
        : (special.item?.facts ?? "")
    // Shown as its icon (nothing better, or it couldn't be shown).
    readonly property bool iconOnly: root.kind === "folder" || root.kind === "other"
        || (root.kind === "image" && picture.status === Image.Error)
        || (root.kind === "text" && !textView.ok)
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
        source: root.kind === "image" ? "file://" + root.path : ""
        // Decoded at the size it's shown, not the file's (memory).
        sourceSize: Qt.size(Math.max(16, width) * Screen.devicePixelRatio, Math.max(16, height) * Screen.devicePixelRatio)
        fillMode: Image.PreserveAspectFit
        asynchronous: true
        autoTransform: true
        cache: false
    }

    // ---- text and code -------------------------------------------------------------------------
    Flickable {
        id: textView
        readonly property var read: root.kind === "text" ? JSON.parse(Fs.readText(root.path, root.big ? 1048576 : 16384)) : ({ ok: false })
        readonly property bool ok: textView.read.ok === true
        readonly property bool more: textView.read.more === true
        visible: root.kind === "text" && textView.ok
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
        source: large ? "file://" + large : root.entry?.icon ? "file://" + root.entry.icon : ""
        asynchronous: true
    }
}
