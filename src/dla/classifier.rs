use crate::models::{ParagraphBlock, Style, TextLine, TextRun};

pub struct HeadingClassifier;

impl HeadingClassifier {
    /// Classifies paragraph roles (H1..H3, P, List), calculates alignments, line spacings, and constructs unified TextRuns
    pub fn process_paragraphs(paragraphs: &mut Vec<ParagraphBlock>, page_width: f64) {
        if paragraphs.is_empty() {
            return;
        }

        // 0. Merge fragmented paragraph blocks on the same line (caused by inline font styling or web tag splits)
        consolidate_paragraph_fragments(paragraphs);

        // 0b. Merge vertically adjacent paragraphs split mid-sentence (continuation lines)
        consolidate_broken_sentence_paragraphs(paragraphs);

        // 1. Calculate page-wide dominant body font size
        let mut all_font_sizes = Vec::new();
        for p in paragraphs.iter() {
            for line in &p.lines {
                for w in &line.words {
                    all_font_sizes.push(w.style.font_size);
                }
            }
        }

        all_font_sizes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median_body_size = if !all_font_sizes.is_empty() {
            all_font_sizes[all_font_sizes.len() / 2]
        } else {
            11.0
        };

        // 2. Classify each paragraph block
        for p in paragraphs.iter_mut() {
            classify_single_paragraph(p, median_body_size, page_width);

            // If classified as a bullet list (list_id == 1), strip leading bullet glyphs
            // from the first line so Word's native <w:numPr> bullet doesn't duplicate them!
            if p.is_list && p.list_id == 1 {
                if let Some(first_line) = p.lines.first_mut() {
                    while !first_line.words.is_empty() && is_bullet_prefix(first_line.words[0].text.trim()) {
                        first_line.words.remove(0);
                    }
                    if let Some(first_w) = first_line.words.first_mut() {
                        let t = first_w.text.trim();
                        for b in &["•", "·", "▪", "▫", "–", "—", "*", "●", "○", "", ">"] {
                            if t.starts_with(b) && t.len() > b.len() {
                                first_w.text = t[b.len()..].trim_start().to_string();
                                break;
                            }
                        }
                    }
                }
            }

            build_line_runs(&mut p.lines);
        }

        // 3. True Geometric Inter-Paragraph Spacing (from bounding-box gaps)
        // Strictly mirrors visual PDF layout without synthetic spacing inflation!
        let num_p = paragraphs.len();
        for i in 0..num_p {
            paragraphs[i].space_after = 0.0;
            if i == 0 {
                paragraphs[i].space_before = 0.0;
            } else {
                let prev_bottom = paragraphs[i - 1].bbox.y_max;
                let curr_top = paragraphs[i].bbox.y_min;
                let gap = (curr_top - prev_bottom).max(0.0);
                // Standard inline reading gap is ~1.5..3.0 pt.
                // Any gap beyond 2.0 pt is intentional inter-paragraph spacing.
                paragraphs[i].space_before = (gap - 2.0).clamp(0.0, 28.0);
            }
        }
    }
}

/// Merges fragmented paragraph blocks on the same horizontal line
pub fn consolidate_paragraph_fragments(paragraphs: &mut Vec<ParagraphBlock>) {
    if paragraphs.len() < 2 {
        return;
    }

    let mut merged: Vec<ParagraphBlock> = Vec::with_capacity(paragraphs.len());

    for p in paragraphs.drain(..) {
        if merged.is_empty() {
            merged.push(p);
            continue;
        }

        let should_merge_same_line = {
            let last = merged.last().unwrap();
            if let (Some(l_last), Some(l_curr)) = (last.lines.last(), p.lines.first()) {
                let y_overlap = (l_last.bbox.y_max.min(l_curr.bbox.y_max) - l_last.bbox.y_min.max(l_curr.bbox.y_min)) > 2.0;
                let x_adjacent = l_curr.bbox.x_min >= (l_last.bbox.x_max - 5.0) && (l_curr.bbox.x_min - l_last.bbox.x_max) <= 30.0;
                y_overlap && x_adjacent && !last.is_list && !p.is_list
            } else {
                false
            }
        };

        if should_merge_same_line {
            let last = merged.last_mut().unwrap();
            let mut p_lines = p.lines;
            if !p_lines.is_empty() {
                let first_p_line = p_lines.remove(0);
                let last_line = last.lines.last_mut().unwrap();
                last_line.bbox.expand(&first_p_line.bbox);
                last_line.words.extend(first_p_line.words);
                last_line.runs.extend(first_p_line.runs);
                last.lines.extend(p_lines);
                last.bbox.expand(&p.bbox);
            }
        } else {
            merged.push(p);
        }
    }

    *paragraphs = merged;
}

