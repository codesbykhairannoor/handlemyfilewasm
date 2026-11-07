use rstar::{RTree, RTreeObject, AABB, PointDistance};
use crate::models::{Glyph, Word};

#[derive(Debug, Clone, Copy)]
pub struct GlyphSpatialItem<'a> {
    pub idx: usize,
    pub x_min: f64,
    pub y_min: f64,
    pub x_max: f64,
    pub y_max: f64,
    pub glyph: &'a Glyph,
}

impl<'a> RTreeObject for GlyphSpatialItem<'a> {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners([self.x_min, self.y_min], [self.x_max, self.y_max])
    }
}

impl<'a> PointDistance for GlyphSpatialItem<'a> {
    fn distance_2(&self, point: &[f64; 2]) -> f64 {
        let cx = (self.x_min + self.x_max) / 2.0;
        let cy = (self.y_min + self.y_max) / 2.0;
        (cx - point[0]).powi(2) + (cy - point[1]).powi(2)
    }
}

/// Adaptive Spacing Metrics calculated via Docstrum Algorithm (O'Gorman 1993)
#[derive(Debug, Clone, PartialEq)]
pub struct DocstrumMetrics {
    pub d_char: f64,        // Average intra-word character spacing
    pub d_word: f64,        // Space threshold to split words
    pub d_line: f64,        // Line height / leading
    pub dominant_size: f64, // Dominant font size on page
}

impl Default for DocstrumMetrics {
    fn default() -> Self {
        Self {
            d_char: 1.0,
            d_word: 3.5,
            d_line: 14.0,
            dominant_size: 11.0,
        }
    }
}

pub struct DocstrumAnalyzer;

impl DocstrumAnalyzer {
    /// Calculates dynamic spatial thresholds using k-Nearest Neighbors
    pub fn compute_metrics(glyphs: &[Glyph]) -> DocstrumMetrics {
        if glyphs.is_empty() {
            return DocstrumMetrics::default();
        }

        let mut font_sizes: Vec<f64> = glyphs.iter().map(|g| g.style.font_size).collect();
        font_sizes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median_font_size = font_sizes[font_sizes.len() / 2];

        if glyphs.len() < 3 {
            return DocstrumMetrics {
                d_char: (median_font_size * 0.05).max(0.5),
                d_word: (median_font_size * 0.25).max(2.5),
                d_line: (median_font_size * 1.2).max(12.0),
                dominant_size: median_font_size,
            };
        }

        let items: Vec<GlyphSpatialItem> = glyphs
            .iter()
            .enumerate()
            .map(|(idx, g)| GlyphSpatialItem {
                idx,
                x_min: g.bbox.x_min,
                y_min: g.bbox.y_min,
                x_max: g.bbox.x_max,
                y_max: g.bbox.y_max,
                glyph: g,
            })
            .collect();

        let rtree = RTree::bulk_load(items);

        let mut horizontal_distances = Vec::new();
        let mut vertical_distances = Vec::new();

        for (i, g) in glyphs.iter().enumerate() {
            let cx = (g.bbox.x_min + g.bbox.x_max) / 2.0;
            let cy = (g.bbox.y_min + g.bbox.y_max) / 2.0;

            // Query 5 nearest neighbors
            let neighbors = rtree.nearest_neighbor_iter(&[cx, cy]).take(5);

            for n in neighbors {
                if n.idx == i {
                    continue;
                }
                let n_cx = (n.x_min + n.x_max) / 2.0;
                let n_cy = (n.y_min + n.y_max) / 2.0;

                let dx = n_cx - cx;
                let dy = n_cy - cy;
                let angle = (dy.atan2(dx)).to_degrees().abs();

                // Horizontal neighbors (within 30 degrees of horizontal axis)
                if angle <= 30.0 || angle >= 150.0 {
                    let edge_dist = if n_cx > cx {
                        n.x_min - g.bbox.x_max
                    } else {
                        g.bbox.x_min - n.x_max
                    };
                    if edge_dist >= 0.0 && edge_dist <= median_font_size * 2.0 {
                        horizontal_distances.push(edge_dist);
                    }
                }
                // Vertical neighbors (between 60 and 120 degrees)
                else if angle >= 60.0 && angle <= 120.0 {
                    let line_dist = dy.abs();
                    if line_dist >= median_font_size * 0.5 && line_dist <= median_font_size * 3.0 {
                        vertical_distances.push(line_dist);
                    }
                }
            }
        }

        horizontal_distances.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        vertical_distances.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let d_char = if !horizontal_distances.is_empty() {
            horizontal_distances[horizontal_distances.len() / 4]
        } else {
            median_font_size * 0.06
        };

        let d_word = if !horizontal_distances.is_empty() {
            (horizontal_distances[horizontal_distances.len() * 3 / 4])
                .clamp(median_font_size * 0.20, median_font_size * 0.38)
        } else {
            median_font_size * 0.25
        };

        let d_line = if !vertical_distances.is_empty() {
            vertical_distances[vertical_distances.len() / 2]
        } else {
            median_font_size * 1.2
        };

        DocstrumMetrics {
            d_char,
            d_word,
            d_line,
            dominant_size: median_font_size,
        }
    }

