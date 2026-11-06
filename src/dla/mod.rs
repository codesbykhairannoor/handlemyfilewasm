pub mod docstrum;
pub mod rxyc;
pub mod classifier;
pub mod tables;
pub mod tabs;

pub use docstrum::{DocstrumAnalyzer, DocstrumMetrics};
pub use rxyc::RecursiveXYCut;
pub use classifier::{HeadingClassifier, build_line_runs};
pub use tables::TableRecognizer;
pub use tabs::TabStopDetector;

use crate::models::DocumentPage;

pub struct LayoutAnalyzer;

impl LayoutAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Primary DLA pipeline: Processes raw extracted pages into structured layout
    pub fn analyze(&self, mut pages: Vec<DocumentPage>) -> Result<Vec<DocumentPage>, String> {
        for page in pages.iter_mut() {
            // 1. If already authoritatively structured via Tagged PDF (/StructTreeRoot):
            if page.is_tagged && (!page.paragraphs.is_empty() || !page.glyphs.is_empty()) {
                // Ensure TextRuns are populated for Tagged PDF paragraphs
                for p in page.paragraphs.iter_mut() {
                    build_line_runs(&mut p.lines);
                }
                HeadingClassifier::process_paragraphs(&mut page.paragraphs, page.width_points);
                TabStopDetector::process_paragraph_tabs(&mut page.paragraphs);

                // Anti-Black-Hole: Harvest unclaimed glyphs (tables, headers, footers, metadata)
                let mut unclaimed_glyphs = Vec::new();
                for g in &page.glyphs {
                    let cx = (g.bbox.x_min + g.bbox.x_max) / 2.0;
                    let cy = (g.bbox.y_min + g.bbox.y_max) / 2.0;

                    let in_para = page.paragraphs.iter().any(|p| {
                        (p.bbox.contains_point(cx, cy) || p.lines.iter().any(|l| l.bbox.contains_point(cx, cy)))
                            && (cy >= (p.bbox.y_min - 2.0) && cy <= (p.bbox.y_max + 2.0))
                    });

                    if !in_para {
                        unclaimed_glyphs.push(g.clone());
                    }
                }

                if !unclaimed_glyphs.is_empty() {
                    let metrics = DocstrumAnalyzer::compute_metrics(&unclaimed_glyphs);
                    let mut unclaimed_words = DocstrumAnalyzer::cluster_words(&unclaimed_glyphs, &metrics);

                    // 1. Extract tables geometrically with 100% precision from vector paths & words
                    let extra_tables = TableRecognizer::extract_tables_from_words(&page.vector_paths, &mut unclaimed_words);
                    page.tables.extend(extra_tables);

                    // 2. Segment remaining unclaimed words into paragraphs
                    if !unclaimed_words.is_empty() {
                        let mut extra_paras = RecursiveXYCut::segment(unclaimed_words, page.width_points, page.height_points);
                        HeadingClassifier::process_paragraphs(&mut extra_paras, page.width_points);
                        TabStopDetector::process_paragraph_tabs(&mut extra_paras);
                        page.paragraphs.extend(extra_paras);
                    }

                    // 3. Re-sort paragraphs and tables vertically
                    page.paragraphs.sort_by(|a, b| a.bbox.y_min.partial_cmp(&b.bbox.y_min).unwrap_or(std::cmp::Ordering::Equal));
                    page.tables.sort_by(|a, b| a.bbox.y_min.partial_cmp(&b.bbox.y_min).unwrap_or(std::cmp::Ordering::Equal));
                }

                compute_and_set_page_margins(page);
            } else if page.paragraphs.is_empty() && !page.glyphs.is_empty() {
                // 2. Fallback Heuristic Path (Untagged PDF): Docstrum + Recursive X-Y Cut + TSR
                // 2a. Docstrum k-NN Spatial Metric Calculation
                let metrics = DocstrumAnalyzer::compute_metrics(&page.glyphs);

                // 2b. Word Clustering
                let mut words = DocstrumAnalyzer::cluster_words(&page.glyphs, &metrics);

                // 2c. Pre-Segmentation Table Extraction (Nurminen 2013): Harvests cell words and isolates them
                let extracted_tables = TableRecognizer::extract_tables_from_words(&page.vector_paths, &mut words);
                page.tables.extend(extracted_tables);

                // 2d. Top-Down Band & Column Layout Segmentation on remaining body words
                let mut paragraphs = RecursiveXYCut::segment(words, page.width_points, page.height_points);

                // 2e. Semantic Heading, List Classification & TextRun Consolidation + Alignment & Spacing
                HeadingClassifier::process_paragraphs(&mut paragraphs, page.width_points);

                // 2f. Custom Tab Stops detection (colon forms, dot leaders)
                TabStopDetector::process_paragraph_tabs(&mut paragraphs);

                page.paragraphs = paragraphs;
                compute_and_set_page_margins(page);
            }

            // 3. Post-processing: Image background classification & Table deduplication
            classify_page_graphics(page);
            deduplicate_page_tables(&mut page.tables);
            assign_paragraph_column_indices(page);
        }

        // 4. Cross-page detection for recurring template/watermark/header graphics
        if pages.len() > 1 {
            let mut recurring_bboxes: Vec<crate::models::Rect> = Vec::new();
            for i in 0..pages.len() {
                for g1 in &pages[i].graphics {
                    for j in (i + 1)..pages.len() {
                        for g2 in &pages[j].graphics {
                            let dx = (g1.bbox.x_min - g2.bbox.x_min).abs();
                            let dy = (g1.bbox.y_min - g2.bbox.y_min).abs();
                            let dw = (g1.bbox.width() - g2.bbox.width()).abs();
                            let dh = (g1.bbox.height() - g2.bbox.height()).abs();
                            if dx < 4.0 && dy < 4.0 && dw < 4.0 && dh < 4.0 {
                                if !recurring_bboxes.iter().any(|b| {
                                    (b.x_min - g1.bbox.x_min).abs() < 4.0
                                        && (b.y_min - g1.bbox.y_min).abs() < 4.0
                                }) {
                                    recurring_bboxes.push(g1.bbox.clone());
                                }
                            }
                        }
                    }
                }
            }

            for page in pages.iter_mut() {
                for g in page.graphics.iter_mut() {
                    if recurring_bboxes.iter().any(|b| {
                        (b.x_min - g.bbox.x_min).abs() < 4.0
                            && (b.y_min - g.bbox.y_min).abs() < 4.0
                            && (b.width() - g.bbox.width()).abs() < 4.0
                            && (b.height() - g.bbox.height()).abs() < 4.0
                    }) {
                        g.is_inline = false;
                        g.is_background = true;
                    }
                }
            }
        }

        // 5. Filter static header/footer/page numbers from body text FIRST
        filter_headers_and_footers(&mut pages);

        // 6. Multi-page paragraph stitching for natural flow on body text ONLY
        stitch_cross_page_continuations(&mut pages);

        // 7. Final Inter-Paragraph Geometric Spacing Calculation
        // Calculate true vertical delta between adjacent paragraphs in the final body list
        for page in pages.iter_mut() {
            let num_p = page.paragraphs.len();
            for i in 0..num_p {
                page.paragraphs[i].space_after = 0.0;
                if i == 0 {
                    page.paragraphs[i].space_before = 0.0;
                } else {
                    let prev_bottom = page.paragraphs[i - 1].bbox.y_max;
                    let curr_top = page.paragraphs[i].bbox.y_min;
                    let gap = (curr_top - prev_bottom).max(0.0);
                    page.paragraphs[i].space_before = (gap - 2.0).clamp(0.0, 24.0);
                }
            }
        }

        Ok(pages)
    }
}

