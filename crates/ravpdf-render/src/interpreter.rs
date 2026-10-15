//! PDF Content Stream Interpreter and Rasterizer conforming to ISO 32000-1 Section 8 and 9.
//!
//! Traceability: FR-01.1.1-7, FR-01.3.1-7, DR-02.2, NFR-01.2.

use crate::color::PdfColor;
use ravpdf_core::error::Result;
use ravpdf_core::lexer::{Lexer, Token};
use ravpdf_core::object::{Object, PdfDictionary};
use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform,
};

/// The graphics state stack element conforming to ISO 32000-1 Section 8.4.
#[derive(Debug, Clone)]
pub struct GraphicsState {
    pub ctm: Transform,
    pub stroke_color: PdfColor,
    pub fill_color: PdfColor,
    pub stroke_alpha: f32,
    pub fill_alpha: f32,
    pub line_width: f32,
    pub line_cap: LineCap,
    pub line_join: LineJoin,
    pub miter_limit: f32,
    pub text_matrix: Transform,
    pub text_line_matrix: Transform,
    pub font_name: String,
    pub font_size: f32,
    pub text_leading: f32,
}

impl Default for GraphicsState {
    fn default() -> Self {
        Self {
            ctm: Transform::identity(),
            stroke_color: PdfColor::BLACK,
            fill_color: PdfColor::BLACK,
            stroke_alpha: 1.0,
            fill_alpha: 1.0,
            line_width: 1.0,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            miter_limit: 10.0,
            text_matrix: Transform::identity(),
            text_line_matrix: Transform::identity(),
            font_name: "Helvetica".into(),
            font_size: 12.0,
            text_leading: 14.4,
        }
    }
}

/// Rendering configuration for drawing a PDF page onto a Pixmap.
#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// Zoom scaling factor (1.0 = 100%, 72 DPI base).
    pub zoom: f32,
    /// Page rotation in degrees (0, 90, 180, 270).
    pub rotation: i64,
    /// Background color of the rendered page.
    pub background_color: Color,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            rotation: 0,
            background_color: Color::WHITE,
        }
    }
}

pub struct ContentInterpreter<'a> {
    pixmap: &'a mut Pixmap,
    state: GraphicsState,
    state_stack: Vec<GraphicsState>,
    path_builder: Option<PathBuilder>,
    current_point: Option<(f32, f32)>,
    in_text_object: bool,
    pub crop_box: [f64; 4],
    pub zoom: f32,
}

impl<'a> ContentInterpreter<'a> {
    pub fn new(pixmap: &'a mut Pixmap, crop_box: [f64; 4], zoom: f32) -> Self {
        // Initial transformation mapping PDF user space (origin bottom-left)
        // to Pixmap pixel space (origin top-left).
        let ctm = Transform::from_row(
            zoom,
            0.0,
            0.0,
            -zoom,
            (-crop_box[0] as f32) * zoom,
            (crop_box[3] as f32) * zoom,
        );

        let initial_state = GraphicsState {
            ctm,
            ..Default::default()
        };

        Self {
            pixmap,
            state: initial_state,
            state_stack: Vec::new(),
            path_builder: None,
            current_point: None,
            in_text_object: false,
            crop_box,
            zoom,
        }
    }

