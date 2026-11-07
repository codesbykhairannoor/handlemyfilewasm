use crate::models::{ParagraphBlock, TextLine, Word};

pub struct RecursiveXYCut;

impl RecursiveXYCut {
    /// Segments a collection of Words into structured ParagraphBlocks and TextLines
    pub fn segment(words: Vec<Word>, page_width: f64, page_height: f64) -> Vec<ParagraphBlock> {
        if words.is_empty() {
            return Vec::new();
        }

        let column_gap_threshold = 8.0;   // In points: minimum whitespace between columns (supports IEEE 9.0 pt)
        let para_gap_threshold = 12.0;    // In points: minimum vertical gap between paragraphs (accommodates 1.5x line spacing)

        let mut blocks = Vec::new();
        rxyc_banded(words, column_gap_threshold, para_gap_threshold, page_width, page_height, &mut blocks);
        blocks
    }
}

/// Band-decomposed Top-Down Layout Segmentation (Breuel 2002 / Shafait 2008)
fn rxyc_banded(
    words: Vec<Word>,
    col_gap_thresh: f64,
    para_gap_thresh: f64,
    page_width: f64,
    page_height: f64,
    out_blocks: &mut Vec<ParagraphBlock>,
) {
    if words.is_empty() {
        return;
    }

    let min_x = words.iter().map(|w| w.bbox.x_min).fold(f64::INFINITY, f64::min);
    let max_x = words.iter().map(|w| w.bbox.x_max).fold(f64::NEG_INFINITY, f64::max);
    let content_w = (max_x - min_x).max(100.0);
    let center = page_width / 2.0;

    // 1. Group words into preliminary line fragments
    let mut sorted_words = words;
    sorted_words.sort_by(|a, b| {
        let cy_a = (a.bbox.y_min + a.bbox.y_max) / 2.0;
        let cy_b = (b.bbox.y_min + b.bbox.y_max) / 2.0;
        cy_a.partial_cmp(&cy_b)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.bbox.x_min.partial_cmp(&b.bbox.x_min).unwrap_or(std::cmp::Ordering::Equal))
    });

    let mut line_fragments: Vec<Vec<Word>> = Vec::new();
    for w in sorted_words {
        let mut matched = false;
        let cy = (w.bbox.y_min + w.bbox.y_max) / 2.0;

        for line in line_fragments.iter_mut() {
            if let Some(first) = line.first() {
                let first_cy = (first.bbox.y_min + first.bbox.y_max) / 2.0;
                let h = first.bbox.height().max(w.bbox.height()).max(8.0);
                if (first_cy - cy).abs() <= (h * 0.4) {
                    let line_min_x = line.iter().map(|item| item.bbox.x_min).fold(f64::INFINITY, f64::min);
                    let line_max_x = line.iter().map(|item| item.bbox.x_max).fold(f64::NEG_INFINITY, f64::max);
                    let is_gutter_gap = if w.bbox.x_min >= line_max_x {
                        let gap = w.bbox.x_min - line_max_x;
                        let gap_mid = (line_max_x + w.bbox.x_min) / 2.0;
                        gap >= 16.0 && (gap_mid - center).abs() <= 50.0
                    } else if w.bbox.x_max <= line_min_x {
                        let gap = line_min_x - w.bbox.x_max;
                        let gap_mid = (w.bbox.x_max + line_min_x) / 2.0;
                        gap >= 16.0 && (gap_mid - center).abs() <= 50.0
                    } else {
                        false
                    };

                    if !is_gutter_gap {
                        line.push(w.clone());
                        matched = true;
                        break;
                    }
                }
            }
        }

        if !matched {
            line_fragments.push(vec![w]);
        }
    }

    // 2. Classify line fragments as spanning vs columnar
    struct LineFragInfo {
        words: Vec<Word>,
        y_min: f64,
        y_max: f64,
        is_spanning: bool,
    }

    let mut frags: Vec<LineFragInfo> = Vec::new();
    let mut has_spanning = false;
    let mut has_columnar = false;

    for line_w in line_fragments {
        let lx_min = line_w.iter().map(|w| w.bbox.x_min).fold(f64::INFINITY, f64::min);
        let lx_max = line_w.iter().map(|w| w.bbox.x_max).fold(f64::NEG_INFINITY, f64::max);
        let ly_min = line_w.iter().map(|w| w.bbox.y_min).fold(f64::INFINITY, f64::min);
        let ly_max = line_w.iter().map(|w| w.bbox.y_max).fold(f64::NEG_INFINITY, f64::max);
        let lw = lx_max - lx_min;

        let is_spanning = lw >= (0.60 * content_w)
            || (lx_min < (center - 15.0) && lx_max > (center + 15.0));

        if is_spanning {
            has_spanning = true;
        } else {
            has_columnar = true;
        }

        frags.push(LineFragInfo {
            words: line_w,
            y_min: ly_min,
            y_max: ly_max,
            is_spanning,
        });
    }

    // If pure single-column or no mixed spanning/columnar lines, fallback to standard RXYC
    if !has_spanning || !has_columnar {
        let all_words: Vec<Word> = frags.into_iter().flat_map(|f| f.words).collect();
        rxyc_recursive(all_words, col_gap_thresh, para_gap_thresh, page_height, out_blocks);
        return;
    }

    // 3. Partition into Horizontal Bands
    frags.sort_by(|a, b| a.y_min.partial_cmp(&b.y_min).unwrap_or(std::cmp::Ordering::Equal));

    struct Band {
        words: Vec<Word>,
        is_spanning: bool,
        y_max: f64,
    }

    let mut bands: Vec<Band> = Vec::new();

    for f in frags {
        let is_new_band = match bands.last() {
            Some(last) => last.is_spanning != f.is_spanning || (f.y_min - last.y_max) >= 20.0,
            None => true,
        };

        if is_new_band {
            bands.push(Band {
                words: f.words,
                is_spanning: f.is_spanning,
                y_max: f.y_max,
            });
        } else {
            let last = bands.last_mut().unwrap();
            last.words.extend(f.words);
            last.y_max = last.y_max.max(f.y_max);
        }
    }

    // 4. Process each band in sequential top-down order
    for band in bands {
        if band.is_spanning {
            rxyc_recursive(band.words, col_gap_thresh, para_gap_thresh, page_height, out_blocks);
        } else {
            rxyc_recursive(band.words, col_gap_thresh, para_gap_thresh, page_height, out_blocks);
        }
    }
}

