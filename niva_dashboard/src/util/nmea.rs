//! Minimal NMEA 0183 parser for the UM982 GNSS receiver output.
//!
//! Only the sentence types actually needed by the dashboard are handled: RMC/GGA/VTG/ZDA
//! for time, date, position, speed and altitude, plus HDT for the UM982's dual-antenna true
//! heading (distinct from RMC/VTG's course-over-ground, which only reflects the direction of
//! travel and is meaningless — or absent — while stationary).
//!
//! `GnssFix` is accumulated incrementally: a single sentence never carries every field, so
//! `update_from_sentence` only overwrites the fields present in whatever sentence just
//! arrived, leaving the rest at their last known value.

const KNOTS_TO_KMH: f32 = 1.852;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct UtcTime {
    pub hour: u8,
    pub minute: u8,
    pub second: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct UtcDate {
    pub day: u8,
    pub month: u8,
    pub year: u16,
}

/// GGA fix quality indicator. Variants beyond Invalid/Gps/DGps matter specifically for the
/// UM982 (an RTK-capable board) — RtkFixed vs RtkFloat is the difference between centimeter
/// and decimeter-ish accuracy, not a detail a plain GPS receiver would ever report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixQuality {
    Invalid,
    Gps,
    DGps,
    PpsFix,
    RtkFixed,
    RtkFloat,
    Estimated,
    Manual,
    Simulation,
    Unknown(u8),
}

impl FixQuality {
    fn from_code(code: u8) -> Self {
        match code {
            0 => Self::Invalid,
            1 => Self::Gps,
            2 => Self::DGps,
            3 => Self::PpsFix,
            4 => Self::RtkFixed,
            5 => Self::RtkFloat,
            6 => Self::Estimated,
            7 => Self::Manual,
            8 => Self::Simulation,
            other => Self::Unknown(other),
        }
    }

    /// Inverse of `from_code` — the raw GGA quality digit. Used by GnssChannelProvider to
    /// pass the fix quality through the u16 HWAnalogProvider boundary (see hw_providers.rs).
    pub fn code(&self) -> u8 {
        match self {
            Self::Invalid => 0,
            Self::Gps => 1,
            Self::DGps => 2,
            Self::PpsFix => 3,
            Self::RtkFixed => 4,
            Self::RtkFloat => 5,
            Self::Estimated => 6,
            Self::Manual => 7,
            Self::Simulation => 8,
            Self::Unknown(code) => *code,
        }
    }
}

/// Accumulated GNSS state, built up across whatever mix of sentences the receiver emits per
/// update cycle. Every field is independently `Option` — there is no single sentence that
/// fills all of them, and fields simply stay at their last known value between updates.
#[derive(Debug, Clone, Copy, Default)]
pub struct GnssFix {
    pub time: Option<UtcTime>,
    pub date: Option<UtcDate>,
    pub latitude_deg: Option<f64>,
    pub longitude_deg: Option<f64>,
    pub altitude_m: Option<f32>,
    pub speed_kmh: Option<f32>,
    /// Course over ground — direction of travel, from RMC/VTG. Only meaningful while moving.
    pub course_deg: Option<f32>,
    /// True heading — the UM982's dual-antenna orientation, from HDT. Valid while
    /// stationary, unlike course_deg, but only present when the heading fix (moving
    /// baseline between the two antennas) is resolved.
    pub heading_deg: Option<f32>,
    pub fix_quality: Option<FixQuality>,
    pub satellites: Option<u8>,
    pub hdop: Option<f32>,
    /// RMC's own "is this an active fix" flag (status field: 'A'/'V'). Distinct from
    /// fix_quality, which comes from GGA and not every receiver/configuration emits both.
    pub status_active: Option<bool>,
}

