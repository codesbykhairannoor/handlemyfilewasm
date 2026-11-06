use std::collections::BTreeMap;
use lopdf::{Document, Object, ObjectId};

/// Resolved Font Information and Glyph Metrics
#[derive(Debug, Clone, PartialEq)]
pub struct FontInfo {
    pub base_font: String,                 // Sanitized font family name, e.g. "Arial"
    pub is_bold: bool,
    pub is_italic: bool,
    pub is_multibyte: bool,                // 16-bit CIDFont or multibyte encoding
    pub first_char: u32,
    pub last_char: u32,
    pub default_width: f64,
    pub widths: BTreeMap<u32, f64>,        // CharCode -> Width in 1/1000 em
    pub to_unicode: BTreeMap<u32, String>, // CharCode -> UTF-8 Decoded String
    pub differences: BTreeMap<u32, String>,// CharCode -> Glyph Name mapped String
}

impl Default for FontInfo {
    fn default() -> Self {
        Self {
            base_font: "Calibri".to_string(),
            is_bold: false,
            is_italic: false,
            is_multibyte: false,
            first_char: 0,
            last_char: 255,
            default_width: 500.0,
            widths: BTreeMap::new(),
            to_unicode: BTreeMap::new(),
            differences: BTreeMap::new(),
        }
    }
}

/// Decoded individual glyph representation
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedGlyph {
    pub text: String,
    pub char_code: u32,
    pub width_pt: f64,
    pub advance_pt: f64,
    pub is_space: bool,
}

impl FontInfo {
    /// Decodes a sequence of raw bytes into individual decoded glyphs with precise metrics
    pub fn decode_bytes(
        &self,
        bytes: &[u8],
        font_size: f64,
        char_spacing: f64,
        word_spacing: f64,
        horizontal_scaling: f64,
    ) -> Vec<DecodedGlyph> {
        let mut glyphs = Vec::new();
        if bytes.is_empty() {
            return glyphs;
        }

        // Auto-detect multibyte (2-byte codes)
        let is_2byte = self.is_multibyte
            || (bytes.len() % 2 == 0 && bytes.len() >= 2 && self.to_unicode.keys().any(|&k| k > 255));

        let step = if is_2byte { 2 } else { 1 };
        let mut i = 0;

        while i < bytes.len() {
            let code = if is_2byte {
                if i + 1 < bytes.len() {
                    ((bytes[i] as u32) << 8) | (bytes[i + 1] as u32)
                } else {
                    bytes[i] as u32
                }
            } else {
                bytes[i] as u32
            };

            // 1. Text resolution:
            // Priority 1: /ToUnicode CMap mapping (ISO 32000-1 §9.10.3)
            // Priority 2: /Differences encoding mapping (ISO 32000-1 §9.6.5)
            // Priority 3: Printable ASCII / ISO-8859-1 Latin-1
            let text = if let Some(s) = self.to_unicode.get(&code) {
                s.clone()
            } else if let Some(s) = self.differences.get(&code) {
                s.clone()
            } else if code >= 32 && code <= 126 {
                (code as u8 as char).to_string()
            } else if code == 9 || code == 10 || code == 13 {
                " ".to_string()
            } else {
                char::from_u32(code).map(|c| c.to_string()).unwrap_or_else(|| " ".to_string())
            };

            let is_space = text == " " || code == 32;

            // 2. Metric width resolution (1/1000 em)
            let width_em = self.widths.get(&code).copied().unwrap_or(self.default_width);
            let width_pt = (width_em / 1000.0) * font_size;

            // 3. Displacement advance (ISO 32000-1 §9.4.4):
            // delta_x = ((w_em / 1000) * Tfs + Tc + (Tw if space)) * (Tz / 100)
            let mut adv = width_pt + char_spacing;
            if is_space {
                adv += word_spacing;
            }
            let advance_pt = adv * (horizontal_scaling / 100.0);

            glyphs.push(DecodedGlyph {
                text,
                char_code: code,
                width_pt,
                advance_pt,
                is_space,
            });

            i += step;
        }

        glyphs
    }