/// Recursive Top-Down X-Y Cut partitioning
fn rxyc_recursive(
    words: Vec<Word>,
    col_gap_thresh: f64,
    para_gap_thresh: f64,
    page_height: f64,
    out_blocks: &mut Vec<ParagraphBlock>,
) {
    if words.is_empty() {
        return;
    }

    // Check block vertical span
    let min_y = words.iter().map(|w| w.bbox.y_min).fold(f64::INFINITY, f64::min);
    let max_y = words.iter().map(|w| w.bbox.y_max).fold(f64::NEG_INFINITY, f64::max);
    let block_height = max_y - min_y;

    // Suppress vertical cut for single-line blocks or inline colon forms to preserve tab stops
    let v_cut = if block_height > 20.0 {
        if let Some((cut_x, gap_x)) = find_vertical_cut_with_gap(&words, col_gap_thresh) {
            // Check if cut_x is an inline colon form boundary (e.g. "Nama Pasien" -> ": Ahmad Dahlan")
            // A true colon form has multiple key-value rows where values start with ':' or keys end with ':'
            let right_colon_count = words.iter().filter(|w| {
                let right_of_cut = w.bbox.x_min >= cut_x && (w.bbox.x_min - cut_x) <= (gap_x + 10.0);
                right_of_cut && (w.text.starts_with(':') || w.text == ":")
            }).count();

            let left_colon_count = words.iter().filter(|w| {
                let left_of_cut = w.bbox.x_max <= cut_x && (cut_x - w.bbox.x_max) <= (gap_x + 10.0);
                left_of_cut && (w.text.ends_with(':') || w.text == ":")
            }).count();

            let is_colon_form = right_colon_count >= 1 || left_colon_count >= 2;

            // Check if cut_x separates items that share horizontal baselines (TOC or key-value rows)
            let right_words: Vec<&Word> = words.iter().filter(|w| w.bbox.x_min >= cut_x).collect();
            let left_words: Vec<&Word> = words.iter().filter(|w| w.bbox.x_max <= cut_x).collect();
            let shared_rows = right_words.iter().filter(|rw| {
                let r_cy = (rw.bbox.y_min + rw.bbox.y_max) / 2.0;
                left_words.iter().any(|lw| {
                    let l_cy = (lw.bbox.y_min + lw.bbox.y_max) / 2.0;
                    (l_cy - r_cy).abs() <= 3.5
                })
            }).count();

            let is_toc_or_tabular = shared_rows >= 2;

            if is_colon_form || is_toc_or_tabular {
                None
            } else {
                Some((cut_x, gap_x))
            }
        } else {
            None
        }
    } else {
        None
    };
    let h_cut = find_horizontal_cut_with_gap(&words, para_gap_thresh);

    // Ha, Haralick & Phillips (1995): Compare whitespace valley depths.
    // When a prominent vertical gutter (gap_x > gap_y) spans across lines,
    // partition parallel columns first; otherwise horizontal cuts partition paragraphs.
    match (v_cut, h_cut) {
        (Some((cut_x, _gap_x)), Some((_cut_y, _gap_y))) => {
            // In document layout analysis (Breuel 2002 / Shafait 2008), when a valid
            // vertical column gutter exists, partition parallel columns FIRST to preserve
            // natural Western reading order (Column 0 top-to-bottom, then Column 1).
            split_vertical(words, cut_x, col_gap_thresh, para_gap_thresh, page_height, out_blocks);
        }
        (Some((cut_x, _)), None) => {
            split_vertical(words, cut_x, col_gap_thresh, para_gap_thresh, page_height, out_blocks);
        }
        (None, Some((cut_y, _))) => {
            split_horizontal(words, cut_y, col_gap_thresh, para_gap_thresh, page_height, out_blocks);
        }
        (None, None) => {
            out_blocks.extend(assemble_paragraphs_from_words(words, page_height));
        }
    }
}

