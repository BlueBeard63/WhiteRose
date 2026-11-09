// The lock screen. ShellState.lock() (Quick Settings, SUPER+L, `qs ipc call
// session lock`, logind's lock-session) sets ShellState.locked; the
// compositor then shows only these surfaces until the password is right.
// If the shell restarts while locked, the compositor says so on connect and
// this locks again straight away.
import QtQuick
import Quickshell
import Quickshell.Wayland

Scope {
    id: root

    WlSessionLock {
        id: lock
        locked: ShellState.locked
        // The compositor refused or ended the lock: don't leave the shell
        // thinking it's locked when it isn't.
        onLockedChanged: if (!locked && ShellState.locked && !Auth.unlocking) ShellState.locked = false

        WlSessionLockSurface {
            color: Theme.bg
            LockView {}
        }
    }
}
