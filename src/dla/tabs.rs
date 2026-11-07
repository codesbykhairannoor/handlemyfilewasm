use crate::models::{ParagraphBlock, TabStop, TextRun};

pub struct TabStopDetector;

impl TabStopDetector {
    /// Detects vertical column alignments, colon alignments, and inserts custom TabStops and Tab Runs
    pub fn process_paragraph_tabs(paragraphs: &mut [ParagraphBlock]) {
        if paragraphs.is_empty() {
            return;
        }

        // 1. First pass: check for dot leaders (Table of Contents)
        for p in paragraphs.iter_mut() {
            for line in p.lines.iter_mut() {
                detect_single_line_dot_leader(line);
            }
        }

        // 2. Second pass: Collect jump candidates across non-prose paragraphs on the page
        let mut jump_candidates: Vec<f64> = Vec::new();

        for p in paragraphs.iter() {
            // Do not inject tabular tabs into multi-line flowing prose paragraphs
            let is_dense_prose = p.lines.len() >= 3
                && (p.lines.iter().map(|l| l.words.len()).sum::<usize>() / p.lines.len()) >= 8;
            if is_dense_prose {
                continue;
            }
            for line in &p.lines {
                for (w_idx, w) in line.words.iter().enumerate() {
                    if w_idx > 0 {
                        let prev = &line.words[w_idx - 1];
                        let gap = w.bbox.x_min - prev.bbox.x_max;
                        if gap >= 18.0 || (gap >= 12.0 && (prev.text.ends_with(':') || w.text.starts_with(':'))) {
                            jump_candidates.push(w.bbox.x_min);
                        }
                    }
                }
            }
        }

        // Cluster jump positions within 6.0 pt (must appear on 2+ lines to be an aligned column)
        let aligned_tabs = cluster_tab_positions(jump_candidates, 6.0);

        // 3. Third pass: Apply aligned tab stops to lines
        for p in paragraphs.iter_mut() {
            let is_dense_prose = p.lines.len() >= 3
                && (p.lines.iter().map(|l| l.words.len()).sum::<usize>() / p.lines.len()) >= 8;
            if is_dense_prose {
                continue;
            }
            for line in p.lines.iter_mut() {
                // If dot leader was already detected on this line, skip colon tab processing
                if line.tab_stops.iter().any(|ts| ts.is_leader_dot) {
                    continue;
                }

                let mut applied_tab_stops = Vec::new();

                for &tab_x in &aligned_tabs {
                    let has_match = line.words.iter().enumerate().any(|(w_idx, w)| {
                        if w_idx > 0 {
                            let prev = &line.words[w_idx - 1];
                            let gap = w.bbox.x_min - prev.bbox.x_max;
                            (w.bbox.x_min - tab_x).abs() <= 6.0 && (gap >= 15.0 || prev.text.ends_with(':') || w.text.starts_with(':'))
                        } else {
                            false
                        }
                    });

                    if has_match {
                        applied_tab_stops.push(TabStop {
                            x_pos_points: tab_x,
                            align_type: "left".to_string(),
                            is_leader_dot: false,
                        });
                    }
                }

                if !applied_tab_stops.is_empty() {
                    line.tab_stops = applied_tab_stops;
                    insert_tab_runs(line);
                }
            }
        }
    }
}

/// Clusters similar jump coordinates into definitive TabStop positions
fn cluster_tab_positions(mut candidates: Vec<f64>, tol: f64) -> Vec<f64> {
    if candidates.is_empty() {
        return Vec::new();
    }

    candidates.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut clusters: Vec<Vec<f64>> = Vec::new();

    for c in candidates {
        let mut matched = false;
        if let Some(last) = clusters.last_mut() {
            let avg: f64 = last.iter().sum::<f64>() / (last.len() as f64);
            if (avg - c).abs() <= tol {
                last.push(c);
                matched = true;
            }
        }
        if !matched {
            clusters.push(vec![c]);
        }
    }

    // Only clusters that appear on at least 2 distinct lines represent a true column alignment
    clusters.retain(|c| c.len() >= 2);

    clusters
        .into_iter()
        .map(|cluster| cluster.iter().sum::<f64>() / (cluster.len() as f64))
        .collect()
}