fn split_vertical(
    words: Vec<Word>,
    cut_x: f64,
    col_gap_thresh: f64,
    para_gap_thresh: f64,
    page_height: f64,
    out_blocks: &mut Vec<ParagraphBlock>,
) {
    let mut left_words = Vec::new();
    let mut right_words = Vec::new();

    for w in words.into_iter() {
        let cx = (w.bbox.x_min + w.bbox.x_max) / 2.0;
        if cx < cut_x {
            left_words.push(w);
        } else {
            right_words.push(w);
        }
    }

    if left_words.is_empty() || right_words.is_empty() {
        let all = if !left_words.is_empty() { left_words } else { right_words };
        out_blocks.extend(assemble_paragraphs_from_words(all, page_height));
        return;
    }

    // Process left column first (natural Western reading order), then right column
    rxyc_recursive(left_words, col_gap_thresh, para_gap_thresh, page_height, out_blocks);
    rxyc_recursive(right_words, col_gap_thresh, para_gap_thresh, page_height, out_blocks);
}

fn split_horizontal(
    words: Vec<Word>,
    cut_y: f64,
    col_gap_thresh: f64,
    para_gap_thresh: f64,
    page_height: f64,
    out_blocks: &mut Vec<ParagraphBlock>,
) {
    let mut top_words = Vec::new();
    let mut bot_words = Vec::new();

    for w in words.into_iter() {
        let cy = (w.bbox.y_min + w.bbox.y_max) / 2.0;
        if cy < cut_y {
            top_words.push(w);
        } else {
            bot_words.push(w);
        }
    }

    if top_words.is_empty() || bot_words.is_empty() {
        let all = if !top_words.is_empty() { top_words } else { bot_words };
        out_blocks.extend(assemble_paragraphs_from_words(all, page_height));
        return;
    }

    // Process top block first, then bottom block
    rxyc_recursive(top_words, col_gap_thresh, para_gap_thresh, page_height, out_blocks);
    rxyc_recursive(bot_words, col_gap_thresh, para_gap_thresh, page_height, out_blocks);
}

