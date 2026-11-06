use std::collections::BTreeMap;
use lopdf::{Document, Object, ObjectId};
use lopdf::content::Content;
use crate::models::{Glyph, GraphicObject, Matrix2D, Point, Rect, Style, VectorPath};
use crate::pdf::state::{GraphicsState, TextState};
use crate::pdf::font::{FontDecoder, FontInfo};

pub struct EvaluatedContent {
    pub glyphs: Vec<Glyph>,
    pub vector_paths: Vec<VectorPath>,
    pub graphics: Vec<GraphicObject>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct MarkedContent {
    tag: String,
    mcid: Option<u32>,
    is_artifact: bool,
}

#[derive(Debug, Clone)]
enum PendingSubpath {
    Line { x1: f64, y1: f64, x2: f64, y2: f64 },
    Rect { p0: Point, p1: Point, p2: Point, p3: Point, w_pts: f64, h_pts: f64 },
}

pub struct StreamEvaluator<'a> {
    doc: &'a Document,
    fonts: &'a BTreeMap<String, ObjectId>,
    font_cache: BTreeMap<String, FontInfo>,
    xobjects: &'a BTreeMap<String, ObjectId>,
    properties: &'a BTreeMap<String, Object>,
    page_height: f64,
}

impl<'a> StreamEvaluator<'a> {
    pub fn new(
        doc: &'a Document,
        fonts: &'a BTreeMap<String, ObjectId>,
        xobjects: &'a BTreeMap<String, ObjectId>,
        properties: &'a BTreeMap<String, Object>,
        page_height: f64,
    ) -> Self {
        let mut font_cache = BTreeMap::new();
        for (name, &obj_id) in fonts {
            if let Ok(info) = FontDecoder::parse_font(doc, obj_id) {
                let clean = name.trim_start_matches('/').to_string();
                font_cache.insert(name.clone(), info.clone());
                font_cache.insert(clean, info);
            }
        }

        Self {
            doc,
            fonts,
            font_cache,
            xobjects,
            properties,
            page_height,
        }
    }