/// Merges vertically adjacent paragraph blocks that were split mid-sentence
pub fn consolidate_broken_sentence_paragraphs(paragraphs: &mut Vec<ParagraphBlock>) {
    if paragraphs.len() < 2 {
        return;
    }

    let mut merged: Vec<ParagraphBlock> = Vec::with_capacity(paragraphs.len());

    for p in paragraphs.drain(..) {
        if merged.is_empty() {
            merged.push(p);
            continue;
        }

        let should_merge = {
            let last = merged.last().unwrap();
            let last_text = get_block_full_text(last);
            let curr_text = get_block_full_text(&p);

            // NEVER merge Table of Contents lines or entries!
            let is_last_toc = last.lines.iter().any(|l| l.tab_stops.iter().any(|ts| ts.is_leader_dot) || crate::dla::rxyc::is_toc_line_entry(l));
            let is_curr_toc = p.lines.iter().any(|l| l.tab_stops.iter().any(|ts| ts.is_leader_dot) || crate::dla::rxyc::is_toc_line_entry(l));

            // NEVER merge frame elements in the header strip or footer strip!
            let in_header_strip = last.bbox.y_max <= 55.0 || p.bbox.y_max <= 55.0;
            let in_footer_strip = last.bbox.y_min >= 700.0 || p.bbox.y_min >= 700.0;

            if is_last_toc || is_curr_toc || in_header_strip || in_footer_strip {
                false
            } else {
                let is_curr_list = p.is_list || starts_with_bullet_or_number(&curr_text);
                if !is_curr_list && !last.is_header && !p.is_header && !last_text.is_empty() && !curr_text.is_empty() {
                    let last_char = last_text.trim_end().chars().last().unwrap_or('.');
                    let ends_sentence = last_char == '.' || last_char == '?' || last_char == '!' || last_char == ':' || last_char == ';' || last_char.is_ascii_digit();

                    let gap_y = p.bbox.y_min - last.bbox.y_max;
                    let x_overlap = p.bbox.x_min < (last.bbox.x_max + 10.0) && p.bbox.x_max > (last.bbox.x_min - 10.0);

                    // If previous paragraph did not finish its sentence and sits directly above with normal line spacing
                    !ends_sentence && gap_y >= 0.0 && gap_y <= 16.0 && x_overlap
                } else {
                    false
                }
            }
        };

        if should_merge {
            let last = merged.last_mut().unwrap();
            last.bbox.expand(&p.bbox);
            last.lines.extend(p.lines);
        } else {
            merged.push(p);
        }
    }

    *paragraphs = merged;
}