/// Stitches paragraphs that were split across physical page boundaries mid-sentence
fn stitch_cross_page_continuations(pages: &mut [DocumentPage]) {
    if pages.len() < 2 {
        return;
    }

    for i in 0..pages.len() - 1 {
        let should_stitch = {
            let p_curr = &pages[i].paragraphs;
            let p_next = &pages[i + 1].paragraphs;
            if !p_curr.is_empty() && !p_next.is_empty() {
                let last_p = &p_curr[p_curr.len() - 1];
                let first_p = &p_next[0];

                let last_text = last_p
                    .lines
                    .iter()
                    .flat_map(|l| l.words.iter().map(|w| w.text.as_str()))
                    .collect::<Vec<_>>()
                    .join(" ");
                let first_text = first_p
                    .lines
                    .iter()
                    .flat_map(|l| l.words.iter().map(|w| w.text.as_str()))
                    .collect::<Vec<_>>()
                    .join(" ");

                let trimmed_last = last_text.trim();
                let trimmed_first = first_text.trim();

                let ends_with_punct = trimmed_last.ends_with('.')
                    || trimmed_last.ends_with(':')
                    || trimmed_last.ends_with('?')
                    || trimmed_last.ends_with('!')
                    || trimmed_last.ends_with(';');

                let first_char = trimmed_first.chars().next().unwrap_or('A');
                let starts_with_lower = first_char.is_lowercase();

                !last_p.is_header
                    && !first_p.is_header
                    && !last_p.is_list
                    && !first_p.is_list
                    && !ends_with_punct
                    && starts_with_lower
            } else {
                false
            }
        };

        if should_stitch {
            let first_p = pages[i + 1].paragraphs.remove(0);
            if let Some(last_p) = pages[i].paragraphs.last_mut() {
                last_p.lines.extend(first_p.lines);
                // Preserve last_p's y_min on page i so it does not get sorted above preceding headings!
                last_p.bbox.x_min = last_p.bbox.x_min.min(first_p.bbox.x_min);
                last_p.bbox.x_max = last_p.bbox.x_max.max(first_p.bbox.x_max);
            }
        }
    }
}