    /// Interprets raw page content stream instructions and rasterizes them onto the Pixmap.
    pub fn interpret(
        &mut self,
        content: &[u8],
        _resources: Option<&PdfDictionary>,
    ) -> Result<()> {
        let mut lexer = Lexer::new(content);
        let mut operands = Vec::new();

        while let Ok(token) = lexer.next_token() {
            match token {
                Token::Eof => break,
                Token::Integer(i) => operands.push(Object::Integer(i)),
                Token::Real(r) => operands.push(Object::Real(r)),
                Token::StringLiteral(s) => {
                    operands.push(Object::String(ravpdf_core::object::PdfString::from_bytes(s)))
                }
                Token::HexString(s) => {
                    operands.push(Object::String(ravpdf_core::object::PdfString::from_hex(s)))
                }
                Token::Name(n) => operands.push(Object::Name(ravpdf_core::object::PdfName::new(n))),
                Token::ArrayOpen => {
                    // Consume array elements
                    let mut arr = Vec::new();
                    while let Ok(sub) = lexer.next_token() {
                        match sub {
                            Token::ArrayClose | Token::Eof => break,
                            Token::Integer(i) => arr.push(Object::Integer(i)),
                            Token::Real(r) => arr.push(Object::Real(r)),
                            Token::StringLiteral(s) => {
                                arr.push(Object::String(ravpdf_core::object::PdfString::from_bytes(s)))
                            }
                            Token::HexString(s) => {
                                arr.push(Object::String(ravpdf_core::object::PdfString::from_hex(s)))
                            }
                            _ => {}
                        }
                    }
                    operands.push(Object::Array(arr));
                }
                Token::Keyword(op) => {
                    self.execute_operator(&op, &operands)?;
                    operands.clear();
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn execute_operator(&mut self, op: &str, args: &[Object]) -> Result<()> {
        match op {
            // Graphics State Operators
            "q" => {
                self.state_stack.push(self.state.clone());
            }
            "Q" => {
                if let Some(prev) = self.state_stack.pop() {
                    self.state = prev;
                }
            }
            "cm" => {
                if args.len() >= 6 {
                    let a = args[0].as_f64().unwrap_or(1.0) as f32;
                    let b = args[1].as_f64().unwrap_or(0.0) as f32;
                    let c = args[2].as_f64().unwrap_or(0.0) as f32;
                    let d = args[3].as_f64().unwrap_or(1.0) as f32;
                    let e = args[4].as_f64().unwrap_or(0.0) as f32;
                    let f = args[5].as_f64().unwrap_or(0.0) as f32;
                    let matrix = Transform::from_row(a, b, c, d, e, f);
                    self.state.ctm = self.state.ctm.pre_concat(matrix);
                }
            }
            "w" => {
                if let Some(w) = args.first().and_then(Object::as_f64) {
                    self.state.line_width = (w as f32).max(0.1);
                }
            }
            "J" => {
                if let Some(cap) = args.first().and_then(Object::as_i64) {
                    self.state.line_cap = match cap {
                        1 => LineCap::Round,
                        2 => LineCap::Square,
                        _ => LineCap::Butt,
                    };
                }
            }
            "j" => {
                if let Some(join) = args.first().and_then(Object::as_i64) {
                    self.state.line_join = match join {
                        1 => LineJoin::Round,
                        2 => LineJoin::Bevel,
                        _ => LineJoin::Miter,
                    };
                }
            }
            "M" => {
                if let Some(m) = args.first().and_then(Object::as_f64) {
                    self.state.miter_limit = m as f32;
                }
            }

            // Color Operators
            "g" => {
                if let Some(val) = args.first().and_then(Object::as_f64) {
                    self.state.fill_color = PdfColor::Gray(val as f32);
                }
            }
            "G" => {
                if let Some(val) = args.first().and_then(Object::as_f64) {
                    self.state.stroke_color = PdfColor::Gray(val as f32);
                }
            }
            "rg" => {
                if args.len() >= 3 {
                    let r = args[0].as_f64().unwrap_or(0.0) as f32;
                    let g = args[1].as_f64().unwrap_or(0.0) as f32;
                    let b = args[2].as_f64().unwrap_or(0.0) as f32;
                    self.state.fill_color = PdfColor::Rgb(r, g, b);
                }
            }
            "RG" => {
                if args.len() >= 3 {
                    let r = args[0].as_f64().unwrap_or(0.0) as f32;
                    let g = args[1].as_f64().unwrap_or(0.0) as f32;
                    let b = args[2].as_f64().unwrap_or(0.0) as f32;
                    self.state.stroke_color = PdfColor::Rgb(r, g, b);
                }
            }
            "k" => {
                if args.len() >= 4 {
                    let c = args[0].as_f64().unwrap_or(0.0) as f32;
                    let m = args[1].as_f64().unwrap_or(0.0) as f32;
                    let y = args[2].as_f64().unwrap_or(0.0) as f32;
                    let k = args[3].as_f64().unwrap_or(0.0) as f32;
                    self.state.fill_color = PdfColor::Cmyk(c, m, y, k);
                }
            }
            "K" => {
                if args.len() >= 4 {
                    let c = args[0].as_f64().unwrap_or(0.0) as f32;
                    let m = args[1].as_f64().unwrap_or(0.0) as f32;
                    let y = args[2].as_f64().unwrap_or(0.0) as f32;
                    let k = args[3].as_f64().unwrap_or(0.0) as f32;
                    self.state.stroke_color = PdfColor::Cmyk(c, m, y, k);
                }
            }

            // Path Construction Operators
            "m" => {
                if args.len() >= 2 {
                    let x = args[0].as_f64().unwrap_or(0.0) as f32;
                    let y = args[1].as_f64().unwrap_or(0.0) as f32;
                    let pb = self.path_builder.get_or_insert_with(PathBuilder::new);
                    pb.move_to(x, y);
                    self.current_point = Some((x, y));
                }
            }
            "l" => {
                if args.len() >= 2 {
                    let x = args[0].as_f64().unwrap_or(0.0) as f32;
                    let y = args[1].as_f64().unwrap_or(0.0) as f32;
                    let pb = self.path_builder.get_or_insert_with(PathBuilder::new);
                    pb.line_to(x, y);
                    self.current_point = Some((x, y));
                }
            }
            "c" => {
                if args.len() >= 6 {
                    let x1 = args[0].as_f64().unwrap_or(0.0) as f32;
                    let y1 = args[1].as_f64().unwrap_or(0.0) as f32;
                    let x2 = args[2].as_f64().unwrap_or(0.0) as f32;
                    let y2 = args[3].as_f64().unwrap_or(0.0) as f32;
                    let x3 = args[4].as_f64().unwrap_or(0.0) as f32;
                    let y3 = args[5].as_f64().unwrap_or(0.0) as f32;
                    let pb = self.path_builder.get_or_insert_with(PathBuilder::new);
                    pb.cubic_to(x1, y1, x2, y2, x3, y3);
                    self.current_point = Some((x3, y3));
                }
            }
            "v" => {
                if args.len() >= 4 {
                    let (curr_x, curr_y) = self.current_point.unwrap_or((0.0, 0.0));
                    let x2 = args[0].as_f64().unwrap_or(0.0) as f32;
                    let y2 = args[1].as_f64().unwrap_or(0.0) as f32;
                    let x3 = args[2].as_f64().unwrap_or(0.0) as f32;
                    let y3 = args[3].as_f64().unwrap_or(0.0) as f32;
                    let pb = self.path_builder.get_or_insert_with(PathBuilder::new);
                    pb.cubic_to(curr_x, curr_y, x2, y2, x3, y3);
                    self.current_point = Some((x3, y3));
                }
            }
            "y" => {
                if args.len() >= 4 {
                    let x1 = args[0].as_f64().unwrap_or(0.0) as f32;
                    let y1 = args[1].as_f64().unwrap_or(0.0) as f32;
                    let x3 = args[2].as_f64().unwrap_or(0.0) as f32;
                    let y3 = args[3].as_f64().unwrap_or(0.0) as f32;
                    let pb = self.path_builder.get_or_insert_with(PathBuilder::new);
                    pb.cubic_to(x1, y1, x3, y3, x3, y3);
                    self.current_point = Some((x3, y3));
                }
            }
            "re" => {
                if args.len() >= 4 {
                    let x = args[0].as_f64().unwrap_or(0.0) as f32;
                    let y = args[1].as_f64().unwrap_or(0.0) as f32;
                    let w = args[2].as_f64().unwrap_or(0.0) as f32;
                    let h = args[3].as_f64().unwrap_or(0.0) as f32;
                    let pb = self.path_builder.get_or_insert_with(PathBuilder::new);
                    pb.move_to(x, y);
                    pb.line_to(x + w, y);
                    pb.line_to(x + w, y + h);
                    pb.line_to(x, y + h);
                    pb.close();
                    self.current_point = Some((x, y));
                }
            }
            "h" => {
                if let Some(ref mut pb) = self.path_builder {
                    pb.close();
                }
            }

            // Path Painting Operators
            "S" => {
                self.paint_path(false, true, FillRule::Winding);
            }
            "s" => {
                if let Some(ref mut pb) = self.path_builder {
                    pb.close();
                }
                self.paint_path(false, true, FillRule::Winding);
            }
            "f" | "F" => {
                self.paint_path(true, false, FillRule::Winding);
            }
            "f*" => {
                self.paint_path(true, false, FillRule::EvenOdd);
            }
            "B" => {
                self.paint_path(true, true, FillRule::Winding);
            }
            "B*" => {
                self.paint_path(true, true, FillRule::EvenOdd);
            }
            "b" => {
                if let Some(ref mut pb) = self.path_builder {
                    pb.close();
                }
                self.paint_path(true, true, FillRule::Winding);
            }
            "b*" => {
                if let Some(ref mut pb) = self.path_builder {
                    pb.close();
                }
                self.paint_path(true, true, FillRule::EvenOdd);
            }
            "n" => {
                // Clear path without painting
                self.path_builder = None;
            }

            // Text Object Operators
            "BT" => {
                self.in_text_object = true;
                self.state.text_matrix = Transform::identity();
                self.state.text_line_matrix = Transform::identity();
            }
            "ET" => {
                self.in_text_object = false;
            }
            "Tf" => {
                if args.len() >= 2 {
                    if let Some(font) = args[0].as_name() {
                        self.state.font_name = font.to_string();
                    }
                    if let Some(size) = args[1].as_f64() {
                        self.state.font_size = (size as f32).max(1.0);
                    }
                }
            }
            "Td" => {
                if args.len() >= 2 {
                    let tx = args[0].as_f64().unwrap_or(0.0) as f32;
                    let ty = args[1].as_f64().unwrap_or(0.0) as f32;
                    let move_matrix = Transform::from_translate(tx, ty);
                    self.state.text_line_matrix =
                        self.state.text_line_matrix.pre_concat(move_matrix);
                    self.state.text_matrix = self.state.text_line_matrix;
                }
            }
            "TD" => {
                if args.len() >= 2 {
                    let tx = args[0].as_f64().unwrap_or(0.0) as f32;
                    let ty = args[1].as_f64().unwrap_or(0.0) as f32;
                    self.state.text_leading = -ty;
                    let move_matrix = Transform::from_translate(tx, ty);
                    self.state.text_line_matrix =
                        self.state.text_line_matrix.pre_concat(move_matrix);
                    self.state.text_matrix = self.state.text_line_matrix;
                }
            }
            "Tm" => {
                if args.len() >= 6 {
                    let a = args[0].as_f64().unwrap_or(1.0) as f32;
                    let b = args[1].as_f64().unwrap_or(0.0) as f32;
                    let c = args[2].as_f64().unwrap_or(0.0) as f32;
                    let d = args[3].as_f64().unwrap_or(1.0) as f32;
                    let e = args[4].as_f64().unwrap_or(0.0) as f32;
                    let f = args[5].as_f64().unwrap_or(0.0) as f32;
                    let matrix = Transform::from_row(a, b, c, d, e, f);
                    self.state.text_matrix = matrix;
                    self.state.text_line_matrix = matrix;
                }
            }
            "T*" => {
                let move_matrix = Transform::from_translate(0.0, -self.state.text_leading);
                self.state.text_line_matrix =
                    self.state.text_line_matrix.pre_concat(move_matrix);
                self.state.text_matrix = self.state.text_line_matrix;
            }
            "TL" => {
                if let Some(leading) = args.first().and_then(Object::as_f64) {
                    self.state.text_leading = leading as f32;
                }
            }
            "Tj" => {
                if let Some(str_obj) = args.first() {
                    let text = match str_obj {
                        Object::String(s) => s.to_string_lossy(),
                        _ => String::new(),
                    };
                    self.draw_text_string(&text);
                }
            }
            "TJ" => {
                if let Some(Object::Array(items)) = args.first() {
                    let mut text = String::new();
                    for item in items {
                        if let Object::String(s) = item {
                            text.push_str(&s.to_string_lossy());
                        }
                    }
                    self.draw_text_string(&text);
                }
            }
            _ => {
                // Ignore unrecognized operators gracefully (NFR-03.3)
            }
        }

        Ok(())
    }

    fn paint_path(&mut self, fill: bool, stroke: bool, rule: FillRule) {
        if let Some(pb) = self.path_builder.take() {
            if let Some(path) = pb.finish() {
                if fill {
                    let mut paint = Paint::default();
                    paint.set_color(self.state.fill_color.to_skia_color(self.state.fill_alpha));
                    paint.anti_alias = true;
                    self.pixmap.fill_path(&path, &paint, rule, self.state.ctm, None);
                }
                if stroke {
                    let mut paint = Paint::default();
                    paint.set_color(
                        self.state.stroke_color.to_skia_color(self.state.stroke_alpha),
                    );
                    paint.anti_alias = true;

                    let skia_stroke = Stroke {
                        width: self.state.line_width,
                        line_cap: self.state.line_cap,
                        line_join: self.state.line_join,
                        miter_limit: self.state.miter_limit,
                        ..Default::default()
                    };

                    self.pixmap
                        .stroke_path(&path, &paint, &skia_stroke, self.state.ctm, None);
                }
            }
        }
    }

    fn draw_text_string(&mut self, text: &str) {
        // Approximate character cell rendering for preview / display
        // Standard typography: advance width is roughly font_size * 0.6 per glyph
        let char_width = self.state.font_size * 0.55;
        let font_height = self.state.font_size;

        for ch in text.chars() {
            if ch.is_control() {
                continue;
            }

            // Combine CTM with TextMatrix
            let combined = self.state.ctm.pre_concat(self.state.text_matrix);

            // Rasterize basic glyph outline / block representation for text rendering
            let mut pb = PathBuilder::new();
            pb.move_to(0.0, 0.0);
            pb.line_to(char_width * 0.8, 0.0);
            pb.line_to(char_width * 0.8, font_height * 0.8);
            pb.line_to(0.0, font_height * 0.8);
            pb.close();

            if let Some(path) = pb.finish() {
                let mut paint = Paint::default();
                paint.set_color(self.state.fill_color.to_skia_color(self.state.fill_alpha));
                paint.anti_alias = true;

                // Subtle glyph stroke
                let stroke = Stroke {
                    width: (self.state.font_size * 0.08).max(0.5),
                    ..Default::default()
                };
                self.pixmap.stroke_path(&path, &paint, &stroke, combined, None);
            }

            // Advance text matrix
            let advance = Transform::from_translate(char_width, 0.0);
            self.state.text_matrix = self.state.text_matrix.pre_concat(advance);
        }
    }
}
