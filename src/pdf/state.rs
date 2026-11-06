use crate::models::Matrix2D;

/// Graphics State in PDF Content Stream
#[derive(Debug, Clone, PartialEq)]
pub struct GraphicsState {
    pub ctm: Matrix2D,
    pub line_width: f64,
    pub stroke_color: String,
    pub fill_color: String,
}

impl Default for GraphicsState {
    fn default() -> Self {
        Self {
            ctm: Matrix2D::identity(),
            line_width: 1.0,
            stroke_color: "000000".to_string(),
            fill_color: "000000".to_string(),
        }
    }
}

/// Text State according to ISO 32000-1 §9.3
#[derive(Debug, Clone, PartialEq)]
pub struct TextState {
    pub font_name: String,
    pub font_size: f64,
    pub char_spacing: f64,        // Tc
    pub word_spacing: f64,        // Tw
    pub horizontal_scaling: f64,  // Tz (percentage, default 100.0)
    pub leading: f64,             // TL
    pub text_rise: f64,           // Ts
    pub text_render_mode: i32,    // Tr (0: Fill, 1: Stroke, 2: Fill+Stroke, 3: Invisible)
    pub tm: Matrix2D,             // Current Text Matrix
    pub tlm: Matrix2D,            // Current Text Line Matrix
}

impl Default for TextState {
    fn default() -> Self {
        Self {
            font_name: "Arial".to_string(),
            font_size: 12.0,
            char_spacing: 0.0,
            word_spacing: 0.0,
            horizontal_scaling: 100.0,
            leading: 0.0,
            text_rise: 0.0,
            text_render_mode: 0,
            tm: Matrix2D::identity(),
            tlm: Matrix2D::identity(),
        }
    }
}

impl TextState {
    /// Resets Tm and Tlm to identity on `BT` operator
    pub fn begin_text(&mut self) {
        self.tm = Matrix2D::identity();
        self.tlm = Matrix2D::identity();
    }

    /// Sets explicit text matrix via `a b c d e f Tm`
    pub fn set_matrix(&mut self, m: Matrix2D) {
        self.tm = m;
        self.tlm = m;
    }

    /// Displaces line matrix via `tx ty Td`
    pub fn move_line(&mut self, tx: f64, ty: f64) {
        let displacement = Matrix2D::new(1.0, 0.0, 0.0, 1.0, tx, ty);
        self.tlm = displacement.multiply(&self.tlm);
        self.tm = self.tlm;
    }

    /// Displaces line matrix and sets leading via `tx ty TD`
    pub fn move_line_set_leading(&mut self, tx: f64, ty: f64) {
        self.leading = -ty;
        self.move_line(tx, ty);
    }

    /// Moves to start of next line via `T*`
    pub fn next_line(&mut self) {
        let l = self.leading;
        self.move_line(0.0, -l);
    }

    /// Advances the text matrix horizontally by delta_x
    pub fn advance(&mut self, delta_x: f64) {
        let shift = Matrix2D::new(1.0, 0.0, 0.0, 1.0, delta_x, 0.0);
        self.tm = shift.multiply(&self.tm);
    }

    /// Calculates the effective text rendering matrix Trm = Tstyle * Tm * CTM
    pub fn get_rendering_matrix(&self, ctm: &Matrix2D) -> Matrix2D {
        let t_style = Matrix2D::new(
            self.font_size * (self.horizontal_scaling / 100.0),
            0.0,
            0.0,
            self.font_size,
            0.0,
            self.text_rise,
        );
        t_style.multiply(&self.tm).multiply(ctm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_state_matrix_advance() {
        let mut state = TextState::default();
        state.set_matrix(Matrix2D::new(10.0, 0.0, 0.0, 10.0, 100.0, 200.0));
        assert_eq!(state.tm.e, 100.0);
        assert_eq!(state.tm.f, 200.0);

        state.advance(50.0);
        assert_eq!(state.tm.e, 150.0);
    }

    #[test]
    fn test_text_displacement() {
        let mut state = TextState::default();
        state.set_matrix(Matrix2D::new(1.0, 0.0, 0.0, 1.0, 50.0, 500.0));
        state.move_line(10.0, -20.0);
        assert_eq!(state.tm.e, 60.0);
        assert_eq!(state.tm.f, 480.0);
    }
}