/// Identifies and filters static header/footer/page-number artifacts from body text
fn filter_headers_and_footers(pages: &mut [DocumentPage]) {
    // 1. Identify recurring running header text at top of pages
    let mut top_header_texts: Vec<String> = Vec::new();
    if pages.len() > 1 {
        let mut header_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for page in pages.iter() {
            for p in &page.paragraphs {
                if p.bbox.y_max <= 50.0 && p.lines.len() <= 1 {
                    let text = get_paragraph_full_text(p);
                    if text.len() >= 4 {
                        *header_counts.entry(text).or_insert(0) += 1;
                    }
                }
            }
        }
        for (text, count) in header_counts {
            if count >= 2 {
                top_header_texts.push(text);
            }
        }
    }

    // 2. Extract and remove headers and footers from body paragraphs
    for page in pages.iter_mut() {
        let page_h = page.height_points;

        let mut header_blocks: Vec<crate::models::ParagraphBlock> = Vec::new();
        let mut footer_blocks: Vec<crate::models::ParagraphBlock> = Vec::new();
        let mut kept_paragraphs = Vec::with_capacity(page.paragraphs.len());

        for p in page.paragraphs.drain(..) {
            let full_text = get_paragraph_full_text(&p);
            if full_text.is_empty() {
                continue;
            }

            let cy = (p.bbox.y_min + p.bbox.y_max) / 2.0;

            // Header Zone: STRICTLY top margin strip (y_max <= 35.0)
            let is_in_top_strip = (p.bbox.y_max <= 35.0 || cy <= 30.0) && p.bbox.y_min < (page_h * 0.35);
            let is_recurring_header = p.bbox.y_max <= 40.0 && p.bbox.y_min < (page_h * 0.35) && top_header_texts.iter().any(|h| h == &full_text);

            if (is_in_top_strip || is_recurring_header) && p.lines.len() <= 2 {
                header_blocks.push(p);
                continue; // Remove from body!
            }

            // Standalone page number (digits, "1/8", roman numerals)
            let is_standalone_num = is_page_number_pattern(&full_text) && p.lines.len() <= 1;

            // Footer Zone: STRICTLY bottom margin strip (y_min >= page_h - 35.0)
            let is_in_bottom_strip = (p.bbox.y_min >= (page_h - 35.0) || cy >= (page_h - 30.0)) && p.bbox.y_min >= (page_h * 0.65);
            let is_bottom_page_num = is_standalone_num && cy >= (page_h - 45.0) && p.bbox.y_min >= (page_h * 0.65);

            if (is_in_bottom_strip || is_bottom_page_num) && p.lines.len() <= 2 {
                footer_blocks.push(p);
                continue; // Remove from body!
            }

            // Top-right standalone page number (e.g. thesis header page number)
            let is_top_page_num = is_standalone_num && (cy <= 45.0 || p.bbox.y_max <= 40.0) && p.bbox.y_min < (page_h * 0.35);
            if is_top_page_num {
                header_blocks.push(p);
                continue; // Remove from body!
            }

            kept_paragraphs.push(p);
        }

        page.paragraphs = kept_paragraphs;

        // Assemble unified header with tab separation for left/right elements
        page.header = if header_blocks.is_empty() {
            None
        } else if header_blocks.len() == 1 {
            Some(header_blocks.remove(0))
        } else {
            header_blocks.sort_by(|a, b| a.bbox.x_min.partial_cmp(&b.bbox.x_min).unwrap_or(std::cmp::Ordering::Equal));
            let mut base = header_blocks.remove(0);
            for next_b in header_blocks {
                base.bbox.expand(&next_b.bbox);
                if let (Some(base_line), Some(next_line)) = (base.lines.first_mut(), next_b.lines.first()) {
                    if base_line.runs.is_empty() {
                        base_line.runs.push(crate::models::TextRun {
                            text: base_line.words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" "),
                            style: base_line.words.first().map(|w| w.style.clone()).unwrap_or_default(),
                            is_tab: false,
                            is_leader_dot: false,
                        });
                    }
                    base_line.runs.push(crate::models::TextRun {
                        text: String::new(),
                        style: base_line.words.first().map(|w| w.style.clone()).unwrap_or_default(),
                        is_tab: true,
                        is_leader_dot: false,
                    });
                    let next_runs = if !next_line.runs.is_empty() {
                        next_line.runs.clone()
                    } else {
                        vec![crate::models::TextRun {
                            text: next_line.words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" "),
                            style: next_line.words.first().map(|w| w.style.clone()).unwrap_or_default(),
                            is_tab: false,
                            is_leader_dot: false,
                        }]
                    };
                    base_line.runs.extend(next_runs);
                    base_line.words.extend(next_line.words.clone());
                }
            }
            Some(base)
        };

        // Assemble unified footer with tab separation for left/right elements
        page.footer = if footer_blocks.is_empty() {
            None
        } else if footer_blocks.len() == 1 {
            Some(footer_blocks.remove(0))
        } else {
            footer_blocks.sort_by(|a, b| a.bbox.x_min.partial_cmp(&b.bbox.x_min).unwrap_or(std::cmp::Ordering::Equal));
            let mut base = footer_blocks.remove(0);
            for next_b in footer_blocks {
                base.bbox.expand(&next_b.bbox);
                if let (Some(base_line), Some(next_line)) = (base.lines.first_mut(), next_b.lines.first()) {
                    if base_line.runs.is_empty() {
                        base_line.runs.push(crate::models::TextRun {
                            text: base_line.words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" "),
                            style: base_line.words.first().map(|w| w.style.clone()).unwrap_or_default(),
                            is_tab: false,
                            is_leader_dot: false,
                        });
                    }
                    base_line.runs.push(crate::models::TextRun {
                        text: String::new(),
                        style: base_line.words.first().map(|w| w.style.clone()).unwrap_or_default(),
                        is_tab: true,
                        is_leader_dot: false,
                    });
                    let next_runs = if !next_line.runs.is_empty() {
                        next_line.runs.clone()
                    } else {
                        vec![crate::models::TextRun {
                            text: next_line.words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" "),
                            style: next_line.words.first().map(|w| w.style.clone()).unwrap_or_default(),
                            is_tab: false,
                            is_leader_dot: false,
                        }]
                    };
                    base_line.runs.extend(next_runs);
                    base_line.words.extend(next_line.words.clone());
                }
            }
            Some(base)
        };
    }
}

