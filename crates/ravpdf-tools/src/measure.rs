//! Measurement and Geospatial Tools conforming to ISO 32000-1 Clause 12.5.6.14 & Clause 12.11.
//!
//! Traceability:
//! - FR-26.1 (Distance measurement tool: point-to-point linear distance)
//! - FR-26.2 (Perimeter measurement tool: cumulative length of connected line segments)
//! - FR-26.3 (Area measurement tool: surface area of a closed polygon)
//! - FR-26.4 (Configurable measurement units: inches, cm, mm, pt, pc, m, ft, yards, and area units)
//! - FR-26.5 (Custom scale ratio and reference calibration e.g. 1 in = 10 ft, 1:100)
//! - FR-26.6 (Snapping to endpoints, midpoints, and intersections)
//! - FR-26.7 (Reading and interpreting geospatial coordinate data / viewports)
//! - FR-26.8 (Display real-time cursor position in geographic coordinates)
//! - FR-26.9 (Calculate true geographic distances and areas using embedded coordinate reference system)

use anyhow::{anyhow, Result};
use ravpdf_core::document::PdfDocument;
use ravpdf_core::object::{Object, ObjectId, PdfDictionary, PdfName, PdfString};
use serde::{Deserialize, Serialize};

/// Standard linear measurement units conforming to FR-26.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinearUnit {
    Points,
    Inches,
    Centimeters,
    Millimeters,
    Picas,
    Meters,
    Feet,
    Yards,
}

impl LinearUnit {
    /// Conversion factor: How many PDF points (1/72 inch) are in 1 unit.
    pub fn points_per_unit(&self) -> f64 {
        match self {
            LinearUnit::Points => 1.0,
            LinearUnit::Inches => 72.0,
            LinearUnit::Centimeters => 72.0 / 2.54,
            LinearUnit::Millimeters => 7.2 / 2.54,
            LinearUnit::Picas => 12.0,
            LinearUnit::Meters => 7200.0 / 2.54,
            LinearUnit::Feet => 72.0 * 12.0,       // 864.0 pt
            LinearUnit::Yards => 72.0 * 36.0,      // 2592.0 pt
        }
    }

    pub fn abbreviation(&self) -> &'static str {
        match self {
            LinearUnit::Points => "pt",
            LinearUnit::Inches => "in",
            LinearUnit::Centimeters => "cm",
            LinearUnit::Millimeters => "mm",
            LinearUnit::Picas => "pc",
            LinearUnit::Meters => "m",
            LinearUnit::Feet => "ft",
            LinearUnit::Yards => "yd",
        }
    }

    pub fn parse_str(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "pt" | "point" | "points" => Some(LinearUnit::Points),
            "in" | "inch" | "inches" | "\"" => Some(LinearUnit::Inches),
            "cm" | "centimeter" | "centimeters" => Some(LinearUnit::Centimeters),
            "mm" | "millimeter" | "millimeters" => Some(LinearUnit::Millimeters),
            "pc" | "pica" | "picas" => Some(LinearUnit::Picas),
            "m" | "meter" | "meters" => Some(LinearUnit::Meters),
            "ft" | "feet" | "foot" | "'" => Some(LinearUnit::Feet),
            "yd" | "yard" | "yards" => Some(LinearUnit::Yards),
            _ => None,
        }
    }
}

/// Standard area measurement units conforming to FR-26.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AreaUnit {
    SquarePoints,
    SquareInches,
    SquareCentimeters,
    SquareMillimeters,
    SquareMeters,
    SquareFeet,
    SquareYards,
    Acres,
    Hectares,
}

impl AreaUnit {
    /// Conversion factor: How many square points are in 1 area unit.
    pub fn sq_points_per_unit(&self) -> f64 {
        match self {
            AreaUnit::SquarePoints => 1.0,
            AreaUnit::SquareInches => 72.0 * 72.0,
            AreaUnit::SquareCentimeters => (72.0 / 2.54) * (72.0 / 2.54),
            AreaUnit::SquareMillimeters => (7.2 / 2.54) * (7.2 / 2.54),
            AreaUnit::SquareMeters => (7200.0 / 2.54) * (7200.0 / 2.54),
            AreaUnit::SquareFeet => (72.0 * 12.0) * (72.0 * 12.0),
            AreaUnit::SquareYards => (72.0 * 36.0) * (72.0 * 36.0),
            AreaUnit::Acres => 43560.0 * (72.0 * 12.0) * (72.0 * 12.0),
            AreaUnit::Hectares => 10000.0 * (7200.0 / 2.54) * (7200.0 / 2.54),
        }
    }

