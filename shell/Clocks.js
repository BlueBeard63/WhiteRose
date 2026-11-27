.pragma library
// Shared presentation maths for the app and the native-size idle island.
function pad(n) { return String(Math.floor(n)).padStart(2, "0"); }
function duration(ms, hundredths) {
    var seconds = Math.max(0, hundredths ? Math.floor(ms / 1000) : Math.ceil(ms / 1000));
    var hours = Math.floor(seconds / 3600), minutes = Math.floor(seconds / 60) % 60;
    var value = (hours || hundredths ? pad(hours) + ":" : "") + pad(minutes) + ":" + pad(seconds % 60);
    return value + (hundredths ? "." + pad(Math.floor(Math.max(0, ms) / 10) % 100) : "");
}
function remaining(timer, now) {
    return timer.running ? Math.max(0, timer.deadline - now) : Math.max(0, timer.remaining);
}
function elapsed(sw, now) {
    return Math.max(0, sw.elapsed || 0) + (sw.running ? Math.max(0, now - sw.started) : 0);
}
function alarmTime(alarm) { return pad(alarm.hour) + ":" + pad(alarm.minute); }
function alarmNext(alarm) {
    return alarm.snooze > 0 && (!alarm.enabled || alarm.snooze < alarm.next) ? alarm.snooze : alarm.enabled ? alarm.next : 0;
}
function repeat(days) {
    if (!days || !days.length) return "Once";
    var names = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    return days.map(function(d) { return names[d]; }).join(", ");
}
function offset(seconds) {
    var absolute = Math.abs(seconds), hours = Math.floor(absolute / 3600), minutes = Math.floor(absolute / 60) % 60;
    return (hours + (minutes ? " h " + minutes + " min" : hours === 1 ? " hour" : " hours")) + (seconds < 0 ? " behind" : " ahead");
}
function zoneTime(now, offset) { var d = new Date(now + offset * 1000); return pad(d.getUTCHours()) + ":" + pad(d.getUTCMinutes()); }
function zoneDate(now, offset) {
    var d = new Date(now + offset * 1000);
    return ["Sun","Mon","Tue","Wed","Thu","Fri","Sat"][d.getUTCDay()] + " " + d.getUTCDate() + " " + ["Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"][d.getUTCMonth()];
}
function idle(state, now) {
    if (!state) return null;
    var alerts = state.alerts || [];
    if (alerts.length) {
        var a = alerts[0];
        return { view: a.kind === "alarm" ? "alarms" : "timers", icon: a.kind === "alarm" ? "bell" : "timer", label: a.name, value: a.kind === "alarm" ? a.silenced ? "Silent" : "Alarm" : "Done", alert: true };
    }
    var running = (state.timers || []).filter(function(t) { return t.running; }).sort(function(a,b) { return a.deadline - b.deadline; });
    if (running.length) {
        var first = running[0];
        return { view:"timers", icon:"timer", label:first.name, value:duration(remaining(first,now)), extra:running.length>1 ? "+"+(running.length-1) : "" };
    }
    if (state.stopwatch && state.stopwatch.running)
        return { view:"stopwatch", icon:"timer", label:"Stopwatch", value:duration(elapsed(state.stopwatch,now)) };
    var paused = (state.timers || []).filter(function(t) { return !t.running && t.remaining > 0 && t.remaining < t.duration; });
    if (paused.length) return { view:"timers", icon:"pause", label:paused[0].name, value:duration(paused[0].remaining) };
    var alarms = (state.alarms || []).filter(function(a) { return alarmNext(a)>0; }).sort(function(a,b) { return alarmNext(a)-alarmNext(b); });
    if (alarms.length) {
        var alarm = alarms[0], minutes = Math.ceil(Math.max(0,alarmNext(alarm)-now)/60000);
        return { view:"alarms", icon:"bell", label:alarmTime(alarm), value:Math.floor(minutes/60)+"h"+pad(minutes%60)+"m" };
    }
    return null;
}