/// Finds a vertical whitespace valley Pv(x) = 0 with width >= threshold
fn find_vertical_cut_with_gap(words: &[Word], min_gap: f64) -> Option<(f64, f64)> {
    if words.len() < 2 {
        return None;
    }

    // A vertical cut (column split) should only be considered if the words span multiple lines vertically.
    // Words residing on a single horizontal line (vertical span < 15.0 pt) represent tab stops
    // or wide horizontal gaps on that line, NOT a multi-column document layout.
    let mut min_y = f64::MAX;
    let mut max_y = f64::MIN;
    for w in words {
        if w.bbox.y_min < min_y { min_y = w.bbox.y_min; }
        if w.bbox.y_max > max_y { max_y = w.bbox.y_max; }
    }
    if (max_y - min_y) < 15.0 {
        return None;
    }

    let mut x_intervals: Vec<(f64, f64)> = words
        .iter()
        .map(|w| (w.bbox.x_min, w.bbox.x_max))
        .collect();

    x_intervals.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut merged: Vec<(f64, f64)> = Vec::new();
    for (start, end) in x_intervals {
        if let Some(last) = merged.last_mut() {
            if start <= last.1 {
                last.1 = last.1.max(end);
                continue;
            }
        }
        merged.push((start, end));
    }

    let mut best_cut = None;
    let mut max_gap = 0.0;

    for i in 0..merged.len().saturating_sub(1) {
        let gap = merged[i + 1].0 - merged[i].1;
        if gap >= min_gap && gap > max_gap {
            max_gap = gap;
            best_cut = Some(((merged[i].1 + merged[i + 1].0) / 2.0, gap));
        }
    }

    best_cut
}

/// Finds a horizontal whitespace valley Ph(y) = 0 with height >= threshold
fn find_horizontal_cut_with_gap(words: &[Word], min_gap: f64) -> Option<(f64, f64)> {
    if words.len() < 2 {
        return None;
    }

    let mut y_intervals: Vec<(f64, f64)> = words
        .iter()
        .map(|w| (w.bbox.y_min, w.bbox.y_max))
        .collect();

    y_intervals.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut merged: Vec<(f64, f64)> = Vec::new();
    for (start, end) in y_intervals {
        if let Some(last) = merged.last_mut() {
            if start <= last.1 {
                last.1 = last.1.max(end);
                continue;
            }
        }
        merged.push((start, end));
    }

    let mut best_cut = None;
    let mut max_gap = 0.0;

    for i in 0..merged.len().saturating_sub(1) {
        let gap = merged[i + 1].0 - merged[i].1;
        if gap >= min_gap && gap > max_gap {
            max_gap = gap;
            best_cut = Some(((merged[i].1 + merged[i + 1].0) / 2.0, gap));
        }
    }

    best_cut
}