    pub fn abbreviation(&self) -> &'static str {
        match self {
            AreaUnit::SquarePoints => "sq pt",
            AreaUnit::SquareInches => "sq in",
            AreaUnit::SquareCentimeters => "sq cm",
            AreaUnit::SquareMillimeters => "sq mm",
            AreaUnit::SquareMeters => "sq m",
            AreaUnit::SquareFeet => "sq ft",
            AreaUnit::SquareYards => "sq yd",
            AreaUnit::Acres => "ac",
            AreaUnit::Hectares => "ha",
        }
    }
}

/// Configurable scale ratio conforming to FR-26.5 (e.g. 1 in = 10 ft, 1:100).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScaleRatio {
    pub drawing_value: f64,
    pub drawing_unit: LinearUnit,
    pub world_value: f64,
    pub world_unit: LinearUnit,
}

impl Default for ScaleRatio {
    fn default() -> Self {
        // Default 1:1 (1 inch on paper = 1 inch in reality)
        Self {
            drawing_value: 1.0,
            drawing_unit: LinearUnit::Inches,
            world_value: 1.0,
            world_unit: LinearUnit::Inches,
        }
    }
}

impl ScaleRatio {
    pub fn new(drawing_value: f64, drawing_unit: LinearUnit, world_value: f64, world_unit: LinearUnit) -> Self {
        Self {
            drawing_value: if drawing_value.abs() < 1e-6 { 1.0 } else { drawing_value },
            drawing_unit,
            world_value: if world_value.abs() < 1e-6 { 1.0 } else { world_value },
            world_unit,
        }
    }

    /// Creates a dimensionless scale ratio e.g. 1:100 or 1:50.
    pub fn dimensionless(world_multiplier: f64) -> Self {
        Self {
            drawing_value: 1.0,
            drawing_unit: LinearUnit::Points,
            world_value: world_multiplier,
            world_unit: LinearUnit::Points,
        }
    }

    /// Architectural scale preset: 1 in = N ft.
    pub fn architectural_inch_to_feet(feet: f64) -> Self {
        Self {
            drawing_value: 1.0,
            drawing_unit: LinearUnit::Inches,
            world_value: feet,
            world_unit: LinearUnit::Feet,
        }
    }

    /// Calibrates scale from a measured distance in PDF points to a known real-world dimension (FR-26.5).
    pub fn calibrate_from_points(measured_points: f64, real_value: f64, real_unit: LinearUnit) -> Self {
        let drawing_inches = measured_points / 72.0;
        Self {
            drawing_value: drawing_inches,
            drawing_unit: LinearUnit::Inches,
            world_value: real_value,
            world_unit: real_unit,
        }
    }

    /// Converts a distance in PDF user points (1/72 inch) to target world units.
    pub fn points_to_world(&self, points: f64, target_unit: LinearUnit) -> f64 {
        // 1. Convert points to drawing units
        let drawing_val = points / self.drawing_unit.points_per_unit();
        // 2. Multiply by scale ratio (world_value / drawing_value)
        let world_base_val = drawing_val * (self.world_value / self.drawing_value);
        // 3. Convert world_base_val (which is in self.world_unit) to target_unit
        let world_in_points = world_base_val * self.world_unit.points_per_unit();
        world_in_points / target_unit.points_per_unit()
    }

    /// Converts an area in square PDF user points to target world area units.
    pub fn sq_points_to_world_area(&self, sq_points: f64, target_unit: AreaUnit) -> f64 {
        // Linear scale factor in points:
        let one_pt_to_world_pts = (self.world_value * self.world_unit.points_per_unit())
            / (self.drawing_value * self.drawing_unit.points_per_unit());
        let area_scale_factor = one_pt_to_world_pts * one_pt_to_world_pts;
        let world_sq_points = sq_points * area_scale_factor;
        world_sq_points / target_unit.sq_points_per_unit()
    }

    /// Returns a human-readable representation (e.g. `1 in = 10 ft` or `1:100`).
    pub fn to_display_string(&self) -> String {
        format!(
            "{:.2} {} = {:.2} {}",
            self.drawing_value,
            self.drawing_unit.abbreviation(),
            self.world_value,
            self.world_unit.abbreviation()
        )
    }
}

/// Point-to-point linear distance measurement result conforming to FR-26.1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DistanceResult {
    pub start: [f64; 2],
    pub end: [f64; 2],
    pub points_distance: f64,
    pub world_distance: f64,
    pub unit: LinearUnit,
    pub delta_x: f64,
    pub delta_y: f64,
    pub angle_degrees: f64,
    pub formatted: String,
}

