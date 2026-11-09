# WhiteRose Files — feature checklist

The build checklist. Tiers: MVP first, then Next, then Advanced.
`[x]` done, `[~]` started (see the note).

## 1. Navigation & file browsing
- [x] Tabbed browsing (MVP)
- [x] Reorder, duplicate, tear off and restore tabs (MVP) — drag or Ctrl+Shift+PgUp/PgDn; tab menu; drag out of the bar for a new window, or onto another Files window's title bar to move it there (across processes too); Ctrl+Shift+T, and every window's tabs come back next launch
- [x] Dual-pane browsing, horizontal and vertical (MVP) — per tab (F3), side by side or one above the other, resizable; restored with the tabs
- [x] Copy/move files between panes using shortcuts or drag and drop (MVP) — F5 / F6, the right-click menu, or dragging
- [ ] Compare folders and one-way/two-way/mirror sync (Advanced)
- [x] Icon/grid, details/list and column views (MVP)
- [x] Gallery view (Next) — one item as large as it fits, arrows on hover, "3 of 9" (no filmstrip, by choice)
- [x] Disk usage view as a file browsing mode (Advanced) — a treemap of the folder, sized as folders are measured
- [x] Adjustable item density and icon/text sizing (Next) — Ctrl+= / Ctrl+- / Ctrl+0
- [x] Cards view and adaptive views (added) — cards with kind, size and date; folders without a view of their own get one to suit them
- [x] Breadcrumb address bar with path suggestions (MVP) — "… > Folder" when long; click "…" for the full path to copy
- [x] Navigation history, back, forward and parent directory (MVP)
- [~] Customisable sidebar with pinned folders and drives (MVP) — places, GTK bookmarks (add/remove), mounted drives; no reordering yet
- [ ] Expandable directory tree in sidebar (Next)
- [ ] Workspaces saving tabs, panes and view settings (Next)
- [ ] Home dashboard with drives, recents and shortcuts (Next)

## 2. Search, filtering & discovery
- [x] Instant indexed filename search (MVP) — home and drives; built in the background at idle priority, read from disk when searching (no memory held)
- [x] Search across multiple locations (MVP) — this folder and below (live), home, or everywhere (home and drives)
- [x] Inline folder filtering as you type (MVP) — type in any folder; Esc clears; take it to subfolders or everywhere
- [x] Search by file type, extension, size and date (MVP) — type, size and modified filters; `*.pdf` / `ext:pdf`
- [ ] Regular expressions and case-sensitive search (Next)
- [ ] Metadata and file-content search (Advanced)
- [ ] Path autocomplete and fuzzy folder navigation (Next)
- [ ] Recently modified files collection (Next)

## 3. File operations & organisation
- [x] Create, copy, cut, paste, move, rename and delete (MVP)
- [x] Multi-selection, drag and drop, clipboard integration (MVP) — drag to folders, the empty space, sidebar places, the Trash, breadcrumbs and tabs (held over a tab opens it); to and from other apps; same drive moves, another copies, Ctrl copies, Shift moves
- [x] Background transfer queue with progress (MVP)
- [x] Pause, resume and cancel transfers (Next)
- [x] Side-by-side conflict resolution (Next) — size and date of both, replace / keep both / skip, same for the rest
- [ ] Text diffs when resolving overwrites (Advanced)
- [~] Merge directories intelligently (Next) — folder onto folder merges, asking about each clash inside
- [ ] Optional checksum verification of copies (Next)
- [ ] Persistent undo/redo operation history (Advanced)
- [ ] Batch rename with regex, numbering, case and preview (Next)
- [ ] Create folder from selected files (Next)
- [ ] File shelf / staging area for collecting items (Next)
- [x] Recycle Bin/Trash integration and restore (MVP) — every Trash (home and drives) in one view: where from, when; restore (folder remade, renamed if taken), delete for good, empty
- [x] File and folder properties, permissions and attributes (MVP) — kind, location, size (folders counted live), dates, owner and group, read / change / run for you, group and everyone
- [ ] Copy file paths, quoted paths and shortcuts (Next)
- [ ] Custom file tags with colours and multiple labels (Next)
- [ ] File hashing with MD5, SHA-1 and SHA-256 (Next)
- [ ] Archive create, extract and in-place browse (Next)
- [ ] Password-protected ZIP/7z and split archives (Advanced)

