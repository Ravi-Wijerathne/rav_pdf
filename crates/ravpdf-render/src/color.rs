//! Color spaces and color conversions conforming to ISO 32000-1 Section 8.6.
//!
//! Traceability: FR-01.1.4, FR-24.

use tiny_skia::Color;

/// Represents a color in one of the standard PDF color spaces (ISO 32000-1 Section 8.6).
#[derive(Debug, Clone, PartialEq)]
pub enum PdfColor {
    /// DeviceGray (ISO 32000-1 Section 8.6.4.2)
    Gray(f32),
    /// DeviceRGB (ISO 32000-1 Section 8.6.4.3)
    Rgb(f32, f32, f32),
    /// DeviceCMYK (ISO 32000-1 Section 8.6.4.4)
    Cmyk(f32, f32, f32, f32),
    /// CalGray (ISO 32000-1 Section 8.6.5.2)
    CalGray {
        gray: f32,
        gamma: f32,
        white_point: [f32; 3],
    },
    /// CalRGB (ISO 32000-1 Section 8.6.5.3)
    CalRGB {
        r: f32,
        g: f32,
        b: f32,
        gamma: [f32; 3],
        white_point: [f32; 3],
        matrix: [f32; 9],
    },
    /// CIE-based Lab (ISO 32000-1 Section 8.6.5.4)
    Lab {
        l: f32,
        a: f32,
        b: f32,
        range: [f32; 4],
        white_point: [f32; 3],
    },
    /// Separation color space (ISO 32000-1 Section 8.6.6.4)
    Separation {
        name: String,
        tint: f32,
        alternate_rgb: (f32, f32, f32),
    },
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
        match self {
            PdfColor::Gray(g) => {
                let val = g.clamp(0.0, 1.0);
                (val, val, val)
            }
            PdfColor::Rgb(r, g, b) => (r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0)),
            PdfColor::Cmyk(c, m, y, k) => {
                // ISO 32000-1 standard formula for DeviceCMYK to DeviceRGB conversion:
                let c = c.clamp(0.0, 1.0);
                let m = m.clamp(0.0, 1.0);
                let y = y.clamp(0.0, 1.0);
                let k = k.clamp(0.0, 1.0);

                let r = (1.0 - (c * (1.0 - k) + k)).clamp(0.0, 1.0);
                let g = (1.0 - (m * (1.0 - k) + k)).clamp(0.0, 1.0);
                let b = (1.0 - (y * (1.0 - k) + k)).clamp(0.0, 1.0);

                (r, g, b)
            }
            PdfColor::CalGray {
                gray,
                gamma,
                white_point,
            } => {
                let g = gray.clamp(0.0, 1.0);
                let a = g.powf(*gamma);
                let y = a * white_point[1];
                let val = (y.clamp(0.0, 1.0)).powf(1.0 / 2.2);
                (val, val, val)
            }
            PdfColor::CalRGB {
                r,
                g,
                b,
                gamma,
                white_point,
                matrix,
            } => {
                let ar = r.clamp(0.0, 1.0).powf(gamma[0]);
                let ag = g.clamp(0.0, 1.0).powf(gamma[1]);
                let ab = b.clamp(0.0, 1.0).powf(gamma[2]);

                let x = matrix[0] * ar + matrix[3] * ag + matrix[6] * ab;
                let y = matrix[1] * ar + matrix[4] * ag + matrix[7] * ab;
                let z = matrix[2] * ar + matrix[5] * ag + matrix[8] * ab;

                xyz_to_srgb(x * white_point[0], y * white_point[1], z * white_point[2])
            }
            PdfColor::Lab {
                l,
                a,
                b,
                white_point,
                ..
            } => {
                let l_clamped = l.clamp(0.0, 100.0);
                let fy = (l_clamped + 16.0) / 116.0;
                let fx = fy + (a / 500.0);
                let fz = fy - (b / 200.0);

                let g = |t: f32| -> f32 {
                    let delta = 6.0 / 29.0;
                    if t > delta {
                        t * t * t
                    } else {
                        3.0 * delta * delta * (t - 4.0 / 29.0)
                    }
                };

                let x = white_point[0] * g(fx);
                let y = white_point[1] * g(fy);
                let z = white_point[2] * g(fz);

                xyz_to_srgb(x, y, z)
            }
            PdfColor::Separation {
                name,
                tint,
                alternate_rgb,
            } => {
                let t = tint.clamp(0.0, 1.0);
                if name == "All" {
                    let val = 1.0 - t;
                    (val, val, val)
                } else if name == "None" {
                    (1.0, 1.0, 1.0)
                } else {
                    let r = (1.0 - t * (1.0 - alternate_rgb.0)).clamp(0.0, 1.0);
                    let g = (1.0 - t * (1.0 - alternate_rgb.1)).clamp(0.0, 1.0);
                    let b = (1.0 - t * (1.0 - alternate_rgb.2)).clamp(0.0, 1.0);
                    (r, g, b)
                }
            }
        }
    }

    /// Converts this color into a `tiny_skia::Color` with the given alpha [0.0, 1.0].
    pub fn to_skia_color(&self, alpha: f32) -> Color {
        let (r, g, b) = self.to_rgb();
        Color::from_rgba(r, g, b, alpha.clamp(0.0, 1.0)).unwrap_or(Color::BLACK)
    }
}

/// Converts CIE 1931 XYZ tristimulus values to sRGB.
fn xyz_to_srgb(x: f32, y: f32, z: f32) -> (f32, f32, f32) {
    let r_lin = 3.2404542 * x - 1.5371385 * y - 0.4985314 * z;
    let g_lin = -0.9692660 * x + 1.8760108 * y + 0.0415560 * z;
    let b_lin = 0.0556434 * x - 0.2040259 * y + 1.0572252 * z;

    let gamma_map = |c: f32| -> f32 {
        let c = c.max(0.0);
        if c <= 0.0031308 {
            12.92 * c
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        }
        .clamp(0.0, 1.0)
    };

    (gamma_map(r_lin), gamma_map(g_lin), gamma_map(b_lin))
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

    #[test]
    fn test_cal_rgb_and_lab_conversion() {
        let d65 = [0.95047, 1.0, 1.08883];
        let lab_white = PdfColor::Lab {
            l: 100.0,
            a: 0.0,
            b: 0.0,
            range: [-100.0, 100.0, -100.0, 100.0],
            white_point: d65,
        };
        let (r, g, b) = lab_white.to_rgb();
        assert!(r > 0.95 && g > 0.95 && b > 0.95, "L*=100 Lab should be white");

        let separation = PdfColor::Separation {
            name: "PANTONE 185 C".into(),
            tint: 1.0,
            alternate_rgb: (0.9, 0.1, 0.2),
        };
        let (sr, sg, sb) = separation.to_rgb();
        assert!((sr - 0.9).abs() < 1e-4);
        assert!((sg - 0.1).abs() < 1e-4);
        assert!((sb - 0.2).abs() < 1e-4);
    }
}
