# WhiteRose Files — feature checklist

The build checklist. Tiers: MVP first, then Next, then Advanced.
`[x]` done, `[~]` started (see the note).

## 1. Navigation & file browsing
- [x] Tabbed browsing (MVP)
- [x] Reorder, duplicate, tear off and restore tabs (MVP) — drag or Ctrl+Shift+PgUp/PgDn; tab menu; drag down out of the bar; Ctrl+Shift+T, and every window's tabs come back next launch
- [ ] Dual-pane browsing, horizontal and vertical (MVP)
- [ ] Copy/move files between panes using shortcuts or drag and drop (MVP)
- [ ] Compare folders and one-way/two-way/mirror sync (Advanced)
- [x] Icon/grid, details/list and column views (MVP)
- [ ] Gallery view with thumbnail filmstrip (Next)
- [ ] Disk usage view as a file browsing mode (Advanced)
- [ ] Adjustable item density and icon/text sizing (Next)
- [x] Breadcrumb address bar with path suggestions (MVP) — "… > Folder" when long; click "…" for the full path to copy
- [x] Navigation history, back, forward and parent directory (MVP)
- [~] Customisable sidebar with pinned folders and drives (MVP) — places, GTK bookmarks (add/remove), mounted drives; no reordering yet
- [ ] Expandable directory tree in sidebar (Next)
- [ ] Workspaces saving tabs, panes and view settings (Next)
- [ ] Home dashboard with drives, recents and shortcuts (Next)

## 2. Search, filtering & discovery
- [ ] Instant indexed filename search (MVP)
- [ ] Search across multiple locations (MVP)
- [ ] Inline folder filtering as you type (MVP)
- [ ] Search by file type, extension, size and date (MVP)
- [ ] Regular expressions and case-sensitive search (Next)
- [ ] Metadata and file-content search (Advanced)
- [ ] Path autocomplete and fuzzy folder navigation (Next)
- [ ] Recently modified files collection (Next)

## 3. File operations & organisation
- [x] Create, copy, cut, paste, move, rename and delete (MVP)
- [~] Multi-selection, drag and drop, clipboard integration (MVP) — selection and the clipboard (works with other file managers both ways); drag and drop next
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
- [ ] Integrated preview/details pane (MVP)
- [ ] Quick preview using the Space key (MVP)
- [ ] Image, audio, video and PDF previews (MVP)
- [ ] Markdown rendering including tables and code (Next)
- [ ] Mermaid and LaTeX rendering in Markdown (Advanced)
- [ ] Syntax-highlighted source code and config previews (Next)
- [ ] JSON/JSONC/JSON5 formatted viewer (Next)
- [ ] Browse and extract individual items from archives (Next)
- [ ] Inspector with editable metadata and default app (Next)

## 5. Storage & cleanup
- [ ] Live folder size calculation (MVP)
- [ ] Recursive size calculation with background caching (Next)
- [ ] Disk usage treemap visualisation (Next)
- [ ] Largest files browser (Next)
- [ ] Duplicate file finder (Next)
- [~] Disk capacity and free-space dashboard (MVP) — usage bars on sidebar drives
- [ ] Safe deletion/cleanup with confirmation and recovery (Next)
- [ ] Project build-cache and dependency cleanup (Advanced)

## 6. Appearance & personalisation
- [x] Native light, dark and system modes (MVP) — follows the desktop theme via the kit
- [ ] Per-folder background images (Next)
- [ ] Custom toolbar buttons and layouts (Next)
- [ ] Customisable right-click context menus (Next)
- [ ] Fully remappable keyboard shortcuts (MVP)
- [~] Custom view columns and sorting preferences (Next) — view and sort remembered per folder, lent to subfolders, or the default for everywhere; choosing columns next
- [ ] Custom app startup and restoration behaviour (Next)
- [~] Accessibility, keyboard navigation and scaling (MVP) — arrows/Enter/Backspace, shortcuts for everything so far

## 7. Developer tools & automation
- [ ] Git status badges beside files (Next)
- [ ] Current Git branch in path/navigation bar (Next)
- [ ] Git clone, init, fetch, pull, push and sync (Advanced)
- [ ] Git branch switching and repository operations (Advanced)
- [ ] Project type recognition / script discovery (Advanced)
- [ ] Run project scripts from an inspector (Advanced)
- [ ] Open folders in terminal (MVP)
- [ ] Open current folder or repo in preferred code editor (MVP)
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
- [ ] Private/local-first file indexing (MVP)
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
