use crate::models::{Cell, ParagraphBlock, Rect, TableMatrix, TextLine, VectorPath, Word};
use crate::dla::classifier::build_line_runs;

pub struct TableRecognizer;

#[derive(Debug, Clone, Copy)]
struct HorizLine {
    y: f64,
    x_min: f64,
    x_max: f64,
}

#[derive(Debug, Clone, Copy)]
struct VertLine {
    x: f64,
    y_min: f64,
    y_max: f64,
}

impl TableRecognizer {
    /// Detects structured TableMatrix grids from vector paths without populating text
    pub fn detect_table_grids(vector_paths: &[VectorPath]) -> Vec<TableMatrix> {
        if vector_paths.is_empty() {
            return Vec::new();
        }

        // 1. Separate and normalize horizontal and vertical lines, filtering page border frames
        let (h_lines, v_lines) = separate_lines(vector_paths);

        if h_lines.len() < 2 || v_lines.len() < 2 {
            return Vec::new();
        }

        // 2. Merge collinear overlapping lines (tolerance: 2.5 pt)
        let merged_h = merge_collinear_h(h_lines, 2.5);
        let merged_v = merge_collinear_v(v_lines, 2.5);

        if merged_h.len() < 2 || merged_v.len() < 2 {
            return Vec::new();
        }

        // 3. Build intersection adjacency between H lines and V lines
        let mut adj_h: Vec<Vec<usize>> = vec![Vec::new(); merged_h.len()];
        let mut adj_v: Vec<Vec<usize>> = vec![Vec::new(); merged_v.len()];

        for (i, h) in merged_h.iter().enumerate() {
            for (j, v) in merged_v.iter().enumerate() {
                if v.x >= (h.x_min - 3.0) && v.x <= (h.x_max + 3.0)
                    && h.y >= (v.y_min - 3.0) && h.y <= (v.y_max + 3.0)
                {
                    adj_h[i].push(j);
                    adj_v[j].push(i);
                }
            }
        }

        // 4. Cluster into connected components (Anssi Nurminen 2013)
        let mut visited_h = vec![false; merged_h.len()];
        let mut visited_v = vec![false; merged_v.len()];
        let mut components: Vec<(Vec<HorizLine>, Vec<VertLine>)> = Vec::new();

        for i in 0..merged_h.len() {
            if visited_h[i] || adj_h[i].is_empty() {
                continue;
            }

            let mut comp_h = Vec::new();
            let mut comp_v = Vec::new();
            let mut queue_h = vec![i];
            visited_h[i] = true;

            while let Some(cur_h) = queue_h.pop() {
                comp_h.push(merged_h[cur_h]);
                for &v_idx in &adj_h[cur_h] {
                    if !visited_v[v_idx] {
                        visited_v[v_idx] = true;
                        comp_v.push(merged_v[v_idx]);
                        for &next_h in &adj_v[v_idx] {
                            if !visited_h[next_h] {
                                visited_h[next_h] = true;
                                queue_h.push(next_h);
                            }
                        }
                    }
                }
            }

            if comp_h.len() >= 2 && comp_v.len() >= 2 {
                components.push((comp_h, comp_v));
            }
        }

        let mut out_tables = Vec::new();

        for (comp_h, comp_v) in components {
            // Find intersection rails for this component
            let mut x_rails = Vec::new();
            let mut y_rails = Vec::new();

            for h in &comp_h {
                y_rails.push(h.y);
                for v in &comp_v {
                    if v.x >= (h.x_min - 3.0) && v.x <= (h.x_max + 3.0)
                        && h.y >= (v.y_min - 3.0) && h.y <= (v.y_max + 3.0)
                    {
                        x_rails.push(v.x);
                    }
                }
            }

            for v in &comp_v {
                x_rails.push(v.x);
                y_rails.push(v.y_min);
                y_rails.push(v.y_max);
            }

            let mut unique_x = cluster_coordinates(x_rails, 2.5);
            let mut unique_y = cluster_coordinates(y_rails, 2.5);

            // Collapse adjacent rails separated by <= 6.5 pt (Double border rails)
            let mut merged_x: Vec<f64> = Vec::new();
            for x in unique_x {
                if let Some(last) = merged_x.last_mut() {
                    if (x - *last).abs() <= 6.5 {
                        *last = (*last + x) / 2.0;
                        continue;
                    }
                }
                merged_x.push(x);
            }
            unique_x = merged_x;

            let mut merged_y: Vec<f64> = Vec::new();
            for y in unique_y {
                if let Some(last) = merged_y.last_mut() {
                    if (y - *last).abs() <= 6.5 {
                        *last = (*last + y) / 2.0;
                        continue;
                    }
                }
                merged_y.push(y);
            }
            unique_y = merged_y;

            if unique_x.len() < 2 || unique_y.len() < 2 {
                continue;
            }

            let num_rows = unique_y.len() - 1;
            let num_cols = unique_x.len() - 1;

            // A valid table matrix must have at least 2 columns (a 1-column box is a section divider or page container)
            if num_cols < 2 || num_rows < 1 {
                continue;
            }

            let table_bbox = Rect::new(
                unique_x[0],
                unique_y[0],
                *unique_x.last().unwrap(),
                *unique_y.last().unwrap(),
            );

            // Filter out full-page decorative borders or tiny decorative pill boxes
            if table_bbox.width() < 50.0 || table_bbox.height() < 20.0 {
                continue;
            }
            if table_bbox.width() >= 500.0 && table_bbox.height() >= 700.0 {
                continue;
            }

            // Reconstruct grid cells with span detection
            let mut table_cells = Vec::new();
            let mut visited = vec![vec![false; num_cols]; num_rows];

            for r in 0..num_rows {
                let mut row_cells = Vec::new();
                for c in 0..num_cols {
                    if visited[r][c] {
                        continue;
                    }

                    let y_top = unique_y[r];
                    let x_left = unique_x[c];

                    // Determine horizontal span (col_span)
                    let mut c_end = c + 1;
                    while c_end < num_cols {
                        let test_x = unique_x[c_end];
                        if has_vertical_segment(&comp_v, test_x, y_top, unique_y[r + 1], 3.0) {
                            break;
                        }
                        c_end += 1;
                    }

                    // Determine vertical span (row_span)
                    let mut r_end = r + 1;
                    while r_end < num_rows {
                        let test_y = unique_y[r_end];
                        if has_horizontal_segment(&comp_h, test_y, x_left, unique_x[c_end], 3.0) {
                            break;
                        }
                        r_end += 1;
                    }

                    for vr in r..r_end {
                        for vc in c..c_end {
                            visited[vr][vc] = true;
                        }
                    }

                    let cell_bbox = Rect::new(x_left, y_top, unique_x[c_end], unique_y[r_end]);
                    let border_top = has_horizontal_segment(&comp_h, y_top, x_left, unique_x[c_end], 3.5);
                    let border_bottom = has_horizontal_segment(&comp_h, unique_y[r_end], x_left, unique_x[c_end], 3.5);
                    let border_left = has_vertical_segment(&comp_v, x_left, y_top, unique_y[r_end], 3.5);
                    let border_right = has_vertical_segment(&comp_v, unique_x[c_end], y_top, unique_y[r_end], 3.5);

                    row_cells.push(Cell {
                        bbox: cell_bbox,
                        start_row: r,
                        end_row: r_end,
                        start_col: c,
                        end_col: c_end,
                        content_blocks: Vec::new(),
                        border_top,
                        border_bottom,
                        border_left,
                        border_right,
                    });
                }

                if !row_cells.is_empty() {
                    table_cells.push(row_cells);
                }
            }

            if table_cells.is_empty() {
                continue;
            }

            let mut grid_cols = Vec::new();
            for i in 0..num_cols {
                grid_cols.push(unique_x[i + 1] - unique_x[i]);
            }

            let mut grid_rows = Vec::new();
            for i in 0..num_rows {
                grid_rows.push(unique_y[i + 1] - unique_y[i]);
            }

            out_tables.push(TableMatrix {
                bbox: table_bbox,
                grid_cols,
                grid_rows,
                cells: table_cells,
            });
        }

        out_tables
    }