## 4. Previews & file inspection
- [x] Integrated preview/details pane (MVP) — Alt+P: preview plus kind, size, dates, dimensions / length / pages
- [x] Quick preview using the Space key (MVP) — full window; arrows go through the folder, Enter opens
- [x] Image, audio, video and PDF previews (MVP) — and the start of text and code; photo thumbnails in the grid
- [ ] Markdown rendering including tables and code (Next)
- [ ] Mermaid and LaTeX rendering in Markdown (Advanced)
- [ ] Syntax-highlighted source code and config previews (Next)
- [ ] JSON/JSONC/JSON5 formatted viewer (Next)
- [ ] Browse and extract individual items from archives (Next)
- [ ] Inspector with editable metadata and default app (Next)

## 5. Storage & cleanup
- [x] Live folder size calculation (MVP) — list view's Size column fills in folder sizes in the background (idle priority, kept a minute)
- [ ] Recursive size calculation with background caching (Next)
- [ ] Disk usage treemap visualisation (Next)
- [ ] Largest files browser (Next)
- [ ] Duplicate file finder (Next)
- [x] Disk capacity and free-space dashboard (MVP) — Storage page (sidebar › Drives): every drive, how full, free of total
- [ ] Safe deletion/cleanup with confirmation and recovery (Next)
- [ ] Project build-cache and dependency cleanup (Advanced)

## 6. Appearance & personalisation
- [x] Native light, dark and system modes (MVP) — follows the desktop theme via the kit
- [ ] Per-folder background images (Next)
- [x] Folder colours and icons (added) — a colour, a symbol, or a picture; follows renames, moves and the Trash
- [ ] Custom toolbar buttons and layouts (Next)
- [ ] Customisable right-click context menus (Next)
- [x] Fully remappable keyboard shortcuts (MVP) — ☰ › Keyboard shortcuts…: every shortcut, press new keys, clashes refused, reset
- [x] Custom view columns and sorting preferences (Next) — view, sort and grouping per folder (lent to subfolders, or everywhere); list columns chosen and resized; folders first / mixed / last
- [x] Group by (added) — kind, date modified, size or name, with headings that fold, in list, grid and cards
- [ ] Custom app startup and restoration behaviour (Next)
- [~] Accessibility, keyboard navigation and scaling (MVP) — arrows/Enter/Backspace, shortcuts for everything so far

## 7. Developer tools & automation
- [ ] Git status badges beside files (Next)
- [ ] Current Git branch in path/navigation bar (Next)
- [ ] Git clone, init, fetch, pull, push and sync (Advanced)
- [ ] Git branch switching and repository operations (Advanced)
- [ ] Project type recognition / script discovery (Advanced)
- [ ] Run project scripts from an inspector (Advanced)
- [x] Open folders in terminal (MVP) — F4 or right-click; terminal chosen in Settings › Apps
- [x] Open current folder or repo in preferred code editor (MVP) — and text files; "Open repository in …" from inside a git repo
- [ ] Built-in terminal integration (Next)
- [ ] Docker/Podman container file browsing (Advanced)
- [ ] Container lifecycle operations and Compose management (Advanced)
- [ ] Command-line launcher and file search (Next)

## 8. Cloud, remote & network
- [~] Local drives and removable media (MVP) — mounted drives listed; no mount/eject yet
- [ ] SMB and other network share browsing (Next)
- [ ] Cloud-synced folders and sync-state badges (Next)
- [ ] OneDrive, Google Drive and iCloud integration (Next)
- [ ] Cloud offline/download/evict actions (Advanced)
- [ ] FTP server support (Next)
- [ ] SFTP and SSH server browsing (Next)
- [ ] S3-compatible object storage browsing (Advanced)
- [ ] Remote upload/download/copy/move/delete (Next)
- [ ] SSH config, agent and keychain/credential integration (Advanced)

## 9. Privacy, reliability & extensibility
- [x] Private/local-first file indexing (MVP) — one file in your cache folder, 0600; hidden folders, node_modules and Go's cache left out
- [ ] Encrypted local indexes and operation history (Next)
- [ ] System credential vault for secrets (Next)
- [ ] Safe archive extraction and path validation (MVP)
- [ ] Prevent unsafe code execution from untrusted folders (MVP)
- [ ] Recovery after interrupted file transfers (Next)
- [ ] Application logging and diagnostics (MVP)
- [ ] Export/import settings and preferences (Next)
- [ ] Signed application updates (Next)
- [ ] Extension/plugin API for third-party actions (Advanced)
- [ ] User-defined automation and batch workflows (Advanced)