    /// Returns width for a single character code in points
    pub fn get_glyph_width(&self, char_code: u32, font_size: f64) -> f64 {
        let em = self.widths.get(&char_code).copied().unwrap_or(self.default_width);
        (em / 1000.0) * font_size
    }
}

pub struct FontDecoder;

impl FontDecoder {
    /// Parses a font dictionary, descendant CIDFonts, /Widths, and /ToUnicode CMap
    pub fn parse_font(doc: &Document, font_id: ObjectId) -> Result<FontInfo, String> {
        let font_obj = doc.get_object(font_id)
            .map_err(|e| format!("Font object {:?} not found: {}", font_id, e))?;
        let font_dict = font_obj.as_dict()
            .map_err(|e| format!("Font object {:?} is not a dictionary: {}", font_id, e))?;

        let mut info = FontInfo::default();

        // 1. Parse /BaseFont
        if let Ok(base_font_name) = font_dict.get(b"BaseFont").and_then(|o| o.as_name_str()) {
            info.base_font = clean_font_name(base_font_name);
            let lower = base_font_name.to_lowercase();
            info.is_bold = lower.contains("bold") || lower.contains("black") || lower.contains("heavy");
            info.is_italic = lower.contains("italic") || lower.contains("oblique");
        }

        // 2. Parse /FontDescriptor if available (flags, bold, italic)
        if let Ok(desc_obj) = font_dict.get(b"FontDescriptor") {
            let desc_dict = match desc_obj {
                Object::Reference(r) => doc.get_object(*r).ok().and_then(|o| o.as_dict().ok()),
                Object::Dictionary(d) => Some(d),
                _ => None,
            };
            if let Some(fd) = desc_dict {
                if let Ok(flags) = fd.get(b"Flags").and_then(|f| f.as_i64()) {
                    if (flags & (1 << 6)) != 0 {
                        info.is_italic = true;
                    }
                }
                if let Some(angle) = fd.get(b"ItalicAngle").ok().and_then(obj_to_f64) {
                    if angle.abs() > 0.01 {
                        info.is_italic = true;
                    }
                }
                if let Ok(weight) = fd.get(b"FontWeight").and_then(|w| w.as_i64()) {
                    if weight >= 700 {
                        info.is_bold = true;
                    }
                }
            }
        }

        // 3. Parse /FirstChar and /LastChar
        if let Ok(fc) = font_dict.get(b"FirstChar").and_then(|o| o.as_i64()) {
            info.first_char = fc as u32;
        }
        if let Ok(lc) = font_dict.get(b"LastChar").and_then(|o| o.as_i64()) {
            info.last_char = lc as u32;
        }

        // 4. Parse /Widths array (Simple fonts: TrueType, Type 1, Type 3)
        if let Ok(widths_obj) = font_dict.get(b"Widths") {
            let widths_arr = match widths_obj {
                Object::Reference(r) => doc.get_object(*r).ok().and_then(|o| o.as_array().ok()),
                Object::Array(arr) => Some(arr),
                _ => None,
            };

            if let Some(arr) = widths_arr {
                let start_code = info.first_char;
                for (idx, w_obj) in arr.iter().enumerate() {
                    let w = match w_obj {
                        Object::Integer(i) => *i as f64,
                        Object::Real(r) => *r as f64,
                        _ => 500.0,
                    };
                    info.widths.insert(start_code + idx as u32, w);
                }
            }
        }

        // 5. Handle Type 0 Composite Font and DescendantFonts (/DW, /W array)
        if let Ok(descendant_obj) = font_dict.get(b"DescendantFonts") {
            let desc_arr = match descendant_obj {
                Object::Reference(r) => doc.get_object(*r).ok().and_then(|o| o.as_array().ok()),
                Object::Array(arr) => Some(arr),
                _ => None,
            };

            if let Some(arr) = desc_arr {
                if let Some(first_desc) = arr.first() {
                    let cid_dict = match first_desc {
                        Object::Reference(r) => doc.get_object(*r).ok().and_then(|o| o.as_dict().ok()),
                        Object::Dictionary(d) => Some(d),
                        _ => None,
                    };

                    if let Some(cd) = cid_dict {
                        // Default Width /DW
                        if let Some(dw) = cd.get(b"DW").ok().and_then(obj_to_f64) {
                            info.default_width = dw;
                        }

                        // CID Widths array /W
                        if let Ok(w_obj) = cd.get(b"W") {
                            let w_arr = match w_obj {
                                Object::Reference(r) => doc.get_object(*r).ok().and_then(|o| o.as_array().ok()),
                                Object::Array(arr) => Some(arr),
                                _ => None,
                            };
                            if let Some(wa) = w_arr {
                                parse_cid_widths(wa, info.default_width, &mut info.widths);
                            }
                        }
                    }
                }
            }
        }

        // 6. Parse /Encoding with /Differences array (ISO 32000-1 §9.6.5)
        if let Ok(enc_obj) = font_dict.get(b"Encoding") {
            let enc_dict = match enc_obj {
                Object::Reference(r) => doc.get_object(*r).ok().and_then(|o| o.as_dict().ok()),
                Object::Dictionary(d) => Some(d),
                _ => None,
            };
            if let Some(ed) = enc_dict {
                if let Ok(diff_obj) = ed.get(b"Differences") {
                    let diff_arr = match diff_obj {
                        Object::Reference(r) => doc.get_object(*r).ok().and_then(|o| o.as_array().ok()),
                        Object::Array(arr) => Some(arr),
                        _ => None,
                    };
                    if let Some(da) = diff_arr {
                        parse_differences_array(da, &mut info.differences);
                    }
                }
            }
        }

        // 7. Parse /ToUnicode CMap stream (ISO 32000-1 §9.10.3)
        if let Ok(to_unicode_obj) = font_dict.get(b"ToUnicode") {
            let cmap_stream = match to_unicode_obj {
                Object::Reference(r) => doc.get_object(*r).ok().and_then(|o| o.as_stream().ok()),
                Object::Stream(s) => Some(s),
                _ => None,
            };

            if let Some(s) = cmap_stream {
                if let Ok(decompressed_cmap) = s.decompressed_content() {
                    parse_cmap_content(&decompressed_cmap, &mut info.to_unicode, &mut info.is_multibyte);
                }
            }
        }

        Ok(info)
    }
}