fn get_paragraph_full_text(p: &crate::models::ParagraphBlock) -> String {
    p.lines
        .iter()
        .flat_map(|l| l.words.iter().map(|w| w.text.as_str()))
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn is_page_number_pattern(text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() {
        return false;
    }

    // 1. Pure digits or bracketed digits: "1", "11", "- 11 -", "[11]", "| 11 |", ". 11 ."
    let clean = t.trim_matches(|c: char| c == '-' || c == '[' || c == ']' || c == '|' || c == '•' || c == '.' || c == ' ');
    if !clean.is_empty() && clean.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }

    // 2. Language-agnostic page fractions and numerical ratios:
    // "11 / 50", "11/50", "11 of 50", "11 dari 50" (any single delimiter word between two pure numbers)
    let words: Vec<&str> = t.split_whitespace().collect();
    if words.len() == 3 && words[0].chars().all(|c| c.is_ascii_digit()) && words[2].chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    // Universal prefixes: "p. 5", "pg. 5", "page 5", "hal. 5", "hlm. 5"
    if words.len() == 2 && words[1].chars().all(|c| c.is_ascii_digit()) {
        let w0 = words[0].to_lowercase();
        let w0_clean = w0.trim_matches(|c: char| c == '.' || c == ':');
        if matches!(w0_clean, "p" | "pg" | "page" | "hal" | "hlm" | "seite" | "pag" | "pagina") {
            return true;
        }
    }
    // "Page 1 of 10", "Hal 1 dari 10"
    if words.len() == 4 && words[1].chars().all(|c| c.is_ascii_digit()) && words[3].chars().all(|c| c.is_ascii_digit()) {
        let w0 = words[0].to_lowercase();
        let w0_clean = w0.trim_matches(|c: char| c == '.' || c == ':');
        if matches!(w0_clean, "p" | "pg" | "page" | "hal" | "hlm" | "seite" | "pag" | "pagina") {
            return true;
        }
    }
    if t.contains('/') {
        let parts: Vec<&str> = t.split('/').collect();
        if parts.len() == 2 && parts[0].trim().chars().all(|c| c.is_ascii_digit()) && parts[1].trim().chars().all(|c| c.is_ascii_digit()) {
            return true;
        }
    }

    // 3. Lowercase Roman numerals (i..xx)
    let lower = t.to_lowercase();
    let roman = [
        "i", "ii", "iii", "iv", "v", "vi", "vii", "viii", "ix", "x",
        "xi", "xii", "xiii", "xiv", "xv", "xvi", "xvii", "xviii", "xix", "xx",
    ];
    if roman.contains(&lower.as_str()) {
        return true;
    }

    false
}