fn get_block_full_text(p: &ParagraphBlock) -> String {
    p.lines
        .iter()
        .flat_map(|l| l.words.iter().map(|w| w.text.as_str()))
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn starts_with_bullet_or_number(text: &str) -> bool {
    let t = text.trim();
    if let Some(first_word) = t.split_whitespace().next() {
        is_bullet_prefix(first_word) || is_numbered_prefix(first_word)
    } else {
        false
    }
}

/// Classifies heading level, paragraph type, list detection, alignment, and leading
fn classify_single_paragraph(p: &mut ParagraphBlock, median_body_size: f64, page_width: f64) {
    if p.lines.is_empty() {
        return;
    }

    let mut max_font_size = 0.0f64;
    let mut has_bold = false;
    let mut total_char_count = 0;

    for line in &p.lines {
        for w in &line.words {
            if w.style.font_size > max_font_size {
                max_font_size = w.style.font_size;
            }
            if w.style.is_bold {
                has_bold = true;
            }
            total_char_count += w.text.chars().count();
        }
    }

    // List detection on first word
    if let Some(first_line) = p.lines.first() {
        if let Some(first_word) = first_line.words.first() {
            let t = first_word.text.trim();
            if is_bullet_prefix(t) {
                p.is_list = true;
                p.list_id = 1;
                p.list_level = 1;
                p.tag = "List".to_string();
                detect_alignment(p, page_width);
                detect_line_spacing(p);
                return;
            } else if is_numbered_prefix(t) {
                p.is_list = true;
                p.list_id = 2;
                p.list_level = 1;
                p.tag = "List".to_string();
                detect_alignment(p, page_width);
                detect_line_spacing(p);
                return;
            }
        }
    }

    // Heading detection for standalone short blocks (1 or 2 lines, < 100 chars)
    // Exclude URLs and web links from headings!
    let full_text: String = p
        .lines
        .iter()
        .flat_map(|l| l.words.iter().map(|w| w.text.as_str()))
        .collect::<Vec<_>>()
        .join(" ");

    let is_url_or_link = full_text.contains("http://")
        || full_text.contains("https://")
        || full_text.contains("www.")
        || full_text.contains(".com")
        || full_text.contains(".io")
        || full_text.contains("github.c");

    let has_alphabetic = full_text.chars().any(|c| c.is_alphabetic());
    let is_all_caps = has_alphabetic && full_text.chars().filter(|c| c.is_alphabetic()).all(|c| c.is_uppercase());
    let is_standalone_heading_layout = p.lines.len() <= 2 && total_char_count < 100 && !is_url_or_link;

    // 100% Language-Agnostic Major Structural Heading:
    // A standalone short block that is:
    // a. ALL-CAPS and bold (universal standard for major chapter headings across Latin/Cyrillic scripts)
    // b. Centered and bold with elevated prominence (universal across CJK, Arabic, Indic, and all scripts)
    // c. Significantly larger font size (>= 16pt or >= 1.35x median)
    let is_prominent_structural_heading = is_standalone_heading_layout
        && ((is_all_caps && has_bold) || (p.alignment == "center" && has_bold));

    if is_standalone_heading_layout {
        if is_prominent_structural_heading || max_font_size >= 16.0 || max_font_size >= median_body_size * 1.35 {
            p.tag = "H1".to_string();
            p.is_header = true;
            p.space_before = 0.0;
            p.space_after = 0.0;
            detect_alignment(p, page_width);
            detect_line_spacing(p);
            return;
        } else if max_font_size >= 13.0 || max_font_size >= median_body_size * 1.20 {
            p.tag = "H2".to_string();
            p.is_header = true;
            p.space_before = 0.0;
            p.space_after = 0.0;
            detect_alignment(p, page_width);
            detect_line_spacing(p);
            return;
        } else if max_font_size >= median_body_size * 1.12 && has_bold {
            p.tag = "H3".to_string();
            p.is_header = true;
            p.space_before = 0.0;
            p.space_after = 0.0;
            detect_alignment(p, page_width);
            detect_line_spacing(p);
            return;
        }
    }

    // Default Body Paragraph
    p.tag = "P".to_string();
    p.is_header = false;
    p.space_before = 0.0;
    p.space_after = 0.0;
    detect_alignment(p, page_width);
    detect_line_spacing(p);
}

/// Detects text alignment ("left", "center", "right", "both") via geometric variance
fn detect_alignment(p: &mut ParagraphBlock, page_width: f64) {
    let n = p.lines.len();
    if n == 0 {
        p.alignment = "left".to_string();
        return;
    }

    // A list item (bullet or numbered) is NEVER right-aligned or centered!
    if p.is_list {
        if n >= 2 {
            let non_last = &p.lines[..n - 1];
            let rights: Vec<f64> = non_last.iter().map(|l| l.bbox.x_max).collect();
            let max_r = rights.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let min_r = rights.iter().cloned().fold(f64::INFINITY, f64::min);
            let delta_right = max_r - min_r;

            // Multi-line list item: if non-last lines have aligned right edges
            if non_last.len() >= 2 && delta_right <= 8.0 && p.lines[0].bbox.width() >= 120.0 {
                p.alignment = "both".to_string();
                return;
            } else if non_last.len() == 1
                && (page_width - p.lines[0].bbox.x_max) <= 65.0
                && p.lines[0].bbox.width() >= 120.0
            {
                // 2-line list item whose first line spans to the right margin
                p.alignment = "both".to_string();
                return;
            }
        }
        p.alignment = "left".to_string();
        return;
    }

    // Single-line elements:
    if n == 1 {
        let line = &p.lines[0];
        let line_w = line.bbox.width();
        let line_cx = (line.bbox.x_min + line.bbox.x_max) / 2.0;
        let page_cx = page_width / 2.0;

        // Centered single line: must be reasonably short (< 75% page width),
        // Checks page centering OR header-column centering when a top logo is on the left
        let is_page_centered = (line_cx - page_cx).abs() <= 20.0 && line.bbox.x_min > 60.0;
        let is_header_column_centered = line.bbox.x_min >= 150.0 && (line_cx - (page_width * 0.63)).abs() <= 35.0;

        if line_w < (page_width * 0.75) && (is_page_centered || is_header_column_centered) {
            p.alignment = "center".to_string();
        } else if line.bbox.x_min > (page_width * 0.45)
            && (page_width - line.bbox.x_max) <= 80.0
            && line_w < (page_width * 0.50)
        {
            // Truly right-aligned single line (e.g. date header or author note on right side)
            let first_word = line.words.first().map(|w| w.text.trim()).unwrap_or("");
            let starts_with_lower_or_punct = first_word.chars().next().map_or(false, |c| {
                c.is_lowercase() || c == '.' || c == ',' || c == '/' || c == '-' || c == ';'
            });
            if !starts_with_lower_or_punct {
                p.alignment = "right".to_string();
            } else {
                p.alignment = "left".to_string();
            }
        } else {
            p.alignment = "left".to_string();
        }
        return;
    }

    // Multi-line elements (n >= 2):
    // 1. Right Alignment Check:
    // Text can ONLY be right-aligned if it actually sits on the right side of the page!
    // A paragraph starting at the left margin (< 40% of page width) is NEVER right-aligned.
    if p.bbox.x_min >= (page_width * 0.40)
        && (page_width - p.bbox.x_max) <= 80.0
        && p.bbox.width() < (page_width * 0.55)
    {
        let first_word = p.lines.first().and_then(|l| l.words.first()).map(|w| w.text.trim()).unwrap_or("");
        let starts_with_lower_or_punct = first_word.chars().next().map_or(false, |c| {
            c.is_lowercase() || c == '.' || c == ',' || c == '/' || c == '-' || c == ';'
        });
        let rights: Vec<f64> = p.lines.iter().map(|l| l.bbox.x_max).collect();
        let max_r = rights.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let min_r = rights.iter().cloned().fold(f64::INFINITY, f64::min);
        if !starts_with_lower_or_punct && (max_r - min_r) <= 8.0 {
            p.alignment = "right".to_string();
            return;
        }
    }

    // 2. Center Alignment Check:
    // All lines must be centered, paragraph must be narrower than page body width, and indented from left margin.
    let x_centers: Vec<f64> = p.lines.iter().map(|l| (l.bbox.x_min + l.bbox.x_max) / 2.0).collect();
    let min_c = x_centers.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_c = x_centers.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let delta_center = max_c - min_c;
    let avg_center: f64 = x_centers.iter().sum::<f64>() / (n as f64);
    let page_cx = page_width / 2.0;

    let x_lefts: Vec<f64> = p.lines.iter().map(|l| l.bbox.x_min).collect();
    let min_l = x_lefts.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_l = x_lefts.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let delta_left = max_l - min_l;

    if delta_center <= 6.0
        && (avg_center - page_cx).abs() <= 20.0
        && min_l > 70.0
        && p.bbox.width() < (page_width * 0.70)
        && delta_left > 8.0
    {
        p.alignment = "center".to_string();
        return;
    }

    // 3. Justified Alignment Check (Rata Kiri-Kanan / "both"):
    // In justified text, lines 0..n-2 (all lines except the short terminal line)
    // are stretched so their right edges align to the right margin!
    let non_last_lines = &p.lines[..n - 1];
    let rights: Vec<f64> = non_last_lines.iter().map(|l| l.bbox.x_max).collect();
    let max_r = rights.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let min_r = rights.iter().cloned().fold(f64::INFINITY, f64::min);
    let delta_right = max_r - min_r;

    let avg_w: f64 = non_last_lines.iter().map(|l| l.bbox.width()).sum::<f64>() / (non_last_lines.len() as f64);

    if non_last_lines.len() >= 2 && delta_right <= 8.0 && avg_w >= 150.0 && min_l < (page_width * 0.40) {
        p.alignment = "both".to_string();
        return;
    }

    // 4. Also check if ALL lines (including last line if n == 2 or last line is full) align on right:
    let all_rights: Vec<f64> = p.lines.iter().map(|l| l.bbox.x_max).collect();
    let max_all_r = all_rights.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let min_all_r = all_rights.iter().cloned().fold(f64::INFINITY, f64::min);
    if (max_all_r - min_all_r) <= 8.0 && avg_w >= 150.0 && min_l < (page_width * 0.40) {
        p.alignment = "both".to_string();
        return;
    }

    // 5. For 2-line paragraphs: if line 0 reaches the right margin (within 65 pt of page edge) and line 1 is terminal
    if n == 2
        && (page_width - p.lines[0].bbox.x_max) <= 65.0
        && p.lines[0].bbox.width() >= 150.0
        && min_l < (page_width * 0.40)
    {
        p.alignment = "both".to_string();
        return;
    }

    // Default to left alignment
    p.alignment = "left".to_string();
}

/// Detects baseline-to-baseline leading / line spacing
fn detect_line_spacing(p: &mut ParagraphBlock) {
    if p.lines.len() < 2 {
        p.line_spacing_twips = None;
        p.line_rule = "auto".to_string();
        return;
    }

    let mut dy_sum = 0.0;
    let mut count = 0;
    for i in 1..p.lines.len() {
        let prev = &p.lines[i - 1];
        let curr = &p.lines[i];
        let dy = curr.bbox.y_min - prev.bbox.y_min;
        if dy > 0.0 && dy < 80.0 {
            dy_sum += dy;
            count += 1;
        }
    }

    if count == 0 {
        p.line_spacing_twips = None;
        p.line_rule = "auto".to_string();
        return;
    }

    let avg_dy = dy_sum / (count as f64);
    let dom_font_size = p.lines[0]
        .words
        .iter()
        .map(|w| w.style.font_size)
        .fold(0.0f64, f64::max)
        .max(6.0);

    let ratio = avg_dy / dom_font_size;

    if ratio >= 0.90 && ratio <= 2.25 {
        // Standard proportional auto line spacing: 240 twips = 1.0x line height
        let auto_line = ((ratio / 1.15) * 240.0).round() as i32;
        p.line_spacing_twips = Some(auto_line);
        p.line_rule = "auto".to_string();
    } else {
        let exact_twips = (avg_dy * 20.0).round() as i32;
        p.line_spacing_twips = Some(exact_twips);
        p.line_rule = "exact".to_string();
    }
}

/// Consolidates consecutive words with identical style into unified TextRuns
pub fn build_line_runs(lines: &mut [TextLine]) {
    for line in lines.iter_mut() {
        if line.words.is_empty() {
            continue;
        }

        let mut runs: Vec<TextRun> = Vec::new();
        let mut curr_text = String::new();
        let mut curr_style = line.words[0].style.clone();

        for (idx, w) in line.words.iter().enumerate() {
            if idx == 0 {
                curr_text.push_str(&w.text);
                curr_style = w.style.clone();
            } else if styles_match(&curr_style, &w.style) {
                curr_text.push(' ');
                curr_text.push_str(&w.text);
            } else {
                // Word `w` starts a new style run. Preserve space between words!
                if !curr_text.ends_with(' ') {
                    curr_text.push(' ');
                }
                runs.push(TextRun {
                    text: std::mem::take(&mut curr_text),
                    style: curr_style,
                    is_tab: false,
                    is_leader_dot: false,
                });
                curr_text.push_str(&w.text);
                curr_style = w.style.clone();
            }
        }

        if !curr_text.is_empty() {
            runs.push(TextRun {
                text: curr_text,
                style: curr_style,
                is_tab: false,
                is_leader_dot: false,
            });
        }

        line.runs = runs;
    }
}

/// Checks if two styles match closely enough to share a single TextRun
fn styles_match(a: &Style, b: &Style) -> bool {
    a.font_family == b.font_family
        && (a.font_size - b.font_size).abs() < 0.25
        && a.is_bold == b.is_bold
        && a.is_italic == b.is_italic
        && a.hex_color == b.hex_color
        && a.vert_align == b.vert_align
}

fn is_bullet_prefix(s: &str) -> bool {
    let bullets = [
        "•", "-", "*", "▪", "▫", "–", "—", ">", "o", "◦", "",
        "·", "●", "○", "■", "□", "◆", "◇", "\u{00B7}", "\u{2022}", "\u{25E6}",
    ];
    bullets.contains(&s)
}

fn is_numbered_prefix(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    // Check patterns like "1.", "1)", "(1)", "a.", "a)"
    let s_clean = s.trim_matches(|c| c == '(' || c == ')' || c == '[' || c == ']');
    if let Some(first_char) = s_clean.chars().next() {
        if first_char.is_ascii_digit() || (first_char.is_ascii_alphabetic() && s_clean.len() <= 2) {
            let last_char = s.chars().last().unwrap_or(' ');
            return last_char == '.' || last_char == ')';
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Rect, Word};

    #[test]
    fn test_heading_classification() {
        let style = Style {
            font_family: "Arial".to_string(),
            font_size: 16.0,
            is_bold: true,
            is_italic: false,
            hex_color: "000000".to_string(),
            vert_align: String::new(),
        };

        let mut block = ParagraphBlock {
            bbox: Rect::new(50.0, 50.0, 200.0, 70.0),
            lines: vec![TextLine {
                bbox: Rect::new(50.0, 50.0, 200.0, 70.0),
                words: vec![
                    Word { text: "Chapter".to_string(), bbox: Rect::new(50.0, 50.0, 110.0, 70.0), style: style.clone(), glyphs: Vec::new() },
                    Word { text: "One".to_string(), bbox: Rect::new(115.0, 50.0, 150.0, 70.0), style: style.clone(), glyphs: Vec::new() },
                ],
                runs: Vec::new(),
                tab_stops: Vec::new(),
            }],
            space_before: 0.0,
            space_after: 0.0,
            is_list: false,
            list_id: 0,
            list_level: 0,
            tag: "P".to_string(),
            is_header: false,
            alignment: "left".to_string(),
            line_spacing_twips: None,
            line_rule: "auto".to_string(),
            column_index: 0,
        };

        let mut blocks = vec![block];
        HeadingClassifier::process_paragraphs(&mut blocks, 595.0);

        assert_eq!(blocks[0].tag, "H1");
        assert!(blocks[0].is_header);
        assert_eq!(blocks[0].lines[0].runs.len(), 1);
        assert_eq!(blocks[0].lines[0].runs[0].text, "Chapter One");
    }
}

