/// Implementation of ISO 32000-1 §7.4.4.4 Predictor Functions for FlateDecode
/// Reverses PNG and TIFF prediction filters applied prior to compression.

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PredictorType {
    None = 1,
    Tiff2 = 2,
    PngNone = 10,
    PngSub = 11,
    PngUp = 12,
    PngAverage = 13,
    PngPaeth = 14,
    PngOptimum = 15,
}

impl PredictorType {
    pub fn from_i64(val: i64) -> Self {
        match val {
            2 => PredictorType::Tiff2,
            10 => PredictorType::PngNone,
            11 => PredictorType::PngSub,
            12 => PredictorType::PngUp,
            13 => PredictorType::PngAverage,
            14 => PredictorType::PngPaeth,
            15 => PredictorType::PngOptimum,
            _ => PredictorType::None,
        }
    }
}

/// Decodes filtered byte stream based on PDF /DecodeParms
pub fn decode_predictor(
    data: &[u8],
    predictor: i64,
    columns: usize,
    colors: usize,
    bits_per_component: usize,
) -> Result<Vec<u8>, String> {
    if predictor <= 1 {
        return Ok(data.to_vec());
    }

    let p_type = PredictorType::from_i64(predictor);

    // Bytes per pixel (sample)
    let bpp = ((colors * bits_per_component) + 7) / 8;
    let bytes_per_row = (columns * colors * bits_per_component + 7) / 8;

    match p_type {
        PredictorType::Tiff2 => {
            // Horizontal differencing
            let mut out = Vec::with_capacity(data.len());
            for chunk in data.chunks(bytes_per_row) {
                let mut row = chunk.to_vec();
                for i in bpp..row.len() {
                    row[i] = row[i].wrapping_add(row[i - bpp]);
                }
                out.extend_from_slice(&row);
            }
            Ok(out)
        }
        PredictorType::PngNone
        | PredictorType::PngSub
        | PredictorType::PngUp
        | PredictorType::PngAverage
        | PredictorType::PngPaeth
        | PredictorType::PngOptimum => {
            decode_png_prediction(data, columns, bpp, bytes_per_row)
        }
        _ => Ok(data.to_vec()),
    }
}

/// Reverses PNG filter types (0: None, 1: Sub, 2: Up, 3: Average, 4: Paeth)
fn decode_png_prediction(
    data: &[u8],
    _columns: usize,
    bpp: usize,
    bytes_per_row: usize,
) -> Result<Vec<u8>, String> {
    let row_stride = bytes_per_row + 1; // 1-byte filter prefix per row
    if data.is_empty() {
        return Ok(Vec::new());
    }

    let estimated_rows = data.len() / row_stride;
    let mut output = Vec::with_capacity(estimated_rows * bytes_per_row);
    let mut prev_row = vec![0u8; bytes_per_row];

    for chunk in data.chunks(row_stride) {
        if chunk.is_empty() {
            break;
        }
        let filter_type = chunk[0];
        let current_raw = &chunk[1..];
        let mut reconstructed_row = vec![0u8; bytes_per_row];

        for i in 0..current_raw.len().min(bytes_per_row) {
            let left = if i >= bpp { reconstructed_row[i - bpp] } else { 0 };
            let above = prev_row[i];
            let upper_left = if i >= bpp { prev_row[i - bpp] } else { 0 };

            let pred = match filter_type {
                0 => 0u8, // None
                1 => left, // Sub
                2 => above, // Up
                3 => ((left as u16 + above as u16) / 2) as u8, // Average
                4 => paeth_predictor(left, above, upper_left), // Paeth
                other => {
                    return Err(format!("Unsupported PNG filter type: {}", other));
                }
            };

            reconstructed_row[i] = current_raw[i].wrapping_add(pred);
        }

        output.extend_from_slice(&reconstructed_row);
        prev_row = reconstructed_row;
    }

    Ok(output)
}

/// ISO 32000-1 / PNG Paeth linear predictor algorithm
#[inline]
fn paeth_predictor(left: u8, above: u8, upper_left: u8) -> u8 {
    let a = left as i32;
    let b = above as i32;
    let c = upper_left as i32;

    let p = a + b - c;
    let pa = (p - a).abs();
    let pb = (p - b).abs();
    let pc = (p - c).abs();

    if pa <= pb && pa <= pc {
        left
    } else if pb <= pc {
        above
    } else {
        upper_left
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paeth_calculation() {
        assert_eq!(paeth_predictor(10, 20, 10), 20);
        assert_eq!(paeth_predictor(50, 50, 50), 50);
        assert_eq!(paeth_predictor(100, 20, 20), 100);
    }

    #[test]
    fn test_png_sub_reconstruction() {
        // Row 1: filter type 1 (Sub), bytes [10, 5, 2] with bpp = 1
        // Original should be: [10, 10+5=15, 15+2=17]
        let raw = vec![1, 10, 5, 2];
        let decoded = decode_predictor(&raw, 15, 3, 1, 8).unwrap();
        assert_eq!(decoded, vec![10, 15, 17]);
    }
}

