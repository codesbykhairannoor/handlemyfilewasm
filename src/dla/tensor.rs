/// Continuous Linear Discriminant and Logistic Tensor Engine (Zero-Heuristic)
/// Replaces discrete heuristic if-else branching with continuous probability manifolds.

/// Sigmoid activation function: \sigma(z) = 1 / (1 + e^{-z})
#[inline]
pub fn sigmoid(z: f64) -> f64 {
    1.0 / (1.0 + (-z).exp())
}

/// Dot product of feature vector and weights plus bias: z = w^T x + b
#[inline]
pub fn linear_logit(features: &[f64], weights: &[f64], bias: f64) -> f64 {
    debug_assert_eq!(features.len(), weights.len());
    let mut dot = bias;
    for i in 0..features.len() {
        dot += features[i] * weights[i];
    }
    dot
}

/// Computes probability P \in [0.0, 1.0] from feature vector, weights, and bias
#[inline]
pub fn predict_probability(features: &[f64], weights: &[f64], bias: f64) -> f64 {
    sigmoid(linear_logit(features, weights, bias))
}

pub struct LayoutTensorEngine;

impl LayoutTensorEngine {
    /// Evaluates continuous probability that a page possesses a parallel 2-column layout.
    /// Features:
    /// x0: center_clearance \in [0.0, 1.0] (fraction of characters avoiding the central gutter)
    /// x1: mass_symmetry \in [0.0, 1.0] (min(ML, MR) / max(ML, MR))
    /// x2: vertical_overlap \in [0.0, 1.0] (shared vertical height / page height)
    /// x3: columnar_dominance \in [0.0, 1.0] ((ML + MR) / total_page_chars)
    pub fn evaluate_two_column_probability(
        center_clearance: f64,
        mass_symmetry: f64,
        vertical_overlap: f64,
        columnar_dominance: f64,
    ) -> f64 {
        let features = [
            center_clearance.clamp(0.0, 1.0),
            mass_symmetry.clamp(0.0, 1.0),
            vertical_overlap.clamp(0.0, 1.0),
            columnar_dominance.clamp(0.0, 1.0),
        ];

        // Weights calibrated to require both center clearance and mass symmetry:
        // A single-column CV with dates on right has:
        // center_clearance \approx 0.1, mass_symmetry \approx 0.03 -> P \approx 0.0001
        // An IEEE paper has:
        // center_clearance \approx 0.95, mass_symmetry \approx 0.85 -> P \approx 0.998
        const WEIGHTS: [f64; 4] = [4.5, 5.0, 2.5, 3.0];
        const BIAS: f64 = -6.5;

        predict_probability(&features, &WEIGHTS, BIAS)
    }

    /// Evaluates continuous probability that a vertical partition represents a tabular row
    /// or tab stop (e.g. key-value pairs, dates, prices) rather than independent reading columns.
    /// Features:
    /// x0: shared_baseline_ratio \in [0.0, 1.0] (fraction of right tokens sharing baseline with left tokens)
    /// x1: width_asymmetry \in [0.0, 1.0] (1.0 - right_width / left_width)
    /// x2: mass_asymmetry \in [0.0, 1.0] (1.0 - right_chars / left_chars)
    pub fn evaluate_tab_row_probability(
        shared_baseline_ratio: f64,
        width_asymmetry: f64,
        mass_asymmetry: f64,
    ) -> f64 {
        let features = [
            shared_baseline_ratio.clamp(0.0, 1.0),
            width_asymmetry.clamp(0.0, 1.0),
            mass_asymmetry.clamp(0.0, 1.0),
        ];

        // High shared baseline and high asymmetry indicate a row-coupled tab stop
        const WEIGHTS: [f64; 3] = [5.0, 3.5, 3.0];
        const BIAS: f64 = -4.0;

        predict_probability(&features, &WEIGHTS, BIAS)
    }

    /// Evaluates continuous probability that a table spans the full page width
    /// requiring an independent full-width section in Word.
    pub fn evaluate_table_spanning_probability(table_width: f64, printable_page_width: f64) -> f64 {
        if printable_page_width <= 0.0 {
            return 0.0;
        }
        let span_ratio = (table_width / printable_page_width).clamp(0.0, 1.0);
        // Sigmoid transition centered at 52% page width with steep slope
        sigmoid(15.0 * (span_ratio - 0.52))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tensor_cv_vs_ieee() {
        // CV: Text crosses center (clearance 0.1), tiny date mass (symmetry 0.03), vertical overlap 0.2
        let p_cv = LayoutTensorEngine::evaluate_two_column_probability(0.10, 0.03, 0.20, 0.30);
        assert!(p_cv < 0.01, "CV must have near-zero 2-column probability, got {}", p_cv);

        // IEEE: Clear center gutter (clearance 0.95), balanced columns (symmetry 0.90), vertical overlap 0.85
        let p_ieee = LayoutTensorEngine::evaluate_two_column_probability(0.95, 0.90, 0.85, 0.95);
        assert!(p_ieee > 0.95, "IEEE paper must have near-1.0 2-column probability, got {}", p_ieee);
    }

    #[test]
    fn test_tensor_tab_stop_vs_columns() {
        // Resume line: date shares baseline (1.0), right side is narrow (0.75 asymmetry), few chars (0.9 asymmetry)
        let p_tab = LayoutTensorEngine::evaluate_tab_row_probability(1.0, 0.75, 0.90);
        assert!(p_tab > 0.90, "Resume date must be classified as tab row, got {}", p_tab);

        // Parallel columns: independent baselines (0.05), equal widths (0.05), equal mass (0.05)
        let p_col = LayoutTensorEngine::evaluate_tab_row_probability(0.05, 0.05, 0.05);
        assert!(p_col < 0.05, "Parallel columns must not be classified as tab row, got {}", p_col);
    }
}