    /// Extracts tables directly from words, populating cells with exact word text and removing them from words pool
    pub fn extract_tables_from_words(
        vector_paths: &[VectorPath],
        words: &mut Vec<Word>,
    ) -> Vec<TableMatrix> {
        let mut tables = Self::detect_table_grids(vector_paths);
        if tables.is_empty() || words.is_empty() {
            return tables;
        }

        let mut word_indices_to_remove = Vec::new();

        for table in tables.iter_mut() {
            for row in table.cells.iter_mut() {
                for cell in row.iter_mut() {
                    let mut cell_words = Vec::new();
                    for (idx, w) in words.iter().enumerate() {
                        let cx = (w.bbox.x_min + w.bbox.x_max) / 2.0;
                        let cy = (w.bbox.y_min + w.bbox.y_max) / 2.0;
                        let is_inside = cx >= (cell.bbox.x_min - 2.0)
                            && cx <= (cell.bbox.x_max + 2.0)
                            && cy >= (cell.bbox.y_min - 2.0)
                            && cy <= (cell.bbox.y_max + 2.0);

                        if is_inside {
                            cell_words.push(w.clone());
                            if !word_indices_to_remove.contains(&idx) {
                                word_indices_to_remove.push(idx);
                            }
                        }
                    }

                    if !cell_words.is_empty() {
                        cell.content_blocks = assemble_cell_paragraphs(cell_words, cell.bbox);
                    }
                }
            }
        }

        word_indices_to_remove.sort_unstable();
        for &idx in word_indices_to_remove.iter().rev() {
            if idx < words.len() {
                words.remove(idx);
            }
        }

        tables
    }