/// Parses one NMEA line and merges any fields it carries into `fix`. Returns true if the
/// sentence had a valid checksum and a recognized type (whether or not that particular
/// sentence happened to update anything). Malformed lines and unsupported sentence types
/// (GSA/GSV/GST/...) are silently ignored — this receiver emits far more sentence types
/// than the dashboard needs.
pub fn update_from_sentence(fix: &mut GnssFix, sentence: &str) -> bool {
    let sentence = sentence.trim();
    if !checksum_valid(sentence) {
        return false;
    }

    // Safe to index: checksum_valid already confirmed '$' at the start and '*' later on.
    let star_pos = sentence.find('*').unwrap();
    let body = &sentence[1..star_pos];
    let fields: Vec<&str> = body.split(',').collect();

    let Some(id) = fields.first() else { return false };
    if id.len() < 3 {
        return false;
    }
    // Standard 2-letter talker ID (GP/GN/GL/GA/GB/GQ/GI/...) precedes the 3-letter type.
    let sentence_type = &id[2..];

    match sentence_type {
        "RMC" => { parse_rmc(&fields, fix); true }
        "GGA" => { parse_gga(&fields, fix); true }
        "VTG" => { parse_vtg(&fields, fix); true }
        "HDT" => { parse_hdt(&fields, fix); true }
        "ZDA" => { parse_zda(&fields, fix); true }
        _ => false,
    }
}

fn checksum_valid(sentence: &str) -> bool {
    if !sentence.starts_with('$') {
        return false;
    }
    let Some(star_pos) = sentence.find('*') else { return false };
    let checksum_hex = &sentence[star_pos + 1..];
    if checksum_hex.len() < 2 {
        return false;
    }
    let Ok(expected) = u8::from_str_radix(&checksum_hex[..2], 16) else { return false };
    let computed = sentence.as_bytes()[1..star_pos].iter().fold(0u8, |acc, &b| acc ^ b);
    computed == expected
}

fn parse_time(raw: &str) -> Option<UtcTime> {
    if raw.len() < 6 {
        return None;
    }
    Some(UtcTime {
        hour: raw[0..2].parse().ok()?,
        minute: raw[2..4].parse().ok()?,
        second: raw[4..].parse().ok()?,
    })
}

fn parse_date_ddmmyy(raw: &str) -> Option<UtcDate> {
    if raw.len() != 6 {
        return None;
    }
    let yy: u16 = raw[4..6].parse().ok()?;
    Some(UtcDate {
        day: raw[0..2].parse().ok()?,
        month: raw[2..4].parse().ok()?,
        year: 2000 + yy,
    })
}

/// Converts NMEA's `ddmm.mmmm` / `dddmm.mmmm` position format to unsigned decimal degrees.
/// The two-vs-three-digit degree prefix doesn't matter here since it's parsed as a plain
/// number, not by fixed string width — dividing by 100 splits degrees from minutes either way.
fn dm_to_decimal(raw: &str) -> Option<f64> {
    if raw.is_empty() {
        return None;
    }
    let val: f64 = raw.parse().ok()?;
    let degrees = (val / 100.0).floor();
    let minutes = val - degrees * 100.0;
    Some(degrees + minutes / 60.0)
}

fn parse_latitude(raw: &str, hemisphere: &str) -> Option<f64> {
    let lat = dm_to_decimal(raw)?;
    Some(if hemisphere == "S" { -lat } else { lat })
}

fn parse_longitude(raw: &str, hemisphere: &str) -> Option<f64> {
    let lon = dm_to_decimal(raw)?;
    Some(if hemisphere == "W" { -lon } else { lon })
}

fn parse_rmc(fields: &[&str], fix: &mut GnssFix) {
    if let Some(t) = fields.get(1).and_then(|s| parse_time(s)) {
        fix.time = Some(t);
    }
    if let Some(status) = fields.get(2) {
        fix.status_active = Some(*status == "A");
    }
    if let (Some(lat), Some(ns)) = (fields.get(3), fields.get(4)) {
        if let Some(v) = parse_latitude(lat, ns) {
            fix.latitude_deg = Some(v);
        }
    }
    if let (Some(lon), Some(ew)) = (fields.get(5), fields.get(6)) {
        if let Some(v) = parse_longitude(lon, ew) {
            fix.longitude_deg = Some(v);
        }
    }
    if let Some(speed_knots) = fields.get(7).and_then(|s| s.parse::<f32>().ok()) {
        fix.speed_kmh = Some(speed_knots * KNOTS_TO_KMH);
    }
    if let Some(course) = fields.get(8).and_then(|s| s.parse::<f32>().ok()) {
        fix.course_deg = Some(course);
    }
    if let Some(date) = fields.get(9).and_then(|s| parse_date_ddmmyy(s)) {
        fix.date = Some(date);
    }
}

