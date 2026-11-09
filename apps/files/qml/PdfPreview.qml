// PDF pages (loaded by Preview.qml only when needed: it needs Qt's PDF
// module). The panel shows the first page; quick look scrolls through all.
import QtQuick
import QtQuick.Pdf

Item {
    id: root
    property string path
    property bool big: false
    readonly property string facts: doc.pageCount > 0 ? doc.pageCount + (doc.pageCount === 1 ? " page" : " pages") : ""

    PdfDocument {
        id: doc
        source: root.path ? "file://" + root.path : ""
    }
    Loader {
        anchors.fill: parent
        sourceComponent: root.big ? allPages : firstPage
    }
    Component {
        id: firstPage
        PdfPageImage {
            document: doc
            currentFrame: 0
            fillMode: Image.PreserveAspectFit
            sourceSize.width: width * 2
            asynchronous: true
        }
    }
    Component {
        id: allPages
        PdfMultiPageView { document: doc }
    }
}
