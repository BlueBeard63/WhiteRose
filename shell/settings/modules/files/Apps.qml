// Files › Open with: the terminal and code editor "Open in…" uses (only
// those installed are offered; Automatic takes the first of them that is).
import QtQuick
import Quickshell
import qs
import qs.settings

Column {
    id: page
    spacing: 16

    PageHeader { title: "Open with"; subtitle: "Files › the terminal and code editor for “Open in…”." }

    // ---- which terminal and code editor "Open in…" uses (Files) ------------------------
    // Only those installed (by their launcher entries); Automatic picks the
    // first of these that is.
    readonly property var terminals: [
        { value: "ghostty", label: "Ghostty", desktop: "com.mitchellh.ghostty" },
        { value: "ptyxis", label: "Ptyxis", desktop: "org.gnome.Ptyxis" },
        { value: "kgx", label: "Console", desktop: "org.gnome.Console" },
        { value: "gnome-terminal", label: "GNOME Terminal", desktop: "org.gnome.Terminal" },
        { value: "konsole", label: "Konsole", desktop: "org.kde.konsole" },
        { value: "kitty", label: "kitty", desktop: "kitty" },
        { value: "alacritty", label: "Alacritty", desktop: "Alacritty" },
        { value: "foot", label: "foot", desktop: "foot" },
        { value: "wezterm", label: "WezTerm", desktop: "org.wezfurlong.wezterm" },
        { value: "xterm", label: "XTerm", desktop: "xterm" },
    ]
    readonly property var editors: [
        { value: "code", label: "VS Code", desktop: "code" },
        { value: "codium", label: "VSCodium", desktop: "codium" },
        { value: "zed", label: "Zed", desktop: "dev.zed.Zed" },
        { value: "subl", label: "Sublime Text", desktop: "sublime_text" },
        { value: "gnome-text-editor", label: "Text Editor", desktop: "org.gnome.TextEditor" },
        { value: "kate", label: "Kate", desktop: "org.kde.kate" },
        { value: "whiterose-notepad", label: "Notepad", desktop: "whiterose-notepad" },
    ]
    function installedOf(list) {
        DesktopEntries.applications.values;
        return [{ value: "", label: "Automatic" }].concat(list.filter(a => DesktopEntries.byId(a.desktop) !== null));
    }
    Card {
        SettingRow {
            title: "Terminal"
            subtitle: "Open in terminal (F4), from a folder or its right-click menu"
            Choice {
                value: Daemon.setting("apps.terminal", "")
                options: page.installedOf(page.terminals)
                onPicked: v => Daemon.set("apps.terminal", v)
            }
        }
        SettingRow {
            title: "Code editor"
            subtitle: "Open in … for folders and text files, and a folder's git repository"
            Choice {
                value: Daemon.setting("apps.editor", "")
                options: page.installedOf(page.editors)
                onPicked: v => Daemon.set("apps.editor", v)
            }
        }
    }

}
