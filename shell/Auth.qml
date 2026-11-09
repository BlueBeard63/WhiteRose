// Password checking for the lock screen (PAM, config shell/pam/whiterose-lock
// or the installed /etc/pam.d/whiterose-lock). One conversation at a time;
// the password is handed to PAM and never kept.
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

    function tryUnlock(password) {
        if (pam.active || password === "") return;
        error = "";
        pending = password;
        pam.start();
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
                root.unlocking = true;
                ShellState.locked = false;
                root.unlocking = false;
            } else {
                root.error = result === PamResult.MaxTries ? "Too many attempts, wait a moment" : "Wrong password";
            }
        }
        onError: err => {
            root.pending = "";
            root.error = "Can't check the password (" + PamError.toString(err) + ")";
        }
    }
}
