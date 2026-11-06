use std::collections::BTreeMap;
use lopdf::{Document, Object, ObjectId};
use crate::models::DocumentPage;
use crate::pdf::predictor::decode_predictor;
use crate::pdf::struct_tree::StructTreeExtractor;

fn get_f64(obj: &Object) -> Option<f64> {
    match obj {
        Object::Real(r) => Some(*r as f64),
        Object::Integer(i) => Some(*i as f64),
        _ => None,
    }
}

pub struct RawPageContent {
    pub page_number: usize,
    pub width: f64,
    pub height: f64,
    pub content_bytes: Vec<u8>,
    pub fonts: BTreeMap<String, ObjectId>,
    pub xobjects: BTreeMap<String, ObjectId>,
    pub properties: BTreeMap<String, Object>,
}

pub struct PdfParser;

impl PdfParser {
    pub fn new() -> Self {
        Self
    }

    /// Loads PDF bytes and extracts raw page metadata, dimensions, and decompressed content streams
    pub fn extract_raw_pages(&self, bytes: &[u8]) -> Result<(Document, Vec<RawPageContent>), String> {
        let doc = Document::load_mem(bytes).map_err(|e| format!("Failed to parse PDF binary: {}", e))?;
        let pages_map = doc.get_pages();
        let mut raw_pages = Vec::with_capacity(pages_map.len());

        for (&page_num, &page_id) in pages_map.iter() {
            let page_obj = doc.get_object(page_id)
                .map_err(|e| format!("Failed to get page object {}: {}", page_num, e))?;
            let page_dict = page_obj.as_dict()
                .map_err(|e| format!("Page object {} is not a dictionary: {}", page_num, e))?;

            // Extract MediaBox or CropBox [x0, y0, x1, y1]
            let mut width = 612.0;  // Standard Letter default
            let mut height = 792.0;

            let box_obj = page_dict.get(b"CropBox")
                .or_else(|_| page_dict.get(b"MediaBox"));

            if let Ok(b) = box_obj {
                if let Ok(arr) = b.as_array() {
                    if arr.len() >= 4 {
                        let x0 = get_f64(&arr[0]).unwrap_or(0.0);
                        let y0 = get_f64(&arr[1]).unwrap_or(0.0);
                        let x1 = get_f64(&arr[2]).unwrap_or(612.0);
                        let y1 = get_f64(&arr[3]).unwrap_or(792.0);
                        width = (x1 - x0).abs();
                        height = (y1 - y0).abs();
                    }
                }
            }

            // Extract Resources (Fonts, XObjects, Properties)
            let mut fonts = BTreeMap::new();
            let mut xobjects = BTreeMap::new();
            let mut properties = BTreeMap::new();

            if let Ok(res_obj) = page_dict.get(b"Resources") {
                let res_dict = match res_obj {
                    Object::Reference(r) => doc.get_object(*r).ok().and_then(|o| o.as_dict().ok()),
                    Object::Dictionary(d) => Some(d),
                    _ => None,
                };

                if let Some(res) = res_dict {
                    // Extract Fonts
                    if let Ok(font_res) = res.get(b"Font") {
                        let font_dict = match font_res {
                            Object::Reference(r) => doc.get_object(*r).ok().and_then(|o| o.as_dict().ok()),
                            Object::Dictionary(d) => Some(d),
                            _ => None,
                        };
                        if let Some(fd) = font_dict {
                            for (name, obj) in fd.iter() {
                                if let Ok(ref_id) = obj.as_reference() {
                                    let font_name = String::from_utf8_lossy(name).to_string();
                                    fonts.insert(font_name, ref_id);
                                }
                            }
                        }
                    }

                    // Extract XObjects (Images and Forms)
                    if let Ok(xobj_res) = res.get(b"XObject") {
                        let xobj_dict = match xobj_res {
                            Object::Reference(r) => doc.get_object(*r).ok().and_then(|o| o.as_dict().ok()),
                            Object::Dictionary(d) => Some(d),
                            _ => None,
                        };
                        if let Some(xd) = xobj_dict {
                            for (name, obj) in xd.iter() {
                                if let Ok(ref_id) = obj.as_reference() {
                                    let xobj_name = String::from_utf8_lossy(name).to_string();
                                    xobjects.insert(xobj_name, ref_id);
                                }
                            }
                        }
                    }

                    // Extract Marked Content Properties (/Properties)
                    if let Ok(prop_res) = res.get(b"Properties") {
                        let prop_dict = match prop_res {
                            Object::Reference(r) => doc.get_object(*r).ok().and_then(|o| o.as_dict().ok()),
                            Object::Dictionary(d) => Some(d),
                            _ => None,
                        };
                        if let Some(pd) = prop_dict {
                            for (name, obj) in pd.iter() {
                                let prop_name = String::from_utf8_lossy(name).to_string();
                                properties.insert(prop_name, obj.clone());
                            }
                        }
                    }
                }
            }

            // Extract and decompress content streams
            let content_bytes = doc.get_page_content(page_id).unwrap_or_default();

            raw_pages.push(RawPageContent {
                page_number: page_num as usize,
                width,
                height,
                content_bytes,
                fonts,
                xobjects,
                properties,
            });
        }

        Ok((doc, raw_pages))
    }