/// Calculates point-to-point linear distance between two points (FR-26.1).
pub fn calculate_distance(
    start: [f64; 2],
    end: [f64; 2],
    scale: &ScaleRatio,
    unit: LinearUnit,
) -> DistanceResult {
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let pt_dist = (dx * dx + dy * dy).sqrt();
    let world_dist = scale.points_to_world(pt_dist, unit);
    let angle_rad = dy.atan2(dx);
    let mut angle_deg = angle_rad.to_degrees();
    if angle_deg < 0.0 {
        angle_deg += 360.0;
    }

    let formatted = format!("{:.2} {}", world_dist, unit.abbreviation());

    DistanceResult {
        start,
        end,
        points_distance: pt_dist,
        world_distance: world_dist,
        unit,
        delta_x: dx,
        delta_y: dy,
        angle_degrees: angle_deg,
        formatted,
    }
}

/// Perimeter measurement result for connected line segments conforming to FR-26.2.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerimeterResult {
    pub vertices: Vec<[f64; 2]>,
    pub segment_lengths_points: Vec<f64>,
    pub total_points_distance: f64,
    pub total_world_distance: f64,
    pub unit: LinearUnit,
    pub formatted: String,
}

/// Calculates cumulative perimeter of connected line segments (FR-26.2).
pub fn calculate_perimeter(
    vertices: &[[f64; 2]],
    scale: &ScaleRatio,
    unit: LinearUnit,
) -> PerimeterResult {
    if vertices.len() < 2 {
        return PerimeterResult {
            vertices: vertices.to_vec(),
            segment_lengths_points: Vec::new(),
            total_points_distance: 0.0,
            total_world_distance: 0.0,
            unit,
            formatted: format!("0.00 {}", unit.abbreviation()),
        };
    }

    let mut segments = Vec::new();
    let mut total_pt = 0.0;

    for i in 0..vertices.len() - 1 {
        let dx = vertices[i + 1][0] - vertices[i][0];
        let dy = vertices[i + 1][1] - vertices[i][1];
        let dist = (dx * dx + dy * dy).sqrt();
        segments.push(dist);
        total_pt += dist;
    }

    let total_world = scale.points_to_world(total_pt, unit);
    let formatted = format!("{:.2} {}", total_world, unit.abbreviation());

    PerimeterResult {
        vertices: vertices.to_vec(),
        segment_lengths_points: segments,
        total_points_distance: total_pt,
        total_world_distance: total_world,
        unit,
        formatted,
    }
}

/// Surface area measurement result for a closed polygon conforming to FR-26.3.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AreaResult {
    pub vertices: Vec<[f64; 2]>,
    pub points_area: f64,
    pub world_area: f64,
    pub unit: AreaUnit,
    pub perimeter_result: PerimeterResult,
    pub formatted: String,
}

/// Calculates surface area of a closed polygon using the Shoelace formula (FR-26.3).
pub fn calculate_area(
    vertices: &[[f64; 2]],
    scale: &ScaleRatio,
    area_unit: AreaUnit,
    linear_unit: LinearUnit,
) -> AreaResult {
    let n = vertices.len();
    if n < 3 {
        return AreaResult {
            vertices: vertices.to_vec(),
            points_area: 0.0,
            world_area: 0.0,
            unit: area_unit,
            perimeter_result: calculate_perimeter(vertices, scale, linear_unit),
            formatted: format!("0.00 {}", area_unit.abbreviation()),
        };
    }

    // Shoelace formula: Area = 0.5 * |sum(x_i * y_{i+1} - x_{i+1} * y_i)|
    let mut sum = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        sum += vertices[i][0] * vertices[j][1];
        sum -= vertices[j][0] * vertices[i][1];
    }
    let pt_area = (sum * 0.5).abs();
    let world_area = scale.sq_points_to_world_area(pt_area, area_unit);

    // Also close the polygon perimeter
    let mut closed_verts = vertices.to_vec();
    closed_verts.push(vertices[0]);
    let perimeter = calculate_perimeter(&closed_verts, scale, linear_unit);

    let formatted = format!("{:.2} {}", world_area, area_unit.abbreviation());

    AreaResult {
        vertices: vertices.to_vec(),
        points_area: pt_area,
        world_area,
        unit: area_unit,
        perimeter_result: perimeter,
        formatted,
    }
}

/// Snapping options conforming to FR-26.6.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapOptions {
    pub snap_endpoints: bool,
    pub snap_midpoints: bool,
    pub snap_intersections: bool,
}

impl Default for SnapOptions {
    fn default() -> Self {
        Self {
            snap_endpoints: true,
            snap_midpoints: true,
            snap_intersections: true,
        }
    }
}

