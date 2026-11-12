// One scratchpad (Figma: "Settings - Scratchpads"): its header says its
// key, app, place and size, with delete and open; open, it's edited in
// place. Each change is sent up as `patched({ key: value })` (null removes
// the key); a new key as `keyed(combo)`, since taking it may change other
// shortcuts too.
import QtQuick
import qs
import qs.settings
import "Pads.js" as P
import "../rules/Rules.js" as R
import "../keyboard/Keys.js" as K

Card {
    id: card
    required property var pad
    required property int index
    property bool open: false
    signal patched(var patch)
    signal keyed(string combo)
    signal removed()
    signal openToggled()

    readonly property string n: P.name(card.pad)

    // A field shows what's saved again when the pad changes under it.
    onPadChanged: {
        nameField.text = pad.name ?? "";
        commandField.text = pad.command ?? "";
        sizeRow.reset();
    }

    SettingRow {
        title: card.n
        subtitle: P.summary(card.pad, K.spoken)
        IconButton {
            anchors.verticalCenter: parent.verticalCenter
            width: 30; height: 30; radius: 8; icon: "trash"; iconSize: 15; iconColor: Theme.muted
            fill: Theme.surface2; hoverFill: Theme.grip
            label: "Delete scratchpad " + card.n
            onClicked: card.removed()
        }
        IconButton {
            anchors.verticalCenter: parent.verticalCenter
            width: 30; height: 30; radius: 8; icon: card.open ? "chevUp" : "chevDown"; iconSize: 15
            fill: "transparent"
            label: (card.open ? "Close scratchpad " : "Edit scratchpad ") + card.n
            onClicked: card.openToggled()
        }
    }

    SettingRow {
        visible: card.open
        title: "Name"
        subtitle: "What it's called here and in Keyboard & Shortcuts"
        Field {
            id: nameField
            placeholder: P.name(card.pad)
            text: card.pad.name ?? ""
            onEditingDone: if (text.trim() !== (card.pad.name ?? "")) card.patched({ name: text.trim() || null })
        }
    }
    SettingRow {
        id: commandRow
        visible: card.open
        title: "Command"
        subtitle: String(card.pad.command ?? "").trim() ? "Run by its key the first time; the window it opens is the scratchpad"
                                                        : "What to run: it's saved once it has one"
        subtitleColor: String(card.pad.command ?? "").trim() ? Theme.muted : Theme.rose
        Field {
            id: commandField
            implicitWidth: 300
            mono: true
            placeholder: "An app's command"
            text: card.pad.command ?? ""
            onEditingDone: {
                // Apps that hand new windows to a running copy are asked
                // for their own, so the key gets the window it started.
                const next = P.ownInstance(text);
                text = next;
                if (next !== (card.pad.command ?? "")) card.patched({ command: next });
            }
        }
    }
    SettingRow {
        visible: card.open
        title: "Key"
        subtitle: keyRec.problem ? keyRec.problem
            : keyRec.clash ? K.spoken(keyRec.captured) + " is used by “" + keyRec.clash.name + "” · enter to take it, esc to cancel"
            : keyRec.recording ? "Press the keys; esc to cancel"
            : (card.pad.keys ?? []).length > 0 ? "Click to record a new one" : "No key: it can't be shown until it has one"
        subtitleColor: keyRec.problem || keyRec.clash || ((card.pad.keys ?? []).length === 0 && !keyRec.recording) ? Theme.rose : Theme.muted
        KeyRecorder {
            id: keyRec
            keys: card.pad.keys ?? []
            shortcutId: K.padShortcut(card.pad.id)
            label: "Key for " + card.n
            onRecorded: c => card.keyed(c)
        }
    }
    SettingRow {
        id: sizeRow
        visible: card.open
        property bool bad: false
        function reset() {
            bad = false;
            sizeW.text = R.lengthText(P.size(card.pad)[0]);
            sizeH.text = R.lengthText(P.size(card.pad)[1]);
        }
        function commit() {
            const w = R.parseLength(sizeW.text, 1), h = R.parseLength(sizeH.text, 1);
            bad = w === null || h === null;
            const old = P.size(card.pad);
            if (!bad && (w !== old[0] || h !== old[1])) card.patched({ size: [w, h] });
        }
        title: "Size"
        subtitle: bad ? "Whole pixels (at least 1), or a % up to 100%" : "Pixels, or a % of the screen"
        subtitleColor: bad ? Theme.rose : Theme.muted
        Field {
            id: sizeW
            implicitWidth: 72
            mono: true
            label: "Width"
            error: sizeRow.bad
            text: R.lengthText(P.size(card.pad)[0])
            onEditingDone: sizeRow.commit()
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: "×"
            color: Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(12.5) }
        }
        Field {
            id: sizeH
            implicitWidth: 72
            mono: true
            label: "Height"
            error: sizeRow.bad
            text: R.lengthText(P.size(card.pad)[1])
            onEditingDone: sizeRow.commit()
        }
    }
    SettingRow {
        visible: card.open
        title: "Place"
        subtitle: "Under the island, or in the middle of the screen"
        Segmented {
            value: P.place(card.pad)
            options: [{ label: "Under the island", value: "top" }, { label: "Centred", value: "centre" }]
            onPicked: v => card.patched({ place: v === "top" ? null : v })
        }
    }
    SettingRow {
        visible: card.open
        title: "Hide when it loses focus"
        subtitle: "Otherwise it stays above your windows until you press its key again"
        Toggle {
            checked: card.pad.hideOnBlur === true
            onToggled: on => card.patched({ hideOnBlur: on || null })
        }
    }
}