    /// Extracts structured TableMatrix from vector paths and associates cell content paragraphs (backward compatibility)
    pub fn extract_tables(
        vector_paths: &[VectorPath],
        paragraphs: &mut Vec<ParagraphBlock>,
    ) -> Vec<TableMatrix> {
        let mut tables = Self::detect_table_grids(vector_paths);
        if tables.is_empty() || paragraphs.is_empty() {
            return tables;
        }

        let mut extracted_para_indices = Vec::new();

        for table in tables.iter_mut() {
            for row in table.cells.iter_mut() {
                for cell in row.iter_mut() {
                    for (p_idx, p) in paragraphs.iter().enumerate() {
                        let cx = (p.bbox.x_min + p.bbox.x_max) / 2.0;
                        let cy = (p.bbox.y_min + p.bbox.y_max) / 2.0;
                        let is_inside = cx >= (cell.bbox.x_min - 2.0)
                            && cx <= (cell.bbox.x_max + 2.0)
                            && cy >= (cell.bbox.y_min - 2.0)
                            && cy <= (cell.bbox.y_max + 2.0);

                        if is_inside {
                            cell.content_blocks.push(p.clone());
                            if !extracted_para_indices.contains(&p_idx) {
                                extracted_para_indices.push(p_idx);
                            }
                        }
                    }
                }
            }
        }

        extracted_para_indices.sort_unstable();
        for &idx in extracted_para_indices.iter().rev() {
            if idx < paragraphs.len() {
                paragraphs.remove(idx);
            }
        }

        tables
    }
}

