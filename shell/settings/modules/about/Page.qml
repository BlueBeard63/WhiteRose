// About: this computer and WhiteRose, read from /proc, /etc/os-release and
// the DMI tables (nothing is sent anywhere).
import QtQuick
import Quickshell
import Quickshell.Io
import qs
import qs.settings

Column {
    id: page
    spacing: 18

    FileView { id: osRelease; path: "/etc/os-release"; blockLoading: true }
    FileView { id: osrelease; path: "/proc/sys/kernel/osrelease"; blockLoading: true }
    FileView { id: hostname; path: "/proc/sys/kernel/hostname"; blockLoading: true }
    FileView { id: cpuinfo; path: "/proc/cpuinfo"; blockLoading: true }
    FileView { id: meminfo; path: "/proc/meminfo"; blockLoading: true }
    FileView { id: vendor; path: "/sys/class/dmi/id/sys_vendor"; blockLoading: true }
    FileView { id: product; path: "/sys/class/dmi/id/product_name"; blockLoading: true }
    FileView { id: uptimeFile; path: "/proc/uptime"; blockLoading: true }

    function field(text, key) {
        const m = new RegExp("^" + key + "\\s*[:=]\\s*\"?([^\"\\n]*)", "m").exec(text ?? "");
        return m ? m[1].trim() : "";
    }
    readonly property string os: field(osRelease.text(), "PRETTY_NAME")
    readonly property string kernel: (osrelease.text() ?? "").trim()
    readonly property string host: (hostname.text() ?? "").trim()
    readonly property string cpu: field(cpuinfo.text(), "model name")
    readonly property int cores: ((cpuinfo.text() ?? "").match(/^processor\s*:/gm) ?? []).length
    readonly property real memKb: parseFloat(field(meminfo.text(), "MemTotal")) || 0
    readonly property string model: [(vendor.text() ?? "").trim(), (product.text() ?? "").trim()].filter(s => s).join(" ")
    readonly property real uptime: parseFloat(uptimeFile.text()) || 0

    PageHeader { title: "About"; subtitle: page.host }

    Card {
        SettingRow { title: "Computer"; divider: false; Text { text: page.model || "—"; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(12.5) } } }
        SettingRow { title: "Processor"; Text { text: page.cpu + (page.cores ? " · " + page.cores + " threads" : ""); color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(12.5) } } }
        SettingRow { title: "Memory"; Text { text: (page.memKb / 1048576).toFixed(1) + " GB"; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(12.5) } } }
    }
    Card {
        SettingRow { title: "Operating system"; divider: false; Text { text: page.os; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(12.5) } } }
        SettingRow { title: "Kernel"; Text { text: page.kernel; color: Theme.muted; font { family: Theme.mono; pointSize: Theme.pt(12) } } }
        SettingRow { title: "Desktop"; Text { text: "WhiteRose (development)"; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(12.5) } } }
        SettingRow {
            title: "Up for"
            Text {
                text: { const h = Math.floor(page.uptime / 3600), d = Math.floor(h / 24); return d > 0 ? d + " d " + (h % 24) + " h" : h + " h " + Math.floor(page.uptime % 3600 / 60) + " min" }
                color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(12.5) }
            }
        }
    }
}
