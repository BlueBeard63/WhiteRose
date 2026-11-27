import QtQuick
import WhiteRose.Kit
import WhiteRose.Tasks

Button {
    anchors.verticalCenter: undefined
    activeFocusOnTab: true
    Keys.onReturnPressed: if (enabled) clicked()
    Keys.onSpacePressed: if (enabled) clicked()
}
