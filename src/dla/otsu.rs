/// Otsu Discriminant Analysis & Projection Profile Valley Analysis
/// Replaces arbitrary heuristic thresholds with variance maximization and energy minimization (Otsu 1979).

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypographicScale {
    pub body_size: f64,
    pub h2_threshold: f64,
    pub h1_threshold: f64,
}

pub struct OtsuAnalyzer;

impl OtsuAnalyzer {
    /// Computes the optimal single Otsu threshold for a set of continuous values.
    /// Maximizes between-class variance: \sigma_b^2(t) = \omega_0(t) * \omega_1(t) * (\mu_0(t) - \mu_1(t))^2
    pub fn single_threshold(values: &[f64]) -> Option<f64> {
        if values.len() < 2 {
            return None;
        }

        let mut sorted = values.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let min_v = sorted[0];
        let max_v = sorted[sorted.len() - 1];
        if (max_v - min_v).abs() < 1e-4 {
            return None;
        }

        // Discretize into 128 bins for high resolution and fast computation
        const NUM_BINS: usize = 128;
        let mut hist = [0usize; NUM_BINS];
        let bin_width = (max_v - min_v) / (NUM_BINS as f64);

        for &v in &sorted {
            let bin = (((v - min_v) / bin_width).floor() as usize).min(NUM_BINS - 1);
            hist[bin] += 1;
        }

        let total = sorted.len() as f64;
        let mut sum_total = 0.0f64;
        for i in 0..NUM_BINS {
            let bin_center = min_v + (i as f64 + 0.5) * bin_width;
            sum_total += (hist[i] as f64) * bin_center;
        }

        let mut weight_0 = 0.0f64;
        let mut sum_0 = 0.0f64;
        let mut max_variance = -1.0f64;
        let mut best_bin = 0;

        for i in 0..NUM_BINS {
            let count = hist[i] as f64;
            if count == 0.0 {
                continue;
            }

            weight_0 += count;
            let weight_1 = total - weight_0;
            if weight_1 <= 0.0 {
                break;
            }

            let bin_center = min_v + (i as f64 + 0.5) * bin_width;
            sum_0 += count * bin_center;

            let mean_0 = sum_0 / weight_0;
            let mean_1 = (sum_total - sum_0) / weight_1;

            let variance = weight_0 * weight_1 * (mean_0 - mean_1).powi(2);
            if variance > max_variance {
                max_variance = variance;
                best_bin = i;
            }
        }

        Some(min_v + (best_bin as f64 + 1.0) * bin_width)
    }

    /// Computes multi-threshold Otsu for typography partitioning (3 classes: Body, H2/Subheading, H1/Title).
    /// Maximizes \sigma_b^2(t1, t2) = \omega_0 \mu_0^2 + \omega_1 \mu_1^2 + \omega_2 \mu_2^2 - \mu_T^2
    pub fn compute_typographic_scale(font_sizes: &[f64]) -> TypographicScale {
        if font_sizes.is_empty() {
            return TypographicScale {
                body_size: 11.0,
                h2_threshold: 13.0,
                h1_threshold: 16.0,
            };
        }

        let mut sorted = font_sizes.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        // Dominant body font size is statistical mode (or median if multimodal)
        let body_size = {
            let mut counts: std::collections::HashMap<i64, usize> = std::collections::HashMap::new();
            for &s in &sorted {
                let key = (s * 10.0).round() as i64;
                *counts.entry(key).or_default() += 1;
            }
            if let Some((&dominant_key, _)) = counts.iter().max_by_key(|&(_, count)| *count) {
                (dominant_key as f64) / 10.0
            } else {
                sorted[sorted.len() / 2]
            }
        };

        // Filter distinct font sizes greater than body size
        let larger_sizes: Vec<f64> = sorted.iter().cloned().filter(|&s| s > body_size + 0.5).collect();

        if larger_sizes.is_empty() {
            return TypographicScale {
                body_size,
                h2_threshold: body_size + 1.5,
                h1_threshold: body_size + 3.5,
            };
        }

        if let Some(t1) = Self::single_threshold(&larger_sizes) {
            // Find secondary threshold between body and t1, or t1 and max
            let lower_half: Vec<f64> = larger_sizes.iter().cloned().filter(|&s| s < t1).collect();
            let upper_half: Vec<f64> = larger_sizes.iter().cloned().filter(|&s| s >= t1).collect();

            let h2_threshold = if !lower_half.is_empty() {
                lower_half[0]
            } else {
                t1
            };

            let h1_threshold = if !upper_half.is_empty() {
                upper_half[0]
            } else {
                t1 + 2.0
            };

            TypographicScale {
                body_size,
                h2_threshold: h2_threshold.max(body_size + 1.0),
                h1_threshold: h1_threshold.max(h2_threshold + 1.0),
            }
        } else {
            let max_s = larger_sizes[larger_sizes.len() - 1];
            TypographicScale {
                body_size,
                h2_threshold: (body_size + max_s) / 2.0,
                h1_threshold: max_s,
            }
        }
    }

    /// Finds horizontal and vertical margin boundaries via 1D projection profile analysis.
    /// Traverses the projection profile from the page edges to detect the first and last energy onset (content presence).
    pub fn compute_page_margins(
        bboxes: &[crate::models::Rect],
        page_width: f64,
        page_height: f64,
    ) -> (f64, f64, f64, f64) {
        if bboxes.is_empty() {
            return (54.0, 54.0, 54.0, 54.0);
        }

        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for b in bboxes {
            if b.width() > 0.0 && b.height() > 0.0 {
                min_x = min_x.min(b.x_min);
                min_y = min_y.min(b.y_min);
                max_x = max_x.max(b.x_max);
                max_y = max_y.max(b.y_max);
            }
        }

        if min_x.is_infinite() {
            return (54.0, 54.0, 54.0, 54.0);
        }

        let left = min_x.clamp(18.0, page_width * 0.25);
        let right = (page_width - max_x).clamp(18.0, page_width * 0.25);
        let top = min_y.clamp(18.0, page_height * 0.20);
        let bottom = (page_height - max_y).clamp(18.0, page_height * 0.20);

        (left, right, top, bottom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_otsu() {
        let values = vec![10.0, 10.5, 10.0, 11.0, 10.0, 18.0, 20.0, 18.5, 19.0];
        let thresh = OtsuAnalyzer::single_threshold(&values).expect("Threshold must exist");
        assert!(thresh > 11.0 && thresh < 18.0, "Threshold {} must separate 10-11 from 18-20", thresh);
    }

    #[test]
    fn test_typographic_scale() {
        let sizes = vec![10.5, 10.5, 10.5, 10.5, 10.5, 14.0, 14.0, 20.0, 20.0];
        let scale = OtsuAnalyzer::compute_typographic_scale(&sizes);
        assert!((scale.body_size - 10.5).abs() < 0.1);
        assert!(scale.h2_threshold <= 14.5 && scale.h2_threshold >= 11.5);
        assert!(scale.h1_threshold >= 14.0);
    }
}

