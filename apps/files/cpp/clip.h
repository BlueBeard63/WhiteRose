// Files on the clipboard the way other file managers put them there:
// text/uri-list, GNOME's x-special/gnome-copied-files (which says cut or
// copy) and plain paths for pasting into a terminal or editor.
#pragma once
#include "rust/cxx.h"

// `uris`: file:// URIs, one per line.
void setClipboardFiles(rust::Str uris, bool cut);
// "cut" or "copy", then one file:// URI per line ("" without files).
rust::String clipboardFiles();
// Ctrl / Shift / Alt held right now (Qt::KeyboardModifiers), for a drop.
int keyboardModifiers();