/// Assembles Words into TextLines, and TextLines into ParagraphBlocks (splitting on bullet items)
fn assemble_paragraphs_from_words(mut words: Vec<Word>, page_height: f64) -> Vec<ParagraphBlock> {
    if words.is_empty() {
        return Vec::new();
    }

    // Sort words: primarily by Y center, secondarily by X
    words.sort_by(|a, b| {
        let cy_a = (a.bbox.y_min + a.bbox.y_max) / 2.0;
        let cy_b = (b.bbox.y_min + b.bbox.y_max) / 2.0;
        cy_a.partial_cmp(&cy_b)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.bbox.x_min.partial_cmp(&b.bbox.x_min).unwrap_or(std::cmp::Ordering::Equal))
    });

    let mut lines_words: Vec<Vec<Word>> = Vec::new();

    for w in words {
        let mut matched = false;
        let cy = (w.bbox.y_min + w.bbox.y_max) / 2.0;

        for line in lines_words.iter_mut() {
            if let Some(first) = line.first() {
                let first_cy = (first.bbox.y_min + first.bbox.y_max) / 2.0;
                let h = first.bbox.height().max(w.bbox.height()).max(8.0);
                if (first_cy - cy).abs() <= (h * 0.4) {
                    // Words within the same segmented column/block sharing the baseline belong to the same TextLine
                    line.push(w.clone());
                    matched = true;
                    break;
                }
            }
        }

        if !matched {
            lines_words.push(vec![w]);
        }
    }

    let mut text_lines = Vec::new();

    for mut line_w in lines_words {
        // Sort line words left-to-right
        line_w.sort_by(|a, b| a.bbox.x_min.partial_cmp(&b.bbox.x_min).unwrap_or(std::cmp::Ordering::Equal));

        let mut line_bbox = line_w[0].bbox;
        for w in &line_w {
            line_bbox.expand(&w.bbox);
        }

        text_lines.push(TextLine {
            bbox: line_bbox,
            words: line_w,
            runs: Vec::new(),
            tab_stops: Vec::new(),
        });
    }

    // Split into distinct ParagraphBlocks on bullet/numbered items, TOC lines, paragraph gaps,
    // font size transitions, or heading style boundaries.
    let mut blocks = Vec::new();
    let mut curr_lines: Vec<TextLine> = Vec::new();

    for line in text_lines {
        let starts_new_item = if let Some(first_word) = line.words.first() {
            let t = first_word.text.trim();
            is_bullet_glyph(t) || is_numbered_list_prefix(t)
        } else {
            false
        };

        let line_is_toc = is_toc_line_entry(&line);

        // Standalone bottom page number: must ALWAYS be an independent paragraph
        let line_text = line.words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ");
        let line_is_bottom_pn = line.bbox.y_min >= (page_height - 90.0) && line.words.len() <= 2 && crate::dla::is_page_number_pattern(&line_text);

        let is_para_break = if let Some(prev) = curr_lines.last() {
            let prev_is_toc = is_toc_line_entry(prev);
            let prev_text = prev.words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ");
            let prev_is_bottom_pn = prev.bbox.y_min >= (page_height - 90.0) && prev.words.len() <= 2 && crate::dla::is_page_number_pattern(&prev_text);

            if line_is_toc || prev_is_toc || starts_new_item || line_is_bottom_pn || prev_is_bottom_pn {
                true
            } else {
                let gap = line.bbox.y_min - prev.bbox.y_max;
                let prev_h = prev.bbox.height().max(8.0);

                let prev_last_word = prev.words.last().map(|w| w.text.trim()).unwrap_or("");
                let ends_sentence = prev_last_word.ends_with('.')
                    || prev_last_word.ends_with('?')
                    || prev_last_word.ends_with('!')
                    || prev_last_word.ends_with(':')
                    || prev_last_word.ends_with(';')
                    || prev_last_word.ends_with(".\"")
                    || prev_last_word.ends_with("?'")
                    || prev_last_word.ends_with("!'");

                // Dominant font sizes
                let prev_size = prev.words.iter().map(|w| w.style.font_size).fold(0.0f64, f64::max);
                let curr_size = line.words.iter().map(|w| w.style.font_size).fold(0.0f64, f64::max);
                let size_diff = (curr_size - prev_size).abs();

                let prev_is_bold = prev.words.iter().all(|w| w.style.is_bold) && prev.words.len() <= 12;
                let curr_is_bold = line.words.iter().all(|w| w.style.is_bold) && line.words.len() <= 12;

                if size_diff >= 2.5 || (curr_is_bold && !prev_is_bold && gap >= 4.0) {
                    true
                } else if !ends_sentence {
                    // Continuation line of a running sentence: never break into a new paragraph
                    // unless vertical gap is unmistakably large (section or block break)
                    gap >= 18.0
                } else {
                    // Sentence completed on previous line: break into new paragraph only if
                    // there is significant paragraph gap or first-line indentation
                    let is_indented = (line.bbox.x_min - prev.bbox.x_min).abs() >= 14.0;
                    gap >= 12.0 || gap >= (prev_h * 1.35) || (is_indented && gap >= 4.0)
                }
            }
        } else {
            false
        };

        if is_para_break && !curr_lines.is_empty() {
            blocks.push(build_para_block(std::mem::take(&mut curr_lines)));
        }
        curr_lines.push(line);
    }

    if !curr_lines.is_empty() {
        blocks.push(build_para_block(curr_lines));
    }

    blocks
}