fn parse_gga(fields: &[&str], fix: &mut GnssFix) {
    if let Some(t) = fields.get(1).and_then(|s| parse_time(s)) {
        fix.time = Some(t);
    }
    if let (Some(lat), Some(ns)) = (fields.get(2), fields.get(3)) {
        if let Some(v) = parse_latitude(lat, ns) {
            fix.latitude_deg = Some(v);
        }
    }
    if let (Some(lon), Some(ew)) = (fields.get(4), fields.get(5)) {
        if let Some(v) = parse_longitude(lon, ew) {
            fix.longitude_deg = Some(v);
        }
    }
    if let Some(q) = fields.get(6).and_then(|s| s.parse::<u8>().ok()) {
        fix.fix_quality = Some(FixQuality::from_code(q));
    }
    if let Some(sats) = fields.get(7).and_then(|s| s.parse::<u8>().ok()) {
        fix.satellites = Some(sats);
    }
    if let Some(hdop) = fields.get(8).and_then(|s| s.parse::<f32>().ok()) {
        fix.hdop = Some(hdop);
    }
    if let Some(alt) = fields.get(9).and_then(|s| s.parse::<f32>().ok()) {
        fix.altitude_m = Some(alt);
    }
}

fn parse_vtg(fields: &[&str], fix: &mut GnssFix) {
    if let Some(course) = fields.get(1).and_then(|s| s.parse::<f32>().ok()) {
        fix.course_deg = Some(course);
    }
    // Prefer the km/h field (7) directly; fall back to converting the knots field (5) if
    // it's missing, which some receiver configurations omit.
    if let Some(speed_kmh) = fields.get(7).and_then(|s| s.parse::<f32>().ok()) {
        fix.speed_kmh = Some(speed_kmh);
    } else if let Some(speed_knots) = fields.get(5).and_then(|s| s.parse::<f32>().ok()) {
        fix.speed_kmh = Some(speed_knots * KNOTS_TO_KMH);
    }
}

fn parse_hdt(fields: &[&str], fix: &mut GnssFix) {
    if let Some(heading) = fields.get(1).and_then(|s| s.parse::<f32>().ok()) {
        fix.heading_deg = Some(heading);
    }
}

