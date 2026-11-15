// "Report this" after a crash, or "Snapshot saved" after SUPER+SHIFT+B
// (ReportPromptView.qml). It stays until answered (a crash), so clicking
// elsewhere doesn't close it.
import QtQuick
import qs

IslandView {
    keyboard: false
    ReportPromptView {}
}