/// Assembles cell words into coherent ParagraphBlocks with TextLines and TextRuns
pub fn assemble_cell_paragraphs(words: Vec<Word>, cell_bbox: Rect) -> Vec<ParagraphBlock> {
    if words.is_empty() {
        return Vec::new();
    }
    let mut sorted_words = words;
    sorted_words.sort_by(|a, b| {
        a.bbox.y_min.partial_cmp(&b.bbox.y_min)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.bbox.x_min.partial_cmp(&b.bbox.x_min).unwrap_or(std::cmp::Ordering::Equal))
    });

    let mut lines_words: Vec<Vec<Word>> = Vec::new();
    for w in sorted_words {
        let mut matched = false;
        let cy = (w.bbox.y_min + w.bbox.y_max) / 2.0;
        for line in lines_words.iter_mut() {
            if let Some(first) = line.first() {
                let first_cy = (first.bbox.y_min + first.bbox.y_max) / 2.0;
                let h = first.bbox.height().max(w.bbox.height()).max(6.0);
                if (first_cy - cy).abs() <= (h * 0.45) {
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
    for mut lw in lines_words {
        lw.sort_by(|a, b| a.bbox.x_min.partial_cmp(&b.bbox.x_min).unwrap_or(std::cmp::Ordering::Equal));
        let mut line_bbox = lw[0].bbox;
        for w in &lw {
            line_bbox.expand(&w.bbox);
        }
        text_lines.push(TextLine {
            bbox: line_bbox,
            words: lw,
            runs: Vec::new(),
            tab_stops: Vec::new(),
        });
    }

    segment_lines_into_cell_paragraphs(text_lines, cell_bbox)
}

/// Assembles cell glyphs into coherent ParagraphBlocks using Docstrum word clustering
pub fn assemble_cell_paragraphs_from_glyphs(glyphs: Vec<crate::models::Glyph>, cell_bbox: Rect) -> Vec<ParagraphBlock> {
    if glyphs.is_empty() {
        return Vec::new();
    }
    let metrics = crate::dla::DocstrumAnalyzer::compute_metrics(&glyphs);
    let words = crate::dla::DocstrumAnalyzer::cluster_words(&glyphs, &metrics);
    assemble_cell_paragraphs(words, cell_bbox)
}

/// Segments a cell's TextLines into multiple ParagraphBlocks (Anti-Squashing: preserving lists, sections, and line breaks)
fn segment_lines_into_cell_paragraphs(mut text_lines: Vec<TextLine>, cell_bbox: Rect) -> Vec<ParagraphBlock> {
    if text_lines.is_empty() {
        return Vec::new();
    }

    build_line_runs(&mut text_lines);

    let mut paragraphs = Vec::new();
    let mut current_para_lines: Vec<TextLine> = Vec::new();

    for line in text_lines {
        let is_new_para = if let Some(prev) = current_para_lines.last() {
            let mut starts_new = false;

            // 1. List / numbering / bullet markers: "1.", "2)", "•", "-", "*", "A.", "B.", "a."
            if let Some(first_word) = line.words.first() {
                let txt = first_word.text.trim();
                if txt.starts_with('•') || txt.starts_with('-') || txt.starts_with('*') {
                    starts_new = true;
                } else if txt.len() >= 2 && (txt.ends_with('.') || txt.ends_with(')')) {
                    let core = &txt[..txt.len() - 1];
                    if core.chars().all(|c| c.is_ascii_digit())
                        || (core.len() == 1 && core.chars().all(|c| c.is_ascii_uppercase()))
                    {
                        starts_new = true;
                    }
                }

                // Bold labels ending with colon (e.g. "Catatan:", "Note:", "Status:")
                if first_word.style.is_bold && txt.ends_with(':') {
                    starts_new = true;
                }
            }

            // 2. Vertical gap between prev line bottom and current line top
            let prev_h = prev.bbox.height().max(8.0);
            let y_gap = line.bbox.y_min - prev.bbox.y_max;
            if y_gap > prev_h * 0.35 {
                starts_new = true;
            }

            // 3. Typographical style shift (e.g. bold heading vs normal text)
            if let (Some(pw), Some(cw)) = (prev.words.first(), line.words.first()) {
                if pw.style.is_bold != cw.style.is_bold && (pw.style.is_bold || cw.style.is_bold) {
                    starts_new = true;
                }
            }

            starts_new
        } else {
            false
        };

        if is_new_para && !current_para_lines.is_empty() {
            let mut p_bbox = current_para_lines[0].bbox;
            for l in &current_para_lines {
                p_bbox.expand(&l.bbox);
            }
            let is_list = current_para_lines[0].words.first()
                .map(|w| {
                    let t = w.text.trim();
                    t.starts_with('•') || t.starts_with('-') || (t.len() >= 2 && (t.ends_with('.') || t.ends_with(')')) && t[..t.len()-1].chars().all(|c| c.is_ascii_digit() || c.is_ascii_uppercase()))
                })
                .unwrap_or(false);

            let align_str = if current_para_lines.len() == 1 && !is_list {
                let l = &current_para_lines[0];
                let line_w = l.bbox.width();
                let line_cx = (l.bbox.x_min + l.bbox.x_max) / 2.0;
                let cell_cx = (cell_bbox.x_min + cell_bbox.x_max) / 2.0;
                let cell_w = cell_bbox.width();
                if (line_cx - cell_cx).abs() <= (cell_w * 0.15) && line_w < (cell_w * 0.85) && cell_w >= 30.0 {
                    "center".to_string()
                } else if (cell_bbox.x_max - l.bbox.x_max) < 15.0 && (l.bbox.x_min - cell_bbox.x_min) > 25.0 {
                    "right".to_string()
                } else {
                    "left".to_string()
                }
            } else {
                "left".to_string()
            };

            paragraphs.push(ParagraphBlock {
                bbox: p_bbox,
                lines: current_para_lines,
                space_before: 0.0,
                space_after: if is_list { 30.0 } else { 60.0 },
                is_list,
                list_id: if is_list { 1 } else { 0 },
                list_level: 0,
                tag: if is_list { "List".to_string() } else { "P".to_string() },
                is_header: false,
                alignment: align_str,
                line_spacing_twips: Some(240),
                line_rule: "auto".to_string(),
                column_index: 0,
            });
            current_para_lines = Vec::new();
        }

        current_para_lines.push(line);
    }

    if !current_para_lines.is_empty() {
        let mut p_bbox = current_para_lines[0].bbox;
        for l in &current_para_lines {
            p_bbox.expand(&l.bbox);
        }
        let is_list = current_para_lines[0].words.first()
            .map(|w| {
                let t = w.text.trim();
                t.starts_with('•') || t.starts_with('-') || (t.len() >= 2 && (t.ends_with('.') || t.ends_with(')')) && t[..t.len()-1].chars().all(|c| c.is_ascii_digit() || c.is_ascii_uppercase()))
            })
            .unwrap_or(false);

        let align_str = if current_para_lines.len() == 1 && !is_list {
            let l = &current_para_lines[0];
            let line_w = l.bbox.width();
            let line_cx = (l.bbox.x_min + l.bbox.x_max) / 2.0;
            let cell_cx = (cell_bbox.x_min + cell_bbox.x_max) / 2.0;
            let cell_w = cell_bbox.width();
            if (line_cx - cell_cx).abs() <= (cell_w * 0.15) && line_w < (cell_w * 0.85) && cell_w >= 30.0 {
                "center".to_string()
            } else if (cell_bbox.x_max - l.bbox.x_max) < 15.0 && (l.bbox.x_min - cell_bbox.x_min) > 25.0 {
                "right".to_string()
            } else {
                "left".to_string()
            }
        } else {
            "left".to_string()
        };

        paragraphs.push(ParagraphBlock {
            bbox: p_bbox,
            lines: current_para_lines,
            space_before: 0.0,
            space_after: 40.0,
            is_list,
            list_id: if is_list { 1 } else { 0 },
            list_level: 0,
            tag: if is_list { "List".to_string() } else { "P".to_string() },
            is_header: false,
            alignment: align_str,
            line_spacing_twips: Some(240),
            line_rule: "auto".to_string(),
            column_index: 0,
        });
    }

    if paragraphs.is_empty() {
        paragraphs.push(ParagraphBlock {
            bbox: cell_bbox,
            lines: Vec::new(),
            space_before: 0.0,
            space_after: 40.0,
            is_list: false,
            list_id: 0,
            list_level: 0,
            tag: "P".to_string(),
            is_header: false,
            alignment: "left".to_string(),
            line_spacing_twips: Some(240),
            line_rule: "auto".to_string(),
            column_index: 0,
        });
    }

    paragraphs
}

fn separate_lines(paths: &[VectorPath]) -> (Vec<HorizLine>, Vec<VertLine>) {
    let mut h_lines = Vec::new();
    let mut v_lines = Vec::new();

    for p in paths {
        // Discard coordinates outside reasonable single-page bounds (e.g. Coda canvas height 5500pt)
        if p.y1 < -5.0 || p.y1 > 1500.0 || p.y2 < -5.0 || p.y2 > 1500.0 {
            continue;
        }
        if p.x1 < -5.0 || p.x1 > 1200.0 || p.x2 < -5.0 || p.x2 > 1200.0 {
            continue;
        }

        let dx = (p.x2 - p.x1).abs();
        let dy = (p.y2 - p.y1).abs();

        // Discard full-page border lines (width >= 480 pt near top/bottom margin, or height >= 680 pt near left/right margin)
        if dx >= 480.0 && (p.y1 <= 50.0 || p.y1 >= 750.0) {
            continue;
        }
        if dy >= 680.0 && (p.x1 <= 50.0 || p.x1 >= 520.0) {
            continue;
        }

        // Horizontal line
        if dy <= 2.0 && dx >= 12.0 {
            h_lines.push(HorizLine {
                y: (p.y1 + p.y2) / 2.0,
                x_min: p.x1.min(p.x2),
                x_max: p.x1.max(p.x2),
            });
        }
        // Vertical line
        else if dx <= 2.0 && dy >= 12.0 {
            v_lines.push(VertLine {
                x: (p.x1 + p.x2) / 2.0,
                y_min: p.y1.min(p.y2),
                y_max: p.y1.max(p.y2),
            });
        }
    }

    (h_lines, v_lines)
}

fn merge_collinear_h(mut lines: Vec<HorizLine>, tol: f64) -> Vec<HorizLine> {
    lines.sort_by(|a, b| {
        a.y.partial_cmp(&b.y)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.x_min.partial_cmp(&b.x_min).unwrap_or(std::cmp::Ordering::Equal))
    });

    let mut merged: Vec<HorizLine> = Vec::new();
    for l in lines {
        if let Some(last) = merged.last_mut() {
            if (last.y - l.y).abs() <= tol && l.x_min <= (last.x_max + tol) {
                last.x_max = last.x_max.max(l.x_max);
                continue;
            }
        }
        merged.push(l);
    }
    merged
}

fn merge_collinear_v(mut lines: Vec<VertLine>, tol: f64) -> Vec<VertLine> {
    lines.sort_by(|a, b| {
        a.x.partial_cmp(&b.x)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.y_min.partial_cmp(&b.y_min).unwrap_or(std::cmp::Ordering::Equal))
    });

    let mut merged: Vec<VertLine> = Vec::new();
    for l in lines {
        if let Some(last) = merged.last_mut() {
            if (last.x - l.x).abs() <= tol && l.y_min <= (last.y_max + tol) {
                last.y_max = last.y_max.max(l.y_max);
                continue;
            }
        }
        merged.push(l);
    }
    merged
}

fn cluster_coordinates(mut coords: Vec<f64>, tol: f64) -> Vec<f64> {
    coords.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut clusters: Vec<Vec<f64>> = Vec::new();

    for c in coords {
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

    clusters
        .into_iter()
        .map(|cluster| cluster.iter().sum::<f64>() / (cluster.len() as f64))
        .collect()
}

fn has_vertical_segment(lines: &[VertLine], x: f64, y1: f64, y2: f64, tol: f64) -> bool {
    let y_min = y1.min(y2);
    let y_max = y1.max(y2);
    lines.iter().any(|v| {
        (v.x - x).abs() <= tol
            && v.y_min <= (y_min + tol)
            && v.y_max >= (y_max - tol)
    })
}

fn has_horizontal_segment(lines: &[HorizLine], y: f64, x1: f64, x2: f64, tol: f64) -> bool {
    let x_min = x1.min(x2);
    let x_max = x1.max(x2);
    lines.iter().any(|h| {
        (h.y - y).abs() <= tol
            && h.x_min <= (x_min + tol)
            && h.x_max >= (x_max - tol)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_simple_table() {
        // 2x2 Table grid:
        // Horizontal lines at y = 100, 150, 200 (x: 50..250)
        // Vertical lines at x = 50, 150, 250 (y: 100..200)
        let paths = vec![
            VectorPath::new(50.0, 100.0, 250.0, 100.0),
            VectorPath::new(50.0, 150.0, 250.0, 150.0),
            VectorPath::new(50.0, 200.0, 250.0, 200.0),
            VectorPath::new(50.0, 100.0, 50.0, 200.0),
            VectorPath::new(150.0, 100.0, 150.0, 200.0),
            VectorPath::new(250.0, 100.0, 250.0, 200.0),
        ];

        let mut paras = Vec::new();
        let tables = TableRecognizer::extract_tables(&paths, &mut paras);

        assert_eq!(tables.len(), 1);
        let table = &tables[0];
        assert_eq!(table.cells.len(), 2);      // 2 rows
        assert_eq!(table.cells[0].len(), 2);   // 2 cols
        assert_eq!(table.cells[0][0].col_span(), 1);
        assert_eq!(table.cells[0][0].row_span(), 1);
    }
}

