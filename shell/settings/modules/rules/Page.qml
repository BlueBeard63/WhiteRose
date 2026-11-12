// Window rules (Figma: "Settings - Window rules"): what happens to a window
// when it opens, matched by its app id and title. Every rule that matches
// applies, in order; a later one wins. Rules are edited in place
// (RuleCard.qml); a new one starts blank or from an open window's app id.
// The compositor applies windows.rules to windows as they open.
import QtQuick
import qs
import qs.settings
import "Rules.js" as R

Column {
    id: page
    spacing: 16

    // (As plain JS: a setting's nested lists come as Qt lists, which
    // Array.isArray doesn't count.)
    readonly property var saved: JSON.parse(JSON.stringify(Daemon.setting("windows.rules", []) ?? []))
    // What's shown: the saved rules plus any not ready to save yet (a new
    // rule with no pattern). Only complete rules are saved.
    property var draft: []
    // The rule open for editing (-1: none).
    property int openIndex: -1

    function same(a, b) { return JSON.stringify(a) === JSON.stringify(b) }
    // Saved from elsewhere (or first read): show that.
    function sync() {
        if (!same(saved.filter(R.complete), draft.filter(R.complete))) draft = saved.slice();
    }
    onSavedChanged: sync()
    Component.onCompleted: draft = saved.slice()

    function edit(next) {
        draft = next;
        const out = next.filter(R.complete);
        if (!same(out, saved)) Daemon.set("windows.rules", out);
    }
    function patch(i, p) {
        let next = draft;
        for (const k in p) next = R.set(next, i, k, p[k]);
        edit(next);
    }
    function add(rule) {
        edit(draft.concat([rule]));
        openIndex = draft.length - 1;
    }
    function monitorName(id) {
        const o = (Compositor.outputs ?? []).find(o => o.id === id);
        if (!o) return id;
        if (o.builtin) return "Built-in display";
        return [o.make, o.model].filter(s => s).join(" ") || o.connector;
    }

    PageHeader { title: "Window rules"; subtitle: "What happens to a window when it opens. Every rule that matches applies; a later one wins." }

    Section { text: "Rules"; visible: page.draft.length > 0 }
    Repeater {
        model: page.draft
        delegate: RuleCard {
            required property var modelData
            rule: modelData
            count: page.draft.length
            open: page.openIndex === index
            monitorName: id => page.monitorName(id)
            onPatched: p => page.patch(index, p)
            onMoveBy: by => {
                page.edit(R.move(page.draft, index, by));
                if (page.openIndex === index) page.openIndex = index + by;
                else if (page.openIndex === index + by) page.openIndex = index;
            }
            onRemoved: {
                page.edit(R.remove(page.draft, index));
                if (page.openIndex === index) page.openIndex = -1;
                else if (page.openIndex > index) page.openIndex--;
            }
            onOpenToggled: page.openIndex = page.openIndex === index ? -1 : index
        }
    }

    Section { text: "Add a rule" }
    Card {
        SettingRow {
            title: "Blank rule"
            subtitle: "Type the app id or title to match"
            Button { text: "Add"; icon: "plus"; label: "Add a blank rule"; onClicked: page.add({}) }
        }
        SettingRow {
            readonly property var offers: R.offers(Compositor.windows, page.draft)
            title: "From an open window"
            subtitle: offers.length > 0 ? "Fills in its app id" : "No open windows with an app id"
        }
        Repeater {
            model: R.offers(Compositor.windows, page.draft)
            delegate: SettingRow {
                required property var modelData
                title: modelData.title || modelData.app
                subtitle: modelData.app + " · workspace " + modelData.workspace + (modelData.has ? " · has a rule" : "")
                Button {
                    kind: "ghost"
                    text: "Add rule"
                    icon: "plus"
                    label: "Add a rule for " + modelData.app
                    onClicked: page.add({ app: R.exactly(modelData.app) })
                }
            }
        }
    }
}