/// Parses CIDFont /W array: `c [ w1 w2 ... ]` or `c_start c_end w`
fn parse_cid_widths(arr: &[Object], default_width: f64, out_widths: &mut BTreeMap<u32, f64>) {
    let mut i = 0;
    while i < arr.len() {
        if let Object::Integer(start_cid) = arr[i] {
            if i + 1 < arr.len() {
                match &arr[i + 1] {
                    Object::Array(widths_sub) => {
                        let mut cid = start_cid as u32;
                        for w_obj in widths_sub {
                            let w = match w_obj {
                                Object::Integer(val) => *val as f64,
                                Object::Real(val) => *val as f64,
                                _ => default_width,
                            };
                            out_widths.insert(cid, w);
                            cid += 1;
                        }
                        i += 2;
                        continue;
                    }
                    Object::Integer(end_cid) => {
                        if i + 2 < arr.len() {
                            let w = match &arr[i + 2] {
                                Object::Integer(val) => *val as f64,
                                Object::Real(val) => *val as f64,
                                _ => default_width,
                            };
                            for cid in (start_cid as u32)..=(*end_cid as u32) {
                                out_widths.insert(cid, w);
                            }
                            i += 3;
                            continue;
                        }
                    }
                    _ => {}
                }
            }
        }
        i += 1;
    }
}

/// Parses `/Differences` array into unicode strings
fn parse_differences_array(arr: &[Object], out_map: &mut BTreeMap<u32, String>) {
    let mut current_code = 0u32;
    for obj in arr {
        match obj {
            Object::Integer(i) => {
                current_code = *i as u32;
            }
            Object::Name(name_bytes) => {
                let name = String::from_utf8_lossy(name_bytes);
                if let Some(unicode_str) = adobe_glyph_to_unicode(&name) {
                    out_map.insert(current_code, unicode_str.to_string());
                } else if name.starts_with("uni") && name.len() >= 7 {
                    if let Ok(val) = u32::from_str_radix(&name[3..7], 16) {
                        if let Some(c) = char::from_u32(val) {
                            out_map.insert(current_code, c.to_string());
                        }
                    }
                } else if name.starts_with('u') && name.len() >= 5 {
                    if let Ok(val) = u32::from_str_radix(&name[1..5], 16) {
                        if let Some(c) = char::from_u32(val) {
                            out_map.insert(current_code, c.to_string());
                        }
                    }
                }
                current_code += 1;
            }
            _ => {}
        }
    }
}

