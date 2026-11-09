.pragma library
// Pure date logic for the calendar panel (tested in tests/run.mjs). Weeks
// start on Monday, as in the design.

function pad(n) { return n < 10 ? "0" + n : "" + n }

// "2026-11-09" for a local date.
function key(d) { return d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate()) }

// The 6 x 7 days shown for `month` (0-11) of `year`: from the Monday on or
// before the 1st. Each: { date, day, inMonth, today, weekend }.
function monthGrid(year, month, today) {
    const first = new Date(year, month, 1);
    const back = (first.getDay() + 6) % 7;   // days since Monday
    const cells = [];
    for (let i = 0; i < 42; i++) {
        const d = new Date(year, month, 1 - back + i);
        cells.push({
            date: d,
            day: d.getDate(),
            inMonth: d.getMonth() === month,
            today: key(d) === key(today),
            weekend: d.getDay() === 0 || d.getDay() === 6,
        });
    }
    return cells;
}

// The range to ask the daemon for: the first shown day and the day after the last.
function gridRange(year, month) {
    const cells = monthGrid(year, month, new Date(0));
    const last = cells[41].date;
    return { from: key(cells[0].date), to: key(new Date(last.getFullYear(), last.getMonth(), last.getDate() + 1)) };
}

// Day keys that have events (multi-day events mark every day they touch;
// an all-day event's end is exclusive).
function busyDays(events) {
    const days = {};
    for (const e of events) {
        const start = new Date(e.start), end = new Date(e.end);
        const d = new Date(start.getFullYear(), start.getMonth(), start.getDate());
        // A timed event ending exactly at midnight doesn't touch the next day.
        const last = end > start ? new Date(end.getTime() - 1) : start;
        for (let i = 0; d <= last && i < 366; i++) {
            days[key(d)] = true;
            d.setDate(d.getDate() + 1);
        }
    }
    return days;
}

// Events on one day (overlapping it at all), in start order.
function onDay(events, day) {
    const start = new Date(day.getFullYear(), day.getMonth(), day.getDate());
    const end = new Date(day.getFullYear(), day.getMonth(), day.getDate() + 1);
    return events.filter(e => new Date(e.start) < end && new Date(e.end) > start);
}

// "TODAY", "TOMORROW", "YESTERDAY", else "TUE 12 NOV".
function dayLabel(day, today) {
    const diff = Math.round((new Date(day.getFullYear(), day.getMonth(), day.getDate()) -
                             new Date(today.getFullYear(), today.getMonth(), today.getDate())) / 86400000);
    if (diff === 0) return "TODAY";
    if (diff === 1) return "TOMORROW";
    if (diff === -1) return "YESTERDAY";
    const days = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];
    const months = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"];
    return days[day.getDay()] + " " + day.getDate() + " " + months[day.getMonth()];
}

// "15:00 – 16:00", "All day", or for a timed event that started on an
// earlier day "until 10:00".
function timeRange(e, day) {
    if (e.all_day) return "All day";
    const s = new Date(e.start), en = new Date(e.end);
    const hm = d => pad(d.getHours()) + ":" + pad(d.getMinutes());
    const dayStart = new Date(day.getFullYear(), day.getMonth(), day.getDate());
    if (s < dayStart) return "until " + hm(en);
    return s.getTime() === en.getTime() ? hm(s) : hm(s) + " – " + hm(en);
}
