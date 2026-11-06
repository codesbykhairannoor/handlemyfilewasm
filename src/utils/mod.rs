/// Mathematical Unit Conversions between PDF (ISO 32000) and WordprocessingML (ISO/IEC 29500)

/// Points to Twips: 1 pt = 20 twips (dxa)
pub fn points_to_twips(points: f64) -> i32 {
    (points * 20.0).round() as i32
}

/// Points to Half-Points (used for w:sz font sizes): 1 pt = 2 half-points
pub fn points_to_half_points(points: f64) -> i32 {
    (points * 2.0).round() as i32
}

/// Points to Eighth-Points (used for table/paragraph border line sizes): 1 pt = 8 eighth-points
pub fn points_to_eighth_points(points: f64) -> i32 {
    (points * 8.0).round() as i32
}

/// Points to EMUs (English Metric Units for DrawingML): 1 pt = 12,700 EMUs
pub fn points_to_emus(points: f64) -> i64 {
    (points * 12_700.0).round() as i64
}

/// Safely escape XML characters: &, <, >, ", '
pub fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            other => {
                // Filter out non-printable ASCII control characters except tab, LF, CR
                if (other as u32) >= 0x20 || other == '\t' || other == '\n' || other == '\r' {
                    out.push(other);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unit_conversions() {
        assert_eq!(points_to_twips(36.0), 720); // 0.5 inch margin
        assert_eq!(points_to_half_points(12.0), 24); // 12 pt font
        assert_eq!(points_to_eighth_points(0.5), 4); // 0.5 pt border
        assert_eq!(points_to_emus(100.0), 1_270_000); // 100 pt image
    }

    #[test]
    fn test_xml_escaping() {
        assert_eq!(escape_xml("Tom & Jerry <10>"), "Tom &amp; Jerry &lt;10&gt;");
    }
}

