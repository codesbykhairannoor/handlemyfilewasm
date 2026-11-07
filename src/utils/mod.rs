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

/// Algorithmic Roman numeral validator and parser (supports standard subtractive notation 1..3999)
pub fn parse_roman_numeral(s: &str) -> Option<u32> {
    let s = s.trim().to_uppercase();
    if s.is_empty() || s.len() > 15 {
        return None;
    }

    let mut total = 0u32;
    let mut prev_val = 0u32;

    for c in s.chars().rev() {
        let val = match c {
            'I' => 1,
            'V' => 5,
            'X' => 10,
            'L' => 50,
            'C' => 100,
            'D' => 500,
            'M' => 1000,
            _ => return None,
        };

        if val < prev_val {
            total = total.checked_sub(val)?;
        } else {
            total = total.checked_add(val)?;
            prev_val = val;
        }
    }

    if total == 0 || total > 3999 {
        return None;
    }

    // Verify canonical representation
    if to_roman_numeral(total) == s {
        Some(total)
    } else {
        None
    }
}

pub fn to_roman_numeral(mut num: u32) -> String {
    let table = [
        (1000, "M"), (900, "CM"), (500, "D"), (400, "CD"),
        (100, "C"), (90, "XC"), (50, "L"), (40, "XL"),
        (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I"),
    ];
    let mut result = String::new();
    for &(val, sym) in &table {
        while num >= val {
            result.push_str(sym);
            num -= val;
        }
    }
    result
}

pub fn is_roman_numeral(s: &str) -> bool {
    parse_roman_numeral(s).is_some()
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

    #[test]
    fn test_roman_numerals() {
        assert!(is_roman_numeral("i"));
        assert!(is_roman_numeral("IV"));
        assert!(is_roman_numeral("xiv"));
        assert!(is_roman_numeral("MCMLXXXIV"));
        assert!(!is_roman_numeral("IIII")); // Invalid canonical
        assert!(!is_roman_numeral("abc"));
        assert!(!is_roman_numeral("123"));
    }
}

