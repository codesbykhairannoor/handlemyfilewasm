pub mod document;
pub mod numbering;
pub mod relationships;
pub mod styles;

use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::docx::document::DocxDocumentBuilder;
use crate::docx::numbering::DocxNumbering;
use crate::docx::relationships::DocxRelationships;
use crate::docx::styles::DocxStyles;
use crate::models::DocumentPage;

pub struct DocxWriter;

impl DocxWriter {
    pub fn new() -> Self {
        Self
    }

    /// Assembles all Open Packaging Conventions (OPC) XML parts and packages into in-memory ZIP DOCX
    pub fn build_docx(&self, pages: &[DocumentPage]) -> Result<Vec<u8>, String> {
        let mut buffer = Cursor::new(Vec::new());
        let mut zip = ZipWriter::new(&mut buffer);

        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        // Collect all images across all pages
        struct ImageEntry<'a> {
            id: usize,
            ext: &'static str,
            data: &'a [u8],
        }

        let mut image_entries = Vec::new();
        let mut image_rels = Vec::new();
        let mut img_counter = 1;

        for page in pages {
            for g in &page.graphics {
                let ext = if g.mime_type == "image/png" {
                    "png"
                } else {
                    "jpeg"
                };
                image_entries.push(ImageEntry {
                    id: img_counter,
                    ext,
                    data: &g.data,
                });
                image_rels.push((img_counter, ext));
                img_counter += 1;
            }
        }

        let has_header = pages.iter().any(|p| p.header.is_some());
        let has_footer = pages.iter().any(|p| p.footer.is_some());
        let header_p = pages
            .iter()
            .filter_map(|p| p.header.as_ref())
            .max_by_key(|p| p.lines.iter().map(|l| l.runs.len()).sum::<usize>());
        let footer_p = pages
            .iter()
            .filter_map(|p| p.footer.as_ref())
            .max_by_key(|p| p.lines.iter().map(|l| l.runs.len()).sum::<usize>());

        // Dominant margins and page width for exact tab stop alignment:
        let dominant_w = pages.first().map(|p| p.width_points).unwrap_or(595.0);
        let left_pts = pages.iter().map(|p| p.margin_left_points).find(|&m| m > 10.0).unwrap_or(54.0);
        let right_pts = pages.iter().map(|p| p.margin_right_points).find(|&m| m > 10.0).unwrap_or(54.0);
        let printable_width_pts = (dominant_w - left_pts - right_pts).max(200.0);
        let printable_width_twips = crate::utils::points_to_twips(printable_width_pts) as u32;

        // 1. [Content_Types].xml
        zip.start_file("[Content_Types].xml", options)
            .map_err(|e| format!("Failed to create [Content_Types].xml: {}", e))?;
        zip.write_all(DocxRelationships::content_types(has_header, has_footer).as_bytes())
            .map_err(|e| format!("Failed to write [Content_Types].xml: {}", e))?;

        // 2. _rels/.rels
        zip.start_file("_rels/.rels", options)
            .map_err(|e| format!("Failed to create _rels/.rels: {}", e))?;
        zip.write_all(DocxRelationships::package_relationships().as_bytes())
            .map_err(|e| format!("Failed to write _rels/.rels: {}", e))?;

        // 3. word/_rels/document.xml.rels
        zip.start_file("word/_rels/document.xml.rels", options)
            .map_err(|e| format!("Failed to create word/_rels/document.xml.rels: {}", e))?;
        zip.write_all(DocxRelationships::document_relationships(&image_rels, has_header, has_footer).as_bytes())
            .map_err(|e| format!("Failed to write word/_rels/document.xml.rels: {}", e))?;

        // 4. word/settings.xml
        zip.start_file("word/settings.xml", options)
            .map_err(|e| format!("Failed to create word/settings.xml: {}", e))?;
        zip.write_all(DocxRelationships::settings().as_bytes())
            .map_err(|e| format!("Failed to write word/settings.xml: {}", e))?;

        // 5. word/styles.xml
        zip.start_file("word/styles.xml", options)
            .map_err(|e| format!("Failed to create word/styles.xml: {}", e))?;
        zip.write_all(DocxStyles::styles().as_bytes())
            .map_err(|e| format!("Failed to write word/styles.xml: {}", e))?;

        // 6. word/numbering.xml
        zip.start_file("word/numbering.xml", options)
            .map_err(|e| format!("Failed to create word/numbering.xml: {}", e))?;
        zip.write_all(DocxNumbering::numbering().as_bytes())
            .map_err(|e| format!("Failed to write word/numbering.xml: {}", e))?;

        // 7. word/header1.xml (if document has running headers)
        if has_header {
            zip.start_file("word/header1.xml", options)
                .map_err(|e| format!("Failed to create word/header1.xml: {}", e))?;
            zip.write_all(DocxRelationships::header(header_p, printable_width_twips).as_bytes())
                .map_err(|e| format!("Failed to write word/header1.xml: {}", e))?;
        }

        // 8. word/footer1.xml (if document has page numbers / footers)
        if has_footer {
            zip.start_file("word/footer1.xml", options)
                .map_err(|e| format!("Failed to create word/footer1.xml: {}", e))?;
            zip.write_all(DocxRelationships::footer(footer_p, printable_width_twips).as_bytes())
                .map_err(|e| format!("Failed to write word/footer1.xml: {}", e))?;
        }

        // 9. Write media files: word/media/image{id}.{ext}
        for entry in &image_entries {
            let path = format!("word/media/image{}.{}", entry.id, entry.ext);
            zip.start_file(&path, options)
                .map_err(|e| format!("Failed to create {}: {}", path, e))?;
            zip.write_all(entry.data)
                .map_err(|e| format!("Failed to write {}: {}", path, e))?;
        }

        // 8. word/document.xml
        let doc_xml = DocxDocumentBuilder::build_document_xml(pages);
        zip.start_file("word/document.xml", options)
            .map_err(|e| format!("Failed to create word/document.xml: {}", e))?;
        zip.write_all(doc_xml.as_bytes())
            .map_err(|e| format!("Failed to write word/document.xml: {}", e))?;

        // Finalize ZIP archive
        zip.finish()
            .map_err(|e| format!("Failed to finalize DOCX zip archive: {}", e))?;

        Ok(buffer.into_inner())
    }
}