    /// Evaluates decompressed content stream bytes into concrete visual elements
    pub fn evaluate(&self, content_bytes: &[u8]) -> Result<EvaluatedContent, String> {
        let content = Content::decode(content_bytes)
            .map_err(|e| format!("Failed to decode content stream: {}", e))?;

        let mut graphics_stack: Vec<GraphicsState> = Vec::new();
        let mut curr_graphics = GraphicsState::default();
        let mut text_state = TextState::default();
        let mut current_point = Point::new(0.0, 0.0);
        let mut subpath_start = Point::new(0.0, 0.0);
        let mut pending_subpaths: Vec<PendingSubpath> = Vec::new();
        let mut marked_stack: Vec<MarkedContent> = Vec::new();

        let mut glyphs = Vec::new();
        let mut vector_paths = Vec::new();
        let mut graphics = Vec::new();

        for op in content.operations.iter() {
            match op.operator.as_str() {
                // Marked Content Operators (ISO 32000-1 §14.6)
                "BMC" => {
                    if let Some(tag_obj) = op.operands.first() {
                        if let Ok(name) = tag_obj.as_name_str() {
                            let is_artifact = name == "Artifact";
                            marked_stack.push(MarkedContent {
                                tag: name.to_string(),
                                mcid: None,
                                is_artifact,
                            });
                        }
                    }
                }
                "BDC" => {
                    if op.operands.len() >= 2 {
                        let tag = op.operands[0].as_name_str().unwrap_or("Span").to_string();
                        let is_artifact = tag == "Artifact";
                        let mut mcid = None;

                        match &op.operands[1] {
                            Object::Dictionary(d) => {
                                if let Ok(m) = d.get(b"MCID").and_then(|o| o.as_i64()) {
                                    mcid = Some(m as u32);
                                }
                            }
                            Object::Name(prop_name) => {
                                let name_str = String::from_utf8_lossy(prop_name);
                                if let Some(prop_obj) = self.properties.get(name_str.as_ref()) {
                                    let prop_dict = match prop_obj {
                                        Object::Reference(r) => self.doc.get_object(*r).ok().and_then(|o| o.as_dict().ok()),
                                        Object::Dictionary(d) => Some(d),
                                        _ => None,
                                    };
                                    if let Some(pd) = prop_dict {
                                        if let Ok(m) = pd.get(b"MCID").and_then(|o| o.as_i64()) {
                                            mcid = Some(m as u32);
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }

                        marked_stack.push(MarkedContent {
                            tag,
                            mcid,
                            is_artifact,
                        });
                    }
                }
                "EMC" => {
                    marked_stack.pop();
                }
                // Graphics State Stack: q / Q
                "q" => {
                    graphics_stack.push(curr_graphics.clone());
                }
                "Q" => {
                    if let Some(g) = graphics_stack.pop() {
                        curr_graphics = g;
                    }
                    pending_subpaths.clear();
                }
                // Current Transformation Matrix: a b c d e f cm
                "cm" => {
                    if op.operands.len() >= 6 {
                        if let (Some(a), Some(b), Some(c), Some(d), Some(e), Some(f)) = (
                            get_f64(&op.operands[0]),
                            get_f64(&op.operands[1]),
                            get_f64(&op.operands[2]),
                            get_f64(&op.operands[3]),
                            get_f64(&op.operands[4]),
                            get_f64(&op.operands[5]),
                        ) {
                            let new_m = Matrix2D::new(a, b, c, d, e, f);
                            curr_graphics.ctm = new_m.multiply(&curr_graphics.ctm);
                        }
                    }
                }
                // Line Width: w
                "w" => {
                    if let Some(w) = op.operands.first().and_then(get_f64) {
                        curr_graphics.line_width = w;
                    }
                }
                // Color Operators
                // Grayscale: gray g / gray G
                "g" => {
                    if let Some(gray) = op.operands.first().and_then(get_f64) {
                        let val = (gray.clamp(0.0, 1.0) * 255.0).round() as u8;
                        curr_graphics.fill_color = format!("{:02X}{:02X}{:02X}", val, val, val);
                    }
                }
                "G" => {
                    if let Some(gray) = op.operands.first().and_then(get_f64) {
                        let val = (gray.clamp(0.0, 1.0) * 255.0).round() as u8;
                        curr_graphics.stroke_color = format!("{:02X}{:02X}{:02X}", val, val, val);
                    }
                }
                // RGB: r g b rg / r g b RG
                "rg" => {
                    if op.operands.len() >= 3 {
                        if let (Some(r), Some(g), Some(b)) = (
                            get_f64(&op.operands[0]),
                            get_f64(&op.operands[1]),
                            get_f64(&op.operands[2]),
                        ) {
                            let r_val = (r.clamp(0.0, 1.0) * 255.0).round() as u8;
                            let g_val = (g.clamp(0.0, 1.0) * 255.0).round() as u8;
                            let b_val = (b.clamp(0.0, 1.0) * 255.0).round() as u8;
                            curr_graphics.fill_color = format!("{:02X}{:02X}{:02X}", r_val, g_val, b_val);
                        }
                    }
                }
                "RG" => {
                    if op.operands.len() >= 3 {
                        if let (Some(r), Some(g), Some(b)) = (
                            get_f64(&op.operands[0]),
                            get_f64(&op.operands[1]),
                            get_f64(&op.operands[2]),
                        ) {
                            let r_val = (r.clamp(0.0, 1.0) * 255.0).round() as u8;
                            let g_val = (g.clamp(0.0, 1.0) * 255.0).round() as u8;
                            let b_val = (b.clamp(0.0, 1.0) * 255.0).round() as u8;
                            curr_graphics.stroke_color = format!("{:02X}{:02X}{:02X}", r_val, g_val, b_val);
                        }
                    }
                }
                // CMYK: c m y k k / c m y k K
                "k" => {
                    if op.operands.len() >= 4 {
                        if let (Some(c), Some(m), Some(y), Some(k)) = (
                            get_f64(&op.operands[0]),
                            get_f64(&op.operands[1]),
                            get_f64(&op.operands[2]),
                            get_f64(&op.operands[3]),
                        ) {
                            let r = (255.0 * (1.0 - c) * (1.0 - k)).clamp(0.0, 255.0).round() as u8;
                            let g = (255.0 * (1.0 - m) * (1.0 - k)).clamp(0.0, 255.0).round() as u8;
                            let b = (255.0 * (1.0 - y) * (1.0 - k)).clamp(0.0, 255.0).round() as u8;
                            curr_graphics.fill_color = format!("{:02X}{:02X}{:02X}", r, g, b);
                        }
                    }
                }
                "K" => {
                    if op.operands.len() >= 4 {
                        if let (Some(c), Some(m), Some(y), Some(k)) = (
                            get_f64(&op.operands[0]),
                            get_f64(&op.operands[1]),
                            get_f64(&op.operands[2]),
                            get_f64(&op.operands[3]),
                        ) {
                            let r = (255.0 * (1.0 - c) * (1.0 - k)).clamp(0.0, 255.0).round() as u8;
                            let g = (255.0 * (1.0 - m) * (1.0 - k)).clamp(0.0, 255.0).round() as u8;
                            let b = (255.0 * (1.0 - y) * (1.0 - k)).clamp(0.0, 255.0).round() as u8;
                            curr_graphics.stroke_color = format!("{:02X}{:02X}{:02X}", r, g, b);
                        }
                    }
                }
                // Begin / End Text Object: BT / ET
                "BT" => {
                    text_state.begin_text();
                }
                "ET" => {
                    // End text block
                }
                // Set Font and Size: /F1 12 Tf
                "Tf" => {
                    if op.operands.len() >= 2 {
                        if let Ok(name) = op.operands[0].as_name_str() {
                            text_state.font_name = name.to_string();
                        }
                        if let Some(size) = get_f64(&op.operands[1]) {
                            text_state.font_size = size;
                        }
                    }
                }
                // Set Text Matrix: a b c d e f Tm
                "Tm" => {
                    if op.operands.len() >= 6 {
                        if let (Some(a), Some(b), Some(c), Some(d), Some(e), Some(f)) = (
                            get_f64(&op.operands[0]),
                            get_f64(&op.operands[1]),
                            get_f64(&op.operands[2]),
                            get_f64(&op.operands[3]),
                            get_f64(&op.operands[4]),
                            get_f64(&op.operands[5]),
                        ) {
                            text_state.set_matrix(Matrix2D::new(a, b, c, d, e, f));
                        }
                    }
                }
                // Move text position: tx ty Td
                "Td" => {
                    if op.operands.len() >= 2 {
                        if let (Some(tx), Some(ty)) = (get_f64(&op.operands[0]), get_f64(&op.operands[1])) {
                            text_state.move_line(tx, ty);
                        }
                    }
                }
                // Move text position & set leading: tx ty TD
                "TD" => {
                    if op.operands.len() >= 2 {
                        if let (Some(tx), Some(ty)) = (get_f64(&op.operands[0]), get_f64(&op.operands[1])) {
                            text_state.move_line_set_leading(tx, ty);
                        }
                    }
                }
                // Move to start of next line: T*
                "T*" => {
                    text_state.next_line();
                }
                // Text parameters
                "Tc" => {
                    if let Some(c) = op.operands.first().and_then(get_f64) {
                        text_state.char_spacing = c;
                    }
                }
                "Tw" => {
                    if let Some(w) = op.operands.first().and_then(get_f64) {
                        text_state.word_spacing = w;
                    }
                }
                "Tz" => {
                    if let Some(z) = op.operands.first().and_then(get_f64) {
                        text_state.horizontal_scaling = z;
                    }
                }
                "TL" => {
                    if let Some(l) = op.operands.first().and_then(get_f64) {
                        text_state.leading = l;
                    }
                }
                "Ts" => {
                    if let Some(s) = op.operands.first().and_then(get_f64) {
                        text_state.text_rise = s;
                    }
                }
                "Tr" => {
                    if let Some(r) = op.operands.first().and_then(get_f64) {
                        text_state.text_render_mode = r as i32;
                    }
                }
                // Show text string: (text) Tj
                "Tj" => {
                    if let Some(s_obj) = op.operands.first() {
                        let text_bytes = get_bytes(s_obj);
                        self.process_text_string(&text_bytes, &mut text_state, &curr_graphics, &marked_stack, &mut glyphs);
                    }
                }
                // Show text array with kerning: [(str) offset (str)] TJ
                "TJ" => {
                    if let Some(arr_obj) = op.operands.first() {
                        if let Ok(arr) = arr_obj.as_array() {
                            for item in arr {
                                match item {
                                    Object::String(bytes, _) => {
                                        self.process_text_string(bytes, &mut text_state, &curr_graphics, &marked_stack, &mut glyphs);
                                    }
                                    Object::Integer(offset) => {
                                        let adj = -(*offset as f64) / 1000.0 * text_state.font_size * (text_state.horizontal_scaling / 100.0);
                                        text_state.advance(adj);
                                    }
                                    Object::Real(offset) => {
                                        let adj = -(*offset as f64) / 1000.0 * text_state.font_size * (text_state.horizontal_scaling / 100.0);
                                        text_state.advance(adj);
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                // Vector path construction: x y m (move to)
                "m" => {
                    if op.operands.len() >= 2 {
                        if let (Some(x), Some(y)) = (get_f64(&op.operands[0]), get_f64(&op.operands[1])) {
                            let p = curr_graphics.ctm.transform_point(Point::new(x, y));
                            current_point = p;
                            subpath_start = p;
                        }
                    }
                }
                // Vector path construction: x y l (line to)
                "l" => {
                    if op.operands.len() >= 2 {
                        if let (Some(x), Some(y)) = (get_f64(&op.operands[0]), get_f64(&op.operands[1])) {
                            let end_point = curr_graphics.ctm.transform_point(Point::new(x, y));
                            pending_subpaths.push(PendingSubpath::Line {
                                x1: current_point.x,
                                y1: current_point.y,
                                x2: end_point.x,
                                y2: end_point.y,
                            });
                            current_point = end_point;
                        }
                    }
                }
                // Close subpath: h
                "h" => {
                    pending_subpaths.push(PendingSubpath::Line {
                        x1: current_point.x,
                        y1: current_point.y,
                        x2: subpath_start.x,
                        y2: subpath_start.y,
                    });
                    current_point = subpath_start;
                }
                // Vector rectangle: x y w h re
                "re" => {
                    if op.operands.len() >= 4 {
                        if let (Some(x), Some(y), Some(w), Some(h)) = (
                            get_f64(&op.operands[0]),
                            get_f64(&op.operands[1]),
                            get_f64(&op.operands[2]),
                            get_f64(&op.operands[3]),
                        ) {
                            let p0 = curr_graphics.ctm.transform_point(Point::new(x, y));
                            let p1 = curr_graphics.ctm.transform_point(Point::new(x + w, y));
                            let p2 = curr_graphics.ctm.transform_point(Point::new(x + w, y + h));
                            let p3 = curr_graphics.ctm.transform_point(Point::new(x, y + h));

                            let w_pts = (p1.x - p0.x).abs().max((p2.x - p3.x).abs());
                            let h_pts = (p3.y - p0.y).abs().max((p2.y - p1.y).abs());

                            pending_subpaths.push(PendingSubpath::Rect {
                                p0,
                                p1,
                                p2,
                                p3,
                                w_pts,
                                h_pts,
                            });
                        }
                    }
                }
                // Stroke path: S or s
                "S" | "s" => {
                    for sp in pending_subpaths.drain(..) {
                        match sp {
                            PendingSubpath::Line { x1, y1, x2, y2 } => {
                                vector_paths.push(VectorPath::new(x1, self.flip_y(y1), x2, self.flip_y(y2)));
                            }
                            PendingSubpath::Rect { p0, p1, p2, p3, .. } => {
                                let y0 = self.flip_y(p0.y);
                                let y1 = self.flip_y(p1.y);
                                let y2 = self.flip_y(p2.y);
                                let y3 = self.flip_y(p3.y);
                                vector_paths.push(VectorPath::new(p0.x, y0, p1.x, y1));
                                vector_paths.push(VectorPath::new(p1.x, y1, p2.x, y2));
                                vector_paths.push(VectorPath::new(p2.x, y2, p3.x, y3));
                                vector_paths.push(VectorPath::new(p3.x, y3, p0.x, y0));
                            }
                        }
                    }
                }
                // Fill path: f, F, f*
                "f" | "F" | "f*" => {
                    for sp in pending_subpaths.drain(..) {
                        match sp {
                            PendingSubpath::Line { x1, y1, x2, y2 } => {
                                vector_paths.push(VectorPath::new(x1, self.flip_y(y1), x2, self.flip_y(y2)));
                            }
                            PendingSubpath::Rect { p0, p1, p2, p3, w_pts, h_pts } => {
                                // In Word/PDF, table borders are drawn as thin filled rectangles (thickness <= 3.5 pt)
                                if w_pts <= 3.5 || h_pts <= 3.5 {
                                    let y0 = self.flip_y(p0.y);
                                    let y1 = self.flip_y(p1.y);
                                    let y2 = self.flip_y(p2.y);
                                    let y3 = self.flip_y(p3.y);
                                    vector_paths.push(VectorPath::new(p0.x, y0, p1.x, y1));
                                    vector_paths.push(VectorPath::new(p1.x, y1, p2.x, y2));
                                    vector_paths.push(VectorPath::new(p2.x, y2, p3.x, y3));
                                    vector_paths.push(VectorPath::new(p3.x, y3, p0.x, y0));
                                }
                            }
                        }
                    }
                }
                // Stroke & Fill: B, B*, b, b*
                "B" | "B*" | "b" | "b*" => {
                    for sp in pending_subpaths.drain(..) {
                        match sp {
                            PendingSubpath::Line { x1, y1, x2, y2 } => {
                                vector_paths.push(VectorPath::new(x1, self.flip_y(y1), x2, self.flip_y(y2)));
                            }
                            PendingSubpath::Rect { p0, p1, p2, p3, .. } => {
                                let y0 = self.flip_y(p0.y);
                                let y1 = self.flip_y(p1.y);
                                let y2 = self.flip_y(p2.y);
                                let y3 = self.flip_y(p3.y);
                                vector_paths.push(VectorPath::new(p0.x, y0, p1.x, y1));
                                vector_paths.push(VectorPath::new(p1.x, y1, p2.x, y2));
                                vector_paths.push(VectorPath::new(p2.x, y2, p3.x, y3));
                                vector_paths.push(VectorPath::new(p3.x, y3, p0.x, y0));
                            }
                        }
                    }
                }
                // No-op path terminator (discards clipping rects like `W* n` and abandoned subpaths)
                "n" => {
                    pending_subpaths.clear();
                }
                // Invoke XObject: /Name Do
                "Do" => {
                    if let Some(name_obj) = op.operands.first() {
                        if let Ok(name) = name_obj.as_name_str() {
                            if let Some(&xobj_id) = self.xobjects.get(name) {
                                self.handle_xobject(xobj_id, &curr_graphics, &mut glyphs, &mut vector_paths, &mut graphics)?;
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(EvaluatedContent {
            glyphs,
            vector_paths,
            graphics,
        })
    }

    /// Converts PDF Cartesian Y (bottom-left) to Top-Left origin
    #[inline]
    fn flip_y(&self, y: f64) -> f64 {
        (self.page_height - y).max(0.0)
    }

    /// Processes text string into Glyphs and advances the Text Matrix
    fn process_text_string(
        &self,
        bytes: &[u8],
        text_state: &mut TextState,
        graphics: &GraphicsState,
        marked_stack: &[MarkedContent],
        out_glyphs: &mut Vec<Glyph>,
    ) {
        if bytes.is_empty() {
            return;
        }

        // Ignore Invisible text (Text Rendering Mode 3) unless desired
        if text_state.text_render_mode == 3 {
            return;
        }

        let clean_font_name = text_state.font_name.trim_start_matches('/');
        let fallback_font = FontInfo::default();
        let font = self.font_cache.get(clean_font_name).unwrap_or(&fallback_font);

        let decoded_glyphs = font.decode_bytes(
            bytes,
            text_state.font_size,
            text_state.char_spacing,
            text_state.word_spacing,
            text_state.horizontal_scaling,
        );

        let font_family = font.base_font.clone();
        let is_bold = font.is_bold || text_state.font_name.to_lowercase().contains("bold");
        let is_italic = font.is_italic || text_state.font_name.to_lowercase().contains("italic");
        let font_size = text_state.font_size;
        let hex_color = graphics.fill_color.clone();

        let active_mcid = marked_stack.iter().rev().find_map(|m| m.mcid);
        let active_is_artifact = marked_stack.iter().any(|m| m.is_artifact);

        for g in decoded_glyphs {
            let trm = text_state.get_rendering_matrix(&graphics.ctm);
            let x_start = trm.e;
            let y_baseline = trm.f;

            // Character bounding box in Top-Down coordinates:
            // In PDF Cartesian: character ascends above baseline (y_baseline + 0.85*size)
            // and descends below baseline (y_baseline - 0.15*size)
            let y_top_pdf = y_baseline + font_size * 0.85;
            let y_bot_pdf = y_baseline - font_size * 0.15;

            let y_min = self.flip_y(y_top_pdf);
            let y_max = self.flip_y(y_bot_pdf);

            let bbox = Rect::new(x_start, y_min, x_start + g.width_pt, y_max);

            out_glyphs.push(Glyph {
                text: g.text,
                bbox,
                style: Style {
                    font_family: font_family.clone(),
                    font_size,
                    is_bold,
                    is_italic,
                    hex_color: hex_color.clone(),
                    vert_align: String::new(),
                },
                mcid: active_mcid,
                is_artifact: active_is_artifact,
            });

            // Advance the text matrix horizontally for the next glyph
            text_state.advance(g.advance_pt);
        }
    }

    /// Handles XObjects (Images and Form XObjects recursively)
    fn handle_xobject(
        &self,
        xobj_id: ObjectId,
        graphics: &GraphicsState,
        out_glyphs: &mut Vec<Glyph>,
        out_paths: &mut Vec<VectorPath>,
        out_graphics: &mut Vec<GraphicObject>,
    ) -> Result<(), String> {
        let xobj = self.doc.get_object(xobj_id).map_err(|e| e.to_string())?;
        let stream = match xobj {
            Object::Stream(s) => s,
            _ => return Ok(()),
        };

        let subtype = stream.dict.get(b"Subtype").and_then(|s| s.as_name_str()).unwrap_or("");

        if subtype == "Image" {
            // In PDF, an image spans [0,0] to [1,1] in current user space
            // The active CTM defines its placement and dimensions
            let p0 = graphics.ctm.transform_point(Point::new(0.0, 0.0));
            let p1 = graphics.ctm.transform_point(Point::new(1.0, 0.0));
            let p2 = graphics.ctm.transform_point(Point::new(0.0, 1.0));
            let p3 = graphics.ctm.transform_point(Point::new(1.0, 1.0));

            let min_pdf_x = p0.x.min(p1.x).min(p2.x).min(p3.x);
            let max_pdf_x = p0.x.max(p1.x).max(p2.x).max(p3.x);
            let min_pdf_y = p0.y.min(p1.y).min(p2.y).min(p3.y);
            let max_pdf_y = p0.y.max(p1.y).max(p2.y).max(p3.y);

            let x_min = min_pdf_x;
            let x_max = max_pdf_x;
            let y_min = self.flip_y(max_pdf_y);
            let y_max = self.flip_y(min_pdf_y);

            let bbox = Rect::new(x_min, y_min, x_max, y_max);
            // Ignore degenerate off-page graphics (width or height < 2.0 pt)
            if bbox.width() >= 2.0 && bbox.height() >= 2.0 {
                if let Some(graphic_obj) = crate::pdf::image::extract_image_object(self.doc, stream, bbox) {
                    out_graphics.push(graphic_obj);
                }
            }
        } else if subtype == "Form" {
            // Form XObject: recursive execution with nested CTM
            let mut form_graphics = graphics.clone();
            if let Ok(m_obj) = stream.dict.get(b"Matrix") {
                if let Ok(m_arr) = m_obj.as_array() {
                    if m_arr.len() >= 6 {
                        if let (Some(a), Some(b), Some(c), Some(d), Some(e), Some(f)) = (
                            get_f64(&m_arr[0]),
                            get_f64(&m_arr[1]),
                            get_f64(&m_arr[2]),
                            get_f64(&m_arr[3]),
                            get_f64(&m_arr[4]),
                            get_f64(&m_arr[5]),
                        ) {
                            let form_matrix = Matrix2D::new(a, b, c, d, e, f);
                            form_graphics.ctm = form_matrix.multiply(&graphics.ctm);
                        }
                    }
                }
            }

            if let Ok(form_content_bytes) = stream.decompressed_content() {
                let nested_evaluator = StreamEvaluator::new(
                    self.doc,
                    self.fonts,
                    self.xobjects,
                    self.properties,
                    self.page_height,
                );
                let nested_result = nested_evaluator.evaluate(&form_content_bytes)?;
                out_glyphs.extend(nested_result.glyphs);
                out_paths.extend(nested_result.vector_paths);
                out_graphics.extend(nested_result.graphics);
            }
        }

        Ok(())
    }
}

fn get_f64(obj: &Object) -> Option<f64> {
    match obj {
        Object::Real(v) => Some(*v as f64),
        Object::Integer(v) => Some(*v as f64),
        _ => None,
    }
}

fn get_bytes(obj: &Object) -> Vec<u8> {
    match obj {
        Object::String(bytes, _) => bytes.clone(),
        _ => Vec::new(),
    }
}