/// Snaps a candidate cursor point to the closest geometric feature within `tolerance` points (FR-26.6).
pub fn snap_point(
    cursor: [f64; 2],
    segments: &[([f64; 2], [f64; 2])],
    tolerance: f64,
    options: SnapOptions,
) -> Option<[f64; 2]> {
    let mut best_pt = None;
    let mut min_dist_sq = tolerance * tolerance;

    for (p1, p2) in segments {
        // 1. Endpoint snapping
        if options.snap_endpoints {
            for pt in [p1, p2] {
                let d2 = (cursor[0] - pt[0]).powi(2) + (cursor[1] - pt[1]).powi(2);
                if d2 < min_dist_sq {
                    min_dist_sq = d2;
                    best_pt = Some(*pt);
                }
            }
        }

        // 2. Midpoint snapping
        if options.snap_midpoints {
            let mid = [(p1[0] + p2[0]) * 0.5, (p1[1] + p2[1]) * 0.5];
            let d2 = (cursor[0] - mid[0]).powi(2) + (cursor[1] - mid[1]).powi(2);
            if d2 < min_dist_sq {
                min_dist_sq = d2;
                best_pt = Some(mid);
            }
        }
    }

    // 3. Segment intersection snapping
    if options.snap_intersections && segments.len() >= 2 {
        for i in 0..segments.len() {
            for j in (i + 1)..segments.len() {
                if let Some(inter) = line_segment_intersection(segments[i], segments[j]) {
                    let d2 = (cursor[0] - inter[0]).powi(2) + (cursor[1] - inter[1]).powi(2);
                    if d2 < min_dist_sq {
                        min_dist_sq = d2;
                        best_pt = Some(inter);
                    }
                }
            }
        }
    }

    best_pt
}

/// Computes line segment intersection if segments cross.
fn line_segment_intersection(
    s1: ([f64; 2], [f64; 2]),
    s2: ([f64; 2], [f64; 2]),
) -> Option<[f64; 2]> {
    let (p1, p2) = s1;
    let (p3, p4) = s2;

    let d = (p2[0] - p1[0]) * (p4[1] - p3[1]) - (p2[1] - p1[1]) * (p4[0] - p3[0]);
    if d.abs() < 1e-9 {
        return None; // parallel or collinear
    }

    let u = ((p3[0] - p1[0]) * (p4[1] - p3[1]) - (p3[1] - p1[1]) * (p4[0] - p3[0])) / d;
    let v = ((p3[0] - p1[0]) * (p2[1] - p1[1]) - (p3[1] - p1[1]) * (p2[0] - p1[0])) / d;

    if (0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v) {
        Some([
            p1[0] + u * (p2[0] - p1[0]),
            p1[1] + u * (p2[1] - p1[1]),
        ])
    } else {
        None
    }
}

// -------------------------------------------------------------------------------------------------
// Geospatial Features conforming to ISO 32000-1 Clause 12.11 & FR-26.7-9
// -------------------------------------------------------------------------------------------------

/// Represents a geographic coordinate on the Earth conforming to FR-26.7.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GeoCoordinate {
    pub latitude: f64,
    pub longitude: f64,
}

impl GeoCoordinate {
    pub fn new(latitude: f64, longitude: f64) -> Self {
        Self { latitude, longitude }
    }

    /// Formats as Decimal Degrees (e.g. `37.774900° N, 122.419400° W`).
    pub fn format_decimal(&self) -> String {
        let lat_dir = if self.latitude >= 0.0 { "N" } else { "S" };
        let lon_dir = if self.longitude >= 0.0 { "E" } else { "W" };
        format!(
            "{:.6}° {}, {:.6}° {}",
            self.latitude.abs(),
            lat_dir,
            self.longitude.abs(),
            lon_dir
        )
    }

    /// Formats as Degrees Minutes Seconds (DMS) conforming to FR-26.8.
    pub fn format_dms(&self) -> String {
        let (lat_d, lat_m, lat_s) = to_dms(self.latitude.abs());
        let (lon_d, lon_m, lon_s) = to_dms(self.longitude.abs());
        let lat_dir = if self.latitude >= 0.0 { 'N' } else { 'S' };
        let lon_dir = if self.longitude >= 0.0 { 'E' } else { 'W' };
        format!(
            "{}°{:02}'{:04.1}\"{} {}°{:02}'{:04.1}\"{}",
            lat_d, lat_m, lat_s, lat_dir, lon_d, lon_m, lon_s, lon_dir
        )
    }
}

fn to_dms(deg_float: f64) -> (u32, u32, f64) {
    let d = deg_float.floor() as u32;
    let rem_m = (deg_float - d as f64) * 60.0;
    let m = rem_m.floor() as u32;
    let s = (rem_m - m as f64) * 60.0;
    (d, m, s)
}

/// Geospatial Viewport conforming to ISO 32000-1 Clause 12.11 and FR-26.7.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeospatialViewport {
    pub bbox: [f64; 4], // [llx, lly, urx, ury] in PDF user space
    pub gcs_name: String, // e.g. "EPSG:4326" or "WGS 84"
    pub local_points: Vec<[f64; 2]>, // [x0, y0], [x1, y1], [x2, y2], [x3, y3]
    pub ground_points: Vec<GeoCoordinate>, // corresponding (lat, lon)
}

