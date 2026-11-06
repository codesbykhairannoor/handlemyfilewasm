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

    // Check if any word is dots e.g. "....."
    let dot_word_idx = line.words.iter().position(|w| {
        w.text.len() >= 3 && w.text.chars().all(|c| c == '.' || c == ' ')
    });

    if let Some(idx) = dot_word_idx {
        if idx + 1 < line.words.len() {
            let next_word = &line.words[idx + 1];
            line.tab_stops.push(TabStop {
                x_pos_points: next_word.bbox.x_min,
                align_type: "right".to_string(),
                is_leader_dot: true,
            });

            // Build runs with tab
            let mut left_text = Vec::new();
            for i in 0..idx {
                left_text.push(line.words[i].text.as_str());
            }

            let mut right_text = Vec::new();
            for i in (idx + 1)..line.words.len() {
                right_text.push(line.words[i].text.as_str());
            }

            line.runs = vec![
                TextRun {
                    text: left_text.join(" "),
                    style: line.words[0].style.clone(),
                    is_tab: false,
                    is_leader_dot: false,
                },
                TextRun {
                    text: "\t".to_string(),
                    style: line.words[0].style.clone(),
                    is_tab: true,
                    is_leader_dot: true,
                },
                TextRun {
                    text: right_text.join(" "),
                    style: line.words.last().unwrap().style.clone(),
                    is_tab: false,
                    is_leader_dot: false,
                },
            ];
        }
    }
}
