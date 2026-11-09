//! Sunrise and sunset for the Auto theme, the same calculation as the
//! shell's Sun.js, so apps turn light and dark at the same moment as the
//! desktop. Times are unix milliseconds.

const RAD: f64 = std::f64::consts::PI / 180.0;
const J1970: f64 = 2440588.0;
const J2000: f64 = 2451545.0;
const DAY: f64 = 86_400_000.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sun {
    Times { rise: f64, set: f64 },
    PolarDay,
    PolarNight,
}

pub fn times(ms: f64, lat: f64, lon: f64) -> Sun {
    let lw = -lon * RAD;
    let phi = lat * RAD;
    let days = ms / DAY - 0.5 + J1970 - J2000;
    let n = (days - 0.0009 - lw / (2.0 * std::f64::consts::PI)).round();
    let ds = 0.0009 + lw / (2.0 * std::f64::consts::PI) + n;
    let m = RAD * (357.5291 + 0.98560028 * ds);
    let c = RAD * (1.9148 * m.sin() + 0.02 * (2.0 * m).sin() + 0.0003 * (3.0 * m).sin());
    let l = m + c + RAD * 102.9372 + std::f64::consts::PI;
    let dec = (l.sin() * (RAD * 23.4397).sin()).asin();
    let transit = J2000 + ds + 0.0053 * m.sin() - 0.0069 * (2.0 * l).sin();
    let cosw = ((-0.833 * RAD).sin() - phi.sin() * dec.sin()) / (phi.cos() * dec.cos());
    if cosw > 1.0 {
        return Sun::PolarNight;
    }
    if cosw < -1.0 {
        return Sun::PolarDay;
    }
    let w = cosw.acos();
    let set = J2000 + 0.0009 + (w + lw) / (2.0 * std::f64::consts::PI) + n + 0.0053 * m.sin()
        - 0.0069 * (2.0 * l).sin();
    let from = |j: f64| (j + 0.5 - J1970) * DAY;
    Sun::Times {
        rise: from(transit - (set - transit)),
        set: from(set),
    }
}

/// Daytime at `ms`? Without a place, 07:00-19:00 at `local_hour`.
pub fn is_day(ms: f64, place: Option<(f64, f64)>, local_hour: u32) -> bool {
    match place {
        None => (7..19).contains(&local_hour),
        Some((lat, lon)) => match times(ms, lat, lon) {
            Sun::PolarDay => true,
            Sun::PolarNight => false,
            Sun::Times { rise, set } => ms >= rise && ms < set,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-06-21 12:00 UTC and 2026-12-21 12:00 UTC.
    const JUNE: f64 = 1_782_043_200_000.0;
    const DEC: f64 = 1_797_854_400_000.0;
    const MIN: f64 = 60_000.0;

    #[test]
    fn london_matches_the_shell() {
        let Sun::Times { rise, set } = times(JUNE, 51.5074, -0.1278) else {
            panic!()
        };
        // 03:43 and 20:21 UTC.
        assert!(
            (rise - (JUNE - (8.0 * 60.0 + 17.0) * MIN)).abs() < 3.0 * MIN,
            "{rise}"
        );
        assert!(
            (set - (JUNE + (8.0 * 60.0 + 21.0) * MIN)).abs() < 3.0 * MIN,
            "{set}"
        );
        let Sun::Times { rise, set } = times(DEC, 51.5074, -0.1278) else {
            panic!()
        };
        // 08:04 and 15:53 UTC.
        assert!((rise - (DEC - (3.0 * 60.0 + 56.0) * MIN)).abs() < 3.0 * MIN);
        assert!((set - (DEC + (3.0 * 60.0 + 53.0) * MIN)).abs() < 3.0 * MIN);
    }

    #[test]
    fn day_night_and_poles() {
        assert!(is_day(JUNE, Some((51.5, -0.13)), 0));
        assert!(!is_day(JUNE + 11.0 * 60.0 * MIN, Some((51.5, -0.13)), 0));
        assert_eq!(times(JUNE, 78.2, 15.6), Sun::PolarDay);
        assert!(!is_day(DEC, Some((78.2, 15.6)), 12));
        assert!(is_day(0.0, None, 12));
        assert!(!is_day(0.0, None, 22));
    }
}
