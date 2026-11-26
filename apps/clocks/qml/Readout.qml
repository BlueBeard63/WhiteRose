import QtQuick
import WhiteRose.Kit

Text {
    property real textSize: 52
    font { family: doto.name; pointSize: Theme.pt(textSize); weight: Font.Bold }
    FontLoader { id: doto; source: Qt.resolvedUrl("../fonts/Doto.ttf") }
    textFormat: Text.PlainText
    color: Theme.text
    verticalAlignment: Text.AlignVCenter
    fontSizeMode: Text.HorizontalFit
    minimumPointSize: Theme.pt(20)
    Accessible.role: Accessible.StaticText
    Accessible.description: text
}
