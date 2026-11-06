pub mod models;
pub mod utils;
pub mod pdf;
pub mod dla;
pub mod docx;

use wasm_bindgen::prelude::*;

/// Primary High-Precision Engine Converter
pub struct ConverterEngine {
    pdf_parser: pdf::PdfParser,
    dla: dla::LayoutAnalyzer,
    docx_writer: docx::DocxWriter,
}

impl ConverterEngine {
    pub fn new() -> Self {
        Self {
            pdf_parser: pdf::PdfParser::new(),
            dla: dla::LayoutAnalyzer::new(),
            docx_writer: docx::DocxWriter::new(),
        }
    }

    /// Converts raw PDF binary bytes to an OOXML (.docx) ZIP container in memory
    pub fn convert(&self, pdf_bytes: &[u8]) -> Result<Vec<u8>, String> {
        let raw_pages = self.pdf_parser.parse_bytes(pdf_bytes)?;
        let structured_pages = self.dla.analyze(raw_pages)?;
        let docx_bytes = self.docx_writer.build_docx(&structured_pages)?;
        Ok(docx_bytes)
    }
}

/// WebAssembly entrypoint for JavaScript/Browser to convert PDF to DOCX
#[wasm_bindgen]
pub fn convert_pdf_to_docx(pdf_data: &[u8]) -> Result<Vec<u8>, JsValue> {
    let engine = ConverterEngine::new();
    engine
        .convert(pdf_data)
        .map_err(|e| JsValue::from_str(&e))
}

/// WebAssembly entrypoint to inspect parsed spatial elements & structured layout
#[wasm_bindgen]
pub fn inspect_pdf_elements(pdf_data: &[u8]) -> Result<JsValue, JsValue> {
    let parser = pdf::PdfParser::new();
    let pages = parser
        .parse_bytes(pdf_data)
        .map_err(|e| JsValue::from_str(&e))?;
    
    let dla = dla::LayoutAnalyzer::new();
    let structured_pages = dla
        .analyze(pages)
        .map_err(|e| JsValue::from_str(&e))?;

    serde_wasm_bindgen::to_value(&structured_pages)
        .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
}

/// WebAssembly entrypoint to inspect document structure tree and outlines (Step 6 verification)
#[wasm_bindgen]
pub fn inspect_pdf_structure(pdf_data: &[u8]) -> Result<JsValue, JsValue> {
    let parser = pdf::PdfParser::new();
    let struct_tree = parser
        .extract_structure_tree(pdf_data)
        .map_err(|e| JsValue::from_str(&e))?;

    serde_wasm_bindgen::to_value(&struct_tree)
        .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
}