/// Cleans random 6-character font subset prefixes (e.g., "BAAAAA+Arial-BoldMT" -> "Arial")
pub fn clean_font_name(raw: &str) -> String {
    let name_part = if let Some(plus_idx) = raw.find('+') {
        &raw[plus_idx + 1..]
    } else {
        raw
    };

    // Remove common suffixes
    let clean = name_part
        .split('-')
        .next()
        .unwrap_or(name_part)
        .replace("MT", "")
        .replace("PS", "");

    if clean.is_empty() {
        "Calibri".to_string()
    } else {
        clean
    }
}

/// Parses PostScript /ToUnicode CMap stream (ISO 32000-1 §9.10.3)
pub fn parse_cmap_content(
    cmap_bytes: &[u8],
    out_map: &mut BTreeMap<u32, String>,
    out_is_multibyte: &mut bool,
) {
    let raw_text = String::from_utf8_lossy(cmap_bytes);

    // Strip PostScript comments (% ...)
    let clean_text: String = raw_text
        .lines()
        .map(|l| {
            if let Some(pos) = l.find('%') {
                &l[..pos]
            } else {
                l
            }
        })
        .collect::<Vec<&str>>()
        .join(" ");

    let tokens: Vec<&str> = clean_text.split_whitespace().collect();
    let mut i = 0;

    while i < tokens.len() {
        let token = tokens[i];

        // 1. Detect multibyte codespace
        if token == "begincodespacerange" {
            i += 1;
            while i < tokens.len() && tokens[i] != "endcodespacerange" {
                let range_tok = tokens[i];
                let clean = range_tok.trim_matches(|c| c == '<' || c == '>');
                if clean.len() >= 4 {
                    *out_is_multibyte = true;
                }
                i += 1;
            }
        }
        // 2. Parse `beginbfchar ... endbfchar`
        else if token == "beginbfchar" {
            i += 1;
            while i < tokens.len() && tokens[i] != "endbfchar" {
                if i + 1 < tokens.len() {
                    let src_tok = tokens[i];
                    let dst_tok = tokens[i + 1];
                    if let (Some(src), Some(dst)) = (parse_hex_token(src_tok), parse_utf16_hex_token(dst_tok)) {
                        out_map.insert(src, dst);
                    }
                    i += 2;
                } else {
                    break;
                }
            }
        }
        // 3. Parse `beginbfrange ... endbfrange`
        else if token == "beginbfrange" {
            i += 1;
            while i < tokens.len() && tokens[i] != "endbfrange" {
                if i + 2 < tokens.len() {
                    let start_tok = tokens[i];
                    let end_tok = tokens[i + 1];
                    let dst_tok = tokens[i + 2];

                    if let (Some(start), Some(end)) = (parse_hex_token(start_tok), parse_hex_token(end_tok)) {
                        if dst_tok.starts_with('[') {
                            // Destination is an array: `[ <dst1> <dst2> ... ]`
                            let mut current_code = start;
                            i += 2; // Move to the array start
                            while i < tokens.len() && tokens[i] != "endbfrange" {
                                let item = tokens[i];
                                let clean = item.trim_matches(|c| c == '[' || c == ']');
                                if !clean.is_empty() {
                                    if let Some(s) = parse_utf16_hex_token(clean) {
                                        if current_code <= end {
                                            out_map.insert(current_code, s);
                                            current_code += 1;
                                        }
                                    }
                                }
                                if item.contains(']') {
                                    break;
                                }
                                i += 1;
                            }
                        } else {
                            // Sequential destination: `<start> <end> <dstStart>`
                            if let Some(mut dst_val) = parse_hex_token(dst_tok) {
                                for code in start..=end {
                                    if let Some(c) = char::from_u32(dst_val) {
                                        out_map.insert(code, c.to_string());
                                    }
                                    dst_val += 1;
                                }
                            }
                            i += 2; // Advancing start + end + dstStart
                        }
                    }
                }
                i += 1;
            }
        }

        i += 1;
    }
}