impl GeospatialViewport {
    /// Creates a rectangular geospatial viewport mapping page bounds to latitude/longitude bounds.
    pub fn from_bounds(
        bbox: [f64; 4],
        min_lat: f64,
        max_lat: f64,
        min_lon: f64,
        max_lon: f64,
        gcs_name: &str,
    ) -> Self {
        let local_points = vec![
            [bbox[0], bbox[1]], // Lower-Left
            [bbox[2], bbox[1]], // Lower-Right
            [bbox[2], bbox[3]], // Upper-Right
            [bbox[0], bbox[3]], // Upper-Left
        ];
        let ground_points = vec![
            GeoCoordinate::new(min_lat, min_lon),
            GeoCoordinate::new(min_lat, max_lon),
            GeoCoordinate::new(max_lat, max_lon),
            GeoCoordinate::new(max_lat, min_lon),
        ];

        Self {
            bbox,
            gcs_name: gcs_name.into(),
            local_points,
            ground_points,
        }
    }

    /// Converts a PDF page coordinate (x, y) to geographic coordinates (FR-26.8).
    pub fn pdf_point_to_geographic(&self, point: [f64; 2]) -> Option<GeoCoordinate> {
        let w = self.bbox[2] - self.bbox[0];
        let h = self.bbox[3] - self.bbox[1];
        if w.abs() < 1e-6 || h.abs() < 1e-6 {
            return None;
        }

        // Normalized relative coordinates u, v in [0, 1]
        let u = ((point[0] - self.bbox[0]) / w).clamp(0.0, 1.0);
        let v = ((point[1] - self.bbox[1]) / h).clamp(0.0, 1.0);

        if self.ground_points.len() >= 4 {
            // Bilinear interpolation
            let p00 = &self.ground_points[0]; // LL
            let p10 = &self.ground_points[1]; // LR
            let p11 = &self.ground_points[2]; // UR
            let p01 = &self.ground_points[3]; // UL

            let lat = (1.0 - u) * (1.0 - v) * p00.latitude
                + u * (1.0 - v) * p10.latitude
                + u * v * p11.latitude
                + (1.0 - u) * v * p01.latitude;

            let lon = (1.0 - u) * (1.0 - v) * p00.longitude
                + u * (1.0 - v) * p10.longitude
                + u * v * p11.longitude
                + (1.0 - u) * v * p01.longitude;

            Some(GeoCoordinate::new(lat, lon))
        } else {
            None
        }
    }
}

/// Computes the true geodesic distance between two points on the WGS84 Earth sphere using the Haversine formula (FR-26.9).
/// Returns distance in meters.
pub fn calculate_geographic_distance_meters(c1: GeoCoordinate, c2: GeoCoordinate) -> f64 {
    const EARTH_RADIUS_METERS: f64 = 6_371_000.0;

    let phi1 = c1.latitude.to_radians();
    let phi2 = c2.latitude.to_radians();
    let delta_phi = (c2.latitude - c1.latitude).to_radians();
    let delta_lambda = (c2.longitude - c1.longitude).to_radians();

    let a = (delta_phi * 0.5).sin().powi(2)
        + phi1.cos() * phi2.cos() * (delta_lambda * 0.5).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

    EARTH_RADIUS_METERS * c
}

/// Computes the true geographic area of a closed spherical polygon in square meters using spherical excess (FR-26.9).
pub fn calculate_geographic_area_sq_meters(coords: &[GeoCoordinate]) -> f64 {
    if coords.len() < 3 {
        return 0.0;
    }
    const EARTH_RADIUS_METERS: f64 = 6_371_000.0;

    // Numerical integration of spherical excess: sum of (lon2 - lon1) * (2 + sin(lat1) + sin(lat2))
    let mut total = 0.0;
    let n = coords.len();

    for i in 0..n {
        let j = (i + 1) % n;
        let p1 = &coords[i];
        let p2 = &coords[j];

        let lat1 = p1.latitude.to_radians();
        let lat2 = p2.latitude.to_radians();
        let dlon = (p2.longitude - p1.longitude).to_radians();

        total += dlon * (2.0 + lat1.sin() + lat2.sin());
    }

    (total.abs() * 0.5) * EARTH_RADIUS_METERS * EARTH_RADIUS_METERS
}

