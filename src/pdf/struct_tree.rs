use std::collections::BTreeMap;
use lopdf::{Document, Object, ObjectId};
use crate::models::{
    Cell, DocumentPage, Glyph, OutlineItem, ParagraphBlock, Rect,
    StandardTag, StructElement, StructTree, TableMatrix, TextLine, VectorPath, Word,
};

pub struct StructTreeExtractor;

impl StructTreeExtractor {
    /// Extracts the Logical Structure Tree and Outlines from the PDF Document
    pub fn extract(doc: &Document) -> Result<StructTree, String> {
        let mut struct_tree = StructTree::default();

        // 1. Extract Outlines / Bookmarks from Document Catalog
        if let Ok(catalog) = doc.catalog() {
            if let Ok(outlines_obj) = catalog.get(b"Outlines") {
                let outlines_id = match outlines_obj {
                    Object::Reference(r) => Some(*r),
                    _ => None,
                };
                if let Some(oid) = outlines_id {
                    struct_tree.outlines = parse_outlines(doc, oid);
                }
            }

            // 2. Extract /StructTreeRoot
            if let Ok(root_obj) = catalog.get(b"StructTreeRoot") {
                let root_dict = match root_obj {
                    Object::Reference(r) => doc.get_object(*r).ok().and_then(|o| o.as_dict().ok()),
                    Object::Dictionary(d) => Some(d),
                    _ => None,
                };

                if let Some(rd) = root_dict {
                    // 2a. Parse /RoleMap
                    if let Ok(role_map_obj) = rd.get(b"RoleMap") {
                        let role_dict = match role_map_obj {
                            Object::Reference(r) => doc.get_object(*r).ok().and_then(|o| o.as_dict().ok()),
                            Object::Dictionary(d) => Some(d),
                            _ => None,
                        };
                        if let Some(rmd) = role_dict {
                            for (custom_name, target_obj) in rmd.iter() {
                                let target_str = match target_obj {
                                    Object::Name(bytes) => String::from_utf8_lossy(bytes).trim_start_matches('/').to_string(),
                                    Object::String(bytes, _) => String::from_utf8_lossy(bytes).trim_start_matches('/').to_string(),
                                    other => other.as_name_str().unwrap_or("").trim_start_matches('/').to_string(),
                                };
                                let custom_str = String::from_utf8_lossy(custom_name).trim_start_matches('/').to_string();
                                if !custom_str.is_empty() && !target_str.is_empty() {
                                    struct_tree.role_map.insert(custom_str, target_str);
                                }
                            }
                        }
                    }

                    // 2b. Parse children elements in /K
                    if let Ok(k_obj) = rd.get(b"K") {
                        struct_tree.root_elements = parse_struct_kids(doc, k_obj, &struct_tree.role_map);
                    }
                }
            }
        }

        Ok(struct_tree)
    }

    /// Authoritative Semantic Reconstructor: Maps tagged elements directly to DocumentPages
    pub fn apply_to_pages(struct_tree: &StructTree, pages: &mut [DocumentPage]) {
        if struct_tree.root_elements.is_empty() {
            return;
        }

        for page in pages.iter_mut() {
            let page_num = page.page_number;
            let mut page_elements = Vec::new();
            collect_page_elements(&struct_tree.root_elements, page_num, &mut page_elements);

            if page_elements.is_empty() {
                continue;
            }

            page.is_tagged = true;

            // Map MCID to Glyphs on this page
            let mut mcid_to_glyphs: BTreeMap<u32, Vec<Glyph>> = BTreeMap::new();
            for g in &page.glyphs {
                if let Some(mcid) = g.mcid {
                    mcid_to_glyphs.entry(mcid).or_default().push(g.clone());
                }
            }

            // Build structural blocks
            for elem in page_elements {
                let mut mcids = Vec::new();
                collect_all_mcids(elem, &mut mcids);

                let mut elem_glyphs = Vec::new();
                for &mcid in &mcids {
                    if let Some(gs) = mcid_to_glyphs.get(&mcid) {
                        elem_glyphs.extend(gs.clone());
                    }
                }
                if elem_glyphs.is_empty() {
                    continue;
                }

                match &elem.tag {
                    tag if tag.is_heading() => {
                        let block = create_paragraph_block_from_glyphs(
                            elem_glyphs,
                            elem.raw_tag.clone(),
                            true,
                            false,
                            0,
                        );
                        page.paragraphs.push(block);
                    }
                    StandardTag::LI => {
                        let block = create_paragraph_block_from_glyphs(
                            elem_glyphs,
                            "List".to_string(),
                            false,
                            true,
                            1,
                        );
                        page.paragraphs.push(block);
                    }
                    StandardTag::Table => {
                        // Tables are detected with 100% geometric precision by TableRecognizer
                    }
                    _ => {
                        let block = create_paragraph_block_from_glyphs(
                            elem_glyphs,
                            "P".to_string(),
                            false,
                            false,
                            0,
                        );
                        page.paragraphs.push(block);
                    }
                }
            }
        }
    }
}

