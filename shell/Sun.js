// Sunrise and sunset, for the Auto theme (light between them). The standard
// sunrise equation (as in NOAA's and suncalc's), accurate to a minute or two
// away from the poles. Without a location, day is 07:00-19:00 local time.
.pragma library

const RAD = Math.PI / 180, J1970 = 2440588, J2000 = 2451545, DAY = 86400000;

function toDays(ms) { return ms / DAY - 0.5 + J1970 - J2000 }
function fromJulian(j) { return (j + 0.5 - J1970) * DAY }

// { rise, set } in ms for the day around `ms` at (lat, lon); rise/set are
// null with `polar` "night" or "day" when the sun doesn't cross the horizon.
function times(ms, lat, lon) {
    const lw = -lon * RAD, phi = lat * RAD;
    const n = Math.round(toDays(ms) - 0.0009 - lw / (2 * Math.PI));
    const ds = 0.0009 + lw / (2 * Math.PI) + n;
    const M = RAD * (357.5291 + 0.98560028 * ds);
    const C = RAD * (1.9148 * Math.sin(M) + 0.02 * Math.sin(2 * M) + 0.0003 * Math.sin(3 * M));
    const L = M + C + RAD * 102.9372 + Math.PI;
    const dec = Math.asin(Math.sin(L) * Math.sin(RAD * 23.4397));
    const transit = J2000 + ds + 0.0053 * Math.sin(M) - 0.0069 * Math.sin(2 * L);
    const cosw = (Math.sin(-0.833 * RAD) - Math.sin(phi) * Math.sin(dec)) / (Math.cos(phi) * Math.cos(dec));
    if (cosw > 1) return { rise: null, set: null, polar: "night" };
    if (cosw < -1) return { rise: null, set: null, polar: "day" };
    const w = Math.acos(cosw);
    const set = J2000 + 0.0009 + (w + lw) / (2 * Math.PI) + n + 0.0053 * Math.sin(M) - 0.0069 * Math.sin(2 * L);
    return { rise: fromJulian(transit - (set - transit)), set: fromJulian(set), polar: "" };
}

function hasPlace(lat, lon) { return typeof lat === "number" && typeof lon === "number" && isFinite(lat) && isFinite(lon) }

// Is it daytime at `ms`?
function isDay(ms, lat, lon) {
    if (!hasPlace(lat, lon)) {
        const h = new Date(ms).getHours();
        return h >= 7 && h < 19;
    }
    const t = times(ms, lat, lon);
    if (t.polar) return t.polar === "day";
    return ms >= t.rise && ms < t.set;
}

// Milliseconds until day turns to night or back (at least a minute, at most
// six hours, so a sleeping laptop or a moved location catches up).
function untilChange(ms, lat, lon) {
    const cap = 6 * 3600000, floor = 60000;
    let next = Infinity;
    if (!hasPlace(lat, lon)) {
        const d = new Date(ms);
        for (const day of [0, 1]) for (const h of [7, 19]) {
            const t = new Date(d.getFullYear(), d.getMonth(), d.getDate() + day, h).getTime();
            if (t > ms) next = Math.min(next, t);
        }
    } else {
        for (const day of [0, 1]) {
            const t = times(ms + day * DAY, lat, lon);
            for (const x of [t.rise, t.set]) if (x !== null && x > ms) next = Math.min(next, x);
        }
    }
    return Math.max(floor, Math.min(cap, next - ms + 1000));
}
