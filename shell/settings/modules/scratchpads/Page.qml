// Scratchpads (Figma: "Settings - Scratchpads"): apps shown and hidden
// with a key, sliding down from the island. The key starts the app the
// first time; the window that process opens is the scratchpad. Each is a
// card edited in place (PadCard.qml); a new one starts as the terminal.
// The compositor reads windows.scratchpads (keys included); their keys
// also show on the Keyboard page.
import QtQuick
import qs
import qs.settings
import "Pads.js" as P
import "../keyboard/Keys.js" as K

Column {
    id: page
    spacing: 16

    // (As plain JS: a setting's nested lists come as Qt lists.)
    readonly property var saved: JSON.parse(JSON.stringify(Daemon.setting("windows.scratchpads", []) ?? []))
    // What's shown: the saved pads plus any not ready to save yet (a new
    // one with no command). Only complete pads are saved.
    property var draft: []
    // The pad open for editing (-1: none).
    property int openIndex: -1

    function same(a, b) { return JSON.stringify(a) === JSON.stringify(b) }
    function sync() {
        if (!same(saved.filter(P.complete), draft.filter(P.complete))) draft = saved.slice();
    }
    onSavedChanged: sync()
    Component.onCompleted: draft = saved.slice()

    function edit(next) {
        draft = next;
        const out = next.filter(P.complete);
        if (!same(out, saved)) Daemon.set("windows.scratchpads", out);
    }
    // A new key for pad `i`, taken from whichever shortcut had it.
    function key(i, combo) {
        const overrides = JSON.parse(JSON.stringify(Daemon.setting("shortcuts.keys", ({})) ?? {}));
        const custom = JSON.parse(JSON.stringify(Daemon.setting("shortcuts.custom", []) ?? []));
        const next = K.assign({ shortcuts: Compositor.shortcuts, overrides, custom, pads: page.draft }, K.padShortcut(page.draft[i].id), combo, false);
        if (!same(next.overrides, overrides)) Daemon.set("shortcuts.keys", next.overrides);
        if (!same(next.custom, custom)) Daemon.set("shortcuts.custom", next.custom);
        edit(next.pads);
    }
    function add() {
        const terminal = Daemon.setting("apps.terminal", "") ?? "";
        edit(page.draft.concat([P.make(page.draft, terminal, k => !K.conflict(Compositor.shortcuts, k, ""))]));
        openIndex = page.draft.length - 1;
    }

    PageHeader { title: "Scratchpads"; subtitle: "Apps you show and hide with a key, sliding down from the island." }

    Section { text: "Scratchpads"; visible: page.draft.length > 0 }
    Repeater {
        model: page.draft
        delegate: PadCard {
            required property var modelData
            pad: modelData
            open: page.openIndex === index
            onPatched: p => page.edit(P.set(page.draft, index, p))
            onKeyed: c => page.key(index, c)
            onRemoved: {
                page.edit(page.draft.filter((_, j) => j !== index));
                if (page.openIndex === index) page.openIndex = -1;
                else if (page.openIndex > index) page.openIndex--;
            }
            onOpenToggled: page.openIndex = page.openIndex === index ? -1 : index
        }
    }

    Section { text: "Add a scratchpad" }
    Card {
        SettingRow {
            title: "New scratchpad"
            subtitle: page.draft.length === 0 ? "Starts as your terminal; change the command for another app" : "Type the command of the app it shows"
            Button { text: "Add"; icon: "plus"; label: "Add a scratchpad"; onClicked: page.add() }
        }
    }
}