/// Identifies whether a TextLine is a Table of Contents (Daftar Isi) entry
pub fn is_toc_line_entry(line: &TextLine) -> bool {
    let full = line.words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ");
    let dot_count = full.chars().filter(|&c| c == '.').count();
    if dot_count >= 3 && (full.contains("..") || full.contains("...")) {
        return true;
    }
    if line.words.len() >= 2 {
        if let Some(last) = line.words.last() {
            let t = last.text.trim().trim_matches(|c: char| c == '.' || c == ' ' || c == '-' || c == '[' || c == ']');
            let is_num = !t.is_empty() && t.len() <= 4 && (t.chars().all(|c| c.is_ascii_digit()) || is_roman_num(t));
            if is_num {
                let prev = &line.words[line.words.len() - 2];
                if (last.bbox.x_min - prev.bbox.x_max) >= 20.0 || last.bbox.x_min >= 350.0 {
                    return true;
                }
            }
        }
    }
    false
}

fn is_roman_num(t: &str) -> bool {
    let lower = t.to_lowercase();
    let roman = [
        "i", "ii", "iii", "iv", "v", "vi", "vii", "viii", "ix", "x",
        "xi", "xii", "xiii", "xiv", "xv", "xvi", "xvii", "xviii", "xix", "xx",
    ];
    roman.contains(&lower.as_str())
}

fn build_para_block(lines: Vec<TextLine>) -> ParagraphBlock {
    let mut block_bbox = lines[0].bbox;
    for l in &lines {
        block_bbox.expand(&l.bbox);
    }

    ParagraphBlock {
        bbox: block_bbox,
        lines,
        space_before: 0.0,
        space_after: 4.0,
        is_list: false,
        list_id: 0,
        list_level: 0,
        tag: "P".to_string(),
        is_header: false,
        alignment: "left".to_string(),
        line_spacing_twips: None,
        line_rule: "auto".to_string(),
        column_index: 0,
    }
}

fn is_bullet_glyph(t: &str) -> bool {
    t == "•" || t == "-" || t == "*" || t == "▪" || t == "▫" || t == "◦" || t == "–" || t == "—"
        || t == "" || t == ">" || t == "o" || t == "·" || t == "●" || t == "○" || t == "■" || t == "□"
        || t == "◆" || t == "◇" || t == "\u{00B7}" || t == "\u{2022}" || t == "\u{25E6}"
}

fn is_numbered_list_prefix(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let s_clean = s.trim_matches(|c| c == '(' || c == ')' || c == '[' || c == ']');
    if let Some(first) = s_clean.chars().next() {
        if first.is_ascii_digit() {
            return s.ends_with('.') || s.ends_with(')');
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Style;

    #[test]
    fn test_find_horizontal_cut() {
        let style = Style::default();
        let words = vec![
            Word { text: "Top".to_string(), bbox: Rect::new(50.0, 50.0, 100.0, 62.0), style: style.clone(), glyphs: Vec::new() },
            Word { text: "Bottom".to_string(), bbox: Rect::new(50.0, 80.0, 100.0, 92.0), style: style.clone(), glyphs: Vec::new() },
        ];

        let cut = find_horizontal_cut(&words, 8.0);
        assert!(cut.is_some());
        let y = cut.unwrap();
        assert!(y > 62.0 && y < 80.0);
    }
}