/// Reads any embedded geospatial viewports from a PDF page dictionary conforming to ISO 32000-1 Clause 12.11 (FR-26.7).
pub fn get_page_geospatial_viewports(
    doc: &PdfDocument,
    page_idx: usize,
) -> Result<Vec<GeospatialViewport>> {
    let mut viewports = Vec::new();
    let page_ids = doc.page_ids();
    let page_id = match page_ids.get(page_idx) {
        Some(id) => *id,
        None => return Ok(viewports),
    };

    let page_dict = match doc.objects.get(&page_id).and_then(Object::as_dict) {
        Some(d) => d,
        None => return Ok(viewports),
    };

    // Viewport array `/VP`
    let vp_obj = match page_dict.get("VP") {
        Some(obj) => match doc.resolve(obj) {
            Some(resolved) => resolved,
            None => return Ok(viewports),
        },
        None => return Ok(viewports),
    };

    let vp_arr: Vec<Object> = match vp_obj.as_array() {
        Some(arr) => arr.to_vec(),
        None => match vp_obj.as_dict() {
            Some(d) => vec![Object::Dictionary(d.clone())],
            None => return Ok(viewports),
        },
    };

    for vp_item in vp_arr {
        if let Some(vp_dict) = doc.resolve(&vp_item).and_then(Object::as_dict) {
            let bbox = vp_dict
                .get_array("BBox")
                .and_then(|arr| {
                    if arr.len() == 4 {
                        Some([
                            arr[0].as_f64().unwrap_or(0.0),
                            arr[1].as_f64().unwrap_or(0.0),
                            arr[2].as_f64().unwrap_or(0.0),
                            arr[3].as_f64().unwrap_or(0.0),
                        ])
                    } else {
                        None
                    }
                })
                .unwrap_or([0.0, 0.0, 612.0, 792.0]);

            // Measure dictionary
            if let Some(measure_obj) = vp_dict.get("Measure") {
                if let Some(measure_dict) = doc.resolve(measure_obj).and_then(Object::as_dict) {
                    let subtype = measure_dict.get_name("Subtype").unwrap_or_default();
                    if subtype == "GEO" {
                        let gcs_name = measure_dict
                            .get_str("GCS")
                            .unwrap_or_else(|| "WGS 84".into());

                        let mut ground_points = Vec::new();
                        if let Some(gpts) = measure_dict.get_array("GPTS") {
                            let mut i = 0;
                            while i + 1 < gpts.len() {
                                if let (Some(lat), Some(lon)) =
                                    (gpts[i].as_f64(), gpts[i + 1].as_f64())
                                {
                                    ground_points.push(GeoCoordinate::new(lat, lon));
                                }
                                i += 2;
                            }
                        }

                        let mut local_points = Vec::new();
                        if let Some(lpts) = measure_dict.get_array("LPTS") {
                            let mut i = 0;
                            while i + 1 < lpts.len() {
                                if let (Some(x), Some(y)) = (lpts[i].as_f64(), lpts[i + 1].as_f64()) {
                                    local_points.push([x, y]);
                                }
                                i += 2;
                            }
                        }

                        if ground_points.is_empty() {
                            // Default to bbox mapping
                            ground_points = vec![
                                GeoCoordinate::new(0.0, 0.0),
                                GeoCoordinate::new(0.0, 1.0),
                                GeoCoordinate::new(1.0, 1.0),
                                GeoCoordinate::new(1.0, 0.0),
                            ];
                        }

                        if local_points.is_empty() {
                            local_points = vec![
                                [bbox[0], bbox[1]],
                                [bbox[2], bbox[1]],
                                [bbox[2], bbox[3]],
                                [bbox[0], bbox[3]],
                            ];
                        }

                        viewports.push(GeospatialViewport {
                            bbox,
                            gcs_name,
                            local_points,
                            ground_points,
                        });
                    }
                }
            }
        }
    }

    Ok(viewports)
}

