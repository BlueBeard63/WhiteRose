// Unlocking the lock screen (PAM, configs in shell/pam, installed in
// /etc/pam.d). Two conversations side by side, as either one unlocks:
//   - the password (whiterose-lock), one try at a time, never kept;
//   - a fingerprint (whiterose-fingerprint, fprintd), listening while the
//     lock screen shows. With no reader or no enrolled finger it fails at
//     once and isn't offered. It gives up after 30 s or three misses (the
//     reader draws power while it listens); any key or click starts it again.
pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Services.Pam

Singleton {
    id: root

    property bool busy: pam.active
    property string error: ""
    // True between a correct password and the lock going away.
    property bool unlocking: false
    property string pending: ""

    // ---- fingerprint ----------------------------------------------------------------------
    // Seen a reader with a finger enrolled (since the shell started).
    property bool fingerAvailable: false
    readonly property bool fingerListening: finger.active && fingerAvailable
    // What the reader says ("Place your finger…", "No match, try again").
    property string fingerMessage: ""
    property bool fingerError: false
    // The reader spoke this round (so a failure is a timeout or a miss, not
    // "no reader").
    property bool fingerHeard: false

    function tryUnlock(password) {
        if (pam.active || password === "") return;
        error = "";
        pending = password;
        pam.start();
    }

    // The lock screen is showing (or you touched it): listen for a finger.
    function wake() {
        if (!ShellState.locked || finger.active || unlocking) return;
        fingerMessage = "";
        fingerError = false;
        fingerHeard = false;
        finger.start();
    }

    function unlocked() {
        if (pam.active) pam.abort();
        if (finger.active) finger.abort();
        root.unlocking = true;
        ShellState.locked = false;
        root.unlocking = false;
    }

    function stopped(tooMany) {
        if (!fingerHeard) fingerAvailable = false;
        fingerMessage = tooMany ? "Too many tries. Use your password." : "";
        fingerError = tooMany;
    }

    Connections {
        target: ShellState
        function onLockedChanged() {
            if (ShellState.locked) root.wake();
            else if (finger.active) finger.abort();
        }
    }

    PamContext {
        id: pam
        config: "whiterose-lock"
        configDirectory: Quickshell.shellDir + "/pam"
        onResponseRequiredChanged: {
            if (!responseRequired) return;
            respond(root.pending);
            root.pending = "";
        }
        onCompleted: result => {
            root.pending = "";
            if (result === PamResult.Success) {
                root.unlocked();
            } else {
                root.error = result === PamResult.MaxTries ? "Too many tries. Wait a moment and try again." : "Wrong password";
            }
        }
        onError: err => {
            root.pending = "";
            root.error = "Can't check the password (" + PamError.toString(err) + ")";
        }
    }

    PamContext {
        id: finger
        config: "whiterose-fingerprint"
        configDirectory: Quickshell.shellDir + "/pam"
        // pam_fprintd only talks (it never needs typing): "Place your …
        // finger on …" when it starts listening, "Failed to match" on a miss.
        onMessageChanged: {
            if (message === "") return;
            root.fingerHeard = root.fingerAvailable = true;
            if (/timed out/i.test(message)) {
                root.fingerMessage = "";
                return;
            }
            root.fingerError = messageIsError || /fail|no match|not match|try again/i.test(message);
            root.fingerMessage = root.fingerError ? "Not recognised. Try again." : "Or touch the fingerprint reader";
        }
        onResponseRequiredChanged: if (responseRequired) respond("")
        onCompleted: result => {
            if (result === PamResult.Success) {
                root.unlocked();
                return;
            }
            // No reader / nothing enrolled: not offered. Timed out or three
            // misses: stopped until you touch the screen.
            root.stopped(result === PamResult.MaxTries);
        }
        // (A timeout comes as an error too.)
        onError: () => root.stopped(false)
    }
}
