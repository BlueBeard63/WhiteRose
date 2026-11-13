# WhiteRose Module API: design

> **Status: design, not built yet.** The roadmap puts a preview in **1.1** and a stable API in **1.2** ([ROADMAP.md](ROADMAP.md)). This document is what gets built; change it first, then the code.

A **module** extends WhiteRose. It can add a Settings page or a section to an existing one, a bar widget, an island view, a quick-settings tile, or launcher results; draw overlays above other apps; script windows; and extend Files. Its **logic** is written in **Go or Rust**, its **UI** in **QML**. It runs **sandboxed by default**, and you grant it more access in **Settings › Modules**.

---

## 1. Goals

- **Safe by default.** A module you install from a git URL can't read your files, reach the network, or watch your windows unless you said it may.
- **Cheap.** A module costs nothing while unused: its process starts when one of its parts is used, and exits when idle. No polling; events are pushed.
- **Native.** Real Go and Rust with their own libraries, not a scripting subset. UI in the same QML the shell uses, so modules look like the rest of WhiteRose.
- **Stable.** One versioned protocol under both SDKs. A module built for API 1.x keeps working through 1.y.
- **Simple to start.** `whiterose module new my-widget --lang go` gives a working bar widget.

**Not goals:** themes that restyle the shell's internals (Appearance covers theming), replacing the compositor's rendering, or running modules as root.

## 2. What a module is

A folder, or a git repository with tagged releases:

```
my-weather/
├── module.toml          the manifest: what it is, what it adds, what it needs
├── qml/                 UI parts (only what's declared in the manifest is loaded)
│   ├── Widget.qml
│   └── SettingsPage.qml
├── src/                 the logic (Go or Rust), built into bin/
└── bin/my-weather       the built logic (shipped in release archives)
```

A module with only QML (a clock style, a static widget) has no `src/` or `bin/` and no process at all.

### The manifest

```toml
id = "dev.example.weather"          # reverse-DNS, unique
name = "Weather Plus"
version = "1.4.0"
api = "1"                           # the module API major version it targets
description = "Radar and hourly forecast in the bar and the calendar panel"
authors = ["Sam Example"]
license = "MIT"
homepage = "https://github.com/example/weather-plus"

[logic]
exec = "bin/weather-plus"           # omitted for QML-only modules
idle_exit = "5m"                    # stopped after this long unused (default 2m)

# What it adds (each is an extension point, section 3).
[[bar.widget]]
id = "radar"
qml = "qml/Widget.qml"
zones = ["right"]

[[settings.page]]
id = "weather-plus"
qml = "qml/SettingsPage.qml"
parent = "calendar"                 # a sub-page of Settings › Calendar & weather

[[launcher.provider]]
id = "forecast"
prefix = "w "                       # results when you type "w london"

# What it needs (section 5). Asked for at install; changeable in Settings › Modules.
[permissions]
network = ["api.open-meteo.com"]    # these hosts only
location = "coarse"
```

## 3. Extension points

| Extension point | Built from | Arrives in | Permission |
|---|---|---|---|
| `settings.page`: a Settings page or sub-page | QML | 1.1 | none |
| `settings.section`: a section in an existing page | QML | 1.1 | none |
| `bar.widget`: in a bar zone, arranged in Settings › Top bar | QML | 1.1 | none |
| `island.view`: a view in the island, opened by key or IPC | QML | 1.1 | none |
| `quick.tile`: a quick-settings tile | QML | 1.1 | none |
| `launcher.provider`: results for what you type | logic | 1.1 | none (sees only what you type after its prefix) |
| `notifications.send` | logic | 1.1 | `notifications` |
| `overlay`: layer-shell surfaces over or under windows (HUDs, desktop widgets, OSDs) | QML + logic | 1.2 | `overlays` |
| `windows`: list, move, resize, focus, rules, events | logic | 1.2 | `windows` |
| `files.action`: context-menu actions in Files | logic | 1.2 | `files` for the selected items |
| `files.place`, `files.column`, `files.preview` | logic + QML | 1.2 | as declared |
| `files.backend`: a storage backend (a cloud drive) | logic | 1.2 | `network`, `files` |

## 4. How it runs

```
 Shell (QuickShell)                 whiterosed                      Module logic
 ─────────────────                  ──────────                      ────────────
 loads the module's QML      ⇄      routes calls, checks     ⇄      sandboxed process
 (restricted imports)               permissions, starts and         (bubblewrap + Landlock
                                    stops processes                  + seccomp)
```

- **whiterosed owns modules.** It reads manifests, starts a module's logic on first use (a widget shown, a launcher prefix typed, an event subscribed), and stops it after `idle_exit`.
- **The protocol** is JSON-RPC 2.0 over a socket pair passed as fd 3: requests both ways, plus notifications for events. The SDKs hide it; any language can speak it.
- **QML parts run in the shell**, loaded through a restricted engine. They may import only `QtQuick`, `QtQuick.Layouts`, `QtQuick.Shapes` and `WhiteRose.Module`: no `Quickshell.Io`, no process launching, no file access. A URL interceptor refuses anything outside the module's own folder. Everything else goes through the module's logic, where permissions apply.
- **Crashes stay contained.** A crashing module process is restarted with backoff, then disabled with a notification; a QML part that errors is replaced by a placeholder.

## 5. The sandbox and permissions

By default a module's process gets:

- its own data folder (`~/.local/share/whiterose/modules/<id>/data`) and cache, read-write;
- its own folder, read-only;
- its own settings namespace (`modules.<id>.*` in the settings database);
- **no** network, **no** home folder, **no** D-Bus, **no** other processes.

