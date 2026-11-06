use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use lopdf::{Document, Object, Stream};
use std::io::{Read, Write};

use crate::models::{GraphicObject, Rect};

/// Pure-Rust PNG Encoder without C/C++ dependencies
pub fn encode_png_rgb(width: u32, height: u32, rgb_data: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(32 + rgb_data.len() / 2);
    // 1. Signature
    out.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);

    // 2. IHDR
    let mut ihdr_data = Vec::with_capacity(13);
    ihdr_data.extend_from_slice(&width.to_be_bytes());
    ihdr_data.extend_from_slice(&height.to_be_bytes());
    ihdr_data.push(8); // 8-bit depth
    ihdr_data.push(2); // RGB color type
    ihdr_data.push(0); // compression method (deflate)
    ihdr_data.push(0); // filter method (standard)
    ihdr_data.push(0); // interlace method (none)
    write_chunk(&mut out, b"IHDR", &ihdr_data);

    // 3. Scanline filtering (filter 0 = None for each row)
    let row_len = (width as usize) * 3;
    let mut filtered = Vec::with_capacity((height as usize) * (row_len + 1));
    for y in 0..(height as usize) {
        filtered.push(0); // Filter byte 0
        let start = y * row_len;
        let end = (start + row_len).min(rgb_data.len());
        if start < rgb_data.len() {
            filtered.extend_from_slice(&rgb_data[start..end]);
        }
    }

    // 4. IDAT (zlib compressed)
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&filtered).map_err(|e| e.to_string())?;
    let compressed = encoder.finish().map_err(|e| e.to_string())?;
    write_chunk(&mut out, b"IDAT", &compressed);

    // 5. IEND
    write_chunk(&mut out, b"IEND", &[]);

    Ok(out)
}

/// Pure-Rust PNG Encoder for RGBA (with Alpha transparency)
pub fn encode_png_rgba(width: u32, height: u32, rgba_data: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(32 + rgba_data.len() / 2);
    out.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);

    let mut ihdr_data = Vec::with_capacity(13);
    ihdr_data.extend_from_slice(&width.to_be_bytes());
    ihdr_data.extend_from_slice(&height.to_be_bytes());
    ihdr_data.push(8);
    ihdr_data.push(6); // RGBA color type
    ihdr_data.push(0);
    ihdr_data.push(0);
    ihdr_data.push(0);
    write_chunk(&mut out, b"IHDR", &ihdr_data);

    let row_len = (width as usize) * 4;
    let mut filtered = Vec::with_capacity((height as usize) * (row_len + 1));
    for y in 0..(height as usize) {
        filtered.push(0);
        let start = y * row_len;
        let end = (start + row_len).min(rgba_data.len());
        if start < rgba_data.len() {
            filtered.extend_from_slice(&rgba_data[start..end]);
        }
    }

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&filtered).map_err(|e| e.to_string())?;
    let compressed = encoder.finish().map_err(|e| e.to_string())?;
    write_chunk(&mut out, b"IDAT", &compressed);

    write_chunk(&mut out, b"IEND", &[]);

    Ok(out)
}

/// Pure-Rust PNG Encoder for Grayscale (Color type 0)
pub fn encode_png_gray(width: u32, height: u32, gray_data: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(32 + gray_data.len() / 2);
    out.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);

    let mut ihdr_data = Vec::with_capacity(13);
    ihdr_data.extend_from_slice(&width.to_be_bytes());
    ihdr_data.extend_from_slice(&height.to_be_bytes());
    ihdr_data.push(8);
    ihdr_data.push(0); // Grayscale
    ihdr_data.push(0);
    ihdr_data.push(0);
    ihdr_data.push(0);
    write_chunk(&mut out, b"IHDR", &ihdr_data);

    let row_len = width as usize;
    let mut filtered = Vec::with_capacity((height as usize) * (row_len + 1));
    for y in 0..(height as usize) {
        filtered.push(0);
        let start = y * row_len;
        let end = (start + row_len).min(gray_data.len());
        if start < gray_data.len() {
            filtered.extend_from_slice(&gray_data[start..end]);
        }
    }

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&filtered).map_err(|e| e.to_string())?;
    let compressed = encoder.finish().map_err(|e| e.to_string())?;
    write_chunk(&mut out, b"IDAT", &compressed);

    write_chunk(&mut out, b"IEND", &[]);

    Ok(out)
}

fn write_chunk(out: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let crc_start = out.len();
    out.extend_from_slice(chunk_type);
    out.extend_from_slice(data);
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(&out[crc_start..]);
    let crc = hasher.finalize();
    out.extend_from_slice(&crc.to_be_bytes());
}

