use crate::models::{Cell, DocumentPage, GraphicObject, ParagraphBlock, TableMatrix, TextRun};
use crate::utils::{escape_xml, points_to_half_points, points_to_twips};

pub struct DocxDocumentBuilder;

impl DocxDocumentBuilder {
    /// Builds word/document.xml from structured document pages
    pub fn build_document_xml(pages: &[DocumentPage]) -> String {
        let mut xml = String::with_capacity(64 * 1024);

        xml.push_str(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:wpc="http://schemas.microsoft.com/office/word/2010/wordprocessingCanvas"
            xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"
            xmlns:o="urn:schemas-microsoft-com:office:office"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
            xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math"
            xmlns:v="urn:schemas-microsoft-com:vml"
            xmlns:wp14="http://schemas.microsoft.com/office/word/2010/wordprocessingDrawing"
            xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
            xmlns:w10="urn:schemas-microsoft-com:office:word"
            xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml"
            xmlns:wpg="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup"
            xmlns:wpi="http://schemas.microsoft.com/office/word/2010/wordprocessingInk"
            xmlns:wne="http://schemas.microsoft.com/office/word/2006/wordml"
            xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"
            xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
            xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"
            mc:Ignorable="w14 wp14">
  <w:body>
"#);

        let total_pages = pages.len();
        let header_count = pages.iter().filter(|p| p.header.is_some()).count();
        let footer_count = pages.iter().filter(|p| p.footer.is_some()).count();

        // A document running header must be present across >= 50% of pages (or >= 2 if <= 3 pages)
        let has_header = if total_pages <= 3 {
            header_count >= 2
        } else {
            (header_count as f64 / total_pages as f64) >= 0.50
        };

        // A document running footer must be present across >= 30% of pages (or >= 1 if <= 3 pages)
        let has_footer = if total_pages <= 3 {
            footer_count >= 1
        } else {
            (footer_count as f64 / total_pages as f64) >= 0.30
        };

        // Section Properties: Dominant / Median Page Margins
        let mut left_margins: Vec<f64> = pages.iter().map(|p| p.margin_left_points).filter(|&m| m > 10.0).collect();
        let mut right_margins: Vec<f64> = pages.iter().map(|p| p.margin_right_points).filter(|&m| m > 10.0).collect();
        let mut top_margins: Vec<f64> = pages.iter().map(|p| p.margin_top_points).filter(|&m| m > 10.0).collect();
        let mut bottom_margins: Vec<f64> = pages.iter().map(|p| p.margin_bottom_points).filter(|&m| m > 10.0).collect();

        left_margins.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        right_margins.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        top_margins.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        bottom_margins.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let left_pts = if !left_margins.is_empty() { left_margins[left_margins.len() / 2] } else { 72.0 };
        let right_pts = if !right_margins.is_empty() { right_margins[right_margins.len() / 2] } else { 72.0 };
        let top_pts = if !top_margins.is_empty() { top_margins[top_margins.len() / 2].clamp(28.0, 72.0) } else { 54.0 };
        let bottom_pts = if !bottom_margins.is_empty() { bottom_margins[bottom_margins.len() / 2].clamp(28.0, 72.0) } else { 54.0 };

        let left_twips = points_to_twips(left_pts);
        let right_twips = points_to_twips(right_pts);
        let top_twips = points_to_twips(top_pts);
        let bottom_twips = points_to_twips(bottom_pts);

        let mut last_page_w = 11906; // Default A4 in twips
        let mut last_page_h = 16838;

        let mut last_rendered_page: Option<&DocumentPage> = None;
        let mut is_prev_first_page = false;
        let mut global_img_idx = 1;
        let mut has_emitted_first_multi_col_sect = false;

        for page in pages.iter() {
            last_page_w = points_to_twips(page.width_points);
            last_page_h = points_to_twips(page.height_points);

            let page_left_pts = if page.margin_left_points > 10.0 { page.margin_left_points } else { left_pts };
            let page_right_pts = if page.margin_right_points > 10.0 { page.margin_right_points } else { right_pts };
            let printable_w_pts = (page.width_points - page_left_pts - page_right_pts).max(100.0);

            // Separate background graphics from inline content graphics
            let mut bg_graphics = Vec::new();
            let mut content_graphics = Vec::new();

            for g in &page.graphics {
                if g.bbox.width() < 2.0 || g.bbox.height() < 2.0 {
                    continue;
                }
                if g.is_background {
                    bg_graphics.push((g, global_img_idx));
                } else {
                    content_graphics.push((g, global_img_idx));
                }
                global_img_idx += 1;
            }

            // Check if page has any content
            let has_paras = page.paragraphs.iter().any(|p| p.lines.iter().any(|l| l.words.iter().any(|w| !w.text.trim().is_empty())));
            let has_tables = !page.tables.is_empty();
            let has_graphics = !page.graphics.is_empty();

            if !has_paras && !has_tables && !has_graphics {
                continue;
            }

            // Insert page break only at semantic chapter/section boundaries or short pages
            if let Some(prev) = last_rendered_page {
                if should_insert_page_break(prev, page, is_prev_first_page) {
                    xml.push_str(r#"    <w:p>
      <w:r>
        <w:br w:type="page"/>
      </w:r>
    </w:p>
"#);
                }
            }
            is_prev_first_page = last_rendered_page.is_none();
            last_rendered_page = Some(page);

            // 1. Render all Background Graphics FIRST as floating DrawingML anchors behind document
            render_background_graphics_container(&mut xml, &bg_graphics, 4);

            // 2. Check if this page is a Multi-Column page
            let is_page_multi_column = page.paragraphs.iter().any(|p| p.column_index == 1);

            enum PageItem<'a> {
                Para(&'a ParagraphBlock),
                Table(&'a TableMatrix),
                Graphic(&'a GraphicObject, usize),
            }

            if is_page_multi_column {
                // Multi-Column Layout: Partition into Column 0 and Column 1
                let gutter_x = page.width_points / 2.0;

                // Check for spanning elements at the top of the first multi-column page
                let mut spanning_items: Vec<PageItem> = Vec::new();
                let mut col0_items: Vec<PageItem> = Vec::new();
                let mut col1_items: Vec<PageItem> = Vec::new();

                let is_first_multi_col_page = !has_emitted_first_multi_col_sect;
                let mut reached_columnar = false;

                for p in &page.paragraphs {
                    let has_text = p.lines.iter().any(|l| l.words.iter().any(|w| !w.text.trim().is_empty()));
                    if !has_text {
                        continue;
                    }

                    if is_first_multi_col_page && !reached_columnar {
                        let is_wide = p.bbox.width() >= (0.55 * page.width_points);
                        let straddles_center = p.bbox.x_min < (gutter_x - 20.0) && p.bbox.x_max > (gutter_x + 20.0);
                        let is_running_header = p.bbox.y_max <= 45.0;

                        if (is_wide || straddles_center || is_running_header) && p.column_index == 0 {
                            spanning_items.push(PageItem::Para(p));
                            continue;
                        } else {
                            reached_columnar = true;
                        }
                    }

                    if p.column_index == 1 {
                        col1_items.push(PageItem::Para(p));
                    } else {
                        col0_items.push(PageItem::Para(p));
                    }
                }

                for t in &page.tables {
                    let cx = (t.bbox.x_min + t.bbox.x_max) / 2.0;
                    if cx >= gutter_x {
                        col1_items.push(PageItem::Table(t));
                    } else {
                        col0_items.push(PageItem::Table(t));
                    }
                }

                for (g, img_idx) in content_graphics {
                    let cx = (g.bbox.x_min + g.bbox.x_max) / 2.0;
                    if cx >= gutter_x {
                        col1_items.push(PageItem::Graphic(g, img_idx));
                    } else {
                        col0_items.push(PageItem::Graphic(g, img_idx));
                    }
                }

                // If there are spanning items at the top of Page 1, render them in a 1-column section
                if !spanning_items.is_empty() {
                    for item in spanning_items {
                        match item {
                            PageItem::Para(p) => render_paragraph(&mut xml, p, printable_w_pts, page_left_pts, 4),
                            PageItem::Table(t) => render_table(&mut xml, t, 4),
                            PageItem::Graphic(g, img_idx) => render_graphic(&mut xml, g, img_idx, page.width_points, left_pts, right_pts, 4),
                        }
                    }

                    let hdr_ref_break = if has_header {
                        r#"          <w:headerReference w:type="default" r:id="rIdHeader1"/>
"#
                    } else {
                        ""
                    };
                    let ftr_ref_break = if has_footer {
                        r#"          <w:footerReference w:type="default" r:id="rIdFooter1"/>
"#
                    } else {
                        ""
                    };

                    // Continuous section break transitioning to 2 columns
                    xml.push_str(&format!(
                        r#"    <w:p>
      <w:pPr>
        <w:spacing w:before="0" w:after="0" w:line="1" w:lineRule="exact"/>
        <w:sectPr>
{}{}{}{}          <w:cols w:num="1" w:space="720"/>
        </w:sectPr>
      </w:pPr>
    </w:p>
"#,
                        hdr_ref_break,
                        ftr_ref_break,
                        format!("          <w:pgSz w:w=\"{}\" w:h=\"{}\"/>\n", last_page_w, last_page_h),
                        format!("          <w:pgMar w:top=\"{}\" w:right=\"{}\" w:bottom=\"{}\" w:left=\"{}\" w:header=\"360\" w:footer=\"360\" w:gutter=\"0\"/>\n", top_twips, right_twips, bottom_twips, left_twips)
                    ));
                    has_emitted_first_multi_col_sect = true;
                }

                // Sort items within each column by Y position
                col0_items.sort_by(|a, b| {
                    let ya = match a {
                        PageItem::Para(p) => p.lines.first().map(|l| l.bbox.y_min).unwrap_or(p.bbox.y_min),
                        PageItem::Table(t) => t.bbox.y_min,
                        PageItem::Graphic(g, _) => g.bbox.y_min,
                    };
                    let yb = match b {
                        PageItem::Para(p) => p.lines.first().map(|l| l.bbox.y_min).unwrap_or(p.bbox.y_min),
                        PageItem::Table(t) => t.bbox.y_min,
                        PageItem::Graphic(g, _) => g.bbox.y_min,
                    };
                    ya.partial_cmp(&yb).unwrap_or(std::cmp::Ordering::Equal)
                });

                col1_items.sort_by(|a, b| {
                    let ya = match a {
                        PageItem::Para(p) => p.lines.first().map(|l| l.bbox.y_min).unwrap_or(p.bbox.y_min),
                        PageItem::Table(t) => t.bbox.y_min,
                        PageItem::Graphic(g, _) => g.bbox.y_min,
                    };
                    let yb = match b {
                        PageItem::Para(p) => p.lines.first().map(|l| l.bbox.y_min).unwrap_or(p.bbox.y_min),
                        PageItem::Table(t) => t.bbox.y_min,
                        PageItem::Graphic(g, _) => g.bbox.y_min,
                    };
                    ya.partial_cmp(&yb).unwrap_or(std::cmp::Ordering::Equal)
                });

                let col_w = (printable_w_pts - 18.0) / 2.0;

                // Render Column 0 (Left Column)
                for item in col0_items {
                    match item {
                        PageItem::Para(p) => render_paragraph(&mut xml, p, col_w, page_left_pts, 4),
                        PageItem::Table(t) => render_table(&mut xml, t, 4),
                        PageItem::Graphic(g, img_idx) => render_graphic(&mut xml, g, img_idx, page.width_points, left_pts, right_pts, 4),
                    }
                }

                // Column break to switch to Right Column
                if !col1_items.is_empty() {
                    xml.push_str(r#"    <w:p>
      <w:r>
        <w:br w:type="column"/>
      </w:r>
    </w:p>
"#);
                }

                // Render Column 1 (Right Column)
                for item in col1_items {
                    match item {
                        PageItem::Para(p) => render_paragraph(&mut xml, p, col_w, page_left_pts + col_w + 18.0, 4),
                        PageItem::Table(t) => render_table(&mut xml, t, 4),
                        PageItem::Graphic(g, img_idx) => render_graphic(&mut xml, g, img_idx, page.width_points, left_pts, right_pts, 4),
                    }
                }
            } else {
                // Single Column Layout: Sort all items by vertical Y position
                let mut items: Vec<PageItem> = Vec::new();

                for p in &page.paragraphs {
                    let has_text = p.lines.iter().any(|l| l.words.iter().any(|w| !w.text.trim().is_empty()));
                    if has_text {
                        items.push(PageItem::Para(p));
                    }
                }
                for t in &page.tables {
                    items.push(PageItem::Table(t));
                }
                for (g, img_idx) in content_graphics {
                    items.push(PageItem::Graphic(g, img_idx));
                }

                items.sort_by(|a, b| {
                    let ya = match a {
                        PageItem::Para(p) => p.lines.first().map(|l| l.bbox.y_min).unwrap_or(p.bbox.y_min),
                        PageItem::Table(t) => t.bbox.y_min,
                        PageItem::Graphic(g, _) => g.bbox.y_min,
                    };
                    let yb = match b {
                        PageItem::Para(p) => p.lines.first().map(|l| l.bbox.y_min).unwrap_or(p.bbox.y_min),
                        PageItem::Table(t) => t.bbox.y_min,
                        PageItem::Graphic(g, _) => g.bbox.y_min,
                    };
                    ya.partial_cmp(&yb).unwrap_or(std::cmp::Ordering::Equal)
                });

                for item in items {
                    match item {
                        PageItem::Para(p) => render_paragraph(&mut xml, p, printable_w_pts, page_left_pts, 4),
                        PageItem::Table(t) => render_table(&mut xml, t, 4),
                        PageItem::Graphic(g, img_idx) => render_graphic(&mut xml, g, img_idx, page.width_points, left_pts, right_pts, 4),
                    }
                }
            }
        }



        let cols_xml = if has_emitted_first_multi_col_sect {
            r#"      <w:type w:val="continuous"/>
      <w:cols w:num="2" w:space="720"/>
"#
        } else {
            ""
        };

        let hdr_ref = if has_header {
            r#"      <w:headerReference w:type="default" r:id="rIdHeader1"/>
"#
        } else {
            ""
        };
        let ftr_ref = if has_footer {
            r#"      <w:footerReference w:type="default" r:id="rIdFooter1"/>
"#
        } else {
            ""
        };

        let page1_has_hf = pages.first().map(|p| p.header.is_some() || p.footer.is_some()).unwrap_or(false);
        let title_pg_xml = if !page1_has_hf && (has_header || has_footer) {
            "      <w:titlePg/>\n"
        } else {
            ""
        };

        xml.push_str(&format!(
            r#"    <w:sectPr>
{}{}      <w:pgSz w:w="{}" w:h="{}"/>
      <w:pgMar w:top="{}" w:right="{}" w:bottom="{}" w:left="{}" w:header="360" w:footer="360" w:gutter="0"/>
{}{}    </w:sectPr>
  </w:body>
</w:document>"#,
            hdr_ref, ftr_ref, last_page_w, last_page_h, top_twips, right_twips, bottom_twips, left_twips, title_pg_xml, cols_xml
        ));

        xml
    }
}

/// Renders a single `<w:p>` paragraph block with styles, tab stops, and runs
fn render_paragraph(
    xml: &mut String,
    p: &ParagraphBlock,
    printable_w_pts: f64,
    left_margin_pts: f64,
    indent: usize,
) {
    // Safety Invariant: If a paragraph block contains multiple TOC lines with dot leader tabs,
    // each line MUST be rendered as an independent <w:p> so Word applies the tab stops to each line!
    let has_dot_leaders = p.lines.iter().any(|l| l.tab_stops.iter().any(|ts| ts.is_leader_dot));
    if has_dot_leaders && p.lines.len() > 1 {
        for line in &p.lines {
            let single_line_para = ParagraphBlock {
                lines: vec![line.clone()],
                bbox: line.bbox,
                tag: p.tag.clone(),
                alignment: p.alignment.clone(),
                space_before: 0.0,
                space_after: 0.0,
                line_spacing_twips: p.line_spacing_twips,
                line_rule: p.line_rule.clone(),
                is_header: p.is_header,
                is_list: p.is_list,
                list_id: p.list_id,
                list_level: p.list_level,
                column_index: p.column_index,
            };
            render_paragraph(xml, &single_line_para, printable_w_pts, left_margin_pts, indent);
        }
        return;
    }

    let ind = " ".repeat(indent);

    xml.push_str(&format!("{}<w:p>\n", ind));
    xml.push_str(&format!("{}  <w:pPr>\n", ind));

    // 1. Style mapping
    let style_id = match p.tag.as_str() {
        "H1" => "Heading1",
        "H2" => "Heading2",
        "H3" => "Heading3",
        "List" => "ListParagraph",
        _ => {
            if p.is_list {
                "ListParagraph"
            } else {
                "Normal"
            }
        }
    };
    xml.push_str(&format!("{}    <w:pStyle w:val=\"{}\"/>\n", ind, style_id));

    // 2. Numbering properties for bullet lists
    if p.is_list && p.list_id == 1 {
        let ilvl = p.list_level.saturating_sub(1);
        xml.push_str(&format!(
            "{}    <w:numPr><w:ilvl w:val=\"{}\"/><w:numId w:val=\"1\"/></w:numPr>\n",
            ind, ilvl
        ));
    }

    // 3. Custom Tab Stops `<w:tabs>`
    let mut all_tab_stops = Vec::new();
    for line in &p.lines {
        for ts in &line.tab_stops {
            all_tab_stops.push(ts);
        }
    }

    if !all_tab_stops.is_empty() {
        xml.push_str(&format!("{}    <w:tabs>\n", ind));
        for ts in all_tab_stops {
            let pos_twips = if ts.is_leader_dot || ts.align_type == "right" {
                points_to_twips(printable_w_pts)
            } else {
                points_to_twips((ts.x_pos_points - left_margin_pts).max(0.0))
            };
            let align_val = match ts.align_type.as_str() {
                "right" => "right",
                "center" => "center",
                _ => "left",
            };
            let leader_val = if ts.is_leader_dot { "dot" } else { "none" };
            xml.push_str(&format!(
                "{}      <w:tab w:val=\"{}\" w:leader=\"{}\" w:pos=\"{}\"/>\n",
                ind, align_val, leader_val, pos_twips
            ));
        }
        xml.push_str(&format!("{}    </w:tabs>\n", ind));
    }

    // 4. Spacing (paragraph spacing & line spacing)
    let before_twips = points_to_twips(p.space_before);
    let after_twips = points_to_twips(p.space_after);

    let line_spacing_attr = if let Some(twips) = p.line_spacing_twips {
        let rule = match p.line_rule.as_str() {
            "exact" if twips >= 200 => "exact",
            "atLeast" => "atLeast",
            _ => "auto",
        };
        format!(" w:line=\"{}\" w:lineRule=\"{}\"", twips, rule)
    } else {
        " w:line=\"240\" w:lineRule=\"auto\"".to_string()
    };

    xml.push_str(&format!(
        "{}    <w:spacing w:before=\"{}\" w:after=\"{}\"{}/>\n",
        ind, before_twips, after_twips, line_spacing_attr
    ));

    // 5. Indentation
    if p.is_list {
        let indent_twips = (p.list_level.max(1) as i64) * 360;
        xml.push_str(&format!(
            "{}    <w:ind w:left=\"{}\" w:hanging=\"360\"/>\n",
            ind, indent_twips
        ));
    } else {
        let page_cx = left_margin_pts + (printable_w_pts / 2.0);
        let para_cx = (p.bbox.x_min + p.bbox.x_max) / 2.0;
        let is_page_centered = (para_cx - page_cx).abs() <= 35.0;

        if (!is_page_centered || p.alignment != "center") && p.bbox.x_min > (left_margin_pts + 15.0) {
            let left_indent_pts = (p.bbox.x_min - left_margin_pts).max(0.0);
            if left_indent_pts < (printable_w_pts * 0.60) {
                let indent_twips = points_to_twips(left_indent_pts);
                if indent_twips >= 200 {
                    xml.push_str(&format!(
                        "{}    <w:ind w:left=\"{}\"/>\n",
                        ind, indent_twips
                    ));
                }
            }
        }
    }

    // 6. Justification / Alignment
    let jc_val = match p.alignment.as_str() {
        "center" => "center",
        "right" => "right",
        "both" => "both",
        _ => "left",
    };
    xml.push_str(&format!("{}    <w:jc w:val=\"{}\"/>\n", ind, jc_val));

    xml.push_str(&format!("{}  </w:pPr>\n", ind));

    // Render Text Runs
    for (line_idx, line) in p.lines.iter().enumerate() {
        if line_idx > 0 && (p.alignment == "center" || p.is_header) {
            xml.push_str(&format!("{}    <w:r><w:br/></w:r>\n", ind));
        }
        for (run_idx, run) in line.runs.iter().enumerate() {
            let mut r = run.clone();
            if line_idx > 0 && run_idx == 0 && !r.is_tab && !(p.alignment == "center" || p.is_header) {
                if let Some(prev_line) = p.lines.get(line_idx - 1) {
                    if let Some(prev_run) = prev_line.runs.last() {
                        if !prev_run.text.ends_with('-') && !prev_run.text.ends_with(' ') && !r.text.starts_with(' ') {
                            r.text.insert(0, ' ');
                        }
                    }
                }
            }
            render_text_run(xml, &r, indent + 2);
        }
    }

    xml.push_str(&format!("{}</w:p>\n", ind));
}

/// Renders a single `<w:r>` text run
fn render_text_run(xml: &mut String, run: &TextRun, indent: usize) {
    let ind = " ".repeat(indent);

    if run.is_tab {
        xml.push_str(&format!("{}<w:r><w:tab/></w:r>\n", ind));
        return;
    }

    if run.text.is_empty() {
        return;
    }

    xml.push_str(&format!("{}<w:r>\n", ind));
    xml.push_str(&format!("{}  <w:rPr>\n", ind));

    // Font Family
    let font = if run.style.font_family.is_empty() {
        "Arial"
    } else {
        &run.style.font_family
    };
    xml.push_str(&format!(
        "{}    <w:rFonts w:ascii=\"{}\" w:hAnsi=\"{}\" w:cs=\"{}\"/>\n",
        ind, escape_xml(font), escape_xml(font), escape_xml(font)
    ));

    // Bold / Italic
    if run.style.is_bold {
        xml.push_str(&format!("{}    <w:b/><w:bCs/>\n", ind));
    }
    if run.style.is_italic {
        xml.push_str(&format!("{}    <w:i/><w:iCs/>\n", ind));
    }

    // Font Size (Half-Points: 1 pt = 2 half-points)
    let half_pts = points_to_half_points(run.style.font_size.max(6.0));
    xml.push_str(&format!(
        "{}    <w:sz w:val=\"{}\"/><w:szCs w:val=\"{}\"/>\n",
        ind, half_pts, half_pts
    ));

    // Font Color
    let hex_color = if run.style.hex_color.len() == 6 {
        &run.style.hex_color
    } else {
        "000000"
    };
    xml.push_str(&format!("{}    <w:color w:val=\"{}\"/>\n", ind, hex_color));

    // Vertical Alignment (superscript / subscript)
    match run.style.vert_align.as_str() {
        "superscript" => xml.push_str(&format!("{}    <w:vertAlign w:val=\"superscript\"/>\n", ind)),
        "subscript" => xml.push_str(&format!("{}    <w:vertAlign w:val=\"subscript\"/>\n", ind)),
        _ => {}
    }

    xml.push_str(&format!("{}  </w:rPr>\n", ind));
    xml.push_str(&format!(
        "{}  <w:t xml:space=\"preserve\">{}</w:t>\n",
        ind, escape_xml(&run.text)
    ));
    xml.push_str(&format!("{}</w:r>\n", ind));
}

/// Renders a full `<w:tbl>` table matrix with fixed layout, exact twip column widths, and vMerge support
fn render_table(xml: &mut String, t: &TableMatrix, indent: usize) {
    let ind = " ".repeat(indent);

    xml.push_str(&format!("{}<w:tbl>\n", ind));

    // Calculate total table width in twips
    let total_w_pts: f64 = if !t.grid_cols.is_empty() {
        t.grid_cols.iter().sum()
    } else {
        t.cells.first().map(|row| row.iter().map(|c| c.bbox.width()).sum()).unwrap_or(0.0)
    };
    let total_w_twips = points_to_twips(total_w_pts).max(1440);

    // Table Properties: Fixed layout (anti-autofit), Width, Borders, Cell Margins
    xml.push_str(&format!(
        "{ind}  <w:tblPr>\n\
         {ind}    <w:tblLayout w:type=\"fixed\"/>\n\
         {ind}    <w:tblW w:w=\"{total_w_twips}\" w:type=\"dxa\"/>\n\
         {ind}    <w:tblBorders>\n\
         {ind}      <w:top w:val=\"none\"/>\n\
         {ind}      <w:left w:val=\"none\"/>\n\
         {ind}      <w:bottom w:val=\"none\"/>\n\
         {ind}      <w:right w:val=\"none\"/>\n\
         {ind}      <w:insideH w:val=\"none\"/>\n\
         {ind}      <w:insideV w:val=\"none\"/>\n\
         {ind}    </w:tblBorders>\n\
         {ind}    <w:tblCellMar>\n\
         {ind}      <w:top w:w=\"120\" w:type=\"dxa\"/>\n\
         {ind}      <w:left w:w=\"160\" w:type=\"dxa\"/>\n\
         {ind}      <w:bottom w:w=\"120\" w:type=\"dxa\"/>\n\
         {ind}      <w:right w:w=\"160\" w:type=\"dxa\"/>\n\
         {ind}    </w:tblCellMar>\n\
         {ind}  </w:tblPr>\n"
    ));

    // Table Grid Columns `<w:tblGrid>`
    xml.push_str(&format!("{}  <w:tblGrid>\n", ind));
    for &col_w in &t.grid_cols {
        let col_twips = points_to_twips(col_w).max(280);
        xml.push_str(&format!("{}    <w:gridCol w:w=\"{}\"/>\n", ind, col_twips));
    }
    xml.push_str(&format!("{}  </w:tblGrid>\n", ind));

    let num_rows = t.cells.len();
    let num_cols = t.grid_cols.len();

    // Rows `<w:tr>`
    for r in 0..num_rows {
        let row_height_twips = if r < t.grid_rows.len() {
            points_to_twips(t.grid_rows[r]).max(280)
        } else {
            360
        };

        xml.push_str(&format!("{}  <w:tr>\n", ind));
        xml.push_str(&format!(
            "{}    <w:trPr><w:trHeight w:val=\"{}\" w:hRule=\"atLeast\"/></w:trPr>\n",
            ind, row_height_twips
        ));

        // Column traversal with vertical merge tracking
        let mut c = 0;
        while c < num_cols {
            // 1. Check if a real cell starts at (r, c)
            let cell_opt = t.cells[r].iter().find(|cell| cell.start_col == c);

            if let Some(cell) = cell_opt {
                let is_v_merge_start = cell.row_span() > 1;
                let v_merge_val = if is_v_merge_start { Some("restart") } else { None };
                render_table_cell(xml, cell, &t.grid_cols, v_merge_val, indent + 4);
                c = cell.end_col;
                continue;
            }

            // 2. Check if an earlier row's cell spans vertically across row r at column c
            let mut v_span_cell: Option<&Cell> = None;
            for prev_r in 0..r {
                if let Some(prev_c) = t.cells[prev_r].iter().find(|cell| cell.start_col <= c && cell.end_col > c && cell.end_row > r) {
                    v_span_cell = Some(prev_c);
                    break;
                }
            }

            if let Some(parent_cell) = v_span_cell {
                if c == parent_cell.start_col {
                    // Emit vertical merge continuation cell `<w:vMerge/>`
                    let span_w_pts: f64 = if parent_cell.end_col <= t.grid_cols.len() {
                        t.grid_cols[parent_cell.start_col..parent_cell.end_col].iter().sum()
                    } else {
                        100.0
                    };
                    let span_w_twips = points_to_twips(span_w_pts).max(280);
                    let col_span = parent_cell.col_span();

                    let is_last_merge_row = (r + 1) == parent_cell.end_row;
                    let v_bot = if is_last_merge_row && parent_cell.border_bottom { "single" } else { "none" };
                    let v_left = if parent_cell.border_left { "single" } else { "none" };
                    let v_right = if parent_cell.border_right { "single" } else { "none" };

                    xml.push_str(&format!("{ind}    <w:tc>\n"));
                    xml.push_str(&format!("{ind}      <w:tcPr>\n"));
                    xml.push_str(&format!("{ind}        <w:tcW w:w=\"{span_w_twips}\" w:type=\"dxa\"/>\n"));
                    if col_span > 1 {
                        xml.push_str(&format!("{ind}        <w:gridSpan w:val=\"{col_span}\"/>\n"));
                    }
                    xml.push_str(&format!("{ind}        <w:vMerge/>\n"));
                    xml.push_str(&format!(
                        "{ind}        <w:tcBorders>\n\
                         {ind}          <w:top w:val=\"none\"/>\n\
                         {ind}          <w:left w:val=\"{v_left}\" w:sz=\"4\" w:space=\"0\" w:color=\"auto\"/>\n\
                         {ind}          <w:bottom w:val=\"{v_bot}\" w:sz=\"4\" w:space=\"0\" w:color=\"auto\"/>\n\
                         {ind}          <w:right w:val=\"{v_right}\" w:sz=\"4\" w:space=\"0\" w:color=\"auto\"/>\n\
                         {ind}        </w:tcBorders>\n"
                    ));
                    xml.push_str(&format!("{ind}      </w:tcPr>\n"));
                    xml.push_str(&format!("{ind}      <w:p/>\n"));
                    xml.push_str(&format!("{ind}    </w:tc>\n"));
                    c = parent_cell.end_col;
                    continue;
                } else {
                    c += 1;
                    continue;
                }
            }

            // 3. Fallback: Empty cell for column c
            let col_w_pts = if c < t.grid_cols.len() { t.grid_cols[c] } else { 100.0 };
            let col_w_twips = points_to_twips(col_w_pts).max(280);

            xml.push_str(&format!("{ind}    <w:tc>\n"));
            xml.push_str(&format!("{ind}      <w:tcPr>\n"));
            xml.push_str(&format!("{ind}        <w:tcW w:w=\"{col_w_twips}\" w:type=\"dxa\"/>\n"));
            xml.push_str(&format!("{ind}      </w:tcPr>\n"));
            xml.push_str(&format!("{ind}      <w:p/>\n"));
            xml.push_str(&format!("{ind}    </w:tc>\n"));
            c += 1;
        }

        xml.push_str(&format!("{}  </w:tr>\n", ind));
    }

    xml.push_str(&format!("{}</w:tbl>\n", ind));
}

/// Renders a table cell `<w:tc>`
fn render_table_cell(
    xml: &mut String,
    cell: &Cell,
    grid_cols: &[f64],
    v_merge_val: Option<&str>,
    indent: usize,
) {
    let ind = " ".repeat(indent);

    let cell_w_pts: f64 = if !grid_cols.is_empty() && cell.start_col < grid_cols.len() {
        let end = cell.end_col.min(grid_cols.len());
        grid_cols[cell.start_col..end].iter().sum()
    } else {
        cell.bbox.width()
    };

    let mut cell_w_twips = points_to_twips(cell_w_pts);
    if cell_w_twips == 0 {
        cell_w_twips = 1440; // 1 inch fallback
    }

    xml.push_str(&format!("{}<w:tc>\n", ind));
    xml.push_str(&format!("{}  <w:tcPr>\n", ind));
    xml.push_str(&format!(
        "{}    <w:tcW w:w=\"{}\" w:type=\"dxa\"/>\n",
        ind, cell_w_twips
    ));

    // ColSpan / GridSpan
    let col_span = cell.end_col.saturating_sub(cell.start_col);
    if col_span > 1 {
        xml.push_str(&format!(
            "{}    <w:gridSpan w:val=\"{}\"/>\n",
            ind, col_span
        ));
    }

    // Vertical merge
    if let Some(vm) = v_merge_val {
        if vm.is_empty() {
            xml.push_str(&format!("{}    <w:vMerge/>\n", ind));
        } else {
            xml.push_str(&format!("{}    <w:vMerge w:val=\"{}\"/>\n", ind, vm));
        }
    }

    // Cell Borders `<w:tcBorders>`
    let top_val = if cell.border_top { "single" } else { "none" };
    let bottom_val = if cell.border_bottom { "single" } else { "none" };
    let left_val = if cell.border_left { "single" } else { "none" };
    let right_val = if cell.border_right { "single" } else { "none" };

    xml.push_str(&format!(
        "{ind}    <w:tcBorders>\n\
         {ind}      <w:top w:val=\"{top_val}\" w:sz=\"4\" w:space=\"0\" w:color=\"auto\"/>\n\
         {ind}      <w:left w:val=\"{left_val}\" w:sz=\"4\" w:space=\"0\" w:color=\"auto\"/>\n\
         {ind}      <w:bottom w:val=\"{bottom_val}\" w:sz=\"4\" w:space=\"0\" w:color=\"auto\"/>\n\
         {ind}      <w:right w:val=\"{right_val}\" w:sz=\"4\" w:space=\"0\" w:color=\"auto\"/>\n\
         {ind}    </w:tcBorders>\n"
    ));

    // Header shading: If row 0 and cell text is bold or header-like:
    let is_header_row = cell.start_row == 0;
    let is_bold_header = is_header_row && cell.content_blocks.iter().any(|p| {
        p.lines.iter().any(|l| l.words.iter().any(|w| w.style.is_bold))
    });
    if is_bold_header {
        xml.push_str(&format!("{}    <w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"EBEBEB\"/>\n", ind));
    }

    // Top vertical alignment for crisp tabular layout
    xml.push_str(&format!("{}    <w:vAlign w:val=\"top\"/>\n", ind));

    xml.push_str(&format!("{}  </w:tcPr>\n", ind));

    // Cell content: Render with reset indents so cell text never squeezes
    if cell.content_blocks.is_empty() {
        xml.push_str(&format!("{}  <w:p/>\n", ind));
    } else {
        for p in &cell.content_blocks {
            render_paragraph_in_cell(xml, p, indent + 2);
        }
    }

    xml.push_str(&format!("{}</w:tc>\n", ind));
}

/// Renders a paragraph inside a table cell with reset zero-margins (Anti-Slivering)
fn render_paragraph_in_cell(xml: &mut String, p: &ParagraphBlock, indent: usize) {
    let ind = " ".repeat(indent);

    xml.push_str(&format!("{}<w:p>\n", ind));
    xml.push_str(&format!("{}  <w:pPr>\n", ind));
    xml.push_str(&format!("{}    <w:pStyle w:val=\"Normal\"/>\n", ind));

    // CRITICAL: Inside table cell, reset indents to 0 so list indents don't squeeze cells into vertical strips
    xml.push_str(&format!(
        "{}    <w:ind w:left=\"0\" w:right=\"0\" w:firstLine=\"0\"/>\n",
        ind
    ));

    // Clean compact spacing inside cell
    let space_after_twips = if p.is_list { 40 } else { 80 };
    xml.push_str(&format!(
        "{}    <w:spacing w:before=\"0\" w:after=\"{}\" w:line=\"240\" w:lineRule=\"auto\"/>\n",
        ind, space_after_twips
    ));

    // Alignment
    let jc_val = match p.alignment.as_str() {
        "center" => "center",
        "right" => "right",
        "both" => "both",
        _ => "left",
    };
    xml.push_str(&format!("{}    <w:jc w:val=\"{}\"/>\n", ind, jc_val));

    xml.push_str(&format!("{}  </w:pPr>\n", ind));

    for (line_idx, line) in p.lines.iter().enumerate() {
        if line_idx > 0 {
            // Anti-Collision: Guarantee whitespace separation between wrapped lines in cell
            xml.push_str(&format!("{}  <w:r><w:t xml:space=\"preserve\"> </w:t></w:r>\n", ind));
        }
        for run in &line.runs {
            render_text_run(xml, run, indent + 2);
        }
    }

    xml.push_str(&format!("{}</w:p>\n", ind));
}

/// Renders all background graphics for a page in a single zero-height paragraph
fn render_background_graphics_container(
    xml: &mut String,
    bg_graphics: &[(&GraphicObject, usize)],
    indent: usize,
) {
    if bg_graphics.is_empty() {
        return;
    }
    let ind = " ".repeat(indent);
    xml.push_str(&format!(
        "{ind}<w:p>\n\
         {ind}  <w:pPr>\n\
         {ind}    <w:spacing w:before=\"0\" w:after=\"0\" w:line=\"1\" w:lineRule=\"exact\"/>\n\
         {ind}    <w:rPr><w:sz w:val=\"2\"/><w:szCs w:val=\"2\"/></w:rPr>\n\
         {ind}  </w:pPr>\n"
    ));

    for &(g, img_idx) in bg_graphics {
        render_background_graphic_run(xml, g, img_idx, indent + 2);
    }

    xml.push_str(&format!("{ind}</w:p>\n"));
}

/// Renders a background or overlay graphic run as a floating DrawingML anchor
fn render_background_graphic_run(
    xml: &mut String,
    g: &GraphicObject,
    img_idx: usize,
    indent: usize,
) {
    let ind = " ".repeat(indent);
    let r_id = format!("rIdImg{}", img_idx);

    let x_emu = (g.bbox.x_min * 12700.0).round() as i64;
    let y_emu = (g.bbox.y_min * 12700.0).round() as i64;
    let cx_emu = (g.bbox.width() * 12700.0).round().max(12700.0) as i64;
    let cy_emu = (g.bbox.height() * 12700.0).round().max(12700.0) as i64;

    let behind_val = if g.behind_doc { "1" } else { "0" };

    xml.push_str(&format!(
        "{ind}<w:r>\n\
         {ind}  <w:drawing>\n\
         {ind}    <wp:anchor distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\" simplePos=\"0\" relativeHeight=\"0\" behindDoc=\"{behind_val}\" locked=\"1\" layoutInCell=\"0\" allowOverlap=\"1\">\n\
         {ind}      <wp:simplePos x=\"0\" y=\"0\"/>\n\
         {ind}      <wp:positionH relativeFrom=\"page\">\n\
         {ind}        <wp:posOffset>{x_emu}</wp:posOffset>\n\
         {ind}      </wp:positionH>\n\
         {ind}      <wp:positionV relativeFrom=\"page\">\n\
         {ind}        <wp:posOffset>{y_emu}</wp:posOffset>\n\
         {ind}      </wp:positionV>\n\
         {ind}      <wp:extent cx=\"{cx_emu}\" cy=\"{cy_emu}\"/>\n\
         {ind}      <wp:wrapNone/>\n\
         {ind}      <wp:docPr id=\"{img_idx}\" name=\"Background Picture {img_idx}\"/>\n\
         {ind}      <wp:cNvGraphicFramePr>\n\
         {ind}        <a:graphicFrameLocks xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" noChangeAspect=\"1\"/>\n\
         {ind}      </wp:cNvGraphicFramePr>\n\
         {ind}      <a:graphic xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\">\n\
         {ind}        <a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\">\n\
         {ind}          <pic:pic xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\">\n\
         {ind}            <pic:nvPicPr>\n\
         {ind}              <pic:cNvPr id=\"0\" name=\"Background Picture {img_idx}\"/>\n\
         {ind}              <pic:cNvPicPr/>\n\
         {ind}            </pic:nvPicPr>\n\
         {ind}            <pic:blipFill>\n\
         {ind}              <a:blip r:embed=\"{r_id}\"/>\n\
         {ind}              <a:stretch>\n\
         {ind}                <a:fillRect/>\n\
         {ind}              </a:stretch>\n\
         {ind}            </pic:blipFill>\n\
         {ind}            <pic:spPr>\n\
         {ind}              <a:xfrm>\n\
         {ind}                <a:off x=\"0\" y=\"0\"/>\n\
         {ind}                <a:ext cx=\"{cx_emu}\" cy=\"{cy_emu}\"/>\n\
         {ind}              </a:xfrm>\n\
         {ind}              <a:prstGeom prst=\"rect\"/>\n\
         {ind}            </pic:spPr>\n\
         {ind}          </pic:pic>\n\
         {ind}        </a:graphicData>\n\
         {ind}      </a:graphic>\n\
         {ind}    </wp:anchor>\n\
         {ind}  </w:drawing>\n\
         {ind}</w:r>\n"
    ));
}

/// Renders an inline drawing image with DrawingML specifications
fn render_graphic(
    xml: &mut String,
    g: &GraphicObject,
    img_idx: usize,
    page_w_pts: f64,
    margin_left_pts: f64,
    margin_right_pts: f64,
    indent: usize,
) {
    let ind = " ".repeat(indent);

    let mut w_pts = g.bbox.width();
    let mut h_pts = g.bbox.height();

    if w_pts <= 1.0 {
        w_pts = 100.0;
    }
    if h_pts <= 1.0 {
        h_pts = 100.0;
    }

    // Ensure image does not exceed printable page width
    let max_printable_w = (page_w_pts - margin_left_pts - margin_right_pts).max(200.0);
    if w_pts > max_printable_w {
        let scale = max_printable_w / w_pts;
        w_pts = max_printable_w;
        h_pts *= scale;
    }

    // 1 pt = 12700 EMUs
    let cx_emu = (w_pts * 12700.0).round() as i64;
    let cy_emu = (h_pts * 12700.0).round() as i64;
    let r_id = format!("rIdImg{}", img_idx);

    // DrawingML inline image paragraph
    xml.push_str(&format!(
        "{ind}<w:p>\n\
         {ind}  <w:pPr>\n\
         {ind}    <w:keepNext/>\n\
         {ind}    <w:spacing w:before=\"120\" w:after=\"120\"/>\n\
         {ind}    <w:jc w:val=\"center\"/>\n\
         {ind}  </w:pPr>\n\
         {ind}  <w:r>\n\
         {ind}    <w:drawing>\n\
         {ind}      <wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\">\n\
         {ind}        <wp:extent cx=\"{cx_emu}\" cy=\"{cy_emu}\"/>\n\
         {ind}        <wp:docPr id=\"{img_idx}\" name=\"Picture {img_idx}\"/>\n\
         {ind}        <wp:cNvGraphicFramePr>\n\
         {ind}          <a:graphicFrameLocks xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" noChangeAspect=\"1\"/>\n\
         {ind}        </wp:cNvGraphicFramePr>\n\
         {ind}        <a:graphic xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\">\n\
         {ind}          <a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\">\n\
         {ind}            <pic:pic xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\">\n\
         {ind}              <pic:nvPicPr>\n\
         {ind}                <pic:cNvPr id=\"0\" name=\"Picture {img_idx}\"/>\n\
         {ind}                <pic:cNvPicPr/>\n\
         {ind}              </pic:nvPicPr>\n\
         {ind}              <pic:blipFill>\n\
         {ind}                <a:blip r:embed=\"{r_id}\"/>\n\
         {ind}                <a:stretch>\n\
         {ind}                  <a:fillRect/>\n\
         {ind}                </a:stretch>\n\
         {ind}              </pic:blipFill>\n\
         {ind}              <pic:spPr>\n\
         {ind}                <a:xfrm>\n\
         {ind}                  <a:off x=\"0\" y=\"0\"/>\n\
         {ind}                  <a:ext cx=\"{cx_emu}\" cy=\"{cy_emu}\"/>\n\
         {ind}                </a:xfrm>\n\
         {ind}                <a:prstGeom prst=\"rect\"/>\n\
         {ind}              </pic:spPr>\n\
         {ind}            </pic:pic>\n\
         {ind}          </a:graphicData>\n\
         {ind}        </a:graphic>\n\
         {ind}      </wp:inline>\n\
         {ind}    </w:drawing>\n\
         {ind}  </w:r>\n\
         {ind}</w:p>\n"
    ));
}

/// Determines whether an explicit page break is needed between two consecutive pages.
/// Enforces intentional page break preservation:
/// 1. Short preliminary / section-ending pages (Cover, Pengesahan, Kata Pengantar, Daftar Isi, Chapter ends)
///    where content terminated early (< 82% of page height).
/// 2. Major section starts where the next page begins with an H1 heading near the top.
/// 3. Orientation changes (Portrait <-> Landscape).
/// Dense pages (>= 82% full) flow naturally in Word without creating single-line orphaned pages.
fn should_insert_page_break(
    prev_page: &DocumentPage,
    curr_page: &DocumentPage,
    _is_prev_first_page: bool,
) -> bool {
    // 1. Orientation change
    let prev_is_landscape = prev_page.width_points > prev_page.height_points;
    let curr_is_landscape = curr_page.width_points > curr_page.height_points;
    if prev_is_landscape != curr_is_landscape {
        return true;
    }

    // 2. Semantic Chapter / Major Section Boundary
    // If the next page begins with a major heading or prominent section title near the top of the page
    // (100% language-agnostic: derived purely from H1/H2 classification, bold uppercase, or centered bold prominence)
    if let Some(first_p) = curr_page.paragraphs.first() {
        if first_p.bbox.y_min <= 160.0 && first_p.lines.len() <= 2 {
            let first_text = first_p.lines.iter().flat_map(|l| l.words.iter().map(|w| w.text.as_str())).collect::<Vec<_>>().join(" ");
            let has_alpha = first_text.chars().any(|c| c.is_alphabetic());
            let is_all_caps = has_alpha && first_text.chars().filter(|c| c.is_alphabetic()).all(|c| c.is_uppercase());
            let has_bold = first_p.lines.iter().any(|l| l.words.iter().any(|w| w.style.is_bold));
            let is_centered = first_p.alignment == "center";

            let is_major_section_header = (first_p.is_header && (first_p.tag == "H1" || first_p.tag == "H2"))
                || ((is_all_caps || is_centered) && has_bold && first_text.len() <= 80);

            if is_major_section_header {
                return true;
            }
        }
    }

    // 3. Intentional termination: Prev page terminated early (< 82% of page height)
    // Examples: Thesis Cover, Lembar Pengesahan, Kata Pengantar, Daftar Isi, End of Chapter
    let mut prev_max_y = 0.0f64;
    for p in &prev_page.paragraphs {
        if !p.lines.is_empty() && p.lines.iter().any(|l| !l.words.is_empty()) {
            if p.bbox.y_max > prev_max_y {
                prev_max_y = p.bbox.y_max;
            }
        }
    }
    for t in &prev_page.tables {
        if t.bbox.y_max > prev_max_y {
            prev_max_y = t.bbox.y_max;
        }
    }
    for g in &prev_page.graphics {
        if !g.is_background && g.bbox.y_max > prev_max_y {
            prev_max_y = g.bbox.y_max;
        }
    }

    let is_short_termination = prev_max_y > 0.0 && prev_max_y < (prev_page.height_points * 0.82);
    if is_short_termination {
        return true;
    }

    // Dense pages (>= 82% full) with continuing body text flow naturally in Word's layout engine
    // without creating single-line orphaned pages.
    false
}
