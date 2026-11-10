# WhiteRose Files — feature checklist

The build checklist. Tiers: MVP first, then Next, then Advanced.
`[x]` done, `[~]` started (see the note), `[-]` not doing (see the note).

## 1. Navigation & file browsing
- [x] Tabbed browsing (MVP)
- [x] Reorder, duplicate, tear off and restore tabs (MVP) — drag or Ctrl+Shift+PgUp/PgDn; tab menu; drag out of the bar for a new window, or onto another Files window's title bar to move it there (across processes too); Ctrl+Shift+T, and every window's tabs come back next launch
- [x] Dual-pane browsing, horizontal and vertical (MVP) — per tab (F3), side by side or one above the other, resizable; restored with the tabs
- [x] Copy/move files between panes using shortcuts or drag and drop (MVP) — F5 / F6, the right-click menu, or dragging
- [x] Compare folders and one-way/two-way/mirror sync (Advanced) — ☰ › This folder › Compare with… (the other side when split): only-here/only-there/newer/different, all the way down, by size and time or byte by byte; update either way, two-way (unclear ones left alone and counted), or mirror (extras to the Trash, after asking); runs as a transfer
- [x] Icon/grid, details/list and column views (MVP)
- [x] Gallery view (Next) — one item as large as it fits, arrows on hover, "3 of 9" (no filmstrip, by choice)
- [x] Disk usage view as a file browsing mode (Advanced) — a treemap of the folder, sized as folders are measured
- [x] Adjustable item density and icon/text sizing (Next) — Ctrl+= / Ctrl+- / Ctrl+0
- [x] Cards view and adaptive views (added) — cards with kind, size and date; folders without a view of their own get one to suit them
- [x] Breadcrumb address bar with path suggestions (MVP) — "… > Folder" when long; click "…" for the full path to copy
- [x] Navigation history, back, forward and parent directory (MVP)
- [x] Customisable sidebar with pinned folders and drives (MVP) — places, bookmarks (add, remove, drag to reorder; shared with GTK file choosers), drives, a tree under each, sections that fold
- [x] Expandable directory tree in sidebar (Next) — a chevron on each place and bookmark opens its folders, as deep as you like; what's open is remembered
- [x] Workspaces saving tabs, panes and view settings (Next) — ☰ › Workspaces or the Start page: save every open window under a name (with its folders' views), open it here or in new windows, rename, delete
- [x] Home dashboard with drives, recents and shortcuts (Next) — Start: workspaces, places, recent files, drives with how full, tags; can be where Files opens (Settings › Files)

## 2. Search, filtering & discovery
- [x] Instant indexed filename search (MVP) — home and drives; built in the background at idle priority, read from disk when searching (no memory held)
- [x] Search across multiple locations (MVP) — this folder and below (live), home, or everywhere (home and drives)
- [x] Inline folder filtering as you type (MVP) — type in any folder; Esc clears; take it to subfolders or everywhere
- [x] Search by file type, extension, size and date (MVP) — type, size and modified filters; `*.pdf` / `ext:pdf`
- [x] Regular expressions and case-sensitive search (Next) — "Aa" and ".*" in the search row; a pattern that doesn't parse says why
- [x] Metadata and file-content search (Advanced) — the search row's "Text inside files…": text files, PDFs (with pdftotext), and files' comments, in this folder, home, or home and the drives; with name, type, size and date filters, match case and regex; each result shows the line it was found on
- [x] Path autocomplete and fuzzy folder navigation (Next) — paths complete as you type; without a leading / or ~ (or Ctrl+J) a few letters jump to a folder from anywhere, the ones you visit most first
- [x] Recently modified files collection (Next) — Recent in the sidebar: files apps opened (the shared recently-used list) and files changed in the last two weeks (from the index), newest first, grouped by day; hidden with Settings › Privacy › Recent files

## 3. File operations & organisation
- [x] Create, copy, cut, paste, move, rename and delete (MVP)
- [x] Multi-selection, drag and drop, clipboard integration (MVP) — drag to folders, the empty space, sidebar places, the Trash, breadcrumbs and tabs (held over a tab opens it); to and from other apps; same drive moves, another copies, Ctrl copies, Shift moves
- [x] Background transfer queue with progress (MVP)
- [x] Pause, resume and cancel transfers (Next)
- [x] Side-by-side conflict resolution (Next) — size and date of both, replace / keep both / skip, same for the rest
- [x] Text diffs when resolving overwrites (Advanced) — a clash between two text files (up to 1 MB) offers Show changes: a line-by-line diff (Myers, as git) of what's there against what's coming, with Skip / Keep both / Replace right there
- [x] Merge directories intelligently (Next) — folder onto folder merges; files already there and the same (same bytes; over 64 MB, same size and time) are skipped without asking (a move lets the original go); Keep newer / Merge, keep newer replaces only older files
- [x] Optional checksum verification of copies (Next) — Settings › Files › Verify copies: each file is read back from the drive (cache dropped) and checked; a mismatch keeps nothing
- [-] Persistent undo/redo operation history (Advanced) — not doing for now
- [x] Batch rename with regex, numbering, case and preview (Next) — Rename… (F2) with several selected: find and replace (text or regex with $1 groups, case, first only), case, text before/after, numbering in view order (position, start, step, digits, separator), extension kept/lowercased/changed; live preview with clashes and taken names shown and skipped; swaps and shifts (1→2, 2→3) handled
- [x] Create folder from selected files (Next) — "New folder with selection…", named from what the files share
- [x] File shelf / staging area for collecting items (Next) — Put on the shelf (Ctrl+Shift+S, right-click, or drop on Shelf in the sidebar or on the strip); it sits along the bottom while anything's on it, shared by every window and kept between runs: drag items (or all) off it anywhere, or Copy here / Move here, Clear
- [x] Recycle Bin/Trash integration and restore (MVP) — every Trash (home and drives) in one view: where from, when; restore (folder remade, renamed if taken), delete for good, empty
- [x] File and folder properties, permissions and attributes (MVP) — kind, location, size (folders counted live), dates, owner and group, read / change / run for you, group and everyone
- [x] Copy file paths, quoted paths and shortcuts (Next) — Copy path › path, quoted for a terminal, file:// links, names; and this folder's path
- [x] Custom file tags with colours and multiple labels (Next) — on the file itself (user.xdg.tags, as Dolphin), dots in every view, a Tags column, Sidebar section, tag: search, group by tag, drop onto a tag
- [x] File hashing with MD5, SHA-1 and SHA-256 (Next) — Properties › Checksums (one read for all three), copy each, paste one to compare
- [x] Password-protected ZIP/7z and split archives (Advanced) — Compress… makes zip or 7z with an AES-256 password and splits any archive into parts (100 MB, 700 MB, 4 GB); extracting asks for the password (memory only) and takes split parts from the .001

## 4. Previews & file inspection
- [x] Integrated preview/details pane (MVP) — Alt+P: preview plus kind, size, dates, dimensions / length / pages
- [x] Quick preview using the Space key (MVP) — full window; arrows go through the folder, Enter opens
- [x] Image, audio, video and PDF previews (MVP) — and the start of text and code; photo thumbnails in the grid
- [x] Markdown rendering including tables and code (Next) — drawn in the preview and quick look (headings, tables, code, task lists, pictures beside it, links in the accent); "Source" shows the text
- [x] Mermaid and LaTeX rendering in Markdown (Advanced) — drawn by mermaid-cli and TeX Live when they're installed (nothing bundled), cached as SVG per theme; otherwise shown as code with what to install
- [x] Syntax-highlighted source code and config previews (Next) — KDE's syntax-highlighting (about 300 formats), chosen by file name, light and dark
- [x] JSON/JSONC/JSON5 formatted viewer (Next) — comments, trailing commas, unquoted keys read; shown formatted in its own order; where it's broken if it doesn't parse
- [x] Browse and extract individual items from archives (Next) — an archive opens like a folder (zip, 7z, tar.*, split; a password asked for if its names are hidden); open or preview what's inside; Extract beside it, to the other side, or drag out onto any folder
- [x] Inspector with editable metadata and default app (Next) — Properties: rename in place, a comment kept on the file (user.xdg.comment), Opens with: the default app for every file of the kind (mimeapps.list, so every app agrees); Open with › any app for it in the menu

## 5. Storage & cleanup
- [x] Live folder size calculation (MVP) — list view's Size column fills in folder sizes in the background (idle priority, kept a minute)
- [x] Recursive size calculation with background caching (Next) — the list's Size column and Properties (see Storage)
- [x] Disk usage treemap visualisation (Next) — the disk-usage view
- [x] Largest files browser (Next) — Clean up › Largest files: the 200 largest under home, a drive or a folder (hidden ones too), on its own drive
- [x] Duplicate file finder (Next) — Clean up › Duplicates: same size, same start, then same contents (SHA-256); hard links aren't copies; "Keep the oldest of each"
- [x] Disk capacity and free-space dashboard (MVP) — Storage page (sidebar › Drives): every drive, how full, free of total
- [x] Safe deletion/cleanup with confirmation and recovery (Next) — Clean up moves to the Trash by default (recoverable); deleting asks first
- [x] Project build-cache and dependency cleanup (Advanced) — Clean up › Build caches: node_modules, Rust target, virtualenvs, __pycache__, JS/Gradle/Zig/.NET build output (each only beside its project file), and home's download caches (pip, npm, Cargo, Go, Gradle, Maven...)

## 6. Appearance & personalisation
- [x] Native light, dark and system modes (MVP) — follows the desktop theme via the kit
- [-] Per-folder background images (Next) — not doing (decided against)
- [x] Folder colours and icons (added) — a colour, a symbol, or a picture; follows renames, moves and the Trash
- [x] Custom toolbar buttons and layouts (Next) — Settings › Files › Toolbar & menus: new folder, look again, terminal, code editor, split, hidden files, details, shelf, compress, compare & sync, new tab beside the views
- [x] Customisable right-click context menus (Next) — leave out built-in entries; add your own actions (a command on what's chosen: %f paths, %n names, %d folder; for files, folders or either, chosen types; in a terminal or not), edited in place in Settings
- [x] Fully remappable keyboard shortcuts (MVP) — ☰ › Keyboard shortcuts…: every shortcut, press new keys, clashes refused, reset
- [x] Custom view columns and sorting preferences (Next) — view, sort and grouping per folder (lent to subfolders, or everywhere); list columns chosen and resized; folders first / mixed / last
- [x] Group by (added) — kind, date modified, size or name, with headings that fold, in list, grid and cards
- [x] Custom app startup and restoration behaviour (Next) — Settings › Files › Open with: last tabs, Home, or a folder
- [~] Accessibility, keyboard navigation and scaling (MVP) — arrows/Enter/Backspace, shortcuts for everything so far

## 7. Developer tools & automation
- [x] Git status badges beside files (Next) — modified, new, untracked, conflict; folders show the worst inside; ignored dimmed
- [x] Current Git branch in path/navigation bar (Next) — in the status bar, with ahead / behind
- [x] Git clone, init, fetch, pull, push and sync (Advanced) — ☰ › This folder › Start a git repository / Clone a repository here (with progress); the status bar's branch: fetch, pull, push (publish a new branch), sync; your git, credential helper and SSH keys; failures said plainly
- [x] Git branch switching and repository operations (Advanced) — switch to local or remote branches, make new ones; right-click › Git: stage, unstage, commit, throw away changes (what you had goes to the Trash)
- [x] Project type recognition / script discovery (Advanced) — Rust, Node (npm/pnpm/yarn/bun scripts), Go, Python (pyproject scripts, uv), Make and just targets, CMake, Compose
- [x] Run project scripts from an inspector (Advanced) — the details panel's Scripts: each runs in your terminal and stays open with how it ended; in untrusted places only after "I trust this project"
- [x] Open folders in terminal (MVP) — F4 or right-click; terminal chosen in Settings › Apps
- [x] Open current folder or repo in preferred code editor (MVP) — and text files; "Open repository in …" from inside a git repo
- [-] Built-in terminal integration (Next) — not doing: Open in terminal (F4) instead
- [x] Docker/Podman container file browsing (Advanced) — Drives › Containers › Files: a running container's folders, and anything copied out to a folder you choose
- [x] Container lifecycle operations and Compose management (Advanced) — start, stop, restart, pause, remove (asked); grouped by Compose project with Up / Down / its folder; logs and a shell in your terminal
- [x] Command-line launcher and file search (Next) — `whiterose-files FOLDER…` opens them; `whiterose-files --find [--in DIR | --everywhere] [--inside TEXT] [--type …] [--changed …] [--case] [--regex] [--json] WORDS` prints matches from the same search (exit 1 when none)

## 8. Cloud, remote & network
- [x] Local drives and removable media (MVP) — plugged-in drives listed, a click mounts (udisks), eject unmounts and powers off
- [x] SMB and other network share browsing (Next) — Connect to server… (☰ or the sidebar's Network): smb:// or \\server\share, ftp://, davs:// (WebDAV), nfs:// through GVfs, browsed as ordinary folders; saved servers and any share mounted now in the sidebar (eject, right-click to forget); passwords in the keyring through whiterosed
- [x] Cloud-synced folders and sync-state badges (Next) — folders the Nextcloud or ownCloud desktop client keeps: synced / syncing / waiting to upload / couldn't sync / only in the cloud, on every file (the clients' own socket, as Nautilus and Dolphin use)
- [x] OneDrive, Google Drive and iCloud integration (Next) — Add cloud account… (sidebar's Network or ☰): Google Drive, OneDrive, Dropbox, iCloud Drive, pCloud through rclone (installed from Settings › Apps), signing in with the browser and asking anything else (iCloud's two-factor code) in Files; opened as folders, cached on disk as used; also GNOME Online Accounts' drives
- [x] Cloud offline/download/evict actions (Advanced) — right-click › Nextcloud: Keep on this device, Free up space, Share…, Copy link (virtual files)
- [x] FTP server support (Next) — ftp:// and ftps:// through Connect to server (guest or with a password)
- [x] SFTP and SSH server browsing (Next) — sftp://user@host (SSH keys and the agent work as with ssh; or a password); a host key not seen before is shown and only trusted when you say so
- [x] S3-compatible object storage browsing (Advanced) — Amazon S3, Cloudflare R2, Wasabi, MinIO or any S3-compatible service, with its keys, as a folder (rclone)
- [x] Remote upload/download/copy/move/delete (Next) — a connected share is an ordinary folder: every copy, move, delete and preview works there, with progress and clash handling
- [x] SSH config, agent and keychain/credential integration (Advanced) — Connect to server offers the hosts in ~/.ssh/config (their user, port and key apply); SSH keys and the agent work as with ssh; share passwords in the keyring

## 9. Privacy, reliability & extensibility
- [x] Private/local-first file indexing (MVP) — one file in your cache folder, 0600; hidden folders, node_modules and Go's cache left out
- [x] Encrypted local indexes and operation history (Next) — Settings › Files › Search › Privacy: the search index, visited folders (Jump) and the operations log sealed (AES-256-CTR + HMAC-SHA256, fresh nonce each write) with a key kept in the keyring through whiterosed; nothing written in the clear when the key can't be had; switching either way rewrites what's there
- [x] System credential vault for secrets (Next) — every secret Files keeps is in the keyring through whiterosed (network share passwords, the sealing key); archive passwords stay in memory for the run; saved servers and exports never hold a password. (rclone keeps its cloud tokens in its own config, as rclone does.)
- [x] Safe archive extraction and path validation (MVP) — zip and tar (gz/xz/bz2/zst): escaping names refuse the whole archive, links kept inside, no devices or setuid, stops before the drive fills
- [x] Archive create, extract and in-place browse (Next) — Compress… (zip, 7z, tar.gz/xz/zst), Extract here, and opening an archive like a folder
- [x] Prevent unsafe code execution from untrusted folders (MVP) — programs and scripts always ask; from Downloads, the Desktop, removable drives, temp folders or the Trash only after "I trust this file"; launchers from there never run
- [x] Recovery after interrupted file transfers (Next) — copies and moves are written down until they end; one cut short (Files closed, a crash, power off) is offered at the next start: Carry on clears the half-written file, skips what's already there and finishes the rest
- [x] Application logging and diagnostics (MVP) — files.log (0600, rolled at 1 MB); ☰ › Help › Copy diagnostics / Open the log
- [x] Export/import settings and preferences (Next) — ☰ › Export / Import settings: every files.* setting, folder views, workspaces, folder colours and icons, saved servers (never passwords) and bookmarks in one file; importing keeps what was there in before-import.json
- [x] Signed application updates (Next) — Files ships in WhiteRose's signed, health-checked updates (Settings › Updates)
- [-] Extension/plugin API for third-party actions (Advanced) — comes with the module API (roadmap 1.2: Files extensions)
- [-] User-defined automation and batch workflows (Advanced) — not doing for now