/// Embeds a geospatial viewport conforming to ISO 32000-1 Clause 12.11 onto a PDF page (FR-26.7).
pub fn embed_geospatial_viewport(
    doc: &mut PdfDocument,
    page_idx: usize,
    viewport: &GeospatialViewport,
) -> Result<()> {
    let page_ids = doc.page_ids();
    let page_id = match page_ids.get(page_idx) {
        Some(id) => *id,
        None => return Err(anyhow!("Page index {} out of range", page_idx)),
    };

    // 1. Create Measure dictionary
    let mut measure_dict = PdfDictionary::new();
    measure_dict.insert("Type", PdfName::new("Measure"));
    measure_dict.insert("Subtype", PdfName::new("GEO"));
    measure_dict.insert("GCS", PdfString::from_str(&viewport.gcs_name));

    let mut gpts_arr = Vec::new();
    for pt in &viewport.ground_points {
        gpts_arr.push(Object::Real(pt.latitude));
        gpts_arr.push(Object::Real(pt.longitude));
    }
    measure_dict.insert("GPTS", Object::Array(gpts_arr));

    let mut lpts_arr = Vec::new();
    for pt in &viewport.local_points {
        lpts_arr.push(Object::Real(pt[0]));
        lpts_arr.push(Object::Real(pt[1]));
    }
    measure_dict.insert("LPTS", Object::Array(lpts_arr));

    // 2. Create Viewport dictionary
    let mut vp_dict = PdfDictionary::new();
    vp_dict.insert("Type", PdfName::new("Viewport"));
    vp_dict.insert(
        "BBox",
        vec![
            Object::Real(viewport.bbox[0]),
            Object::Real(viewport.bbox[1]),
            Object::Real(viewport.bbox[2]),
            Object::Real(viewport.bbox[3]),
        ],
    );
    vp_dict.insert("Name", PdfString::from_str("Geospatial Map View"));
    vp_dict.insert("Measure", Object::Dictionary(measure_dict));

    // 3. Attach Viewport to page dictionary
    let page_dict = doc
        .objects
        .get_mut(&page_id)
        .and_then(Object::as_dict_mut)
        .ok_or_else(|| anyhow!("Invalid page dictionary"))?;

    if !page_dict.contains_key("VP") {
        page_dict.insert("VP", Object::Array(Vec::new()));
    }
    let vp_array = page_dict.get_array_mut("VP").unwrap();
    vp_array.push(Object::Dictionary(vp_dict));

    Ok(())
}

