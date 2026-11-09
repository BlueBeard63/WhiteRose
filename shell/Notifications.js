.pragma library
// Pure rules for notifications (tested in tests/run.mjs).

const LOW = 0, NORMAL = 1, CRITICAL = 2;

// "now", "2m", "1h", "3d" for a notification `seconds` old.
function ago(seconds) {
    const s = Math.max(0, Math.floor(seconds));
    if (s < 60) return "now";
    if (s < 3600) return Math.floor(s / 60) + "m";
    if (s < 86400) return Math.floor(s / 3600) + "h";
    return Math.floor(s / 86400) + "d";
}

// Whether a new notification pops up. Not while the panel is open (it's
// listed there already); under Do Not Disturb only critical ones do.
function shouldPopup(urgency, doNotDisturb, panelOpen) {
    if (panelOpen) return false;
    return !doNotDisturb || urgency === CRITICAL;
}

// How long the popup stays, in ms; 0 means until dismissed. Critical ones
// stay. Otherwise the app's own timeout (D-Bus: -1 = server default, 0 =
// never), kept between 3 and 30 s so nothing flashes past or lingers forever.
function popupTimeout(urgency, expireTimeout) {
    if (urgency === CRITICAL) return 0;
    if (expireTimeout === undefined || expireTimeout < 0 || expireTimeout === 0) return 5000;
    return Math.max(3000, Math.min(30000, expireTimeout));
}

// The buttons to show: every action except "default" (that's clicking the
// notification itself), in the order the app sent them.
function buttons(actions) {
    return (actions || []).filter(a => a.identifier !== "default");
}

function hasDefault(actions) {
    return (actions || []).some(a => a.identifier === "default");
}

// The theme icon name behind an image URL QuickShell made from an icon name
// ("image://icon/battery-caution" -> "battery-caution"), else "". Such names
// must be checked against the theme: a missing one renders as a checkerboard.
function themedImageName(image) {
    const m = /^image:\/\/icon\/([^?]+)/.exec(String(image ?? ""));
    return m ? decodeURIComponent(m[1]) : "";
}

// What an app icon string is: a URL to load as is, a file path, or an icon
// theme name; "" when there's nothing.
function iconKind(appIcon) {
    const s = String(appIcon ?? "").trim();
    if (s === "") return { kind: "none", value: "" };
    if (/^(file|image|qrc|https?):/.test(s)) return { kind: "url", value: s };
    if (s.startsWith("/")) return { kind: "url", value: "file://" + s };
    return { kind: "name", value: s };
}