/// Computes visual margins based on dominant content bounding edges
fn compute_and_set_page_margins(page: &mut DocumentPage) {
    let mut left_xs: Vec<f64> = Vec::new();
    let mut right_xs: Vec<f64> = Vec::new();
    let mut top_ys: Vec<f64> = Vec::new();
    let mut bottom_ys: Vec<f64> = Vec::new();

    for p in &page.paragraphs {
        for l in &p.lines {
            if l.bbox.width() > 10.0 {
                left_xs.push(l.bbox.x_min);
                right_xs.push(page.width_points - l.bbox.x_max);
                top_ys.push(l.bbox.y_min);
                bottom_ys.push(page.height_points - l.bbox.y_max);
            }
        }
    }
    for t in &page.tables {
        left_xs.push(t.bbox.x_min);
        right_xs.push(page.width_points - t.bbox.x_max);
        top_ys.push(t.bbox.y_min);
        bottom_ys.push(page.height_points - t.bbox.y_max);
    }

    if left_xs.is_empty() {
        page.margin_left_points = 72.0;
        page.margin_right_points = 72.0;
        page.margin_top_points = 72.0;
        page.margin_bottom_points = 72.0;
        return;
    }

    left_xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    right_xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    top_ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    bottom_ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // Horizontal margins: 15th percentile of valid margins
    let valid_left: Vec<f64> = left_xs.into_iter().filter(|&x| x >= 14.0 && x <= page.width_points * 0.40).collect();
    let valid_right: Vec<f64> = right_xs.into_iter().filter(|&x| x >= 14.0 && x <= page.width_points * 0.40).collect();

    let p10_left = if !valid_left.is_empty() {
        let idx = ((valid_left.len() as f64) * 0.15).floor() as usize;
        valid_left[idx.min(valid_left.len() - 1)]
    } else {
        72.0
    };

    let p10_right = if !valid_right.is_empty() {
        let idx = ((valid_right.len() as f64) * 0.15).floor() as usize;
        valid_right[idx.min(valid_right.len() - 1)]
    } else {
        72.0
    };

    // Vertical margins: top content position and bottom content position
    let top_margin = if !top_ys.is_empty() {
        let first_y = top_ys[0];
        if first_y < 25.0 {
            // Running header at top; body starts at second line or ~36 pt
            let second_y = top_ys.iter().cloned().find(|&y| y >= 25.0).unwrap_or(first_y);
            second_y.clamp(28.0, 72.0)
        } else {
            first_y.clamp(28.0, 72.0)
        }
    } else {
        72.0
    };

    let bottom_margin = if !bottom_ys.is_empty() {
        let first_b = bottom_ys[0];
        if first_b < 25.0 {
            // Running footer at bottom
            let second_b = bottom_ys.iter().cloned().find(|&b| b >= 25.0).unwrap_or(first_b);
            second_b.clamp(28.0, 72.0)
        } else {
            first_b.clamp(28.0, 72.0)
        }
    } else {
        72.0
    };

    page.margin_left_points = p10_left.clamp(28.0, 108.0);
    page.margin_right_points = p10_right.clamp(28.0, 108.0);
    page.margin_top_points = top_margin;
    page.margin_bottom_points = bottom_margin;
}