    /// Decompresses stream data using predictor parameters if present
    pub fn decompress_stream(&self, stream_obj: &lopdf::Stream) -> Result<Vec<u8>, String> {
        let mut data = stream_obj.decompressed_content()
            .map_err(|e| format!("Decompression failed: {}", e))?;

        if let Ok(decode_parms) = stream_obj.dict.get(b"DecodeParms") {
            if let Ok(parms_dict) = decode_parms.as_dict() {
                let predictor = parms_dict.get(b"Predictor").and_then(|p| p.as_i64()).unwrap_or(1);
                let columns = parms_dict.get(b"Columns").and_then(|c| c.as_i64()).unwrap_or(1) as usize;
                let colors = parms_dict.get(b"Colors").and_then(|c| c.as_i64()).unwrap_or(1) as usize;
                let bpc = parms_dict.get(b"BitsPerComponent").and_then(|b| b.as_i64()).unwrap_or(8) as usize;

                if predictor > 1 {
                    data = decode_predictor(&data, predictor, columns, colors, bpc)?;
                }
            }
        }

        Ok(data)
    }

    /// Parses raw pages into DocumentPage structures (Step 6 extracts Tagged PDF & Marked Content)
    pub fn parse_bytes(&self, bytes: &[u8]) -> Result<Vec<DocumentPage>, String> {
        let (doc, raw_pages) = self.extract_raw_pages(bytes)?;
        let struct_tree = StructTreeExtractor::extract(&doc).unwrap_or_default();
        let mut document_pages = Vec::with_capacity(raw_pages.len());

        for raw in raw_pages {
            let evaluator = crate::pdf::evaluator::StreamEvaluator::new(
                &doc,
                &raw.fonts,
                &raw.xobjects,
                &raw.properties,
                raw.height,
            );

            let evaluated = evaluator.evaluate(&raw.content_bytes)?;

            document_pages.push(DocumentPage {
                page_number: raw.page_number,
                width_points: raw.width,
                height_points: raw.height,
                glyphs: evaluated.glyphs,
                vector_paths: evaluated.vector_paths,
                paragraphs: Vec::new(),
                tables: Vec::new(),
                graphics: evaluated.graphics,
                is_tagged: false,
                margin_left_points: 72.0,
                margin_right_points: 72.0,
                margin_top_points: 72.0,
                margin_bottom_points: 72.0,
                header: None,
                footer: None,
            });
        }

        // Apply authoritative Tagged PDF logical structure if present
        StructTreeExtractor::apply_to_pages(&struct_tree, &mut document_pages);

        Ok(document_pages)
    }

    /// Extracts logical structure tree and outlines without evaluating content streams
    pub fn extract_structure_tree(&self, bytes: &[u8]) -> Result<crate::models::StructTree, String> {
        let doc = Document::load_mem(bytes).map_err(|e| format!("Failed to parse PDF binary: {}", e))?;
        StructTreeExtractor::extract(&doc)
    }
}
