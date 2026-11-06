use serde::{Deserialize, Serialize};

/// 2D Point
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// 2D Bounding Box in absolute points (origin: top-left or bottom-left depending on stage)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x_min: f64,
    pub y_min: f64,
    pub x_max: f64,
    pub y_max: f64,
}

impl Rect {
    pub fn new(x_min: f64, y_min: f64, x_max: f64, y_max: f64) -> Self {
        Self {
            x_min: x_min.min(x_max),
            y_min: y_min.min(y_max),
            x_max: x_min.max(x_max),
            y_max: y_min.max(y_max),
        }
    }

    pub fn width(&self) -> f64 {
        (self.x_max - self.x_min).max(0.0)
    }

    pub fn height(&self) -> f64 {
        (self.y_max - self.y_min).max(0.0)
    }

    pub fn area(&self) -> f64 {
        self.width() * self.height()
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.x_min <= other.x_max
            && self.x_max >= other.x_min
            && self.y_min <= other.y_max
            && self.y_max >= other.y_min
    }

    pub fn contains_point(&self, x: f64, y: f64) -> bool {
        x >= self.x_min && x <= self.x_max && y >= self.y_min && y <= self.y_max
    }

    pub fn expand(&mut self, other: &Rect) {
        if self.width() == 0.0 && self.height() == 0.0 {
            *self = *other;
            return;
        }
        self.x_min = self.x_min.min(other.x_min);
        self.y_min = self.y_min.min(other.y_min);
        self.x_max = self.x_max.max(other.x_max);
        self.y_max = self.y_max.max(other.y_max);
    }
}

/// 2D Affine Transformation Matrix [a, b, c, d, e, f]
/// | a  b  0 |
/// | c  d  0 |
/// | e  f  1 |
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Matrix2D {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Matrix2D {
    pub fn identity() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
        }
    }

    pub fn new(a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) -> Self {
        Self { a, b, c, d, e, f }
    }

    /// Multiply self * other
    pub fn multiply(&self, other: &Matrix2D) -> Matrix2D {
        Matrix2D {
            a: self.a * other.a + self.b * other.c,
            b: self.a * other.b + self.b * other.d,
            c: self.c * other.a + self.d * other.c,
            d: self.c * other.b + self.d * other.d,
            e: self.e * other.a + self.f * other.c + other.e,
            f: self.e * other.b + self.f * other.d + other.f,
        }
    }

    /// Transform a 2D point (x, y)
    pub fn transform_point(&self, p: Point) -> Point {
        Point {
            x: self.a * p.x + self.c * p.y + self.e,
            y: self.b * p.x + self.d * p.y + self.f,
        }
    }

    /// Calculate effective font scaling magnitude
    pub fn font_scaling(&self) -> f64 {
        (self.a.powi(2) + self.b.powi(2)).sqrt().max(
            (self.c.powi(2) + self.d.powi(2)).sqrt()
        )
    }
}

/// Text font style and decoration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Style {
    pub font_family: String,
    pub font_size: f64, // In typographic points
    pub is_bold: bool,
    pub is_italic: bool,
    pub hex_color: String,
    pub vert_align: String, // "superscript" or "subscript" or empty
}

impl Default for Style {
    fn default() -> Self {
        Self {
            font_family: "Calibri".to_string(),
            font_size: 11.0,
            is_bold: false,
            is_italic: false,
            hex_color: "000000".to_string(),
            vert_align: String::new(),
        }
    }
}

/// Atomic glyph character with spatial bounding box
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Glyph {
    pub text: String,
    pub bbox: Rect,
    pub style: Style,
    #[serde(default)]
    pub mcid: Option<u32>,
    #[serde(default)]
    pub is_artifact: bool,
}

/// Clustered word
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Word {
    pub text: String,
    pub bbox: Rect,
    pub style: Style,
    pub glyphs: Vec<Glyph>,
}

/// Tab Stop definition
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TabStop {
    pub x_pos_points: f64,
    pub align_type: String, // "left", "right", "center", "decimal"
    pub is_leader_dot: bool,
}