It runs under **bubblewrap** (namespaces, a minimal filesystem view), **Landlock** (path rules the kernel enforces even if bubblewrap is bypassed) and **seccomp** (no `ptrace`, no kernel-module or mount syscalls).

Extra access is declared in the manifest, shown at install, and changeable any time in **Settings › Modules › (module) › Access**:

| Permission | Grants | Shown as |
|---|---|---|
| `network = ["host", ...]` or `"any"` | Outbound connections (filtered per host by a proxy in whiterosed) | "Connects to api.open-meteo.com" |
| `files = ["~/Music", ...]` / `"pick"` | Those folders; or only what you pick in a file chooser | "Reads and changes files in Music" |
| `location = "coarse" \| "precise"` | Location from whiterosed | "Knows your approximate location" |
| `notifications` | Send notifications | "Shows notifications" |
| `windows = "read" \| "control"` | Window list and events; or moving and focusing them too | "Sees your open windows" |
| `overlays` | Layer-shell surfaces | "Draws over other apps" |
| `dbus = ["org.mpris.*", ...]` | Talk to those D-Bus names (via a filtering proxy) | "Controls media players" |
| `exec = ["ffmpeg", ...]` | Run those host programs | "Runs ffmpeg" |
| `full` | No sandbox: runs as you | "Full access to your account", in rose, behind a confirmation |

Updates that ask for **more** access wait for your approval; the old version keeps running meanwhile.

## 6. The SDKs

Both SDKs are generated from one protocol schema (`api/v1/schema.json`), so they can't drift apart.

**Go** (`github.com/BlueBeard63/whiterose-sdk-go`):

```go
package main

import wr "github.com/BlueBeard63/whiterose-sdk-go"

func main() {
	m := wr.New()

	// Launcher results for "w <city>".
	m.Launcher("forecast", func(q wr.Query) ([]wr.Result, error) {
		f, err := fetchForecast(q.Text) // network: api.open-meteo.com only
		if err != nil {
			return nil, err
		}
		return []wr.Result{{Title: f.Summary, Subtitle: q.Text, Icon: "cloudSun"}}, nil
	})

	// State the bar widget's QML binds to.
	m.Expose("radar", func(ctx wr.Context) (any, error) {
		return currentRadar(ctx.Settings.String("city"))
	})

	m.Run() // serves until idle_exit
}
```

**Rust** (the `whiterose-sdk` crate):

```rust
use whiterose_sdk::{Module, Query, ResultItem};

fn main() -> whiterose_sdk::Result<()> {
    Module::new()
        .launcher("forecast", |q: Query| async move {
            let f = fetch_forecast(&q.text).await?;
            Ok(vec![ResultItem::new(f.summary).subtitle(q.text).icon("cloudSun")])
        })
        .expose("radar", |ctx| async move { current_radar(ctx.settings().string("city")?).await })
        .run()
}
```

**QML** (`import WhiteRose.Module`):

```qml
import QtQuick
import WhiteRose.Module

BarWidget {
    // From the module's logic (m.Expose("radar", ...)); refreshed when it pushes.
    readonly property var radar: Module.state("radar")
    Text { text: radar ? radar.temp + "°" : "…"; color: Theme.text; font: Theme.barFont }
    onClicked: Module.open("island.view", "radar")
}
```

`WhiteRose.Module` provides the shell's `Theme`, the building blocks (`BarWidget`, `SettingsPage`, `SettingRow`, `Toggle`, `Choice`, `Button`, `Card`, `IslandView`, `Tile`), `Module.state` / `Module.call` / `Module.settings`, and the icon set.

## 7. Installing and updating

- **From a git URL:** `whiterose module add https://github.com/example/weather-plus`, or Settings › Modules › Add. WhiteRose reads the repository's latest **tagged release**, downloads its `<id>-<version>-x86_64.tar.zst` and checksums (verified, and optionally a minisign signature with the key pinned on first install), shows the manifest's permissions, and installs into `~/.local/share/whiterose/modules/<id>/`. New tags show up in Settings › Modules as updates.
- **From a local folder:** for development. Settings › Modules › Add › Folder, or `whiterose module dev ./my-widget`, which links the folder and reloads on change.
- **Removing** deletes the module's folder, data and settings namespace (after asking).

## 8. Versions and compatibility

- `api = "1"` in the manifest. Within 1.x, additions only; nothing is removed or changed.
- The preview (1.1) is `api = "1-preview"` and may change. 1.2 freezes it as `api = "1"`.
- whiterosed tells a module which minor version it speaks; the SDKs check before using newer calls.

## 9. Building it (the order)

1. **Protocol and loader:** schema, whiterosed's module manager (manifests, activation, idle exit), the restricted QML engine.
2. **Sandbox:** `whiterose-sandbox` (bubblewrap + Landlock + seccomp), the network and D-Bus filtering proxies, Settings › Modules › Access.
3. **SDKs:** Go and Rust generated from the schema; `whiterose module new|dev|add`.
4. **Shell extension points** (1.1), then **overlays, windows, Files** (1.2).
5. **Docs and examples:** a bar widget, a launcher provider, a Settings page, a Files action, a cloud backend.

## 10. Open questions

- **QML sandboxing:** the restricted engine trusts QtQuick's own types. If that proves leaky, module UI moves to its own process and surfaces (heavier; decided after the 1.1 preview).
- **Release artifacts vs. building from source:** prebuilt archives are fast and reproducible to check; building from source avoids trusting a binary. The plan is prebuilt first, `--from-source` later.
- **Today's Settings modules** (QML pages from `module.json` in `~/.local/share/whiterose/modules`) become `settings.page` modules without logic. A converter keeps existing ones working.
