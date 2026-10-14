//! Color spaces and color conversions conforming to ISO 32000-1 Section 8.6.
//!
//! Traceability: FR-01.1.4, FR-24.

use tiny_skia::Color;

/// Represents a color in one of the standard PDF color spaces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PdfColor {
    Gray(f32),
    Rgb(f32, f32, f32),
    Cmyk(f32, f32, f32, f32),
}

impl Default for PdfColor {
    fn default() -> Self {
        PdfColor::Gray(0.0) // Black
    }
}

impl PdfColor {
    pub const BLACK: Self = PdfColor::Gray(0.0);
    pub const WHITE: Self = PdfColor::Gray(1.0);

    /// Converts the color to sRGB (r, g, b) components in [0.0, 1.0].
    pub fn to_rgb(&self) -> (f32, f32, f32) {
        match *self {
            PdfColor::Gray(g) => {
                let val = g.clamp(0.0, 1.0);
                (val, val, val)
            }
            PdfColor::Rgb(r, g, b) => (
                r.clamp(0.0, 1.0),
                g.clamp(0.0, 1.0),
                b.clamp(0.0, 1.0),
            ),
            PdfColor::Cmyk(c, m, y, k) => {
                // ISO 32000-1 standard formula for DeviceCMYK to DeviceRGB conversion:
                // R = 1.0 - min(1.0, C + K)
                // G = 1.0 - min(1.0, M + K)
                // B = 1.0 - min(1.0, Y + K)
                let c = c.clamp(0.0, 1.0);
                let m = m.clamp(0.0, 1.0);
                let y = y.clamp(0.0, 1.0);
                let k = k.clamp(0.0, 1.0);

                let r = (1.0 - (c * (1.0 - k) + k)).clamp(0.0, 1.0);
                let g = (1.0 - (m * (1.0 - k) + k)).clamp(0.0, 1.0);
                let b = (1.0 - (y * (1.0 - k) + k)).clamp(0.0, 1.0);

                (r, g, b)
            }
        }
    }

    /// Converts this color into a `tiny_skia::Color` with the given alpha [0.0, 1.0].
    pub fn to_skia_color(&self, alpha: f32) -> Color {
        let (r, g, b) = self.to_rgb();
        Color::from_rgba(r, g, b, alpha.clamp(0.0, 1.0)).unwrap_or(Color::BLACK)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cmyk_conversion() {
        let black = PdfColor::Cmyk(0.0, 0.0, 0.0, 1.0);
        let (r, g, b) = black.to_rgb();
        assert!((r - 0.0).abs() < 1e-4);
        assert!((g - 0.0).abs() < 1e-4);
        assert!((b - 0.0).abs() < 1e-4);

        let white = PdfColor::Cmyk(0.0, 0.0, 0.0, 0.0);
        let (r, g, b) = white.to_rgb();
        assert!((r - 1.0).abs() < 1e-4);
        assert!((g - 1.0).abs() < 1e-4);
        assert!((b - 1.0).abs() < 1e-4);
    }
}
