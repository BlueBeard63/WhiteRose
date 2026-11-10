// Files on the clipboard the way other file managers put them there:
// text/uri-list, GNOME's x-special/gnome-copied-files (which says cut or
// copy) and plain paths for pasting into a terminal or editor.
#pragma once
#include "rust/cxx.h"

// `uris`: file:// URIs, one per line.
void setClipboardFiles(rust::Str uris, bool cut);
// "cut" or "copy", then one file:// URI per line ("" without files).
rust::String clipboardFiles();
// Plain text on the clipboard.
void setClipboardText(rust::Str text);
// Ctrl / Shift / Alt held right now (Qt::KeyboardModifiers), for a drop.
int keyboardModifiers();
// The application's link colour (#rrggbb): Qt's Markdown takes links'
// colour from it when it reads the text.
void setLinkColour(rust::Str colour);
