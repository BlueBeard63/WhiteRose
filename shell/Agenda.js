// The agenda (the Agenda module; the clock, SUPER+A): today's next event,
// how long until it, and the day as a line with its events on it.
// Events are whiterosed's calendar_events ({title, location, start, end,
// all_day, calendar, color}).
.pragma library

function pad(n) { return n < 10 ? "0" + n : "" + n }
function hm(d) { return pad(d.getHours()) + ":" + pad(d.getMinutes()) }

// Today's timed events (all-day ones aren't on the line), in start order.
function timed(events, now) {
    const day = new Date(now.getFullYear(), now.getMonth(), now.getDate());
    const next = new Date(day.getFullYear(), day.getMonth(), day.getDate() + 1);
    return (events ?? [])
        .filter(e => !e.all_day && new Date(e.start) < next && new Date(e.end) > day)
        .sort((a, b) => new Date(a.start) - new Date(b.start));
}

// The event on now, else the next one today: { event, now: bool,
// minutes: until it starts (0 if on) }, or null.
function next(events, now) {
    for (const e of timed(events, now)) {
        const s = new Date(e.start), en = new Date(e.end);
        if (en <= now) continue;
        if (s <= now) return { event: e, now: true, minutes: 0 };
        return { event: e, now: false, minutes: Math.ceil((s - now) / 60000) };
    }
    return null;
}

// "in 48 min", "in 2 h 5 min", "in 3 h", or "now · until 16:00".
function when(n) {
    if (!n) return "";
    if (n.now) return "now · until " + hm(new Date(n.event.end));
    const h = Math.floor(n.minutes / 60), m = n.minutes % 60;
    if (h === 0) return "in " + m + " min";
    return "in " + h + " h" + (m ? " " + m + " min" : "");
}

// The hours the line spans: 08:00–20:00, widened to whole hours to take in
// any of today's events outside that. { from, to } in hours.
function span(events, now) {
    let from = 8, to = 20;
    const day = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
    for (const e of timed(events, now)) {
        const s = Math.max(0, (new Date(e.start) - day) / 3600000);
        const en = Math.min(24, (new Date(e.end) - day) / 3600000);
        from = Math.min(from, Math.floor(s));
        to = Math.max(to, Math.ceil(en));
    }
    return { from, to };
}

// Where `t` (a Date today) is on a line spanning `sp`: 0 to 1.
function at(t, now, sp) {
    const day = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
    const h = (t - day) / 3600000;
    return Math.min(1, Math.max(0, (h - sp.from) / (sp.to - sp.from)));
}

// Today's events on the line: [{ x0, x1, title, next, past }] (0 to 1);
// `nextEvent` is next()'s event, lit.
function blocks(events, now, sp, nextEvent) {
    return timed(events, now).map(e => {
        const s = new Date(e.start), en = new Date(e.end);
        return {
            x0: at(s, now, sp), x1: at(en, now, sp), title: e.title,
            next: !!nextEvent && e.start === nextEvent.start && e.title === nextEvent.title,
            past: en <= now,
        };
    });
}