fn parse_zda(fields: &[&str], fix: &mut GnssFix) {
    if let Some(t) = fields.get(1).and_then(|s| parse_time(s)) {
        fix.time = Some(t);
    }
    if let (Some(day), Some(month), Some(year)) = (
        fields.get(2).and_then(|s| s.parse::<u8>().ok()),
        fields.get(3).and_then(|s| s.parse::<u8>().ok()),
        fields.get(4).and_then(|s| s.parse::<u16>().ok()),
    ) {
        fix.date = Some(UtcDate { day, month, year });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_checksum(body: &str) -> String {
        let checksum = body.bytes().fold(0u8, |acc, b| acc ^ b);
        format!("${}*{:02X}", body, checksum)
    }

    #[test]
    fn rejects_bad_checksum() {
        let mut fix = GnssFix::default();
        assert!(!update_from_sentence(&mut fix, "$GNRMC,154849.00,V,,,,,,,290726,0.0,E,N,V*00"));
    }

    #[test]
    fn parses_real_captured_rmc_with_no_fix() {
        // Captured from actual hardware immediately after cold start, before acquiring a fix.
        let mut fix = GnssFix::default();
        assert!(update_from_sentence(&mut fix, "$GNRMC,154849.00,V,,,,,,,290726,0.0,E,N,V*7F"));

        assert_eq!(fix.time, Some(UtcTime { hour: 15, minute: 48, second: 49.0 }));
        assert_eq!(fix.date, Some(UtcDate { day: 29, month: 7, year: 2026 }));
        assert_eq!(fix.status_active, Some(false));
        assert_eq!(fix.latitude_deg, None);
        assert_eq!(fix.longitude_deg, None);
        assert_eq!(fix.speed_kmh, None);
        assert_eq!(fix.course_deg, None);
    }

    #[test]
    fn parses_rmc_with_valid_fix() {
        let mut fix = GnssFix::default();
        let line = with_checksum("GNRMC,120000.00,A,5530.1234,N,03730.5678,E,10.5,45.0,290726,,,A");
        assert!(update_from_sentence(&mut fix, &line));

        assert_eq!(fix.status_active, Some(true));
        let lat = fix.latitude_deg.unwrap();
        let lon = fix.longitude_deg.unwrap();
        assert!((lat - 55.502056).abs() < 1e-5);
        assert!((lon - 37.509463).abs() < 1e-5);
        let speed = fix.speed_kmh.unwrap();
        assert!((speed - 10.5 * KNOTS_TO_KMH).abs() < 1e-4);
        assert_eq!(fix.course_deg, Some(45.0));
    }

    #[test]
    fn parses_gga_position_altitude_and_fix_quality() {
        let mut fix = GnssFix::default();
        let line = with_checksum("GNGGA,120000.00,5530.1234,N,03730.5678,W,4,12,0.9,150.2,M,13.1,M,,");
        assert!(update_from_sentence(&mut fix, &line));

        assert!(fix.latitude_deg.unwrap() > 0.0);
        assert!(fix.longitude_deg.unwrap() < 0.0); // W hemisphere -> negative
        assert_eq!(fix.fix_quality, Some(FixQuality::RtkFixed));
        assert_eq!(fix.satellites, Some(12));
        assert_eq!(fix.hdop, Some(0.9));
        assert_eq!(fix.altitude_m, Some(150.2));
    }

    #[test]
    fn parses_vtg_speed_and_course() {
        let mut fix = GnssFix::default();
        let line = with_checksum("GNVTG,90.0,T,,M,5.0,N,9.26,K,A");
        assert!(update_from_sentence(&mut fix, &line));

        assert_eq!(fix.course_deg, Some(90.0));
        assert_eq!(fix.speed_kmh, Some(9.26));
    }

    #[test]
    fn parses_hdt_dual_antenna_heading() {
        let mut fix = GnssFix::default();
        let line = with_checksum("GNHDT,123.4,T");
        assert!(update_from_sentence(&mut fix, &line));

        assert_eq!(fix.heading_deg, Some(123.4));
    }

    #[test]
    fn parses_zda_full_date_time() {
        let mut fix = GnssFix::default();
        let line = with_checksum("GPZDA,120000.00,29,07,2026,00,00");
        assert!(update_from_sentence(&mut fix, &line));

        assert_eq!(fix.time, Some(UtcTime { hour: 12, minute: 0, second: 0.0 }));
        assert_eq!(fix.date, Some(UtcDate { day: 29, month: 7, year: 2026 }));
    }

    #[test]
    fn fix_quality_code_round_trips() {
        // Known codes (0-8) round-trip exactly; anything above the known range is preserved
        // as-is via the Unknown variant rather than clamped or misreported.
        for code in 0..=8u8 {
            assert_eq!(FixQuality::from_code(code).code(), code);
        }
        assert_eq!(FixQuality::from_code(42).code(), 42);
    }

    #[test]
    fn ignores_unsupported_sentence_types() {
        let mut fix = GnssFix::default();
        let line = with_checksum("GPGSV,3,1,11,10,63,137,17,12,08,046,,");
        assert!(!update_from_sentence(&mut fix, &line));
    }

    #[test]
    fn fields_persist_across_sentences() {
        // A heading fix from HDT should survive a later RMC that doesn't mention heading —
        // GnssFix accumulates rather than being overwritten wholesale per sentence.
        let mut fix = GnssFix::default();
        update_from_sentence(&mut fix, &with_checksum("GNHDT,45.0,T"));
        update_from_sentence(&mut fix, "$GNRMC,154849.00,V,,,,,,,290726,0.0,E,N,V*7F");

        assert_eq!(fix.heading_deg, Some(45.0));
        assert_eq!(fix.time, Some(UtcTime { hour: 15, minute: 48, second: 49.0 }));
    }
}