/// Recursively parses kids in /K
fn parse_struct_kids(
    doc: &Document,
    k_obj: &Object,
    role_map: &BTreeMap<String, String>,
) -> Vec<StructElement> {
    let mut elements = Vec::new();

    match k_obj {
        Object::Array(arr) => {
            for item in arr {
                elements.extend(parse_single_struct_elem(doc, item, role_map));
            }
        }
        Object::Reference(r) => {
            if let Ok(obj) = doc.get_object(*r) {
                elements.extend(parse_single_struct_elem(doc, obj, role_map));
            }
        }
        Object::Dictionary(_) => {
            elements.extend(parse_single_struct_elem(doc, k_obj, role_map));
        }
        _ => {}
    }

    elements
}

/// Parses a single `/StructElem` dictionary or reference
fn parse_single_struct_elem(
    doc: &Document,
    obj: &Object,
    role_map: &BTreeMap<String, String>,
) -> Option<StructElement> {
    let dict = match obj {
        Object::Reference(r) => doc.get_object(*r).ok()?.as_dict().ok()?,
        Object::Dictionary(d) => d,
        _ => return None,
    };

    // 1. Structure Type /S
    let s_obj = dict.get(b"S").ok()?;
    let raw_tag = match s_obj {
        Object::Name(bytes) => String::from_utf8_lossy(bytes).trim_start_matches('/').to_string(),
        Object::String(bytes, _) => String::from_utf8_lossy(bytes).trim_start_matches('/').to_string(),
        other => other.as_name_str().ok()?.trim_start_matches('/').to_string(),
    };
    let resolved_tag_name = role_map.get(&raw_tag).unwrap_or(&raw_tag);
    let standard_tag = StandardTag::from_str_name(resolved_tag_name);

    let mut elem = StructElement::new(standard_tag, raw_tag);

    // 2. Page Reference /Pg
    if let Ok(pg_ref) = dict.get(b"Pg").and_then(|o| o.as_reference()) {
        elem.page_num = get_page_number_for_ref(doc, pg_ref);
    }

    // 3. Metadata: /Title, /Alt, /ActualText
    if let Ok(title_obj) = dict.get(b"Title") {
        elem.title = decode_pdf_string(title_obj);
    }
    if let Ok(alt_obj) = dict.get(b"Alt") {
        elem.alt = decode_pdf_string(alt_obj);
    }
    if let Ok(actual_obj) = dict.get(b"ActualText") {
        elem.actual_text = decode_pdf_string(actual_obj);
    }

    // 4. Attributes /A (e.g. Table /ColSpan, /RowSpan)
    if let Ok(a_obj) = dict.get(b"A") {
        let a_dict = match a_obj {
            Object::Reference(r) => doc.get_object(*r).ok().and_then(|o| o.as_dict().ok()),
            Object::Dictionary(d) => Some(d),
            _ => None,
        };
        if let Some(ad) = a_dict {
            if let Ok(cs) = ad.get(b"ColSpan").and_then(|o| o.as_i64()) {
                elem.col_span = cs.max(1) as usize;
            }
            if let Ok(rs) = ad.get(b"RowSpan").and_then(|o| o.as_i64()) {
                elem.row_span = rs.max(1) as usize;
            }
        }
    }

    // 5. Children in /K
    if let Ok(k_obj) = dict.get(b"K") {
        match k_obj {
            Object::Integer(mcid) => {
                elem.mcids.push(*mcid as u32);
            }
            Object::Array(arr) => {
                for item in arr {
                    match item {
                        Object::Integer(mcid) => {
                            elem.mcids.push(*mcid as u32);
                        }
                        Object::Dictionary(d) => {
                            // Check if Marked Content Reference /MCR
                            if let Ok(type_name) = d.get(b"Type").and_then(|o| o.as_name_str()) {
                                if type_name == "MCR" {
                                    if let Ok(m) = d.get(b"MCID").and_then(|o| o.as_i64()) {
                                        elem.mcids.push(m as u32);
                                    }
                                    continue;
                                }
                            }
                            if let Some(child) = parse_single_struct_elem(doc, item, role_map) {
                                elem.children.push(child);
                            }
                        }
                        Object::Reference(r) => {
                            if let Ok(target_obj) = doc.get_object(*r) {
                                if let Ok(d) = target_obj.as_dict() {
                                    if let Ok(type_name) = d.get(b"Type").and_then(|o| o.as_name_str()) {
                                        if type_name == "MCR" {
                                            if let Ok(m) = d.get(b"MCID").and_then(|o| o.as_i64()) {
                                                elem.mcids.push(m as u32);
                                            }
                                            continue;
                                        }
                                    }
                                }
                                if let Some(child) = parse_single_struct_elem(doc, target_obj, role_map) {
                                    elem.children.push(child);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Object::Reference(r) => {
                if let Ok(target_obj) = doc.get_object(*r) {
                    if let Some(child) = parse_single_struct_elem(doc, target_obj, role_map) {
                        elem.children.push(child);
                    }
                }
            }
            Object::Dictionary(_) => {
                if let Some(child) = parse_single_struct_elem(doc, k_obj, role_map) {
                    elem.children.push(child);
                }
            }
            _ => {}
        }
    }

    Some(elem)
}

/// Recursively collects all MCIDs from a StructElement and its children
fn collect_all_mcids(elem: &StructElement, out: &mut Vec<u32>) {
    out.extend(&elem.mcids);
    for child in &elem.children {
        collect_all_mcids(child, out);
    }
}

/// Helper to find page number for an element from itself or its descendants
fn find_elem_page_num(elem: &StructElement) -> Option<usize> {
    if let Some(p) = elem.page_num {
        return Some(p);
    }
    for child in &elem.children {
        if let Some(p) = find_elem_page_num(child) {
            return Some(p);
        }
    }
    None
}

/// Recursively collects all TR elements from a table element (handling TBody, THead, TFoot)
fn find_all_trs<'a>(elem: &'a StructElement, out: &mut Vec<&'a StructElement>) {
    if elem.tag == StandardTag::TR {
        out.push(elem);
        return;
    }
    for child in &elem.children {
        find_all_trs(child, out);
    }
}

/// Recursively collects all TD/TH elements from a TR element
fn find_all_tds<'a>(elem: &'a StructElement, out: &mut Vec<&'a StructElement>) {
    if elem.tag == StandardTag::TD || elem.tag == StandardTag::TH {
        out.push(elem);
        return;
    }
    for child in &elem.children {
        find_all_tds(child, out);
    }
}

/// Collects block-level elements belonging to a specific page
fn collect_page_elements<'a>(
    elements: &'a [StructElement],
    target_page: usize,
    out: &mut Vec<&'a StructElement>,
) {
    for elem in elements {
        if elem.tag == StandardTag::Table {
            let elem_page = find_elem_page_num(elem);
            if elem_page == Some(target_page) {
                out.push(elem);
            }
            // Do NOT recurse into table children - they belong to the table
            continue;
        }

        let elem_page = elem.page_num.or_else(|| find_elem_page_num(elem));
        if elem_page == Some(target_page) {
            // Block-level content elements: Headings, Paragraphs, ListItems, BlockQuotes
            let is_block = elem.tag.is_heading()
                || elem.tag == StandardTag::P
                || elem.tag == StandardTag::LI
                || elem.tag == StandardTag::BlockQuote;

            if is_block {
                out.push(elem);
                // Consume all descendants (Span, Lbl, LBody, etc.) within this block; do not recurse
                continue;
            }
        }

        // For structural containers (Document, Part, Sect, Div, Art, L), recurse into children
        collect_page_elements(&elem.children, target_page, out);
    }
}

/// Helper to create a ParagraphBlock from a list of Glyphs via Docstrum word clustering
fn create_paragraph_block_from_glyphs(
    glyphs: Vec<Glyph>,
    tag: String,
    is_header: bool,
    is_list: bool,
    list_level: i32,
) -> ParagraphBlock {
    if glyphs.is_empty() {
        return ParagraphBlock {
            bbox: Rect::new(0.0, 0.0, 0.0, 0.0),
            lines: Vec::new(),
            space_before: 0.0,
            space_after: 0.0,
            is_list,
            list_id: if is_list { 1 } else { 0 },
            list_level,
            tag,
            is_header,
            alignment: "left".to_string(),
            line_spacing_twips: None,
            line_rule: "auto".to_string(),
            column_index: 0,
        };
    }

    let metrics = crate::dla::DocstrumAnalyzer::compute_metrics(&glyphs);
    let words = crate::dla::DocstrumAnalyzer::cluster_words(&glyphs, &metrics);

    let mut sorted_words = words;
    sorted_words.sort_by(|a, b| {
        let cy_a = (a.bbox.y_min + a.bbox.y_max) / 2.0;
        let cy_b = (b.bbox.y_min + b.bbox.y_max) / 2.0;
        cy_a.partial_cmp(&cy_b)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.bbox.x_min.partial_cmp(&b.bbox.x_min).unwrap_or(std::cmp::Ordering::Equal))
    });

    let mut lines_words: Vec<Vec<Word>> = Vec::new();
    for w in sorted_words {
        let cy = (w.bbox.y_min + w.bbox.y_max) / 2.0;
        let mut matched = false;
        for line in lines_words.iter_mut() {
            if let Some(first) = line.first() {
                let first_cy = (first.bbox.y_min + first.bbox.y_max) / 2.0;
                let h = first.bbox.height().max(w.bbox.height()).max(8.0);
                if (first_cy - cy).abs() <= (h * 0.4) {
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
    let mut block_bbox = Rect::new(0.0, 0.0, 0.0, 0.0);
    let mut is_first = true;

    for mut line_w in lines_words {
        line_w.sort_by(|a, b| a.bbox.x_min.partial_cmp(&b.bbox.x_min).unwrap_or(std::cmp::Ordering::Equal));
        let mut line_bbox = line_w[0].bbox;
        for w in &line_w {
            line_bbox.expand(&w.bbox);
        }
        if is_first {
            block_bbox = line_bbox;
            is_first = false;
        } else {
            block_bbox.expand(&line_bbox);
        }
        text_lines.push(TextLine {
            bbox: line_bbox,
            words: line_w,
            runs: Vec::new(),
            tab_stops: Vec::new(),
        });
    }

    ParagraphBlock {
        bbox: block_bbox,
        lines: text_lines,
        space_before: 0.0,
        space_after: 0.0,
        is_list,
        list_id: if is_list { 1 } else { 0 },
        list_level,
        tag,
        is_header,
        alignment: "left".to_string(),
        line_spacing_twips: None,
        line_rule: "auto".to_string(),
        column_index: 0,
    }
}

/// Builds a structured TableMatrix from a `/Table` StructElement
fn build_table_from_struct_elem(
    table_elem: &StructElement,
    mcid_to_glyphs: &BTreeMap<u32, Vec<Glyph>>,
    vector_paths: &[VectorPath],
    page_width: f64,
) -> Option<TableMatrix> {
    let mut tr_elements = Vec::new();
    find_all_trs(table_elem, &mut tr_elements);

    if tr_elements.is_empty() {
        return None;
    }

    let mut table_cells: Vec<Vec<Cell>> = Vec::new();
    let mut table_bbox = Rect::new(0.0, 0.0, 0.0, 0.0);
    let mut has_bbox = false;

    for (row_idx, tr) in tr_elements.iter().enumerate() {
        let mut row_cells = Vec::new();
        let mut td_elements = Vec::new();
        find_all_tds(tr, &mut td_elements);

        let mut current_col = 0;
        for td in td_elements {
            let mut mcids = Vec::new();
            collect_all_mcids(td, &mut mcids);

            let mut cell_glyphs = Vec::new();
            for &mcid in &mcids {
                if let Some(gs) = mcid_to_glyphs.get(&mcid) {
                    cell_glyphs.extend(gs.clone());
                }
            }

            let mut cell_bbox = Rect::new(0.0, 0.0, 0.0, 0.0);
            if !cell_glyphs.is_empty() {
                if !has_bbox {
                    table_bbox = cell_glyphs[0].bbox;
                    has_bbox = true;
                }
                cell_bbox = cell_glyphs[0].bbox;
                for g in &cell_glyphs {
                    cell_bbox.expand(&g.bbox);
                    table_bbox.expand(&g.bbox);
                }
            }

            let paras = crate::dla::tables::assemble_cell_paragraphs_from_glyphs(
                cell_glyphs,
                cell_bbox,
            );

            let start_col = current_col;
            let end_col = start_col + td.col_span.max(1);
            let start_row = row_idx;
            let end_row = start_row + td.row_span.max(1);

            row_cells.push(Cell {
                bbox: cell_bbox,
                start_row,
                end_row,
                start_col,
                end_col,
                content_blocks: paras,
                border_top: true,
                border_bottom: true,
                border_left: true,
                border_right: true,
            });

            current_col = end_col;
        }

        // Validate row Y-monotonicity: PDF table rows must progress downwards in Y
        let mut row_min_y = f64::INFINITY;
        let mut row_has_text = false;
        for c in &row_cells {
            if c.bbox.width() > 0.0 && c.bbox.height() > 0.0 {
                row_min_y = row_min_y.min(c.bbox.y_min);
                row_has_text = true;
            }
        }

        if row_has_text && !table_cells.is_empty() {
            let last_row_y = table_cells
                .last()
                .unwrap()
                .iter()
                .map(|c| c.bbox.y_min)
                .filter(|&y| y > 0.0)
                .fold(f64::INFINITY, f64::min);
            if last_row_y < f64::INFINITY && (row_min_y < (last_row_y - 20.0) || (row_min_y - last_row_y) > 600.0) {
                // Out-of-order row: backwards jump or detached page split
                continue;
            }
        }

        table_cells.push(row_cells);
    }

    if table_cells.is_empty() {
        return None;
    }

    let num_rows = table_cells.len();
    let num_cols = table_cells
        .iter()
        .map(|row| row.iter().map(|c| c.end_col).max().unwrap_or(0))
        .max()
        .unwrap_or(0);

    if num_cols == 0 {
        return None;
    }

    // Compute column horizontal boundaries
    let mut col_min_x: Vec<Option<f64>> = vec![None; num_cols];
    let mut col_max_x: Vec<Option<f64>> = vec![None; num_cols];

    for row in &table_cells {
        for cell in row {
            if cell.bbox.width() > 0.0 && cell.start_col + 1 == cell.end_col && cell.start_col < num_cols {
                let c = cell.start_col;
                col_min_x[c] = Some(col_min_x[c].map_or(cell.bbox.x_min, |v| v.min(cell.bbox.x_min)));
                col_max_x[c] = Some(col_max_x[c].map_or(cell.bbox.x_max, |v| v.max(cell.bbox.x_max)));
            }
        }
    }

    // Snap table horizontal boundaries and row rails to vector rulings if present
    let mut vec_left = None;
    let mut vec_right = None;
    let mut h_rulings: Vec<f64> = Vec::new();

    for vp in vector_paths {
        let is_in_y = vp.y1.min(vp.y2) <= (table_bbox.y_max + 15.0) && vp.y1.max(vp.y2) >= (table_bbox.y_min - 15.0);
        if is_in_y {
            // Horizontal rulings define table width and row boundaries
            if (vp.y1 - vp.y2).abs() <= 1.5 && (vp.x1 - vp.x2).abs() >= 100.0 {
                let h_min = vp.x1.min(vp.x2);
                let h_max = vp.x1.max(vp.x2);
                vec_left = Some(vec_left.map_or(h_min, |v: f64| v.min(h_min)));
                vec_right = Some(vec_right.map_or(h_max, |v: f64| v.max(h_max)));
                h_rulings.push((vp.y1 + vp.y2) / 2.0);
            }
        }
    }

    h_rulings.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut clustered_y_rulings: Vec<f64> = Vec::new();
    for y in h_rulings {
        if let Some(last) = clustered_y_rulings.last() {
            if (y - last).abs() <= 3.5 {
                continue;
            }
        }
        clustered_y_rulings.push(y);
    }

    if let (Some(&first_y), Some(&last_y)) = (clustered_y_rulings.first(), clustered_y_rulings.last()) {
        table_bbox.y_min = if table_bbox.y_min > 0.0 { table_bbox.y_min.min(first_y) } else { first_y };
        table_bbox.y_max = if table_bbox.y_max > 0.0 { table_bbox.y_max.max(last_y) } else { last_y };
    }

    let t_left = vec_left.unwrap_or_else(|| if table_bbox.width() > 0.0 { table_bbox.x_min } else { 72.0 });
    let t_right = vec_right.unwrap_or_else(|| {
        if table_bbox.width() >= (page_width * 0.40) {
            (page_width - 54.0).max(table_bbox.x_max)
        } else if table_bbox.width() > 0.0 {
            table_bbox.x_max
        } else {
            page_width - 72.0
        }
    });

    let mut x_rails = vec![0.0; num_cols + 1];
    x_rails[0] = t_left;
    x_rails[num_cols] = t_right;

    for c in 0..num_cols - 1 {
        let mid = match (col_max_x[c], col_min_x[c + 1]) {
            (Some(max_c), Some(min_next)) if max_c < min_next => (max_c + min_next) / 2.0,
            (Some(max_c), _) => max_c + 10.0,
            (_, Some(min_next)) => min_next - 10.0,
            _ => t_left + ((c + 1) as f64 / num_cols as f64) * (t_right - t_left),
        };
        x_rails[c + 1] = mid.clamp(x_rails[c] + 15.0, t_right - 15.0 * (num_cols - 1 - c) as f64);
    }

    let mut grid_cols = Vec::with_capacity(num_cols);
    for c in 0..num_cols {
        grid_cols.push((x_rails[c + 1] - x_rails[c]).max(20.0));
    }

    let mut y_rails = vec![0.0; num_rows + 1];
    if clustered_y_rulings.len() == num_rows + 1 {
        y_rails = clustered_y_rulings;
    } else {
        let y_top = clustered_y_rulings.first().copied().unwrap_or(table_bbox.y_min);
        let y_bot = clustered_y_rulings.last().copied().unwrap_or(table_bbox.y_max);
        y_rails[0] = y_top;
        y_rails[num_rows] = y_bot;
        for r in 0..num_rows - 1 {
            y_rails[r + 1] = y_top + ((r + 1) as f64 / num_rows as f64) * (y_bot - y_top);
        }
    }

    let mut grid_rows = Vec::with_capacity(num_rows);
    for r in 0..num_rows {
        grid_rows.push((y_rails[r + 1] - y_rails[r]).max(15.0));
    }

    for row in table_cells.iter_mut() {
        for cell in row.iter_mut() {
            let x1 = if cell.start_col < x_rails.len() { x_rails[cell.start_col] } else { t_left };
            let x2 = if cell.end_col < x_rails.len() { x_rails[cell.end_col] } else { t_right };
            let y1 = if cell.start_row < y_rails.len() { y_rails[cell.start_row] } else { table_bbox.y_min };
            let y2 = if cell.end_row < y_rails.len() { y_rails[cell.end_row] } else { table_bbox.y_max };
            cell.bbox = Rect::new(x1, y1, x2, y2);
        }
    }

    Some(TableMatrix {
        bbox: table_bbox,
        grid_cols,
        grid_rows,
        cells: table_cells,
    })
}

/// Parses Document Outlines / Bookmarks recursively
fn parse_outlines(doc: &Document, outline_dict_id: ObjectId) -> Vec<OutlineItem> {
    let mut items = Vec::new();

    let dict = match doc.get_object(outline_dict_id).ok().and_then(|o| o.as_dict().ok()) {
        Some(d) => d,
        None => return items,
    };

    // Follow linked list via /First and /Next
    let mut curr_ref = dict.get(b"First").ok().and_then(|o| o.as_reference().ok());

    while let Some(item_id) = curr_ref {
        if let Ok(item_obj) = doc.get_object(item_id) {
            if let Ok(item_dict) = item_obj.as_dict() {
                let mut title = String::new();
                if let Ok(t_obj) = item_dict.get(b"Title") {
                    title = decode_pdf_string(t_obj).unwrap_or_default();
                }

                let mut page_num = None;
                let mut dest_y = None;

                if let Ok(dest_obj) = item_dict.get(b"Dest") {
                    if let Ok(dest_arr) = dest_obj.as_array() {
                        if !dest_arr.is_empty() {
                            if let Ok(page_ref) = dest_arr[0].as_reference() {
                                page_num = get_page_number_for_ref(doc, page_ref);
                            }
                            if dest_arr.len() >= 4 {
                                match &dest_arr[2] {
                                    Object::Real(r) => dest_y = Some(*r as f64),
                                    Object::Integer(i) => dest_y = Some(*i as f64),
                                    _ => {}
                                }
                            }
                        }
                    }
                }

                let mut child_items = Vec::new();
                if let Some(first_child) = item_dict.get(b"First").ok().and_then(|o| o.as_reference().ok()) {
                    child_items = parse_outlines(doc, first_child);
                }

                items.push(OutlineItem {
                    title,
                    page_number: page_num,
                    dest_y,
                    children: child_items,
                });

                curr_ref = item_dict.get(b"Next").ok().and_then(|o| o.as_reference().ok());
            } else {
                break;
            }
        } else {
            break;
        }
    }

    items
}

/// Maps an ObjectId to a 1-based page number
fn get_page_number_for_ref(doc: &Document, page_ref: ObjectId) -> Option<usize> {
    for (&num, &id) in doc.get_pages().iter() {
        if id == page_ref {
            return Some(num as usize);
        }
    }
    None
}

/// Decodes PDF String (ASCII or UTF-16BE with BOM 0xFEFF)
fn decode_pdf_string(obj: &Object) -> Option<String> {
    match obj {
        Object::String(bytes, _) => {
            if bytes.starts_with(&[0xFE, 0xFF]) {
                // UTF-16BE
                let u16_chars: Vec<u16> = bytes[2..]
                    .chunks(2)
                    .filter_map(|chunk| {
                        if chunk.len() == 2 {
                            Some(((chunk[0] as u16) << 8) | (chunk[1] as u16))
                        } else {
                            None
                        }
                    })
                    .collect();
                String::from_utf16(&u16_chars).ok()
            } else {
                Some(String::from_utf8_lossy(bytes).to_string())
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_pdf_string_utf16() {
        let bytes = vec![
            0xFE, 0xFF, 0x00, 0x44, 0x00, 0x75, 0x00, 0x6D, 0x00, 0x6D, 0x00, 0x79,
        ];
        let obj = Object::String(bytes, lopdf::StringFormat::Literal);
        assert_eq!(decode_pdf_string(&obj).unwrap(), "Dummy");
    }

    #[test]
    fn test_standard_tag_classification() {
        let tag = StandardTag::from_str_name("Heading1");
        assert_eq!(tag, StandardTag::Other("Heading1".to_string()));

        let h1 = StandardTag::from_str_name("H1");
        assert!(h1.is_heading());
        assert_eq!(h1.heading_level(), Some(1));

        let p = StandardTag::from_str_name("P");
        assert!(!p.is_heading());
    }
}