    /// Clusters atomic glyphs into coherent Words using Docstrum adaptive thresholds
    pub fn cluster_words(glyphs: &[Glyph], metrics: &DocstrumMetrics) -> Vec<Word> {
        if glyphs.is_empty() {
            return Vec::new();
        }

        // Group glyphs by physical line based on vertical overlap
        let mut sorted_glyphs = glyphs.to_vec();
        // Sort primarily by Y, secondarily by X
        sorted_glyphs.sort_by(|a, b| {
            a.bbox.y_min.partial_cmp(&b.bbox.y_min)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.bbox.x_min.partial_cmp(&b.bbox.x_min).unwrap_or(std::cmp::Ordering::Equal))
        });

        let mut lines_glyphs: Vec<Vec<Glyph>> = Vec::new();

        for g in sorted_glyphs {
            let mut matched_line = false;
            for line in lines_glyphs.iter_mut() {
                if let Some(first) = line.first() {
                    let h = first.bbox.height().max(g.bbox.height()).max(8.0);
                    let overlap_y = first.bbox.y_max.min(g.bbox.y_max) - first.bbox.y_min.max(g.bbox.y_min);
                    if overlap_y > 0.4 * h || (first.bbox.y_min - g.bbox.y_min).abs() <= 3.0 {
                        line.push(g.clone());
                        matched_line = true;
                        break;
                    }
                }
            }
            if !matched_line {
                lines_glyphs.push(vec![g]);
            }
        }

        let mut words = Vec::new();

        for mut line in lines_glyphs {
            // Sort line glyphs strictly from left to right
            line.sort_by(|a, b| a.bbox.x_min.partial_cmp(&b.bbox.x_min).unwrap_or(std::cmp::Ordering::Equal));

            let mut curr_word_glyphs: Vec<Glyph> = Vec::new();

            for g in line {
                // If glyph is an explicit space character: trigger word boundary
                if g.text == " " {
                    if !curr_word_glyphs.is_empty() {
                        words.push(build_word(std::mem::take(&mut curr_word_glyphs)));
                    }
                    continue;
                }

                // Check distance from previous glyph in current word
                if let Some(prev) = curr_word_glyphs.last() {
                    let gap = g.bbox.x_min - prev.bbox.x_max;
                    let font_sz = g.style.font_size.max(prev.style.font_size).max(6.0);
                    // O'Gorman decision boundary between inter-character and inter-word spacing:
                    // bounded dynamically by font size [0.18 em, 0.32 em]
                    let space_thresh = (metrics.d_word * 0.6).clamp(font_sz * 0.18, font_sz * 0.32);

                    if gap >= space_thresh {
                        words.push(build_word(std::mem::take(&mut curr_word_glyphs)));
                    }
                }

                curr_word_glyphs.push(g);
            }

            if !curr_word_glyphs.is_empty() {
                words.push(build_word(curr_word_glyphs));
            }
        }

        words
    }
}

fn build_word(glyphs: Vec<Glyph>) -> Word {
    let mut bbox = glyphs[0].bbox;
    for g in &glyphs {
        bbox.expand(&g.bbox);
    }
    let text: String = glyphs.iter().map(|g| g.text.as_str()).collect();
    let style = glyphs[0].style.clone();

    Word {
        text,
        bbox,
        style,
        glyphs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_docstrum_word_clustering() {
        let style = Style::default();
        let glyphs = vec![
            Glyph { text: "H".to_string(), bbox: Rect::new(10.0, 50.0, 20.0, 62.0), style: style.clone(), mcid: None, is_artifact: false },
            Glyph { text: "i".to_string(), bbox: Rect::new(20.5, 50.0, 25.0, 62.0), style: style.clone(), mcid: None, is_artifact: false },
            Glyph { text: " ".to_string(), bbox: Rect::new(25.5, 50.0, 30.0, 62.0), style: style.clone(), mcid: None, is_artifact: false },
            Glyph { text: "A".to_string(), bbox: Rect::new(31.0, 50.0, 41.0, 62.0), style: style.clone(), mcid: None, is_artifact: false },
            Glyph { text: "I".to_string(), bbox: Rect::new(41.5, 50.0, 46.0, 62.0), style: style.clone(), mcid: None, is_artifact: false },
        ];

        let metrics = DocstrumAnalyzer::compute_metrics(&glyphs);
        let words = DocstrumAnalyzer::cluster_words(&glyphs, &metrics);

        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "Hi");
        assert_eq!(words[1].text, "AI");
    }
}

