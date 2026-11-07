pub mod docstrum;
pub mod otsu;
pub mod rxyc;
pub mod classifier;
pub mod tables;
pub mod tabs;

pub use docstrum::{DocstrumAnalyzer, DocstrumMetrics};
pub use otsu::{OtsuAnalyzer, TypographicScale};
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
                HeadingClassifier::process_paragraphs(&mut page.paragraphs, page.width_points, page.height_points);
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
                        HeadingClassifier::process_paragraphs(&mut extra_paras, page.width_points, page.height_points);
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
                HeadingClassifier::process_paragraphs(&mut paragraphs, page.width_points, page.height_points);

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
            let is_multi_col = page.paragraphs.iter().any(|x| x.column_index == 1);
            let w_page = page.width_points;

            struct FlowElem {
                y_max: f64,
                col: usize,
                is_wide: bool,
            }

            let mut elements: Vec<FlowElem> = Vec::new();
            for p in &page.paragraphs {
                let is_wide = p.bbox.width() >= (w_page * 0.55);
                elements.push(FlowElem {
                    y_max: p.bbox.y_max,
                    col: p.column_index,
                    is_wide,
                });
            }
            for t in &page.tables {
                let is_wide = t.bbox.width() >= (w_page * 0.55);
                let cx = (t.bbox.x_min + t.bbox.x_max) / 2.0;
                let col = if cx >= (w_page / 2.0) { 1 } else { 0 };
                elements.push(FlowElem {
                    y_max: t.bbox.y_max,
                    col,
                    is_wide,
                });
            }

            for p in page.paragraphs.iter_mut() {
                p.space_after = 0.0;
                let curr_top = p.bbox.y_min;
                let p_col = p.column_index;

                let mut prev_bottom = 0.0f64;
                let mut prev_is_wide = false;

                for elem in &elements {
                    let is_same_flow = !is_multi_col || elem.col == p_col;
                    if is_same_flow && elem.y_max <= (curr_top + 3.0) && elem.y_max > prev_bottom {
                        prev_bottom = elem.y_max;
                        prev_is_wide = elem.is_wide;
                    }
                }

                // If this is the first paragraph in a column under a section break or page margin:
                // start with 0 space_before so the column begins cleanly at the margin.
                if prev_bottom > 0.0 && !prev_is_wide {
                    let gap = (curr_top - prev_bottom).max(0.0);
                    if gap > 4.0 {
                        p.space_before = (gap - 2.0).clamp(0.0, 16.0);
                    } else {
                        p.space_before = 0.0;
                    }
                } else {
                    p.space_before = 0.0;
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

                // Safety Invariants for Cross-Page Stitching:
                // 1. Never stitch if near page footer/header strips
                let is_near_bottom = last_p.bbox.y_min >= (pages[i].height_points - 80.0);
                let is_near_top = first_p.bbox.y_max <= 65.0;

                // 2. Never stitch if containing dot-leader tab stops (Table of Contents)
                let has_toc_tabs = last_p.lines.iter().any(|l| l.tab_stops.iter().any(|ts| ts.is_leader_dot))
                    || first_p.lines.iter().any(|l| l.tab_stops.iter().any(|ts| ts.is_leader_dot));

                // 3. Never stitch URLs or command strings
                let has_url = trimmed_last.contains("http://") || trimmed_last.contains("https://")
                    || trimmed_first.contains("http://") || trimmed_first.contains("https://");

                // 4. Font size and family must match (same continuous prose styling)
                let font_matches = {
                    let f1 = last_p.lines.first().and_then(|l| l.words.first()).map(|w| (w.style.font_size, &w.style.font_family));
                    let f2 = first_p.lines.first().and_then(|l| l.words.first()).map(|w| (w.style.font_size, &w.style.font_family));
                    match (f1, f2) {
                        (Some((s1, fam1)), Some((s2, fam2))) => (s1 - s2).abs() <= 1.5 && fam1 == fam2,
                        _ => true,
                    }
                };

                !last_p.is_header
                    && !first_p.is_header
                    && !last_p.is_list
                    && !first_p.is_list
                    && !ends_with_punct
                    && starts_with_lower
                    && !is_near_bottom
                    && !is_near_top
                    && !has_toc_tabs
                    && !has_url
                    && font_matches
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
    let total_pages = pages.len();
    if total_pages <= 1 {
        // Single-page documents have NO running headers or footers! All text is body content.
        return;
    }

    // Minimum distinct page occurrences required for a template or page number series to be confirmed:
    // For 2-5 pages: must appear on at least 2 pages.
    // For 6+ pages: must appear on at least 2 pages OR >= 20% of total pages.
    let min_recur_pages = if total_pages <= 5 { 2 } else { 2.max((total_pages as f64 * 0.20) as usize) };

    // 1. Collect candidate headers across all pages
    let mut header_template_pages: std::collections::HashMap<String, std::collections::HashSet<usize>> = std::collections::HashMap::new();
    let mut top_page_num_pages: std::collections::HashSet<usize> = std::collections::HashSet::new();

    for (page_idx, page) in pages.iter().enumerate() {
        let page_h = page.height_points;
        let header_y_limit = 60.0_f64.max(page_h * 0.08);
        for p in &page.paragraphs {
            let is_in_header_strip = p.bbox.y_max <= header_y_limit && p.lines.len() <= 2;
            let has_toc_tabs = p.lines.iter().any(|l| l.tab_stops.iter().any(|ts| ts.is_leader_dot));
            if is_in_header_strip && !has_toc_tabs {
                let cand = analyze_candidate(p);
                if cand.is_pure_page_num {
                    top_page_num_pages.insert(page_idx);
                } else if cand.normalized_template.len() >= 4 {
                    header_template_pages.entry(cand.normalized_template).or_default().insert(page_idx);
                }
            }
        }
    }

    let confirmed_header_templates: Vec<String> = header_template_pages
        .into_iter()
        .filter(|(_, p_set)| p_set.len() >= min_recur_pages)
        .map(|(tmpl, _)| tmpl)
        .collect();
    let confirmed_top_page_nums = top_page_num_pages.len() >= 2;

    // 2. Collect candidate footers across all pages
    let mut footer_template_pages: std::collections::HashMap<String, std::collections::HashSet<usize>> = std::collections::HashMap::new();
    let mut bottom_page_num_pages: std::collections::HashSet<usize> = std::collections::HashSet::new();

    for (page_idx, page) in pages.iter().enumerate() {
        let page_h = page.height_points;
        let footer_y_limit = page_h - 85.0_f64.max(page_h * 0.10);
        let footer_cy_limit = page_h - 75.0_f64.max(page_h * 0.09);
        for p in &page.paragraphs {
            let cy = (p.bbox.y_min + p.bbox.y_max) / 2.0;
            let is_in_footer_strip = (p.bbox.y_min >= footer_y_limit || cy >= footer_cy_limit) && p.lines.len() <= 2;
            let has_toc_tabs = p.lines.iter().any(|l| l.tab_stops.iter().any(|ts| ts.is_leader_dot));
            if is_in_footer_strip && !has_toc_tabs {
                let cand = analyze_candidate(p);
                if cand.is_pure_page_num {
                    bottom_page_num_pages.insert(page_idx);
                } else if cand.normalized_template.len() >= 4 {
                    footer_template_pages.entry(cand.normalized_template).or_default().insert(page_idx);
                }
            }
        }
    }

    let confirmed_footer_templates: Vec<String> = footer_template_pages
        .into_iter()
        .filter(|(_, p_set)| p_set.len() >= min_recur_pages)
        .map(|(tmpl, _)| tmpl)
        .collect();
    let confirmed_bottom_page_nums = bottom_page_num_pages.len() >= 2;

    // 3. Extract confirmed recurring headers and footers from each page
    for (_page_idx, page) in pages.iter_mut().enumerate() {
        let page_h = page.height_points;
        let header_y_limit = 60.0_f64.max(page_h * 0.08);
        let footer_y_limit = page_h - 85.0_f64.max(page_h * 0.10);
        let footer_cy_limit = page_h - 75.0_f64.max(page_h * 0.09);

        let mut header_blocks: Vec<crate::models::ParagraphBlock> = Vec::new();
        let mut footer_blocks: Vec<crate::models::ParagraphBlock> = Vec::new();
        let mut kept_paragraphs = Vec::with_capacity(page.paragraphs.len());

        for mut p in page.paragraphs.drain(..) {
            let full_text = get_paragraph_full_text(&p);
            if full_text.is_empty() {
                continue;
            }

            let has_toc_tabs = p.lines.iter().any(|l| l.tab_stops.iter().any(|ts| ts.is_leader_dot));
            if has_toc_tabs {
                kept_paragraphs.push(p);
                continue;
            }

            let cy = (p.bbox.y_min + p.bbox.y_max) / 2.0;
            let is_in_header_strip = p.bbox.y_max <= header_y_limit && p.lines.len() <= 2;
            let is_in_footer_strip = (p.bbox.y_min >= footer_y_limit || cy >= footer_cy_limit) && p.lines.len() <= 2;

            if is_in_header_strip {
                let cand = analyze_candidate(&p);
                let is_confirmed = (confirmed_top_page_nums && cand.is_pure_page_num)
                    || (!confirmed_header_templates.is_empty()
                        && confirmed_header_templates.iter().any(|t| t == &cand.normalized_template));

                if is_confirmed {
                    if cand.has_gap_split && !cand.left_text.is_empty() && !cand.right_text.is_empty() {
                        apply_gap_split_to_paragraph(&mut p, &cand.left_text, &cand.right_text);
                    }
                    header_blocks.push(p);
                    continue;
                }
            }

            if is_in_footer_strip {
                let cand = analyze_candidate(&p);
                let is_confirmed = (confirmed_bottom_page_nums && cand.is_pure_page_num)
                    || (!confirmed_footer_templates.is_empty()
                        && confirmed_footer_templates.iter().any(|t| t == &cand.normalized_template));

                if is_confirmed {
                    if cand.has_gap_split && !cand.left_text.is_empty() && !cand.right_text.is_empty() {
                        apply_gap_split_to_paragraph(&mut p, &cand.left_text, &cand.right_text);
                    }
                    footer_blocks.push(p);
                    continue;
                }
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

/// Structure holding layout analysis metadata for candidate running headers and footers
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct HeaderFooterCandidate {
    raw_text: String,
    normalized_template: String,
    has_page_token: bool,
    is_pure_page_num: bool,
    has_gap_split: bool,
    left_text: String,
    right_text: String,
}

fn analyze_candidate(p: &crate::models::ParagraphBlock) -> HeaderFooterCandidate {
    let raw_text = get_paragraph_full_text(p);
    let trimmed = raw_text.trim().to_string();

    if is_page_number_pattern(&trimmed) && p.lines.len() <= 1 {
        return HeaderFooterCandidate {
            raw_text,
            normalized_template: String::new(),
            has_page_token: true,
            is_pure_page_num: true,
            has_gap_split: false,
            left_text: String::new(),
            right_text: trimmed,
        };
    }

    // Check for large horizontal gap (>= 28.0 pt) between words on line 0 (dual header/footer)
    let mut found_gap = false;
    let mut left_words = Vec::new();
    let mut right_words = Vec::new();

    if let Some(line) = p.lines.first() {
        if line.words.len() >= 2 {
            for i in 0..line.words.len() {
                if i > 0 && !found_gap {
                    let prev = &line.words[i - 1];
                    let curr = &line.words[i];
                    let gap = curr.bbox.x_min - prev.bbox.x_max;
                    if gap >= 28.0 {
                        found_gap = true;
                    }
                }
                if found_gap {
                    right_words.push(line.words[i].text.as_str());
                } else {
                    left_words.push(line.words[i].text.as_str());
                }
            }
        }
    }

    let left_text = left_words.join(" ").trim().to_string();
    let right_text = right_words.join(" ").trim().to_string();
    let has_gap_split = found_gap && !left_text.is_empty() && !right_text.is_empty();

    let mut norm = trimmed.clone();
    let mut has_page_token = false;

    if has_gap_split {
        if is_page_fraction_or_num(&right_text) {
            has_page_token = true;
            norm = left_text.clone();
        } else {
            norm = format!("{} | {}", left_text, right_text);
        }
    } else {
        if let Some(stripped) = strip_trailing_page_token(&trimmed) {
            norm = stripped;
            has_page_token = true;
        } else if let Some(stripped) = strip_leading_page_token(&trimmed) {
            norm = stripped;
            has_page_token = true;
        }
    }

    HeaderFooterCandidate {
        raw_text,
        normalized_template: norm.trim().to_string(),
        has_page_token,
        is_pure_page_num: false,
        has_gap_split,
        left_text,
        right_text,
    }
}

fn apply_gap_split_to_paragraph(p: &mut crate::models::ParagraphBlock, left: &str, right: &str) {
    if let Some(line) = p.lines.first_mut() {
        let style = line.words.first().map(|w| w.style.clone()).unwrap_or_default();
        line.runs.clear();
        line.runs.push(crate::models::TextRun {
            text: left.to_string(),
            style: style.clone(),
            is_tab: false,
            is_leader_dot: false,
        });
        line.runs.push(crate::models::TextRun {
            text: String::new(),
            style: style.clone(),
            is_tab: true,
            is_leader_dot: false,
        });
        line.runs.push(crate::models::TextRun {
            text: right.to_string(),
            style,
            is_tab: false,
            is_leader_dot: false,
        });
    }
}

pub fn is_page_fraction_or_num(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty() {
        return false;
    }
    if is_page_number_pattern(t) {
        return true;
    }
    // Fraction format: "1/8", "1 / 8", "12/55"
    if let Some((l, r)) = t.split_once('/') {
        let left = l.trim();
        let right = r.trim();
        if !left.is_empty()
            && !right.is_empty()
            && left.chars().all(|c| c.is_ascii_digit())
            && right.chars().all(|c| c.is_ascii_digit())
        {
            return true;
        }
    }
    // "1 of 8", "1 dari 8"
    let parts: Vec<&str> = t.split_whitespace().collect();
    if parts.len() == 3
        && parts[0].chars().all(|c| c.is_ascii_digit())
        && parts[2].chars().all(|c| c.is_ascii_digit())
    {
        return true;
    }
    false
}

pub fn strip_trailing_page_token(s: &str) -> Option<String> {
    let t = s.trim();
    if let Some(idx) = t.rfind(' ') {
        let last_word = &t[idx + 1..];
        if is_page_fraction_or_num(last_word) {
            let prefix = t[..idx].trim();
            if !prefix.is_empty() {
                return Some(prefix.to_string());
            }
        }
        // Check last 2 words: "Page 1", "Hal 1"
        if let Some(idx2) = t[..idx].rfind(' ') {
            let last_two = &t[idx2 + 1..];
            if is_page_number_pattern(last_two) {
                let prefix = t[..idx2].trim();
                if !prefix.is_empty() {
                    return Some(prefix.to_string());
                }
            }
        }
    }
    None
}

pub fn strip_leading_page_token(s: &str) -> Option<String> {
    let t = s.trim();
    if let Some(idx) = t.find(' ') {
        let first_word = &t[..idx];
        if is_page_fraction_or_num(first_word) {
            let suffix = t[idx + 1..].trim();
            if !suffix.is_empty() {
                return Some(suffix.to_string());
            }
        }
    }
    None
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

pub fn is_page_number_pattern(text: &str) -> bool {
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

/// Universal Language-Agnostic Geometric Classification: Background/Underlay vs Overlay Anchors vs Inline Content
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
            g.behind_doc = true;
            continue;
        }

        // Universal Geometric Rule 2: Table Overlays (Signatures, Stamps, Seals, Emblems, Checkmarks)
        // If an image physically intersects or is contained within any table matrix on the page,
        // it cannot be an inline block element without destroying the tabular grid.
        // It floats directly in front of the table cell (behindDoc="0").
        let overlaps_table = page.tables.iter().any(|t| t.bbox.intersects(&g.bbox));
        if overlaps_table {
            g.is_inline = false;
            g.is_background = true;
            g.behind_doc = false;
            continue;
        }

        // Universal Geometric Rule 3: Side-by-Side Anchors (Corner Logos, Header Emblems)
        // If an image shares vertical band overlap (>= 35%) with a paragraph side-by-side,
        // treating it as an inline block element would force it onto a separate paragraph line,
        // artificially inflating page height and pushing following content to subsequent pages.
        let shares_band_side_by_side = page.paragraphs.iter().any(|p| {
            let y_overlap = (g.bbox.y_max.min(p.bbox.y_max) - g.bbox.y_min.max(p.bbox.y_min)).max(0.0);
            let min_h = g.bbox.height().min(p.bbox.height());
            if min_h > 0.0 && (y_overlap / min_h) >= 0.35 {
                // Horizontally separated (side-by-side, not colliding in the same column)
                (g.bbox.x_max <= p.bbox.x_min + 15.0) || (p.bbox.x_max <= g.bbox.x_min + 15.0)
            } else {
                false
            }
        });

        if shares_band_side_by_side {
            g.is_inline = false;
            g.is_background = true;
            g.behind_doc = false;
            continue;
        }

        // Universal Geometric Rule 4: Standalone Inline Content (Charts, Plots, Flowcharts, Diagrams)
        // Graphic spans the column/page width with complete clearance across its vertical band.
        g.is_inline = true;
        g.is_background = false;
        g.behind_doc = false;
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

/// Universal Geometric Multi-Column Classification
fn assign_paragraph_column_indices(page: &mut DocumentPage) {
    if page.paragraphs.is_empty() {
        return;
    }

    let w = page.width_points;
    let x_mid = w / 2.0;

    let mut left_count = 0;
    let mut right_count = 0;
    let mut left_min_y = f64::INFINITY;
    let mut left_max_y = f64::NEG_INFINITY;
    let mut right_min_y = f64::INFINITY;
    let mut right_max_y = f64::NEG_INFINITY;

    for p in &page.paragraphs {
        let is_wide = p.bbox.width() >= (w * 0.55)
            || (p.bbox.x_min < (x_mid - 25.0) && p.bbox.x_max > (x_mid + 25.0));
        if !is_wide {
            // Margin check: ignore tiny marginal annotations (like line numbers < 35 pt wide in margins)
            if p.bbox.width() < 35.0 && (p.bbox.x_max <= 75.0 || p.bbox.x_min >= (w - 75.0)) {
                continue;
            }
            if p.bbox.x_max <= (x_mid + 15.0) {
                left_count += 1;
                left_min_y = left_min_y.min(p.bbox.y_min);
                left_max_y = left_max_y.max(p.bbox.y_max);
            } else if p.bbox.x_min >= (x_mid - 15.0) {
                right_count += 1;
                right_min_y = right_min_y.min(p.bbox.y_min);
                right_max_y = right_max_y.max(p.bbox.y_max);
            }
        }
    }

    // Two-column layout exists if both columns have content AND they share vertical extent
    let y_overlap = (left_max_y.min(right_max_y) - left_min_y.max(right_min_y)).max(0.0);
    let is_two_col = left_count >= 1 && right_count >= 1 && y_overlap > 30.0;

    if is_two_col {
        for p in page.paragraphs.iter_mut() {
            let cx = (p.bbox.x_min + p.bbox.x_max) / 2.0;
            let is_wide = p.bbox.width() >= (w * 0.55)
                || (p.bbox.x_min < (x_mid - 25.0) && p.bbox.x_max > (x_mid + 25.0));

            if is_wide {
                p.column_index = 0;
            } else if cx >= x_mid || p.bbox.x_min >= (x_mid - 15.0) {
                p.column_index = 1;
            } else {
                p.column_index = 0;
            }
        }
    }
}
