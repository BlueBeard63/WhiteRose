.pragma library
// Dragging a bookmark up or down the sidebar to reorder it.

// Where a row dragged to `at` (y, in the list's coordinates) lands: its new
// index among `others` (the other rows, in order: [{ y, height }]). It goes
// before the first row whose middle is below the pointer.
function slot(others, at) {
    let i = 0;
    while (i < others.length && others[i].y + others[i].height / 2 < at) i++;
    return i;
}