/// Universal Language-Agnostic Geometric Classification: Background/Underlay vs Inline Content
fn classify_page_graphics(page: &mut DocumentPage) {
    let page_area = (page.width_points * page.height_points).max(1.0);

    for g in page.graphics.iter_mut() {
        let g_area = g.bbox.area();
        let area_ratio = g_area / page_area;

        // Universal Geometric Rule 1: Full-Bleed Page Canvas Underlay (e.g. cover page background)
        // Strictly requires taking up almost the entire page canvas:
        // Width >= 85%, Height >= 80%, and Area >= 68% of total page dimensions.
        let is_full_page = g.bbox.width() >= (page.width_points * 0.85)
            && g.bbox.height() >= (page.height_points * 0.80)
            && area_ratio >= 0.68;

        if is_full_page {
            g.is_inline = false;
            g.is_background = true;
        } else {
            // Universal Geometric Rule 2: All content graphics (charts, plots, figures, photos, logos, diagrams)
            // across all languages are strictly INLINE elements (<wp:inline>) in Microsoft Word.
            // This prevents body text from ever typing over illustrations.
            g.is_inline = true;
            g.is_background = false;
        }
    }
}

/// Deduplicates overlapping tables (e.g. Tagged PDF artifacts vs Vector TSR tables)
fn deduplicate_page_tables(tables: &mut Vec<crate::models::TableMatrix>) {
    if tables.len() < 2 {
        return;
    }
    let mut to_remove = Vec::new();
    for i in 0..tables.len() {
        for j in (i + 1)..tables.len() {
            let t1 = &tables[i];
            let t2 = &tables[j];
            if t1.bbox.intersects(&t2.bbox) {
                let inter_w = (t1.bbox.x_max.min(t2.bbox.x_max) - t1.bbox.x_min.max(t2.bbox.x_min)).max(0.0);
                let inter_h = (t1.bbox.y_max.min(t2.bbox.y_max) - t1.bbox.y_min.max(t2.bbox.y_min)).max(0.0);
                let inter_area = inter_w * inter_h;
                let min_area = t1.bbox.area().min(t2.bbox.area());
                if min_area > 0.0 && (inter_area / min_area) > 0.40 {
                    // Prefer table with no tiny columns (< 25 pt) and more realistic column count
                    let t1_has_tiny = t1.grid_cols.iter().any(|&w| w < 25.0);
                    let t2_has_tiny = t2.grid_cols.iter().any(|&w| w < 25.0);
                    if t1_has_tiny && !t2_has_tiny {
                        to_remove.push(i);
                    } else if t2_has_tiny && !t1_has_tiny {
                        to_remove.push(j);
                    } else if t2.grid_cols.len() >= t1.grid_cols.len() {
                        to_remove.push(i);
                    } else {
                        to_remove.push(j);
                    }
                }
            }
        }
    }
    to_remove.sort_unstable();
    to_remove.dedup();
    for &idx in to_remove.iter().rev() {
        if idx < tables.len() {
            tables.remove(idx);
        }
    }
}

/// Detects 2-column layout and assigns column_index (0 for left, 1 for right)
fn assign_paragraph_column_indices(page: &mut DocumentPage) {
    if page.paragraphs.len() < 2 {
        return;
    }
    let w = page.width_points;
    let x_mid = w / 2.0;

    let mut left_count = 0;
    let mut right_count = 0;

    for p in &page.paragraphs {
        if p.bbox.width() <= (w * 0.60) {
            if p.bbox.x_max <= (x_mid + 12.0) {
                left_count += 1;
            } else if p.bbox.x_min >= (x_mid - 12.0) {
                right_count += 1;
            }
        }
    }

    if left_count >= 2 && right_count >= 2 {
        for p in page.paragraphs.iter_mut() {
            let cx = (p.bbox.x_min + p.bbox.x_max) / 2.0;
            if p.bbox.width() >= (w * 0.62) || (p.bbox.x_min < (x_mid - 25.0) && p.bbox.x_max > (x_mid + 25.0)) {
                // Spanning paragraph (Title, Abstract banner, full-width figure caption)
                p.column_index = 0;
            } else if cx >= x_mid || p.bbox.x_min >= (x_mid - 10.0) {
                p.column_index = 1;
            } else {
                p.column_index = 0;
            }
        }
    }
}
