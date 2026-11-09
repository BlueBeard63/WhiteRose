//! Stable monitor IDs from EDID, identical to the daemon's
//! (daemon/internal/monitor): a monitor keeps its saved layout whichever port
//! or dock it's plugged into. Both sides check testdata/monitor-ids.json.
//!
//! - the one built-in panel (eDP/LVDS/DSI) is "builtin";
//! - otherwise "edid:" + sha256(identity fields)[..12], strong when the
//!   EDID has a real serial;
//! - identical monitors get "@<MST path or connector>" appended (weak);
//! - no EDID: "connector:<name>".

use sha2::{Digest, Sha256};

/// The identity fields of an EDID base block.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Edid {
    pub manufacturer: String,
    pub product: u16,
    pub serial: u32,
    pub week: u8,
    pub year: u16,
    pub name: String,
    pub serial_string: String,
}

/// Parses the base block (extensions and a bad checksum are ignored: the
/// identity fields are still usable). None if it isn't an EDID.
pub fn parse(data: &[u8]) -> Option<Edid> {
    let b = data.get(..128)?;
    if b[..8] != [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00] {
        return None;
    }
    let packed = u16::from(b[8]) << 8 | u16::from(b[9]);
    let manufacturer = [10u16, 5, 0]
        .iter()
        .map(|s| match (packed >> s) & 0x1F {
            c @ 1..=26 => (b'A' + c as u8 - 1) as char,
            _ => '?',
        })
        .collect();
    let mut e = Edid {
        manufacturer,
        product: u16::from(b[10]) | u16::from(b[11]) << 8,
        serial: u32::from_le_bytes([b[12], b[13], b[14], b[15]]),
        week: if b[16] == 0xFF { 0 } else { b[16] },
        year: if b[17] != 0 { 1990 + u16::from(b[17]) } else { 0 },
        ..Default::default()
    };
    for off in (54..=108).step_by(18) {
        let d = &b[off..off + 18];
        if d[0] != 0 || d[1] != 0 || d[2] != 0 {
            continue; // a detailed timing, not a descriptor
        }
        let text = descriptor_text(&d[5..]);
        match d[3] {
            0xFC => e.name = text,
            0xFF => e.serial_string = text,
            _ => {}
        }
    }
    Some(e)
}

fn descriptor_text(p: &[u8]) -> String {
    let p = p.iter().position(|&c| c == 0x0A).map_or(p, |i| &p[..i]);
    p.iter().filter(|&&c| (0x20..0x7F).contains(&c)).map(|&c| c as char).collect::<String>().trim().to_string()
}

/// "edid:" + the first 12 hex digits of sha256 over the identity fields.
/// Changing this changes every user's monitor IDs.
pub fn fingerprint(e: &Edid) -> String {
    let fields = [
        e.manufacturer.clone(),
        e.product.to_string(),
        e.serial.to_string(),
        e.serial_string.clone(),
        e.week.to_string(),
        e.year.to_string(),
        e.name.clone(),
    ]
    .join("\x1f");
    let sum = Sha256::digest(fields.as_bytes());
    let hex: String = sum.iter().take(6).map(|b| format!("{b:02x}")).collect();
    format!("edid:{hex}")
}

pub fn has_serial(e: &Edid) -> bool {
    !e.serial_string.is_empty() || !matches!(e.serial, 0 | 0x0101_0101 | 0xFFFF_FFFF)
}

/// A connected output as found on the hardware.
#[derive(Debug, Clone)]
pub struct Found {
    pub connector: String,
    /// The DRM PATH property for MST (docks, daisy chains), e.g. "mst:62-1-8".
    pub path: String,
    pub edid: Option<Edid>,
}

fn builtin(connector: &str) -> bool {
    ["eDP", "LVDS", "DSI"].iter().any(|p| connector.starts_with(p))
}

/// (connector, id, strong) for each output, sorted by connector.
pub fn identify(outputs: &[Found]) -> Vec<(String, String, bool)> {
    let mut outs = outputs.to_vec();
    outs.sort_by(|a, b| a.connector.cmp(&b.connector));
    let builtins = outs.iter().filter(|o| builtin(&o.connector)).count();
    let mut counts = std::collections::HashMap::<String, usize>::new();
    for o in &outs {
        if let Some(e) = &o.edid {
            *counts.entry(fingerprint(e)).or_default() += 1;
        }
    }
    outs.iter()
        .map(|o| {
            let (id, strong) = match &o.edid {
                _ if builtin(&o.connector) && builtins == 1 => ("builtin".to_string(), true),
                None => (format!("connector:{}", o.connector), false),
                Some(e) => {
                    let fp = fingerprint(e);
                    if counts[&fp] > 1 {
                        let at = if o.path.is_empty() { &o.connector } else { &o.path };
                        (format!("{fp}@{at}"), false)
                    } else {
                        (fp, has_serial(e))
                    }
                }
            };
            (o.connector.clone(), id, strong)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    #[test]
    fn matches_the_daemon_on_the_shared_fixture() {
        let text =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/monitor-ids.json")).unwrap();
        let cases: serde_json::Value = serde_json::from_str(&text).unwrap();
        for c in cases.as_array().unwrap() {
            let outs: Vec<Found> = c["outputs"]
                .as_array()
                .unwrap()
                .iter()
                .map(|o| Found {
                    connector: o["connector"].as_str().unwrap().into(),
                    path: o["path"].as_str().unwrap_or("").into(),
                    edid: o["edid"].as_str().and_then(|h| parse(&unhex(h))),
                })
                .collect();
            let got = identify(&outs);
            for (i, w) in c["want"].as_array().unwrap().iter().enumerate() {
                assert_eq!(
                    (got[i].0.as_str(), got[i].1.as_str(), got[i].2),
                    (w["connector"].as_str().unwrap(), w["id"].as_str().unwrap(), w["strong"].as_bool().unwrap()),
                    "{}",
                    c["name"]
                );
            }
        }
    }

    #[test]
    fn rejects_non_edid_data() {
        assert_eq!(parse(&[0u8; 128]), None);
        assert_eq!(parse(&[0x00, 0xFF]), None);
    }
}