/// Inserts `<w:tab/>` TextRuns at the detected tab boundaries
fn insert_tab_runs(line: &mut crate::models::TextLine) {
    if line.tab_stops.is_empty() || line.words.len() < 2 {
        return;
    }

    let mut new_runs = Vec::new();
    let mut curr_text = String::new();
    let mut curr_style = line.words[0].style.clone();

    for (w_idx, w) in line.words.iter().enumerate() {
        // Check if there is a tab stop right before this word
        let is_tab_boundary = line.tab_stops.iter().any(|ts| {
            (w.bbox.x_min - ts.x_pos_points).abs() <= 6.0
        });

        if is_tab_boundary && w_idx > 0 {
            if !curr_text.is_empty() {
                new_runs.push(TextRun {
                    text: std::mem::take(&mut curr_text),
                    style: curr_style.clone(),
                    is_tab: false,
                    is_leader_dot: false,
                });
            }

            // Insert Tab Run
            new_runs.push(TextRun {
                text: "\t".to_string(),
                style: w.style.clone(),
                is_tab: true,
                is_leader_dot: false,
            });

            curr_text.push_str(&w.text);
            curr_style = w.style.clone();
        } else {
            if !curr_text.is_empty() {
                curr_text.push(' ');
            }
            curr_text.push_str(&w.text);
            curr_style = w.style.clone();
        }
    }

    if !curr_text.is_empty() {
        new_runs.push(TextRun {
            text: curr_text,
            style: curr_style,
            is_tab: false,
            is_leader_dot: false,
        });
    }

    line.runs = new_runs;
}

/// Detects dot leaders in Table of Contents or single line references
fn detect_single_line_dot_leader(line: &mut crate::models::TextLine) {
    if line.words.len() < 2 {
        return;
    }

    let full_line_text = line.words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ");
    let dot_count = full_line_text.chars().filter(|&c| c == '.').count();
    let has_dot_sequence = dot_count >= 2 && (full_line_text.contains("..") || full_line_text.contains(". .") || dot_count >= 4);

    let last_word_raw = line.words.last().map(|w| w.text.trim()).unwrap_or("");
    let clean_num = last_word_raw.trim_matches(|c: char| c == '.' || c == ' ' || c == '-' || c == '[' || c == ']');
    let is_digit_page_num = !clean_num.is_empty() && clean_num.len() <= 5 && clean_num.chars().all(|c| c.is_ascii_digit());
    let is_roman_page_num = crate::utils::is_roman_numeral(clean_num);
    let ends_with_page_num = is_digit_page_num || is_roman_page_num;

    // Check gap before last word
    let prev_w = &line.words[line.words.len() - 2];
    let trailing_gap = line.words.last().unwrap().bbox.x_min - prev_w.bbox.x_max;
    let has_wide_jump = trailing_gap >= 25.0;

    // A valid TOC entry either has a dot sequence OR a wide jump leading to the terminal page number
    if !ends_with_page_num || (!has_dot_sequence && !has_wide_jump) {
        return;
    }

    // Extract title words on the left, and page number on the right
    let mut title_parts: Vec<String> = Vec::new();
    let page_number = clean_num.to_string();

    for i in 0..line.words.len().saturating_sub(1) {
        let w_text = line.words[i].text.trim();
        let clean_w = w_text.trim_matches(|c: char| c == '.' || c == ' ');
        if !clean_w.is_empty() {
            title_parts.push(clean_w.to_string());
        }
    }
    // If last word had text before dots, e.g. "Data...11"
    if let Some(pos) = last_word_raw.find("..") {
        let prefix = last_word_raw[..pos].trim_matches(|c: char| c == '.' || c == ' ');
        if !prefix.is_empty() && !prefix.chars().all(|c| c.is_ascii_digit()) {
            title_parts.push(prefix.to_string());
        }
    }

    let title = title_parts.join(" ");
    if title.is_empty() {
        return;
    }

    // Set tab stop with dot leader (pos will be set to exact printable_width in OOXML emitter)
    line.tab_stops = vec![TabStop {
        x_pos_points: 0.0,
        align_type: "right".to_string(),
        is_leader_dot: true,
    }];

    let base_style = line.words.first().map(|w| w.style.clone()).unwrap_or_default();
    let num_style = line.words.last().map(|w| w.style.clone()).unwrap_or_else(|| base_style.clone());

    line.runs = vec![
        TextRun {
            text: title,
            style: base_style,
            is_tab: false,
            is_leader_dot: false,
        },
        TextRun {
            text: "\t".to_string(),
            style: num_style.clone(),
            is_tab: true,
            is_leader_dot: true,
        },
        TextRun {
            text: page_number,
            style: num_style,
            is_tab: false,
            is_leader_dot: false,
        },
    ];
}