fn obj_to_f64(obj: &Object) -> Option<f64> {
    match obj {
        Object::Real(r) => Some(*r as f64),
        Object::Integer(i) => Some(*i as f64),
        _ => None,
    }
}

/// Extracts hexadecimal number from `<XXXX>`
fn parse_hex_token(token: &str) -> Option<u32> {
    let clean = token.trim_matches(|c| c == '<' || c == '>');
    if clean.is_empty() {
        return None;
    }
    u32::from_str_radix(clean, 16).ok()
}

/// Decodes UTF-16BE hex string `<0041>` or `<00660069>` (ligatures) to UTF-8 String
fn parse_utf16_hex_token(token: &str) -> Option<String> {
    let clean = token.trim_matches(|c| c == '<' || c == '>');
    if clean.is_empty() {
        return None;
    }

    // Fallback for single byte hex e.g. <41>
    if clean.len() == 2 {
        if let Ok(b) = u8::from_str_radix(clean, 16) {
            return Some((b as char).to_string());
        }
    }

    if clean.len() % 4 != 0 {
        return None;
    }

    let mut u16_values = Vec::new();
    for chunk in clean.as_bytes().chunks(4) {
        let chunk_str = std::str::from_utf8(chunk).ok()?;
        let val = u16::from_str_radix(chunk_str, 16).ok()?;
        u16_values.push(val);
    }

    String::from_utf16(&u16_values).ok()
}