/// WordprocessingML Text Run
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextRun {
    pub text: String,
    pub style: Style,
    pub is_tab: bool,
    pub is_leader_dot: bool,
}

/// Line of words grouped horizontally
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextLine {
    pub bbox: Rect,
    pub words: Vec<Word>,
    pub runs: Vec<TextRun>,
    pub tab_stops: Vec<TabStop>,
}

/// Structural Paragraph Block
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParagraphBlock {
    pub bbox: Rect,
    pub lines: Vec<TextLine>,
    pub space_before: f64, // In points
    pub space_after: f64,  // In points
    pub is_list: bool,
    pub list_id: i32,
    pub list_level: i32,
    #[serde(default)]
    pub tag: String,       // "H1", "H2", "H3", "P", "Title", "Caption", etc.
    #[serde(default)]
    pub is_header: bool,
    #[serde(default)]
    pub alignment: String, // "left", "center", "right", "both"
    #[serde(default)]
    pub line_spacing_twips: Option<i32>,
    #[serde(default)]
    pub line_rule: String, // "auto" or "exact"
    #[serde(default)]
    pub column_index: usize,
}

fn default_true() -> bool {
    true
}

/// Table Grid Cell
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    pub bbox: Rect,
    pub start_row: usize,
    pub end_row: usize,
    pub start_col: usize,
    pub end_col: usize,
    pub content_blocks: Vec<ParagraphBlock>,
    #[serde(default = "default_true")]
    pub border_top: bool,
    #[serde(default = "default_true")]
    pub border_bottom: bool,
    #[serde(default = "default_true")]
    pub border_left: bool,
    #[serde(default = "default_true")]
    pub border_right: bool,
}

impl Cell {
    pub fn col_span(&self) -> usize {
        (self.end_col - self.start_col).max(1)
    }

    pub fn row_span(&self) -> usize {
        (self.end_row - self.start_row).max(1)
    }
}

/// Structured Table Matrix
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableMatrix {
    pub bbox: Rect,
    pub grid_cols: Vec<f64>, // Horizontal rail boundaries
    pub grid_rows: Vec<f64>, // Vertical rail boundaries
    pub cells: Vec<Vec<Cell>>,
}

/// Vector path segment for table line extraction
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VectorPath {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

impl VectorPath {
    pub fn new(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        Self { x1, y1, x2, y2 }
    }

    pub fn is_horizontal(&self, tolerance: f64) -> bool {
        (self.y1 - self.y2).abs() <= tolerance
    }

    pub fn is_vertical(&self, tolerance: f64) -> bool {
        (self.x1 - self.x2).abs() <= tolerance
    }

    pub fn length(&self) -> f64 {
        ((self.x2 - self.x1).powi(2) + (self.y2 - self.y1).powi(2)).sqrt()
    }
}

/// Image Graphic Object
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphicObject {
    pub bbox: Rect,
    pub data: Vec<u8>,
    pub mime_type: String, // "image/jpeg", "image/png"
    pub is_inline: bool,
    #[serde(default)]
    pub is_background: bool,
}

fn default_margin_points() -> f64 {
    72.0
}

/// High-level Document Page
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentPage {
    pub page_number: usize,
    pub width_points: f64,
    pub height_points: f64,
    pub glyphs: Vec<Glyph>,
    pub vector_paths: Vec<VectorPath>,
    pub paragraphs: Vec<ParagraphBlock>,
    pub tables: Vec<TableMatrix>,
    pub graphics: Vec<GraphicObject>,
    #[serde(default)]
    pub is_tagged: bool,
    #[serde(default = "default_margin_points")]
    pub margin_left_points: f64,
    #[serde(default = "default_margin_points")]
    pub margin_right_points: f64,
    #[serde(default = "default_margin_points")]
    pub margin_top_points: f64,
    #[serde(default = "default_margin_points")]
    pub margin_bottom_points: f64,
    #[serde(default)]
    pub header: Option<ParagraphBlock>,
    #[serde(default)]
    pub footer: Option<ParagraphBlock>,
}

/// Standard Tagged PDF Semantic Tag (ISO 32000-1 §14.8.4)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StandardTag {
    Document,
    Part,
    Art,
    Sect,
    Div,
    BlockQuote,
    Caption,
    TOC,
    TOCI,
    H1,
    H2,
    H3,
    H4,
    H5,
    H6,
    H,
    P,
    L,      // List
    LI,     // List Item
    Lbl,    // List Label (bullet/number)
    LBody,  // List Body
    Table,
    TR,
    TH,
    TD,
    Span,
    Quote,
    Note,
    Reference,
    BibEntry,
    Code,
    Figure,
    Formula,
    Artifact,
    Other(String),
}

