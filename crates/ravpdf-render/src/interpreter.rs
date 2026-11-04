//! PDF Content Stream Interpreter and Rasterizer conforming to ISO 32000-1 Section 8 and 9.
//!
//! Traceability: FR-01.1.1-7, FR-01.3.1-7, DR-02.2, NFR-01.2.

use crate::color::PdfColor;
use ravpdf_core::cmap::FontEncodingInfo;
use ravpdf_core::error::Result;
use ravpdf_core::lexer::{Lexer, Token};
use ravpdf_core::object::{Object, PdfDictionary};
use std::collections::HashMap;
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
    doc: Option<&'a ravpdf_core::document::PdfDocument>,
    state: GraphicsState,
    state_stack: Vec<GraphicsState>,
    path_builder: Option<PathBuilder>,
    current_point: Option<(f32, f32)>,
    in_text_object: bool,
    pub crop_box: [f64; 4],
    pub zoom: f32,
    resources: Option<&'a PdfDictionary>,
    fonts: HashMap<String, FontEncodingInfo>,
}

impl<'a> ContentInterpreter<'a> {
    pub fn new(pixmap: &'a mut Pixmap, crop_box: [f64; 4], zoom: f32) -> Self {
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
            doc: None,
            state: initial_state,
            state_stack: Vec::new(),
            path_builder: None,
            current_point: None,
            in_text_object: false,
            crop_box,
            zoom,
            resources: None,
            fonts: HashMap::new(),
        }
    }

    /// Creates an interpreter with access to the underlying `PdfDocument` for resolving indirect resources.
    pub fn with_doc(
        pixmap: &'a mut Pixmap,
        doc: &'a ravpdf_core::document::PdfDocument,
        crop_box: [f64; 4],
        zoom: f32,
    ) -> Self {
        let mut interp = Self::new(pixmap, crop_box, zoom);
        interp.doc = Some(doc);
        interp
    }

    fn load_fonts(&mut self, resources: Option<&'a PdfDictionary>) {
        if let Some(doc) = self.doc {
            self.fonts = FontEncodingInfo::load_page_fonts(doc, resources);
        }
    }

    fn decode_string_bytes(&self, bytes: &[u8]) -> String {
        let font_key = self
            .state
            .font_name
            .strip_prefix('/')
            .unwrap_or(&self.state.font_name);
        if let Some(info) = self.fonts.get(font_key) {
            info.decode_string(bytes)
        } else {
            FontEncodingInfo::default().decode_string(bytes)
        }
    }

    /// Interprets raw page content stream instructions and rasterizes them onto the Pixmap.
    pub fn interpret(
        &mut self,
        content: &[u8],
        resources: Option<&'a PdfDictionary>,
    ) -> Result<()> {
        self.resources = resources;
        self.load_fonts(resources);
        let mut lexer = Lexer::new(content);
        let mut operands = Vec::new();

        while let Ok(token) = lexer.next_token() {
            match token {
                Token::Eof => break,
                Token::Integer(i) => operands.push(Object::Integer(i)),
                Token::Real(r) => operands.push(Object::Real(r)),
                Token::StringLiteral(s) => operands.push(Object::String(
                    ravpdf_core::object::PdfString::from_bytes(s),
                )),
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
                            Token::StringLiteral(s) => arr.push(Object::String(
                                ravpdf_core::object::PdfString::from_bytes(s),
                            )),
                            Token::HexString(s) => arr
                                .push(Object::String(ravpdf_core::object::PdfString::from_hex(s))),
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
                self.state.text_line_matrix = self.state.text_line_matrix.pre_concat(move_matrix);
                self.state.text_matrix = self.state.text_line_matrix;
            }
            "TL" => {
                if let Some(leading) = args.first().and_then(Object::as_f64) {
                    self.state.text_leading = leading as f32;
                }
            }
            "Tj" => {
                if let Some(str_obj) = args.first() {
                    let bytes = match str_obj {
                        Object::String(s) => s.bytes.as_slice(),
                        _ => &[],
                    };
                    let text = self.decode_string_bytes(bytes);
                    self.draw_text_string(&text);
                }
            }
            "TJ" => {
                if let Some(Object::Array(items)) = args.first() {
                    for item in items {
                        match item {
                            Object::String(s) => {
                                let text = self.decode_string_bytes(&s.bytes);
                                self.draw_text_string(&text);
                            }
                            Object::Real(n) => {
                                let adj = (-*n as f32 / 1000.0) * self.state.font_size;
                                let trans = Transform::from_translate(adj, 0.0);
                                self.state.text_matrix = self.state.text_matrix.pre_concat(trans);
                            }
                            Object::Integer(n) => {
                                let adj = (-*n as f32 / 1000.0) * self.state.font_size;
                                let trans = Transform::from_translate(adj, 0.0);
                                self.state.text_matrix = self.state.text_matrix.pre_concat(trans);
                            }
                            _ => {}
                        }
                    }
                }
            }
            // XObject Invocation (FR-01.1.1, FR-05.1 - FR-05.12)
            "Do" => {
                if let Some(name_obj) = args.first() {
                    if let Some(xobj_name) = name_obj.as_name() {
                        let _ = self.render_xobject(xobj_name);
                    }
                }
            }
            // Extended Graphics State (FR-01.1.5, FR-05.10)
            "gs" => {
                if let Some(name_obj) = args.first() {
                    if let Some(gs_name) = name_obj.as_name() {
                        self.apply_extgstate(gs_name);
                    }
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
                    self.pixmap
                        .fill_path(&path, &paint, rule, self.state.ctm, None);
                }
                if stroke {
                    let mut paint = Paint::default();
                    paint.set_color(
                        self.state
                            .stroke_color
                            .to_skia_color(self.state.stroke_alpha),
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

    fn render_xobject(&mut self, name: &str) -> Result<()> {
        let doc = match self.doc {
            Some(d) => d,
            None => return Ok(()),
        };
        let resources = match self.resources {
            Some(r) => r,
            None => return Ok(()),
        };
        let xobj_dict = match resources.get("XObject") {
            Some(Object::Dictionary(d)) => Some(d),
            Some(Object::Reference(id)) => doc.objects.get(id).and_then(Object::as_dict),
            _ => None,
        };
        let xobj_dict = match xobj_dict {
            Some(d) => d,
            None => return Ok(()),
        };

        let stream = match xobj_dict.get(name) {
            Some(Object::Stream(s)) => Some(s),
            Some(Object::Reference(id)) => doc.objects.get(id).and_then(Object::as_stream),
            _ => None,
        };
        let stream = match stream {
            Some(s) => s,
            None => return Ok(()),
        };

        let subtype = stream.dict.get_name("Subtype").unwrap_or("");
        if subtype == "Image" {
            self.render_image_xobject(stream)?;
        } else if subtype == "Form" {
            self.render_form_xobject(stream)?;
        }

        Ok(())
    }

    fn render_image_xobject(&mut self, stream: &'a ravpdf_core::object::PdfStream) -> Result<()> {
        let width = stream.dict.get_i64("Width").unwrap_or(0).max(1) as u32;
        let height = stream.dict.get_i64("Height").unwrap_or(0).max(1) as u32;
        let color_space = stream.dict.get_name("ColorSpace").unwrap_or("DeviceRGB");

        let is_jpeg = stream.dict.get_name("Filter") == Some("DCTDecode")
            || (stream.raw_data.len() >= 2
                && stream.raw_data[0] == 0xFF
                && stream.raw_data[1] == 0xD8);

        let decoded_bytes = if is_jpeg {
            if let Ok(dyn_img) = image::load_from_memory(&stream.raw_data) {
                let rgba = dyn_img.to_rgba8();
                return self.draw_rgba_image(rgba.width(), rgba.height(), &rgba.into_raw());
            }
            stream.decode().unwrap_or_default()
        } else {
            stream.decode().unwrap_or_default()
        };

        if decoded_bytes.is_empty() {
            return Ok(());
        }

        if let Ok(dyn_img) = image::load_from_memory(&decoded_bytes) {
            let rgba = dyn_img.to_rgba8();
            return self.draw_rgba_image(rgba.width(), rgba.height(), &rgba.into_raw());
        }

        let mut rgba_pixels = Vec::with_capacity((width * height * 4) as usize);
        match color_space {
            "DeviceGray" => {
                for &g in &decoded_bytes {
                    rgba_pixels.extend_from_slice(&[g, g, g, 255]);
                }
            }
            "DeviceCMYK" => {
                for chunk in decoded_bytes.chunks_exact(4) {
                    let (c, m, y, k) = (
                        chunk[0] as f32 / 255.0,
                        chunk[1] as f32 / 255.0,
                        chunk[2] as f32 / 255.0,
                        chunk[3] as f32 / 255.0,
                    );
                    let (r, g, b) = PdfColor::Cmyk(c, m, y, k).to_rgb();
                    rgba_pixels.extend_from_slice(&[
                        (r * 255.0).round() as u8,
                        (g * 255.0).round() as u8,
                        (b * 255.0).round() as u8,
                        255,
                    ]);
                }
            }
            _ => {
                for chunk in decoded_bytes.chunks_exact(3) {
                    rgba_pixels.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
                }
            }
        }

        if rgba_pixels.len() >= (width * height * 4) as usize {
            self.draw_rgba_image(width, height, &rgba_pixels[..(width * height * 4) as usize])?;
        }

        Ok(())
    }

    fn draw_rgba_image(&mut self, img_w: u32, img_h: u32, rgba_bytes: &[u8]) -> Result<()> {
        if img_w == 0 || img_h == 0 {
            return Ok(());
        }

        let mut img_pixmap = match Pixmap::new(img_w, img_h) {
            Some(p) => p,
            None => return Ok(()),
        };

        let pixels = img_pixmap.pixels_mut();
        for (i, chunk) in rgba_bytes.chunks_exact(4).enumerate() {
            if i >= pixels.len() {
                break;
            }
            let (r, g, b, a) = (chunk[0], chunk[1], chunk[2], chunk[3]);
            if let Some(c) = tiny_skia::PremultipliedColorU8::from_rgba(r, g, b, a) {
                pixels[i] = c;
            }
        }

        let img_to_unit = Transform::from_row(
            1.0 / (img_w as f32),
            0.0,
            0.0,
            -1.0 / (img_h as f32),
            0.0,
            1.0,
        );

        let final_transform = self.state.ctm.pre_concat(img_to_unit);
        let paint = tiny_skia::PixmapPaint {
            quality: tiny_skia::FilterQuality::Bicubic,
            opacity: self.state.fill_alpha,
            ..Default::default()
        };

        self.pixmap
            .draw_pixmap(0, 0, img_pixmap.as_ref(), &paint, final_transform, None);

        Ok(())
    }

    fn render_form_xobject(&mut self, stream: &'a ravpdf_core::object::PdfStream) -> Result<()> {
        self.state_stack.push(self.state.clone());

        if let Some(m_arr) = stream.dict.get_array("Matrix") {
            if m_arr.len() >= 6 {
                let a = m_arr[0].as_f64().unwrap_or(1.0) as f32;
                let b = m_arr[1].as_f64().unwrap_or(0.0) as f32;
                let c = m_arr[2].as_f64().unwrap_or(0.0) as f32;
                let d = m_arr[3].as_f64().unwrap_or(1.0) as f32;
                let e = m_arr[4].as_f64().unwrap_or(0.0) as f32;
                let f = m_arr[5].as_f64().unwrap_or(0.0) as f32;
                let matrix = Transform::from_row(a, b, c, d, e, f);
                self.state.ctm = self.state.ctm.pre_concat(matrix);
            }
        }

        let form_content = stream.decode()?;
        let form_res = stream.dict.get_dict("Resources").or(self.resources);
        let _ = self.interpret(&form_content, form_res);

        if let Some(prev) = self.state_stack.pop() {
            self.state = prev;
        }

        Ok(())
    }

    fn apply_extgstate(&mut self, name: &str) {
        let doc = match self.doc {
            Some(d) => d,
            None => return,
        };
        let resources = match self.resources {
            Some(r) => r,
            None => return,
        };
        let extg_dict = match resources.get("ExtGState") {
            Some(Object::Dictionary(d)) => Some(d),
            Some(Object::Reference(id)) => doc.objects.get(id).and_then(Object::as_dict),
            _ => None,
        };
        if let Some(dict) = extg_dict {
            let gs_obj = match dict.get(name) {
                Some(Object::Dictionary(d)) => Some(d),
                Some(Object::Reference(id)) => doc.objects.get(id).and_then(Object::as_dict),
                _ => None,
            };
            if let Some(gs) = gs_obj {
                if let Some(ca) = gs.get_f64("ca") {
                    self.state.fill_alpha = (ca as f32).clamp(0.0, 1.0);
                }
                if let Some(ca_stroke) = gs.get_f64("CA") {
                    self.state.stroke_alpha = (ca_stroke as f32).clamp(0.0, 1.0);
                }
                if let Some(lw) = gs.get_f64("LW") {
                    self.state.line_width = (lw as f32).max(0.1);
                }
            }
        }
    }

    fn draw_text_string(&mut self, text: &str) {
        let font_key = self
            .state
            .font_name
            .strip_prefix('/')
            .unwrap_or(&self.state.font_name);

        let embedded_bytes = self.fonts.get(font_key).and_then(|f| f.font_data.as_deref());

        let fallback_bytes = if self.state.font_name.to_lowercase().contains("courier")
            || self.state.font_name.to_lowercase().contains("mono")
        {
            epaint_default_fonts::HACK_REGULAR
        } else {
            epaint_default_fonts::UBUNTU_LIGHT
        };

        let primary_font = embedded_bytes
            .and_then(|b| ab_glyph::FontRef::try_from_slice(b).ok())
            .or_else(|| ab_glyph::FontRef::try_from_slice(fallback_bytes).ok());

        let fallback_font = ab_glyph::FontRef::try_from_slice(fallback_bytes).ok();
        let emoji_font =
            ab_glyph::FontRef::try_from_slice(epaint_default_fonts::NOTO_EMOJI_REGULAR).ok();

        let font_px_scale = (self.state.font_size * self.zoom).max(4.0);
        let combined = self.state.ctm.pre_concat(self.state.text_matrix);
        let mut pt = tiny_skia::Point::from_xy(0.0, 0.0);
        combined.map_point(&mut pt);
        let (base_x, base_y) = (pt.x, pt.y);

        if let Some(font) = primary_font {
            use ab_glyph::{Font, ScaleFont};
            let scaled_font = font.as_scaled(font_px_scale);
            let mut cur_x = base_x;
            let baseline_y = base_y;

            let fill_col = self.state.fill_color.to_skia_color(self.state.fill_alpha);
            let (r, g, b, a_base) = (
                fill_col.red(),
                fill_col.green(),
                fill_col.blue(),
                fill_col.alpha(),
            );

            let pixmap_w = self.pixmap.width() as i32;
            let pixmap_h = self.pixmap.height() as i32;

            for ch in text.chars() {
                if ch.is_control() && ch != '\t' && ch != '\n' && ch != '\r' {
                    continue;
                }

                let mut gid = scaled_font.glyph_id(ch);
                let mut active_font = &font;

                // If missing in primary font (.notdef), fall back to fallback_font or emoji_font
                if gid.0 == 0 {
                    if let Some(ref fb) = fallback_font {
                        let sfb = fb.as_scaled(font_px_scale);
                        let fb_gid = sfb.glyph_id(ch);
                        if fb_gid.0 != 0 {
                            gid = fb_gid;
                            active_font = fb;
                        }
                    }
                    if gid.0 == 0 {
                        if let Some(ref ef) = emoji_font {
                            let sef = ef.as_scaled(font_px_scale);
                            let ef_gid = sef.glyph_id(ch);
                            if ef_gid.0 != 0 {
                                gid = ef_gid;
                                active_font = ef;
                            }
                        }
                    }
                }

                let active_scaled = active_font.as_scaled(font_px_scale);
                let mut h_advance = active_scaled.h_advance(gid);
                if h_advance <= 0.0 {
                    h_advance = font_px_scale * 0.28;
                }

                let glyph = gid.with_scale_and_position(
                    font_px_scale,
                    ab_glyph::Point {
                        x: cur_x,
                        y: baseline_y - active_scaled.ascent(),
                    },
                );

                if let Some(outlined) = active_font.outline_glyph(glyph) {
                    let bounds = outlined.px_bounds();
                    let pixels = self.pixmap.pixels_mut();

                    outlined.draw(|gx, gy, coverage| {
                        let px = bounds.min.x as i32 + gx as i32;
                        let py = bounds.min.y as i32 + gy as i32;
                        if px >= 0 && px < pixmap_w && py >= 0 && py < pixmap_h {
                            let idx = (py * pixmap_w + px) as usize;
                            let eff_a = coverage * a_base;
                            if eff_a > 0.005 {
                                let existing = pixels[idx];
                                let inv_a = 1.0 - eff_a;
                                let new_r = ((r * eff_a + (existing.red() as f32 / 255.0) * inv_a)
                                    * 255.0)
                                    .round() as u8;
                                let new_g = ((g * eff_a
                                    + (existing.green() as f32 / 255.0) * inv_a)
                                    * 255.0)
                                    .round() as u8;
                                let new_b = ((b * eff_a + (existing.blue() as f32 / 255.0) * inv_a)
                                    * 255.0)
                                    .round() as u8;
                                let new_a = ((eff_a + (existing.alpha() as f32 / 255.0) * inv_a)
                                    * 255.0)
                                    .round() as u8;
                                if let Some(c) = tiny_skia::PremultipliedColorU8::from_rgba(
                                    new_r, new_g, new_b, new_a,
                                ) {
                                    pixels[idx] = c;
                                }
                            }
                        }
                    });
                }

                cur_x += h_advance;
                let user_advance = h_advance / self.zoom;
                let adv_m = Transform::from_translate(user_advance, 0.0);
                self.state.text_matrix = self.state.text_matrix.pre_concat(adv_m);
            }
        } else {
            let char_width = self.state.font_size * 0.55;
            for _ in text.chars() {
                let advance = Transform::from_translate(char_width, 0.0);
                self.state.text_matrix = self.state.text_matrix.pre_concat(advance);
            }
        }
    }
}