/// Extracts a clean standalone image (JPEG or PNG) from a PDF Image XObject Stream
pub fn extract_image_object(
    doc: &Document,
    stream: &Stream,
    bbox: Rect,
) -> Option<GraphicObject> {
    let content = &stream.content;
    if content.is_empty() {
        return None;
    }

    // 1. Check if stream content is directly a JPEG (FF D8 ... FF D9)
    if content.len() >= 4 && content[0] == 0xFF && content[1] == 0xD8 {
        return Some(GraphicObject {
            bbox,
            data: content.clone(),
            mime_type: "image/jpeg".to_string(),
            is_inline: true,
            is_background: false,
        });
    }

    // 2. Check if stream content is directly a PNG (89 50 4E 47 ...)
    if content.len() >= 8 && content.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]) {
        return Some(GraphicObject {
            bbox,
            data: content.clone(),
            mime_type: "image/png".to_string(),
            is_inline: true,
            is_background: false,
        });
    }

    // 3. FlateDecode or Raw Stream: Decompress if zlib compressed
    let mut decompressed = Vec::new();
    let uncompressed_bytes = if content.len() >= 2 && content[0] == 0x78 && (content[1] == 0x9C || content[1] == 0x01 || content[1] == 0xDA || content[1] == 0x5E) {
        let mut decoder = ZlibDecoder::new(content.as_slice());
        if decoder.read_to_end(&mut decompressed).is_ok() {
            &decompressed
        } else {
            content
        }
    } else {
        content
    };

    let width = stream.dict.get(b"Width").and_then(|w| w.as_i64()).unwrap_or(0) as u32;
    let height = stream.dict.get(b"Height").and_then(|h| h.as_i64()).unwrap_or(0) as u32;

    if width == 0 || height == 0 {
        return None;
    }

    // Check for Soft Mask (/SMask) for alpha transparency
    let mut alpha_channel = Vec::new();
    if let Ok(smask_obj) = stream.dict.get(b"SMask") {
        if let Ok(smask_ref) = smask_obj.as_reference() {
            if let Ok(smask_xobj) = doc.get_object(smask_ref) {
                if let Object::Stream(smask_stream) = smask_xobj {
                    let mut smask_decompressed = Vec::new();
                    if smask_stream.content.len() >= 2 && smask_stream.content[0] == 0x78 {
                        let mut decoder = ZlibDecoder::new(smask_stream.content.as_slice());
                        let _ = decoder.read_to_end(&mut smask_decompressed);
                    } else {
                        smask_decompressed = smask_stream.content.clone();
                    }
                    alpha_channel = smask_decompressed;
                }
            }
        }
    }

    let total_pixels = (width as usize) * (height as usize);

    // If 3 bytes per pixel (RGB)
    if uncompressed_bytes.len() >= total_pixels * 3 {
        if !alpha_channel.is_empty() && alpha_channel.len() >= total_pixels {
            // Interleave RGB + Alpha into RGBA
            let mut rgba = Vec::with_capacity(total_pixels * 4);
            for i in 0..total_pixels {
                rgba.push(uncompressed_bytes[i * 3]);
                rgba.push(uncompressed_bytes[i * 3 + 1]);
                rgba.push(uncompressed_bytes[i * 3 + 2]);
                rgba.push(alpha_channel[i]);
            }
            if let Ok(png_data) = encode_png_rgba(width, height, &rgba) {
                return Some(GraphicObject {
                    bbox,
                    data: png_data,
                    mime_type: "image/png".to_string(),
                    is_inline: true,
                    is_background: false,
                });
            }
        } else if let Ok(png_data) = encode_png_rgb(width, height, uncompressed_bytes) {
            return Some(GraphicObject {
                bbox,
                data: png_data,
                mime_type: "image/png".to_string(),
                is_inline: true,
                is_background: false,
            });
        }
    } else if uncompressed_bytes.len() >= total_pixels {
        // Grayscale (1 byte per pixel)
        if !alpha_channel.is_empty() && alpha_channel.len() >= total_pixels {
            let mut rgba = Vec::with_capacity(total_pixels * 4);
            for i in 0..total_pixels {
                let gray = uncompressed_bytes[i];
                rgba.push(gray);
                rgba.push(gray);
                rgba.push(gray);
                rgba.push(alpha_channel[i]);
            }
            if let Ok(png_data) = encode_png_rgba(width, height, &rgba) {
                return Some(GraphicObject {
                    bbox,
                    data: png_data,
                    mime_type: "image/png".to_string(),
                    is_inline: true,
                    is_background: false,
                });
            }
        } else if let Ok(png_data) = encode_png_gray(width, height, uncompressed_bytes) {
            return Some(GraphicObject {
                bbox,
                data: png_data,
                mime_type: "image/png".to_string(),
                is_inline: true,
                is_background: false,
            });
        }
    }

    None
}