/// Adds a standard ISO 32000-1 Clause 12.5.6.14 measurement line annotation to a page (FR-26.4).
pub fn add_measurement_line_annotation(
    doc: &mut PdfDocument,
    page_idx: usize,
    result: &DistanceResult,
    color: (f32, f32, f32),
) -> Result<ObjectId> {
    let page_ids = doc.page_ids();
    let page_id = match page_ids.get(page_idx) {
        Some(id) => *id,
        None => return Err(anyhow!("Page index out of range")),
    };

    let annot_id = doc.next_object_id();
    let mut annot = PdfDictionary::new();
    annot.insert("Type", PdfName::new("Annot"));
    annot.insert("Subtype", PdfName::new("Line"));
    annot.insert("IT", PdfName::new("LineDimension")); // Intent: LineDimension
    annot.insert("Contents", PdfString::from_str(&result.formatted));

    let min_x = result.start[0].min(result.end[0]) - 10.0;
    let max_x = result.start[0].max(result.end[0]) + 10.0;
    let min_y = result.start[1].min(result.end[1]) - 10.0;
    let max_y = result.start[1].max(result.end[1]) + 10.0;

    annot.insert(
        "Rect",
        vec![
            Object::Real(min_x),
            Object::Real(min_y),
            Object::Real(max_x),
            Object::Real(max_y),
        ],
    );

    // Line endpoints [/L [x1 y1 x2 y2]]
    annot.insert(
        "L",
        vec![
            Object::Real(result.start[0]),
            Object::Real(result.start[1]),
            Object::Real(result.end[0]),
            Object::Real(result.end[1]),
        ],
    );

    // Line endings: OpenArrow / OpenArrow
    annot.insert(
        "LE",
        vec![
            Object::Name(PdfName::new("OpenArrow")),
            Object::Name(PdfName::new("OpenArrow")),
        ],
    );

    // Color
    annot.insert(
        "C",
        vec![
            Object::Real(color.0 as f64),
            Object::Real(color.1 as f64),
            Object::Real(color.2 as f64),
        ],
    );

    // Measure dictionary
    let mut measure = PdfDictionary::new();
    measure.insert("Type", PdfName::new("Measure"));
    measure.insert("Subtype", PdfName::new("RL")); // Rectilinear
    measure.insert("R", PdfString::from_str(result.unit.abbreviation()));
    annot.insert("Measure", Object::Dictionary(measure));

    doc.objects.insert(annot_id, Object::Dictionary(annot));

    // Register annotation on the page
    let page_dict = doc
        .objects
        .get_mut(&page_id)
        .and_then(Object::as_dict_mut)
        .ok_or_else(|| anyhow!("Invalid page dictionary"))?;

    if !page_dict.contains_key("Annots") {
        page_dict.insert("Annots", Object::Array(Vec::new()));
    }
    let annots_arr = page_dict.get_array_mut("Annots").unwrap();
    annots_arr.push(Object::Reference(annot_id));

    Ok(annot_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distance_and_scale_conversion() {
        // Standard 1:1 scale in inches
        let scale_1_to_1 = ScaleRatio::default();
        let p1 = [72.0, 72.0];
        let p2 = [288.0, 72.0]; // exactly 216 pt = 3.0 inches

        let res_in = calculate_distance(p1, p2, &scale_1_to_1, LinearUnit::Inches);
        assert_eq!(res_in.points_distance, 216.0);
        assert!((res_in.world_distance - 3.0).abs() < 1e-4);
        assert_eq!(res_in.formatted, "3.00 in");
        assert_eq!(res_in.angle_degrees, 0.0);

        // Architectural scale: 1 inch = 10 feet
        let arch_scale = ScaleRatio::architectural_inch_to_feet(10.0);
        let res_ft = calculate_distance(p1, p2, &arch_scale, LinearUnit::Feet);
        // 3 inches on paper = 30 feet
        assert!((res_ft.world_distance - 30.0).abs() < 1e-4);
        assert_eq!(res_ft.formatted, "30.00 ft");

        // Calibrated scale from measured reference line
        // Suppose user measures 144 points and calibrates it to 12.0 meters
        let calib_scale = ScaleRatio::calibrate_from_points(144.0, 12.0, LinearUnit::Meters);
        let res_m = calculate_distance([0.0, 0.0], [288.0, 0.0], &calib_scale, LinearUnit::Meters);
        assert!((res_m.world_distance - 24.0).abs() < 1e-4);
        assert_eq!(res_m.formatted, "24.00 m");
    }

    #[test]
    fn test_perimeter_and_area_shoelace() {
        let scale = ScaleRatio::default();
        // A 100 pt x 50 pt rectangle in user space
        let rect_pts = [
            [10.0, 10.0],
            [110.0, 10.0],
            [110.0, 60.0],
            [10.0, 60.0],
        ];

        let perim = calculate_perimeter(&rect_pts, &scale, LinearUnit::Points);
        assert_eq!(perim.total_points_distance, 250.0); // 100 + 50 + 100 = 250 without closing

        let area_res = calculate_area(&rect_pts, &scale, AreaUnit::SquarePoints, LinearUnit::Points);
        assert_eq!(area_res.points_area, 5000.0); // 100 * 50 = 5000 sq pt
        assert_eq!(area_res.perimeter_result.total_points_distance, 300.0); // closed perimeter
        assert_eq!(area_res.formatted, "5000.00 sq pt");
    }

    #[test]
    fn test_snapping_engine() {
        let seg1 = ([0.0, 0.0], [100.0, 0.0]);
        let seg2 = ([50.0, -50.0], [50.0, 50.0]);
        let segments = vec![seg1, seg2];
        let options = SnapOptions::default();

        // 1. Close to endpoint (100, 0)
        let snap1 = snap_point([98.5, 1.2], &segments, 5.0, options);
        assert_eq!(snap1, Some([100.0, 0.0]));

        // 2. Close to midpoint of seg2 (50, 0) - which is also intersection!
        let snap2 = snap_point([50.8, -0.4], &segments, 5.0, options);
        assert_eq!(snap2, Some([50.0, 0.0]));

        // 3. Far away from any segment -> No snap
        let snap3 = snap_point([200.0, 200.0], &segments, 5.0, options);
        assert_eq!(snap3, None);
    }

    #[test]
    fn test_geospatial_viewports_and_calculations() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        // Embed viewport for San Francisco (approx 37.77 N, 122.42 W)
        let vp = GeospatialViewport::from_bounds(
            [50.0, 50.0, 550.0, 750.0],
            37.70,
            37.80,
            -122.50,
            -122.35,
            "WGS 84",
        );
        embed_geospatial_viewport(&mut doc, 0, &vp).expect("embed viewport");

        let viewports = get_page_geospatial_viewports(&doc, 0).expect("get viewports");
        assert_eq!(viewports.len(), 1);
        assert_eq!(viewports[0].gcs_name, "WGS 84");

        // Cursor conversion at center
        let center_pt = [300.0, 400.0];
        let geo = viewports[0].pdf_point_to_geographic(center_pt).expect("geo coord");
        assert!((geo.latitude - 37.75).abs() < 0.01);
        assert!((geo.longitude - (-122.425)).abs() < 0.01);

        let dms = geo.format_dms();
        assert!(dms.contains("37°"));
        assert!(dms.contains("N"));

        // Geodesic distance calculation
        let sf = GeoCoordinate::new(37.7749, -122.4194);
        let oakland = GeoCoordinate::new(37.8044, -122.2712);
        let dist_m = calculate_geographic_distance_meters(sf, oakland);
        // Approx 13-14 km
        assert!((dist_m - 13500.0).abs() < 1500.0);

        // Measurement line annotation
        let dist_res = calculate_distance([50.0, 100.0], [200.0, 100.0], &ScaleRatio::default(), LinearUnit::Inches);
        let annot_id = add_measurement_line_annotation(&mut doc, 0, &dist_res, (0.0, 0.7, 0.9)).expect("add annot");
        assert!(doc.objects.contains_key(&annot_id));
    }
}
