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

    // 1. Check for Soft Mask (/SMask) for alpha transparency
    let mut alpha_channel = Vec::new();
    let mut matte = Vec::new();

    if let Ok(smask_obj) = stream.dict.get(b"SMask") {
        if let Ok(smask_ref) = smask_obj.as_reference() {
            if let Ok(smask_xobj) = doc.get_object(smask_ref) {
                if let Object::Stream(smask_stream) = smask_xobj {
                    let mut smask_decompressed = Vec::new();
                    if smask_stream.content.len() >= 2 && smask_stream.content[0] == 0x78 {
                        let mut decoder = ZlibDecoder::new(smask_stream.content.as_slice());
                        let _ = decoder.read_to_end(&mut smask_decompressed);
                    } else if smask_stream.content.len() >= 4 && smask_stream.content[0] == 0xFF && smask_stream.content[1] == 0xD8 {
                        let mut dec = jpeg_decoder::Decoder::new(smask_stream.content.as_slice());
                        if let Ok(pixels) = dec.decode() {
                            smask_decompressed = pixels;
                        }
                    } else {
                        smask_decompressed = smask_stream.content.clone();
                    }
                    alpha_channel = smask_decompressed;

                    if let Ok(matte_obj) = smask_stream.dict.get(b"Matte") {
                        if let Ok(arr) = matte_obj.as_array() {
                            for item in arr {
                                match item {
                                    Object::Real(val) => {
                                        matte.push((*val as f64 * 255.0).round().clamp(0.0, 255.0) as u8);
                                    }
                                    Object::Integer(val) => {
                                        matte.push((*val as u8).clamp(0, 255));
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Check if stream content is directly a JPEG (FF D8 ... FF D9)
    if content.len() >= 4 && content[0] == 0xFF && content[1] == 0xD8 {
        if alpha_channel.is_empty() {
            // No alpha mask: cleanly return raw JPEG!
            return Some(GraphicObject {
                bbox,
                data: content.clone(),
                mime_type: "image/jpeg".to_string(),
                is_inline: true,
                is_background: false,
                behind_doc: false,
            });
        }

        // JPEG with SMask: decode JPEG to raw RGB, unmatte, and encode to PNG RGBA!
        let mut decoder = jpeg_decoder::Decoder::new(content.as_slice());
        if let Ok(rgb_pixels) = decoder.decode() {
            if let Some(info) = decoder.info() {
                let img_w = info.width as u32;
                let img_h = info.height as u32;
                let total_px = (img_w as usize) * (img_h as usize);

                if rgb_pixels.len() >= total_px * 3 && alpha_channel.len() >= total_px {
                    let mut rgba = Vec::with_capacity(total_px * 4);
                    let m_r = matte.get(0).copied().unwrap_or(0) as f64;
                    let m_g = matte.get(1).copied().unwrap_or(0) as f64;
                    let m_b = matte.get(2).copied().unwrap_or(0) as f64;
                    let has_matte = !matte.is_empty();

                    for i in 0..total_px {
                        let a_byte = alpha_channel[i];
                        let a_norm = (a_byte as f64) / 255.0;

                        if a_byte == 0 {
                            rgba.push(0);
                            rgba.push(0);
                            rgba.push(0);
                            rgba.push(0);
                        } else if has_matte && a_norm > 0.0 && a_norm < 1.0 {
                            let r_pre = rgb_pixels[i * 3] as f64;
                            let g_pre = rgb_pixels[i * 3 + 1] as f64;
                            let b_pre = rgb_pixels[i * 3 + 2] as f64;

                            let r_true = ((r_pre - m_r * (1.0 - a_norm)) / a_norm).round().clamp(0.0, 255.0) as u8;
                            let g_true = ((g_pre - m_g * (1.0 - a_norm)) / a_norm).round().clamp(0.0, 255.0) as u8;
                            let b_true = ((b_pre - m_b * (1.0 - a_norm)) / a_norm).round().clamp(0.0, 255.0) as u8;

                            rgba.push(r_true);
                            rgba.push(g_true);
                            rgba.push(b_true);
                            rgba.push(a_byte);
                        } else {
                            rgba.push(rgb_pixels[i * 3]);
                            rgba.push(rgb_pixels[i * 3 + 1]);
                            rgba.push(rgb_pixels[i * 3 + 2]);
                            rgba.push(a_byte);
                        }
                    }

                    if let Ok(png_data) = encode_png_rgba(img_w, img_h, &rgba) {
                        return Some(GraphicObject {
                            bbox,
                            data: png_data,
                            mime_type: "image/png".to_string(),
                            is_inline: true,
                            is_background: false,
                            behind_doc: false,
                        });
                    }
                }
            }
        }
    }

    // 3. Check if stream content is directly a PNG (89 50 4E 47 ...)
    if content.len() >= 8 && content.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]) {
        return Some(GraphicObject {
            bbox,
            data: content.clone(),
            mime_type: "image/png".to_string(),
            is_inline: true,
            is_background: false,
            behind_doc: false,
        });
    }

    // 4. FlateDecode or Raw Stream: Decompress if zlib compressed
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

    // Check /Decode array for color inversion (e.g. [1 0] for inverted monochrome/grayscale)
    let is_inverted = if let Ok(dec_obj) = stream.dict.get(b"Decode") {
        if let Ok(arr) = dec_obj.as_array() {
            if arr.len() >= 2 {
                let to_f64 = |obj: &Object| match obj {
                    Object::Real(r) => *r as f64,
                    Object::Integer(i) => *i as f64,
                    _ => 0.0,
                };
                let d0 = to_f64(&arr[0]);
                let d1 = to_f64(&arr[1]);
                d0 > d1
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    };

    // Check if image is a 1-bit or stencil /ImageMask
    let is_image_mask = stream.dict.get(b"ImageMask")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let total_pixels = (width as usize) * (height as usize);

    // 4a. 1-bit Stencil Mask or 1-bit Monochrome Image
    let bits_per_component = stream.dict.get(b"BitsPerComponent").and_then(|b| b.as_i64()).unwrap_or(8) as u32;
    let row_stride_1bit = ((width as usize) + 7) / 8;

    if is_image_mask || (bits_per_component == 1 && uncompressed_bytes.len() >= row_stride_1bit * (height as usize)) {
        let mut rgba = Vec::with_capacity(total_pixels * 4);
        for y in 0..(height as usize) {
            let row_start = y * row_stride_1bit;
            for x in 0..(width as usize) {
                let byte_idx = row_start + (x / 8);
                let bit_idx = 7 - (x % 8);
                let bit = if byte_idx < uncompressed_bytes.len() {
                    (uncompressed_bytes[byte_idx] >> bit_idx) & 1
                } else {
                    0
                };

                let is_ink = if is_inverted { bit == 0 } else { bit == 1 };

                if is_image_mask {
                    // Stencil mask: ink pixels are solid black, background is fully transparent!
                    if is_ink {
                        rgba.push(0);
                        rgba.push(0);
                        rgba.push(0);
                        rgba.push(255);
                    } else {
                        rgba.push(255);
                        rgba.push(255);
                        rgba.push(255);
                        rgba.push(0);
                    }
                } else {
                    // Standard 1-bit monochrome image
                    let val = if is_ink { 0 } else { 255 };
                    rgba.push(val);
                    rgba.push(val);
                    rgba.push(val);
                    rgba.push(255);
                }
            }
        }

        if let Ok(png_data) = encode_png_rgba(width, height, &rgba) {
            return Some(GraphicObject {
                bbox,
                data: png_data,
                mime_type: "image/png".to_string(),
                is_inline: true,
                is_background: false,
                behind_doc: false,
            });
        }
    }

    // If 3 bytes per pixel (RGB)
    if uncompressed_bytes.len() >= total_pixels * 3 {
        if !alpha_channel.is_empty() && alpha_channel.len() >= total_pixels {
            let mut rgba = Vec::with_capacity(total_pixels * 4);
            let m_r = matte.get(0).copied().unwrap_or(0) as f64;
            let m_g = matte.get(1).copied().unwrap_or(0) as f64;
            let m_b = matte.get(2).copied().unwrap_or(0) as f64;
            let has_matte = !matte.is_empty();

            for i in 0..total_pixels {
                let a_byte = alpha_channel[i];
                let a_norm = (a_byte as f64) / 255.0;

                if a_byte == 0 {
                    rgba.push(0);
                    rgba.push(0);
                    rgba.push(0);
                    rgba.push(0);
                } else if has_matte && a_norm > 0.0 && a_norm < 1.0 {
                    let r_pre = uncompressed_bytes[i * 3] as f64;
                    let g_pre = uncompressed_bytes[i * 3 + 1] as f64;
                    let b_pre = uncompressed_bytes[i * 3 + 2] as f64;

                    let r_true = ((r_pre - m_r * (1.0 - a_norm)) / a_norm).round().clamp(0.0, 255.0) as u8;
                    let g_true = ((g_pre - m_g * (1.0 - a_norm)) / a_norm).round().clamp(0.0, 255.0) as u8;
                    let b_true = ((b_pre - m_b * (1.0 - a_norm)) / a_norm).round().clamp(0.0, 255.0) as u8;

                    rgba.push(r_true);
                    rgba.push(g_true);
                    rgba.push(b_true);
                    rgba.push(a_byte);
                } else {
                    rgba.push(uncompressed_bytes[i * 3]);
                    rgba.push(uncompressed_bytes[i * 3 + 1]);
                    rgba.push(uncompressed_bytes[i * 3 + 2]);
                    rgba.push(a_byte);
                }
            }
            if let Ok(png_data) = encode_png_rgba(width, height, &rgba) {
                return Some(GraphicObject {
                    bbox,
                    data: png_data,
                    mime_type: "image/png".to_string(),
                    is_inline: true,
                    is_background: false,
                    behind_doc: false,
                });
            }
        } else if let Ok(png_data) = encode_png_rgb(width, height, uncompressed_bytes) {
            return Some(GraphicObject {
                bbox,
                data: png_data,
                mime_type: "image/png".to_string(),
                is_inline: true,
                is_background: false,
                behind_doc: false,
            });
        }
    } else if uncompressed_bytes.len() >= total_pixels {
        // Grayscale (1 byte per pixel)
        let processed_gray: Vec<u8> = if is_inverted {
            uncompressed_bytes.iter().take(total_pixels).map(|&b| 255 - b).collect()
        } else {
            uncompressed_bytes[..total_pixels].to_vec()
        };

        if !alpha_channel.is_empty() && alpha_channel.len() >= total_pixels {
            let mut rgba = Vec::with_capacity(total_pixels * 4);
            for i in 0..total_pixels {
                let gray = processed_gray[i];
                let a_byte = alpha_channel[i];
                if a_byte == 0 {
                    rgba.push(0);
                    rgba.push(0);
                    rgba.push(0);
                    rgba.push(0);
                } else {
                    rgba.push(gray);
                    rgba.push(gray);
                    rgba.push(gray);
                    rgba.push(a_byte);
                }
            }
            if let Ok(png_data) = encode_png_rgba(width, height, &rgba) {
                return Some(GraphicObject {
                    bbox,
                    data: png_data,
                    mime_type: "image/png".to_string(),
                    is_inline: true,
                    is_background: false,
                    behind_doc: false,
                });
            }
        } else if let Ok(png_data) = encode_png_gray(width, height, &processed_gray) {
            return Some(GraphicObject {
                bbox,
                data: png_data,
                mime_type: "image/png".to_string(),
                is_inline: true,
                is_background: false,
                behind_doc: false,
            });
        }
    }

    None
}

