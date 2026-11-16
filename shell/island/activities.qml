// SUPER+SHIFT+A: the activity picker, shown while the compositor holds
// the keys (it owns them, so this doesn't take the keyboard).
import QtQuick
import qs

IslandView {
    keyboard: false
    ActivitiesView {}
}