impl StandardTag {
    pub fn from_str_name(name: &str) -> Self {
        let clean = name.trim_start_matches('/');
        match clean {
            "Document" => StandardTag::Document,
            "Part" => StandardTag::Part,
            "Art" => StandardTag::Art,
            "Sect" => StandardTag::Sect,
            "Div" => StandardTag::Div,
            "BlockQuote" => StandardTag::BlockQuote,
            "Caption" => StandardTag::Caption,
            "TOC" => StandardTag::TOC,
            "TOCI" => StandardTag::TOCI,
            "H1" => StandardTag::H1,
            "H2" => StandardTag::H2,
            "H3" => StandardTag::H3,
            "H4" => StandardTag::H4,
            "H5" => StandardTag::H5,
            "H6" => StandardTag::H6,
            "H" => StandardTag::H,
            "P" => StandardTag::P,
            "L" => StandardTag::L,
            "LI" => StandardTag::LI,
            "Lbl" => StandardTag::Lbl,
            "LBody" => StandardTag::LBody,
            "Table" => StandardTag::Table,
            "TR" => StandardTag::TR,
            "TH" => StandardTag::TH,
            "TD" => StandardTag::TD,
            "Span" => StandardTag::Span,
            "Quote" => StandardTag::Quote,
            "Note" => StandardTag::Note,
            "Reference" => StandardTag::Reference,
            "BibEntry" => StandardTag::BibEntry,
            "Code" => StandardTag::Code,
            "Figure" => StandardTag::Figure,
            "Formula" => StandardTag::Formula,
            "Artifact" => StandardTag::Artifact,
            other => StandardTag::Other(other.to_string()),
        }
    }

    pub fn is_heading(&self) -> bool {
        matches!(
            self,
            StandardTag::H1
                | StandardTag::H2
                | StandardTag::H3
                | StandardTag::H4
                | StandardTag::H5
                | StandardTag::H6
                | StandardTag::H
        )
    }

    pub fn heading_level(&self) -> Option<usize> {
        match self {
            StandardTag::H1 => Some(1),
            StandardTag::H2 => Some(2),
            StandardTag::H3 => Some(3),
            StandardTag::H4 => Some(4),
            StandardTag::H5 => Some(5),
            StandardTag::H6 => Some(6),
            StandardTag::H => Some(1),
            _ => None,
        }
    }
}

/// Logical Structure Element (/StructElem)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructElement {
    pub tag: StandardTag,
    pub raw_tag: String,
    pub title: Option<String>,
    pub alt: Option<String>,
    pub actual_text: Option<String>,
    pub page_num: Option<usize>,
    pub mcids: Vec<u32>,
    pub col_span: usize,
    pub row_span: usize,
    pub children: Vec<StructElement>,
}

impl StructElement {
    pub fn new(tag: StandardTag, raw_tag: String) -> Self {
        Self {
            tag,
            raw_tag,
            title: None,
            alt: None,
            actual_text: None,
            page_num: None,
            mcids: Vec::new(),
            col_span: 1,
            row_span: 1,
            children: Vec::new(),
        }
    }
}

/// Document Outline / Bookmark item
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutlineItem {
    pub title: String,
    pub page_number: Option<usize>,
    pub dest_y: Option<f64>,
    pub children: Vec<OutlineItem>,
}

/// Complete Document Logical Structure Tree
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct StructTree {
    pub root_elements: Vec<StructElement>,
    pub role_map: std::collections::BTreeMap<String, String>,
    pub outlines: Vec<OutlineItem>,
}

