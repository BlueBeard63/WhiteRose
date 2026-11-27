import QtQuick
import WhiteRose.Kit
import WhiteRose.Clocks

Button {
    // Buttons always have their meaningful Lucide icon, including empty-state
    // and dialog actions. Don't let the kit's titlebar alignment override a row.
    anchors.verticalCenter: undefined
    enabled: Clock.connected && !Clock.busy
    activeFocusOnTab: true
    Keys.onReturnPressed: if (enabled) clicked()
    Keys.onSpacePressed: if (enabled) clicked()
}