/// Maps standard Adobe Glyph Names to Unicode characters
pub fn adobe_glyph_to_unicode(name: &str) -> Option<&'static str> {
    match name {
        "space" => Some(" "),
        "exclam" => Some("!"),
        "quotedbl" => Some("\""),
        "numbersign" => Some("#"),
        "dollar" => Some("$"),
        "percent" => Some("%"),
        "ampersand" => Some("&"),
        "quotesingle" => Some("'"),
        "parenleft" => Some("("),
        "parenright" => Some(")"),
        "asterisk" => Some("*"),
        "plus" => Some("+"),
        "comma" => Some(","),
        "hyphen" | "minus" => Some("-"),
        "period" => Some("."),
        "slash" => Some("/"),
        "zero" => Some("0"),
        "one" => Some("1"),
        "two" => Some("2"),
        "three" => Some("3"),
        "four" => Some("4"),
        "five" => Some("5"),
        "six" => Some("6"),
        "seven" => Some("7"),
        "eight" => Some("8"),
        "nine" => Some("9"),
        "colon" => Some(":"),
        "semicolon" => Some(";"),
        "less" => Some("<"),
        "equal" => Some("="),
        "greater" => Some(">"),
        "question" => Some("?"),
        "at" => Some("@"),
        "A" => Some("A"),
        "B" => Some("B"),
        "C" => Some("C"),
        "D" => Some("D"),
        "E" => Some("E"),
        "F" => Some("F"),
        "G" => Some("G"),
        "H" => Some("H"),
        "I" => Some("I"),
        "J" => Some("J"),
        "K" => Some("K"),
        "L" => Some("L"),
        "M" => Some("M"),
        "N" => Some("N"),
        "O" => Some("O"),
        "P" => Some("P"),
        "Q" => Some("Q"),
        "R" => Some("R"),
        "S" => Some("S"),
        "T" => Some("T"),
        "U" => Some("U"),
        "V" => Some("V"),
        "W" => Some("W"),
        "X" => Some("X"),
        "Y" => Some("Y"),
        "Z" => Some("Z"),
        "bracketleft" => Some("["),
        "backslash" => Some("\\"),
        "bracketright" => Some("]"),
        "asciicircum" => Some("^"),
        "underscore" => Some("_"),
        "grave" => Some("`"),
        "a" => Some("a"),
        "b" => Some("b"),
        "c" => Some("c"),
        "d" => Some("d"),
        "e" => Some("e"),
        "f" => Some("f"),
        "g" => Some("g"),
        "h" => Some("h"),
        "i" => Some("i"),
        "j" => Some("j"),
        "k" => Some("k"),
        "l" => Some("l"),
        "m" => Some("m"),
        "n" => Some("n"),
        "o" => Some("o"),
        "p" => Some("p"),
        "q" => Some("q"),
        "r" => Some("r"),
        "s" => Some("s"),
        "t" => Some("t"),
        "u" => Some("u"),
        "v" => Some("v"),
        "w" => Some("w"),
        "x" => Some("x"),
        "y" => Some("y"),
        "z" => Some("z"),
        "braceleft" => Some("{"),
        "bar" => Some("|"),
        "braceright" => Some("}"),
        "asciitilde" => Some("~"),
        "bullet" => Some("•"),
        "endash" => Some("–"),
        "emdash" => Some("—"),
        "ellipsis" => Some("…"),
        "fi" => Some("fi"),
        "fl" => Some("fl"),
        "ffi" => Some("ffi"),
        "ffl" => Some("ffl"),
        "ff" => Some("ff"),
        "oe" => Some("œ"),
        "OE" => Some("Œ"),
        "ae" => Some("æ"),
        "AE" => Some("Æ"),
        "euro" => Some("€"),
        "registered" => Some("®"),
        "copyright" => Some("©"),
        "trademark" => Some("™"),
        "degree" => Some("°"),
        "plusminus" => Some("±"),
        "cent" => Some("¢"),
        "sterling" => Some("£"),
        "yen" => Some("¥"),
        "section" => Some("§"),
        "paragraph" => Some("¶"),
        "germandbls" => Some("ß"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_font_name() {
        assert_eq!(clean_font_name("BAAAAA+Arial-BoldMT"), "Arial");
        assert_eq!(clean_font_name("TimesNewRomanPS-Italic"), "TimesNewRoman");
    }

    #[test]
    fn test_parse_bfchar() {
        let cmap = r#"
            1 beginbfchar
            <01> <0044>
            <02> <0075>
            <03> <00660069>
            endbfchar
        "#;
        let mut map = BTreeMap::new();
        let mut is_mb = false;
        parse_cmap_content(cmap.as_bytes(), &mut map, &mut is_mb);

        assert_eq!(map.get(&1).unwrap(), "D");
        assert_eq!(map.get(&2).unwrap(), "u");
        assert_eq!(map.get(&3).unwrap(), "fi"); // Ligature expansion
    }

    #[test]
    fn test_parse_bfrange_sequential() {
        let cmap = r#"
            1 beginbfrange
            <0A> <0C> <0030>
            endbfrange
        "#;
        let mut map = BTreeMap::new();
        let mut is_mb = false;
        parse_cmap_content(cmap.as_bytes(), &mut map, &mut is_mb);

        assert_eq!(map.get(&10).unwrap(), "0");
        assert_eq!(map.get(&11).unwrap(), "1");
        assert_eq!(map.get(&12).unwrap(), "2");
    }

    #[test]
    fn test_parse_bfrange_array() {
        let cmap = r#"
            1 beginbfrange
            <01> <03> [ <0041> <0042> <0043> ]
            endbfrange
        "#;
        let mut map = BTreeMap::new();
        let mut is_mb = false;
        parse_cmap_content(cmap.as_bytes(), &mut map, &mut is_mb);

        assert_eq!(map.get(&1).unwrap(), "A");
        assert_eq!(map.get(&2).unwrap(), "B");
        assert_eq!(map.get(&3).unwrap(), "C");
    }

    #[test]
    fn test_multibyte_codespace_detection() {
        let cmap = r#"
            1 begincodespacerange
            <0000> <FFFF>
            endcodespacerange
        "#;
        let mut map = BTreeMap::new();
        let mut is_mb = false;
        parse_cmap_content(cmap.as_bytes(), &mut map, &mut is_mb);

        assert!(is_mb);
    }
}
