//! RavPDF Command-Line Interface (CLI).
//!
//! Traceability: FR-37.1 - FR-37.6 (Complete CLI command suite).

use anyhow::{anyhow, Result};
use clap::{Parser as ClapParser, Subcommand};
use ravpdf_core::document::PdfDocument;
use ravpdf_crypto::{
    decrypt_document, encrypt_document_with_algorithm, sign_pdf_document_advanced, DigitalId,
    DocMdpPermission, EncryptionAlgorithm, Permissions, SigningOptions,
};
use ravpdf_edit::{
    create_pdf_from_markdown, extract_page_text_spans, merge_documents,
    replace_text_span_in_page, rotate_page, split_document, MarkdownPdfOptions,
};
use ravpdf_forms::{
    apply_auto_detected_fields, apply_page_tab_order, auto_detect_form_fields,
    export_form_data, flatten_form, import_form_data, FormDataFormat, TabOrder,
};
use ravpdf_render::{pixmap_to_png, render_page, Color, RenderOptions};
use ravpdf_tools::{
    add_printer_marks, convert_to_standard, flatten_document_transparency,
    get_document_inks, get_trap_presets, set_trap_presets, validate_standard_compliance,
    PrinterMarksConfig, StandardConversionConfig, StandardFormat, TransparencyFlattenConfig,
    TrapPreset,
    add_3d_annotation, add_measurement_line_annotation, add_rich_media_annotation,
    apply_bates_numbering, apply_text_watermark, audit_accessibility, batch_scan_to_pdf,
    calculate_distance, convert_document_colors, create_portfolio, embed_output_intent,
    execute_all_field_calculations, execute_batch_cancellable, execute_preflight_fixup,
    execute_script, export_comments_to_csv, export_page_to_image, export_to_html,
    export_to_plain_text, extract_3d_stream, extract_all_text, extract_portfolio_item,
    extract_rich_media_content, generate_postscript, get_3d_annotations, get_document_comments,
    get_document_info, get_document_javascript, get_output_intents, get_portfolio,
    get_rich_media_annotations, linearize_document, optimize_document, perform_ocr_document,
    redact_pattern, remediate_all_automatic, run_preflight, AccessibilityStatus, ActionSequence,
    BatchJob, BookletBinding, ExportImageFormat, JsExecutionContext, LinearUnit, MediaContent,
    NUpCount, NUpOrder, OcrConfig, OcrOutputMode, PluginPermission, PluginRegistry, PortfolioItem,
    PortfolioLayout, PreflightFixup, PreflightProfile, PrintJobConfig, PrintLayoutMode,
    RichMediaActivation, RichMediaConfig, RichMediaDeactivation, SampleWatermarkPlugin, ScaleRatio,
    ScannerJobConfig, StandardIccProfile, ThreeDConfig, ThreeDFormat, ThreeDLighting,
    ThreeDRenderMode, ThreeDView,
};
use std::fs;
use std::path::PathBuf;

#[derive(ClapParser)]
#[command(name = "ravpdf", version = "1.0", about = "RavPDF Command-Line Utility")]
struct Cli {
    #[arg(long, global = true, help = "Output results in JSON format")]
    json: bool,

    #[arg(short, long, global = true, help = "Verbose diagnostic logging")]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Display document metadata and structure information")]
    Info {
        #[arg(help = "Path to the input PDF file")]
        input: PathBuf,
    },

    #[command(about = "Merge multiple PDF documents into a single document")]
    Merge {
        #[arg(required = true, help = "Input PDF files to merge in order")]
        inputs: Vec<PathBuf>,

        #[arg(short, long, required = true, help = "Output merged PDF path")]
        output: PathBuf,
    },

    #[command(about = "Split a PDF document into multiple documents")]
    Split {
        #[arg(help = "Input PDF file to split")]
        input: PathBuf,

        #[arg(
            long,
            help = "Page ranges (e.g., '1-3,4-5' 1-based)",
            default_value = "1-1"
        )]
        pages: String,

        #[arg(short, long, required = true, help = "Output directory")]
        output: PathBuf,
    },

    #[command(about = "Rotate pages in a PDF document")]
    Rotate {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(short, long, required = true, help = "Output PDF file")]
        output: PathBuf,

        #[arg(long, default_value_t = 90, help = "Angle in degrees (90, 180, 270)")]
        angle: i64,

        #[arg(long, help = "Comma-separated list of 1-based page numbers")]
        pages: Option<String>,
    },

    #[command(about = "Apply a semi-transparent text watermark")]
    Watermark {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(short, long, required = true, help = "Output PDF file")]
        output: PathBuf,

        #[arg(long, default_value = "DRAFT", help = "Watermark text string")]
        text: String,

        #[arg(long, default_value_t = 0.3, help = "Opacity from 0.0 to 1.0")]
        opacity: f32,

        #[arg(long, default_value_t = 45.0, help = "Angle in degrees")]
        angle: f64,
    },

    #[command(about = "Apply Bates numbering across pages")]
    Bates {
        #[arg(required = true, help = "Input PDF file(s)")]
        inputs: Vec<PathBuf>,

        #[arg(long, default_value = "DOC", help = "Bates prefix")]
        prefix: String,

        #[arg(long, default_value_t = 1, help = "Starting number")]
        start: u32,

        #[arg(long, default_value_t = 6, help = "Digit count (zero-padded)")]
        digits: usize,

        #[arg(long, default_value = "", help = "Bates suffix")]
        suffix: String,

        #[arg(short, long, help = "Output PDF file (for single input)")]
        output: Option<PathBuf>,
    },

    #[command(about = "Flatten interactive form fields into static graphics")]
    Flatten {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(short, long, required = true, help = "Output PDF file")]
        output: PathBuf,
    },

    #[command(about = "Extract plain text from document")]
    ExtractText {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(short, long, help = "Output text file path (prints to stdout if omitted)")]
        output: Option<PathBuf>,
    },

    #[command(about = "Redact text matching a regular expression")]
    Redact {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(short, long, required = true, help = "Output PDF file")]
        output: PathBuf,

        #[arg(long, required = true, help = "Regex pattern to redact")]
        pattern: String,

        #[arg(long, default_value = "[REDACTED]", help = "Replacement string")]
        replacement: String,
    },

    #[command(about = "Encrypt a PDF file with password protection")]
    Encrypt {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(short, long, required = true, help = "Output PDF file")]
        output: PathBuf,

        #[arg(long, required = true, help = "User password")]
        password: String,

        #[arg(long, default_value = "", help = "Owner password (optional)")]
        owner_password: String,

        #[arg(long, default_value = "aes256", help = "Encryption algorithm: aes128, aes256 (FR-14.4, FR-14.5)")]
        algorithm: String,
    },

    #[command(about = "Decrypt a password-protected PDF")]
    Decrypt {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(short, long, required = true, help = "Output PDF file")]
        output: PathBuf,

        #[arg(long, required = true, help = "Password")]
        password: String,
    },

    #[command(about = "Render a PDF page to a PNG image")]
    Render {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(short, long, required = true, help = "Output PNG image file")]
        output: PathBuf,

        #[arg(long, default_value_t = 1, help = "1-based page index")]
        page: usize,

        #[arg(long, default_value_t = 1.0, help = "Zoom factor")]
        zoom: f32,
    },

    #[command(about = "Export document to HTML, plain text, or image (FR-03.2)")]
    Export {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(short, long, required = true, help = "Output file path")]
        output: PathBuf,

        #[arg(long, default_value = "html", help = "Export format: html, txt, png, jpg")]
        format: String,

        #[arg(long, default_value_t = 1, help = "1-based page index for image export")]
        page: usize,

        #[arg(long, default_value_t = 300.0, help = "Resolution DPI for image export")]
        dpi: f32,
    },

    #[command(about = "Audit and remediate PDF/UA and WCAG 2.1 accessibility (FR-21)")]
    Audit {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(long, help = "Automatically remediate accessibility violations")]
        auto_fix: bool,

        #[arg(short, long, help = "Output path for remediated PDF")]
        output: Option<PathBuf>,
    },

    #[command(about = "Summarize and export review comments as CSV (FR-31.5)")]
    Comments {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(short, long, help = "Output CSV file path")]
        output: Option<PathBuf>,
    },

    #[command(about = "Measure linear distance between two points on a PDF page (FR-26.1)")]
    Measure {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(long, default_value_t = 1, help = "1-based page index")]
        page: usize,

        #[arg(long, required = true, help = "Start X coordinate in PDF user points")]
        x1: f64,

        #[arg(long, required = true, help = "Start Y coordinate in PDF user points")]
        y1: f64,

        #[arg(long, required = true, help = "End X coordinate in PDF user points")]
        x2: f64,

        #[arg(long, required = true, help = "End Y coordinate in PDF user points")]
        y2: f64,

        #[arg(long, default_value = "in", help = "Measurement unit: pt, in, cm, mm, ft, m, yd")]
        unit: String,

        #[arg(long, help = "Configurable scale ratio e.g. '1:100' or '1in=10ft'")]
        scale: Option<String>,

        #[arg(short, long, help = "Output path to save PDF with dimension line annotation")]
        output: Option<PathBuf>,
    },

    #[command(about = "Create, list, or extract files from a PDF Portfolio (FR-29.2)")]
    Portfolio {
        #[command(subcommand)]
        action: PortfolioCommands,
    },

    #[command(about = "Optimize, compress, or linearize PDF for fast web view (FR-25, FR-02.8)")]
    Optimize {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(short, long, required = true, help = "Output PDF file")]
        output: PathBuf,

        #[arg(long, help = "Linearize document for fast web view streaming (FR-02.8)")]
        linearize: bool,
    },

    #[command(about = "Execute sandboxed Acrobat JavaScript or form calculations (FR-12)")]
    Js {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(long, help = "JavaScript expression or statement to evaluate")]
        eval: Option<String>,

        #[arg(long, help = "Run all AcroForm field calculation scripts")]
        calculate_all: bool,

        #[arg(short, long, help = "Output path to save updated PDF")]
        output: Option<PathBuf>,
    },

    #[command(about = "Enhance, deskew, despeckle, and convert scanned images to PDF (FR-32)")]
    ScanCleanup {
        #[arg(required = true, help = "Input scanned image files (PNG, JPG, TIFF)")]
        inputs: Vec<PathBuf>,

        #[arg(short, long, required = true, help = "Output PDF file path")]
        output: PathBuf,

        #[arg(long, help = "Disable automatic deskew rotation")]
        no_deskew: bool,

        #[arg(long, help = "Disable automatic page boundary cropping")]
        no_crop: bool,

        #[arg(long, help = "Convert to pure black and white via Otsu binarization")]
        binarize: bool,
    },

    #[command(about = "Generate PostScript or calculate print imposition (Booklet, N-Up, Poster) (FR-35)")]
    Print {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(short, long, required = true, help = "Output PostScript (.ps) file path")]
        output: PathBuf,

        #[arg(long, default_value = "all", help = "Page range (e.g. 'all', '1-3, 5')")]
        pages: String,

        #[arg(long, help = "Saddle-stitch booklet imposition")]
        booklet: bool,

        #[arg(long, help = "N-up pages per sheet (2, 4, 6, 9, 16)")]
        nup: Option<usize>,
    },

    #[command(about = "List registered plugins, capabilities, and extension manifests (FR-36)")]
    Plugins {},

    #[command(about = "Manage AcroForm fields, import/export data, autodetect, and tab order (FR-11)")]
    Form {
        #[command(subcommand)]
        action: FormCommands,
    },

    #[command(about = "Embed, inspect, or extract multimedia and rich media annotations (FR-27)")]
    Media {
        #[command(subcommand)]
        action: MediaCommands,
    },

    #[command(about = "Embed, inspect, or extract 3D U3D and PRC models (FR-28)")]
    ThreeD {
        #[command(subcommand)]
        action: ThreeDCommands,
    },

    #[command(about = "Action Wizard batch processing and template recipes (FR-30)")]
    Batch {
        #[command(subcommand)]
        action: BatchCommands,
    },

    #[command(about = "Perform Optical Character Recognition (OCR) and generate searchable text layer (FR-16)")]
    Ocr {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(short, long, required = true, help = "Output PDF file path with searchable text layer")]
        output: PathBuf,

        #[arg(long, default_value = "eng", help = "Recognition language code (e.g. eng, spa, fra, deu) (FR-16.3)")]
        lang: String,

        #[arg(long, default_value = "searchable-image", help = "Output mode: searchable-image, exact, editable (FR-16.4)")]
        mode: String,

        #[arg(long, default_value_t = 300, help = "Processing DPI resolution (72-600) (FR-16.8)")]
        dpi: u32,

        #[arg(long, help = "Disable automatic page orientation correction")]
        no_rotate: bool,

        #[arg(long, help = "Page range to process (e.g. '1-3, 5' 1-based, default all)")]
        pages: Option<String>,
    },

    #[command(about = "Digitally sign a PDF using PKCS#7/CMS detached cryptographic signatures (FR-13.1, FR-13.6)")]
    Sign {
        #[arg(help = "Input PDF file to sign")]
        input: PathBuf,

        #[arg(short, long, required = true, help = "Output signed PDF file path")]
        output: PathBuf,

        #[arg(long, default_value = "Authorized Signer", help = "Signer full name")]
        signer: String,

        #[arg(long, default_value = "RavPDF Authority", help = "Signer organization")]
        org: String,

        #[arg(long, default_value = "signer@ravpdf.org", help = "Signer email address")]
        email: String,

        #[arg(long, default_value = "Document Approval", help = "Reason for signing")]
        reason: String,

        #[arg(long, default_value = "Headquarters", help = "Location of signing")]
        location: String,

        #[arg(long, default_value_t = 1, help = "1-based page index for visible signature box")]
        page: usize,

        #[arg(long, help = "Render a visible signature box on the specified page")]
        visible: bool,

        #[arg(long, help = "Author certification signature with DocMDP restrictions (FR-13.3)")]
        certify: bool,

        #[arg(long, default_value = "no-changes", help = "Certification permission: no-changes, form-filling, annotations")]
        permission: String,

        #[arg(long, help = "Embed Document Security Store (/DSS) for Long-Term Validation (LTV) (FR-13.8)")]
        ltv: bool,
    },

    #[command(about = "Preflight standards validation (PDF/A, Print Prepress) and automated fixups (FR-22, FR-23)")]
    Preflight {
        #[arg(help = "Input PDF file to audit")]
        input: PathBuf,

        #[arg(long, default_value = "pdfa1b", help = "Validation profile: pdfa1b, print, quality")]
        profile: String,

        #[arg(long, help = "Automatically execute preflight fixups on non-compliant documents (FR-23.1.4)")]
        fix: bool,

        #[arg(short, long, help = "Output path for fixed PDF (required if --fix is set)")]
        output: Option<PathBuf>,
    },

    #[command(about = "Color management, ICC profiles, Output Intent, and color conversions (FR-24)")]
    Color {
        #[command(subcommand)]
        action: ColorCommands,
    },

    #[command(about = "Commercial print production, printer marks, ink management, and trap presets (FR-23.3)")]
    Prepress {
        #[command(subcommand)]
        action: PrepressCommands,
    },

    #[command(about = "Standards compliance conversion and verification for PDF/A, PDF/X, and PDF/E (FR-22)")]
    Standards {
        #[command(subcommand)]
        action: StandardsCommands,
    },

    #[command(about = "Extract text spans with coordinates, font, size, and color on a page (FR-04.1, FR-04.3)")]
    TextSpans {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(short, long, default_value_t = 1, help = "Page number (1-based)")]
        page: usize,
    },

    #[command(about = "In-place text editing and font/color replacement on a page (FR-04.1, FR-04.3, FR-04.12)")]
    EditText {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(short, long, required = true, help = "Output PDF file")]
        output: PathBuf,

        #[arg(short, long, default_value_t = 1, help = "Page number (1-based)")]
        page: usize,

        #[arg(long, required = true, help = "Target text to find")]
        find: String,

        #[arg(long, required = true, help = "Replacement text")]
        replace: String,

        #[arg(long, help = "New font name (e.g. Helvetica-Bold, Times-Roman, Courier)")]
        font: Option<String>,

        #[arg(long, help = "New font size in points")]
        font_size: Option<f64>,

        #[arg(long, help = "New color as comma-separated RGB (e.g. '0.2,0.4,0.8')")]
        color: Option<String>,
    },

    #[command(about = "Create a multi-page PDF document from Markdown with headings, lists, quotes, and code blocks (FR-02.5, FR-02.6, FR-03.1.6)")]
    FromMarkdown {
        #[arg(help = "Input Markdown file path")]
        input: PathBuf,

        #[arg(short, long, required = true, help = "Output PDF file path")]
        output: PathBuf,

        #[arg(long, help = "Document title header")]
        title: Option<String>,

        #[arg(long, default_value_t = 11.0, help = "Base font size in points")]
        font_size: f64,

        #[arg(long, default_value_t = 54.0, help = "Page margin in points")]
        margin: f64,
    },
}

#[derive(Subcommand)]
enum PrepressCommands {
    #[command(about = "Add prepress printer marks (trim, bleed, registration bullseyes, color bars, page info) (FR-23.3.4)")]
    Marks {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(short, long, required = true, help = "Output PDF file with printer marks")]
        output: PathBuf,

        #[arg(long, default_value_t = true, help = "Include trim corner ticks")]
        trim: bool,

        #[arg(long, default_value_t = true, help = "Include bleed corner ticks")]
        bleed: bool,

        #[arg(long, default_value_t = true, help = "Include registration bullseye targets")]
        registration: bool,

        #[arg(long, default_value_t = true, help = "Include CMYK tint step color bars")]
        color_bars: bool,

        #[arg(long, default_value_t = true, help = "Include page title and numbering info in slug")]
        info: bool,

        #[arg(long, default_value_t = 36.0, help = "Slug margin in points added outside the page (default: 36.0 pt = 0.5 in)")]
        slug_margin: f64,

        #[arg(long, default_value_t = 9.0, help = "Bleed offset in points (default: 9.0 pt = 1/8 in)")]
        bleed_offset: f64,
    },

    #[command(about = "Inspect process and spot color ink plates in document (FR-23.3.2)")]
    Inks {
        #[arg(help = "Input PDF file")]
        input: PathBuf,
    },

    #[command(about = "Flatten document transparency across ExtGState and page groups (FR-23.3.3)")]
    Flatten {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(short, long, required = true, help = "Output PDF file with flattened transparency")]
        output: PathBuf,

        #[arg(long, default_value_t = 75, help = "Raster/Vector balance from 0 (all raster) to 100 (keep vectors)")]
        balance: u8,
    },

    #[command(about = "Configure or inspect prepress ink trap presets (FR-23.3.5)")]
    Trap {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(short, long, help = "Output PDF file (if setting presets)")]
        output: Option<PathBuf>,

        #[arg(long, help = "Trap width in points (e.g. 0.25)")]
        trap_width: Option<f64>,

        #[arg(long, help = "Black width in points (e.g. 0.50)")]
        black_width: Option<f64>,

        #[arg(long, help = "Trap preset name")]
        name: Option<String>,
    },
}

#[derive(Subcommand)]
enum StandardsCommands {
    #[command(about = "Convert document to ISO standard format (PDF/A, PDF/X, PDF/E) (FR-22.1, FR-22.2, FR-22.3, FR-22.5)")]
    Convert {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(short, long, required = true, help = "Output conformant PDF file")]
        output: PathBuf,

        #[arg(long, default_value = "pdfa-1b", help = "Target format: pdfa-1b, pdfa-2b, pdfa-3b, pdfx-1a, pdfx-3, pdfx-4, pdfe-1")]
        format: String,

        #[arg(long, default_value = "srgb", help = "ICC profile for OutputIntent: srgb, swop, fogra39, gracol, japancolor")]
        icc: String,
    },

    #[command(about = "Validate document against ISO standard profile rules (FR-22.4)")]
    Validate {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(long, default_value = "pdfa-1b", help = "Target format to validate: pdfa-1b, pdfa-2b, pdfa-3b, pdfx-1a, pdfx-3, pdfx-4, pdfe-1")]
        format: String,
    },
}

#[derive(Subcommand)]
enum ColorCommands {
    #[command(about = "Embed ISO-compliant Output Intent and ICC profile (FR-24.1, FR-24.4)")]
    EmbedOutputIntent {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(short, long, required = true, help = "Output PDF file")]
        output: PathBuf,

        #[arg(long, default_value = "swop", help = "ICC Profile: srgb, adobergb, p3, swop, fogra39, gracol, japancolor, dotgain20")]
        profile: String,

        #[arg(long, default_value = "U.S. Web Coated (SWOP) v2", help = "Output condition identifier")]
        condition: String,

        #[arg(long, default_value = "http://www.color.org", help = "Registry name")]
        registry: String,
    },

    #[command(about = "List Output Intent profiles configured in document catalog (FR-24.4)")]
    ListOutputIntents {
        #[arg(help = "Input PDF file")]
        input: PathBuf,
    },

    #[command(about = "Convert document colors to target color space (FR-24.6)")]
    Convert {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(short, long, required = true, help = "Output PDF file")]
        output: PathBuf,

        #[arg(long, default_value = "DeviceCMYK", help = "Target color space: DeviceCMYK, DeviceRGB, DeviceGray")]
        target: String,
    },
}

#[derive(Subcommand)]
enum FormCommands {
    #[command(about = "Export form data to FDF, XFDF, CSV, XML, or JSON (FR-11.3.2)")]
    Export {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(short, long, required = true, help = "Output data file path")]
        output: PathBuf,

        #[arg(long, default_value = "json", help = "Data format: fdf, xfdf, csv, xml, json")]
        format: String,
    },

    #[command(about = "Import form data into PDF fields (FR-11.3.2)")]
    Import {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(short, long, required = true, help = "Data file to import")]
        data: PathBuf,

        #[arg(short, long, required = true, help = "Output PDF file path")]
        output: PathBuf,

        #[arg(long, help = "Data format: fdf, xfdf, csv, xml, json (inferred from extension if omitted)")]
        format: Option<String>,
    },

    #[command(about = "Automatically detect form fields from static content layout (FR-11.3.1)")]
    AutoDetect {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(short, long, required = true, help = "Output PDF file with fields applied")]
        output: PathBuf,

        #[arg(long, help = "1-based page number to detect on (or all pages if omitted)")]
        page: Option<usize>,
    },

    #[command(about = "Set or recalculate tab order for page form fields (FR-11.2.7)")]
    TabOrder {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(short, long, required = true, help = "Output PDF file")]
        output: PathBuf,

        #[arg(long, default_value = "row", help = "Tab order strategy: row, column, structure, manual")]
        order: String,

        #[arg(long, help = "1-based page number (or all pages if omitted)")]
        page: Option<usize>,
    },
}

#[derive(Subcommand)]
enum MediaCommands {
    #[command(about = "Embed audio or video into a PDF page (FR-27.1 - FR-27.6)")]
    Embed {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(short, long, required = true, help = "Output PDF file")]
        output: PathBuf,

        #[arg(long, required = true, help = "Media file path (MP4, MP3, MOV, AAC) or streaming URL")]
        media: String,

        #[arg(long, default_value_t = 1, help = "1-based page index")]
        page: usize,

        #[arg(long, default_value = "72,400,360,600", help = "Annotation bounding box: x1,y1,x2,y2")]
        rect: String,

        #[arg(long, help = "Auto-play on activation")]
        autoplay: bool,

        #[arg(long, help = "Loop playback continuously")]
        loop_playback: bool,

        #[arg(long, help = "Optional poster image file")]
        poster: Option<PathBuf>,

        #[arg(long, default_value = "Embedded Media", help = "Media title")]
        title: String,
    },

    #[command(about = "List rich media annotations in a PDF (FR-27.1)")]
    List {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(long, help = "1-based page index (lists all pages if omitted)")]
        page: Option<usize>,
    },

    #[command(about = "Extract embedded media stream from a rich media annotation (FR-27.1)")]
    Extract {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(long, default_value_t = 1, help = "1-based page index")]
        page: usize,

        #[arg(long, default_value_t = 0, help = "0-based annotation index on the page")]
        index: usize,

        #[arg(short, long, required = true, help = "Output media file path")]
        output: PathBuf,
    },
}

#[derive(Subcommand)]
enum ThreeDCommands {
    #[command(about = "Embed 3D U3D/PRC model into a PDF page (FR-28.1 - FR-28.7)")]
    Embed {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(short, long, required = true, help = "Output PDF file")]
        output: PathBuf,

        #[arg(long, required = true, help = "Path to 3D model file (.u3d or .prc)")]
        model: PathBuf,

        #[arg(long, default_value_t = 1, help = "1-based page index")]
        page: usize,

        #[arg(long, default_value = "72,300,400,600", help = "Annotation bounding box: x1,y1,x2,y2")]
        rect: String,

        #[arg(long, default_value = "solid", help = "Render mode: solid, wireframe, transparent, shaded, etc.")]
        render_mode: String,

        #[arg(long, default_value = "day", help = "Lighting preset: day, night, white, cad, headlamp, etc.")]
        lighting: String,
    },

    #[command(about = "List 3D annotations in a PDF (FR-28.1)")]
    List {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(long, help = "1-based page index (lists all pages if omitted)")]
        page: Option<usize>,
    },

    #[command(about = "Extract 3D model stream from annotation (FR-28.1)")]
    Extract {
        #[arg(help = "Input PDF file")]
        input: PathBuf,

        #[arg(long, default_value_t = 1, help = "1-based page index")]
        page: usize,

        #[arg(long, default_value_t = 0, help = "0-based annotation index on the page")]
        index: usize,

        #[arg(short, long, required = true, help = "Output model file path (.u3d or .prc)")]
        output: PathBuf,
    },
}

#[derive(Subcommand)]
enum BatchCommands {
    #[command(about = "Execute a multi-step Action Wizard sequence across PDF files (FR-30.1, FR-30.2)")]
    Run {
        #[arg(required = true, help = "Input PDF files to process")]
        inputs: Vec<PathBuf>,

        #[arg(short, long, required = true, help = "Path to ActionSequence JSON recipe file")]
        sequence: PathBuf,

        #[arg(short, long, required = true, help = "Output directory for processed files")]
        output: PathBuf,
    },

    #[command(about = "Generate a predefined Action Wizard sequence template JSON (FR-30.3, FR-30.7)")]
    Template {
        #[arg(long, default_value = "distribute", help = "Template name: distribute, archive, web")]
        name: String,

        #[arg(short, long, required = true, help = "Output JSON file path")]
        output: PathBuf,
    },
}

#[derive(Subcommand)]
enum PortfolioCommands {
    #[command(about = "Create a new PDF Portfolio bundling multiple files (FR-29.2.1)")]
    Create {
        #[arg(short, long, required = true, help = "Output PDF portfolio file")]
        output: PathBuf,

        #[arg(long, default_value = "PDF Portfolio", help = "Portfolio title")]
        title: String,

        #[arg(long, default_value = "tile", help = "Portfolio layout: tile, details")]
        layout: String,

        #[arg(required = true, help = "Files to bundle into the portfolio")]
        inputs: Vec<PathBuf>,
    },

    #[command(about = "List embedded files in a PDF Portfolio (FR-29.2.3)")]
    List {
        #[arg(help = "Input PDF Portfolio file")]
        input: PathBuf,
    },

    #[command(about = "Extract files from a PDF Portfolio (FR-29.2.4)")]
    Extract {
        #[arg(help = "Input PDF Portfolio file")]
        input: PathBuf,

        #[arg(short, long, required = true, help = "Output directory")]
        output: PathBuf,

        #[arg(long, help = "Specific filename to extract (extracts all if omitted)")]
        filename: Option<String>,
    },
}

fn parse_scale_ratio(s: &str) -> ScaleRatio {
    let s = s.trim();
    if let Some((left, right)) = s.split_once(':') {
        if let (Ok(l), Ok(r)) = (left.trim().parse::<f64>(), right.trim().parse::<f64>()) {
            if l > 0.0 && r > 0.0 {
                return ScaleRatio::dimensionless(r / l);
            }
        }
    } else if let Some((left, right)) = s.split_once('=') {
        let parse_val_unit = |text: &str| -> (f64, LinearUnit) {
            let num_str: String = text.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
            let unit_str: String = text.chars().skip_while(|c| c.is_ascii_digit() || *c == '.').collect();
            let val = num_str.parse::<f64>().unwrap_or(1.0);
            let unit = LinearUnit::parse_str(&unit_str).unwrap_or(LinearUnit::Inches);
            (val, unit)
        };
        let (d_val, d_unit) = parse_val_unit(left.trim());
        let (w_val, w_unit) = parse_val_unit(right.trim());
        return ScaleRatio::new(d_val, d_unit, w_val, w_unit);
    }
    ScaleRatio::default()
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.verbose {
        tracing_subscriber::fmt()
            .with_env_filter("ravpdf=debug")
            .init();
    }

    match cli.command {
        Commands::Info { input } => {
            let doc = PdfDocument::load_from_file(&input)?;
            let info = get_document_info(&doc)?;
            let count = doc.page_count();

            if cli.json {
                let json = serde_json::json!({
                    "path": input.to_string_lossy(),
                    "version": format!("{}.{}", doc.version.0, doc.version.1),
                    "page_count": count,
                    "metadata": info,
                });
                println!("{}", serde_json::to_string_pretty(&json).unwrap());
            } else {
                println!("=== RavPDF Document Info ===");
                println!("File: {}", input.display());
                println!("Version: PDF {}.{}", doc.version.0, doc.version.1);
                println!("Total Pages: {}", count);
                if let Some(t) = info.title {
                    println!("Title: {}", t);
                }
                if let Some(a) = info.author {
                    println!("Author: {}", a);
                }
                if let Some(s) = info.subject {
                    println!("Subject: {}", s);
                }
            }
        }

        Commands::Merge { inputs, output } => {
            let mut docs = Vec::new();
            for p in &inputs {
                docs.push(PdfDocument::load_from_file(p)?);
            }
            let merged = merge_documents(&docs)?;
            merged.save_to_file(&output)?;

            if cli.json {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "success",
                        "merged_files": inputs.len(),
                        "output": output.to_string_lossy(),
                    })
                );
            } else {
                println!(
                    "Successfully merged {} documents into {}",
                    inputs.len(),
                    output.display()
                );
            }
        }

        Commands::Split {
            input,
            pages,
            output,
        } => {
            let doc = PdfDocument::load_from_file(&input)?;
            let mut ranges = Vec::new();

            for part in pages.split(',') {
                let bounds: Vec<&str> = part.split('-').collect();
                if bounds.len() == 2 {
                    if let (Ok(s), Ok(e)) = (bounds[0].trim().parse::<usize>(), bounds[1].trim().parse::<usize>()) {
                        if s >= 1 && e >= s {
                            ranges.push((s - 1, e - 1));
                        }
                    }
                } else if bounds.len() == 1 {
                    if let Ok(s) = bounds[0].trim().parse::<usize>() {
                        if s >= 1 {
                            ranges.push((s - 1, s - 1));
                        }
                    }
                }
            }

            fs::create_dir_all(&output)?;
            let split_docs = split_document(&doc, &ranges)?;

            for (i, d) in split_docs.iter().enumerate() {
                let out_file = output.join(format!("part_{}.pdf", i + 1));
                d.save_to_file(&out_file)?;
            }

            println!(
                "Successfully split document into {} parts in {}",
                split_docs.len(),
                output.display()
            );
        }

        Commands::Rotate {
            input,
            output,
            angle,
            pages,
        } => {
            let mut doc = PdfDocument::load_from_file(&input)?;
            let total = doc.page_count();

            let target_pages: Vec<usize> = if let Some(p_str) = pages {
                p_str
                    .split(',')
                    .filter_map(|s| s.trim().parse::<usize>().ok())
                    .filter(|&p| p >= 1 && p <= total)
                    .map(|p| p - 1)
                    .collect()
            } else {
                (0..total).collect()
            };

            for p_idx in target_pages {
                rotate_page(&mut doc, p_idx, angle)?;
            }

            doc.save_to_file(&output)?;
            println!("Rotated pages by {}° and saved to {}", angle, output.display());
        }

        Commands::Watermark {
            input,
            output,
            text,
            opacity,
            angle,
        } => {
            let mut doc = PdfDocument::load_from_file(&input)?;
            apply_text_watermark(&mut doc, &text, opacity, angle, 54.0)?;
            doc.save_to_file(&output)?;
            println!("Watermark applied and saved to {}", output.display());
        }

        Commands::Bates {
            inputs,
            prefix,
            start,
            digits,
            suffix,
            output,
        } => {
            let out_path = output.unwrap_or_else(|| PathBuf::from("bates_output.pdf"));
            let mut doc = PdfDocument::load_from_file(&inputs[0])?;
            apply_bates_numbering(&mut doc, &prefix, start, digits, &suffix)?;
            doc.save_to_file(&out_path)?;
            println!("Bates numbering applied to {}", out_path.display());
        }

        Commands::Flatten { input, output } => {
            let mut doc = PdfDocument::load_from_file(&input)?;
            flatten_form(&mut doc)?;
            doc.save_to_file(&output)?;
            println!("Form fields flattened in {}", output.display());
        }

        Commands::ExtractText { input, output } => {
            let doc = PdfDocument::load_from_file(&input)?;
            let text = extract_all_text(&doc)?;

            if let Some(out_path) = output {
                fs::write(out_path, text)?;
            } else {
                println!("{}", text);
            }
        }

        Commands::Redact {
            input,
            output,
            pattern,
            replacement,
        } => {
            let mut doc = PdfDocument::load_from_file(&input)?;
            let count = redact_pattern(&mut doc, &pattern, &replacement)?;
            doc.save_to_file(&output)?;
            println!(
                "Redacted {} matching instances; saved to {}",
                count,
                output.display()
            );
        }

        Commands::Encrypt {
            input,
            output,
            password,
            owner_password,
            algorithm,
        } => {
            let mut doc = PdfDocument::load_from_file(&input)?;
            let owner = if owner_password.is_empty() {
                &password
            } else {
                &owner_password
            };
            let algo = match algorithm.to_ascii_lowercase().as_str() {
                "aes128" | "128" => EncryptionAlgorithm::Aes128,
                _ => EncryptionAlgorithm::Aes256,
            };
            encrypt_document_with_algorithm(&mut doc, &password, owner, Permissions::all_permitted(), algo)?;
            doc.save_to_file(&output)?;
            println!("Document encrypted with {}: {}", algo.display_name(), output.display());
        }

        Commands::Decrypt {
            input,
            output,
            password,
        } => {
            let mut doc = PdfDocument::load_from_file(&input)?;
            decrypt_document(&mut doc, &password)?;
            doc.save_to_file(&output)?;
            println!("Document decrypted: {}", output.display());
        }

        Commands::Render {
            input,
            output,
            page,
            zoom,
        } => {
            let doc = PdfDocument::load_from_file(&input)?;
            if page == 0 || page > doc.page_count() {
                eprintln!("Error: page number out of range (1-{})", doc.page_count());
                std::process::exit(1);
            }

            let options = RenderOptions {
                zoom,
                rotation: 0,
                background_color: Color::WHITE,
            };

            let pixmap = render_page(&doc, page - 1, &options)?;
            let png_bytes = pixmap_to_png(&pixmap)?;
            fs::write(&output, png_bytes)?;
            println!("Rendered page {} to {}", page, output.display());
        }

        Commands::Export {
            input,
            output,
            format,
            page,
            dpi,
        } => {
            let doc = PdfDocument::load_from_file(&input)?;
            let fmt_lower = format.to_lowercase();
            match fmt_lower.as_str() {
                "html" => {
                    let html = export_to_html(&doc, None)?;
                    fs::write(&output, html)?;
                    println!("Exported HTML to {}", output.display());
                }
                "txt" | "text" => {
                    let txt = export_to_plain_text(&doc, None)?;
                    fs::write(&output, txt)?;
                    println!("Exported text to {}", output.display());
                }
                "png" => {
                    if page == 0 || page > doc.page_count() {
                        eprintln!("Error: page number out of range (1-{})", doc.page_count());
                        std::process::exit(1);
                    }
                    let bytes = export_page_to_image(&doc, page - 1, dpi, ExportImageFormat::Png)?;
                    fs::write(&output, bytes)?;
                    println!("Exported page {} as PNG ({:.0} DPI) to {}", page, dpi, output.display());
                }
                "jpg" | "jpeg" => {
                    if page == 0 || page > doc.page_count() {
                        eprintln!("Error: page number out of range (1-{})", doc.page_count());
                        std::process::exit(1);
                    }
                    let bytes = export_page_to_image(&doc, page - 1, dpi, ExportImageFormat::Jpeg)?;
                    fs::write(&output, bytes)?;
                    println!("Exported page {} as JPEG ({:.0} DPI) to {}", page, dpi, output.display());
                }
                other => {
                    eprintln!("Error: Unsupported export format '{}'. Choose from: html, txt, png, jpg", other);
                    std::process::exit(1);
                }
            }
        }

        Commands::Audit {
            input,
            auto_fix,
            output,
        } => {
            let mut doc = PdfDocument::load_from_file(&input)?;
            let report = if auto_fix {
                let default_title = input.file_stem().and_then(|s| s.to_str()).unwrap_or("Accessible Document");
                let remediated = remediate_all_automatic(&mut doc, default_title, "en-US")?;
                if let Some(ref out_path) = output {
                    doc.save_to_file(out_path)?;
                    println!("Saved remediated PDF to {}", out_path.display());
                }
                remediated
            } else {
                audit_accessibility(&doc)?
            };

            if cli.json {
                println!("{}", serde_json::to_string_pretty(&report).unwrap());
            } else {
                println!("=== PDF/UA & WCAG 2.1 Accessibility Audit ===");
                println!("Document: {}", input.display());
                println!("Status: {}", if report.is_compliant { "COMPLIANT" } else { "NON-COMPLIANT" });
                println!("Results: {} Passed, {} Failed, {} Needs Manual Check\n",
                    report.passed_count, report.failed_count, report.manual_check_count);

                for rule in &report.rules {
                    let tag = match rule.status {
                        AccessibilityStatus::Passed => "[PASS]",
                        AccessibilityStatus::Failed => "[FAIL]",
                        AccessibilityStatus::NeedsManualCheck => "[WARN]",
                    };
                    println!("{} {}: {}", tag, rule.title, rule.description);
                }
            }
        }

        Commands::Comments {
            input,
            output,
        } => {
            let doc = PdfDocument::load_from_file(&input)?;
            let comments = get_document_comments(&doc)?;

            if let Some(ref out_path) = output {
                let csv = export_comments_to_csv(&comments);
                fs::write(out_path, csv)?;
                println!("Exported {} comments to CSV: {}", comments.len(), out_path.display());
            } else if cli.json {
                println!("{}", serde_json::to_string_pretty(&comments).unwrap());
            } else {
                println!("=== Document Review Comments (Total: {}) ===", comments.len());
                for (i, c) in comments.iter().enumerate() {
                    println!("#{}. [Page {}] {}: {} - \"{}\"",
                        i + 1, c.page_index + 1, c.comment_type, c.author, c.contents);
                }
            }
        }

        Commands::Measure {
            input,
            page,
            x1,
            y1,
            x2,
            y2,
            unit,
            scale,
            output,
        } => {
            let mut doc = PdfDocument::load_from_file(&input)?;
            let page_idx = page.saturating_sub(1);
            let linear_unit = LinearUnit::parse_str(&unit).unwrap_or(LinearUnit::Inches);
            let scale_ratio = match scale {
                Some(ref s) => parse_scale_ratio(s),
                None => ScaleRatio::default(),
            };

            let dist_res = calculate_distance([x1, y1], [x2, y2], &scale_ratio, linear_unit);

            if let Some(ref out_path) = output {
                add_measurement_line_annotation(&mut doc, page_idx, &dist_res, (0.0, 0.45, 0.85))?;
                doc.save_to_file(out_path)?;
                println!("Saved measured and annotated PDF to {}", out_path.display());
            }

            if cli.json {
                println!("{}", serde_json::to_string_pretty(&dist_res).unwrap());
            } else {
                println!("=== RavPDF Dimension Measurement ===");
                println!("Document: {}", input.display());
                println!("Page: {}", page);
                println!("From: ({:.1}, {:.1}) pt -> To: ({:.1}, {:.1}) pt", x1, y1, x2, y2);
                println!("Scale: {}", scale_ratio.to_display_string());
                println!("Raw Distance: {:.2} pt", dist_res.points_distance);
                println!("Measured Result: {}", dist_res.formatted);
                println!("Angle: {:.1}°", dist_res.angle_degrees);
            }
        }

        Commands::Portfolio { action } => match action {
            PortfolioCommands::Create {
                output,
                title,
                layout,
                inputs,
            } => {
                let mut items = Vec::new();
                for in_path in inputs {
                    let item = PortfolioItem::from_file_path(&in_path, None)?;
                    items.push(item);
                }

                let port_layout = match layout.to_lowercase().as_str() {
                    "details" | "table" => PortfolioLayout::Details,
                    _ => PortfolioLayout::Tile,
                };

                let doc = create_portfolio(&items, port_layout, &title, None)?;
                doc.save_to_file(&output)?;
                println!("Created PDF Portfolio with {} files: {}", items.len(), output.display());
            }

            PortfolioCommands::List { input } => {
                let doc = PdfDocument::load_from_file(&input)?;
                let portfolio = get_portfolio(&doc)?;

                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&portfolio).unwrap());
                } else {
                    println!("=== PDF Portfolio: \"{}\" ===", portfolio.title);
                    if let Some(ref desc) = portfolio.description {
                        println!("Description: {}", desc);
                    }
                    println!("Total Embedded Files: {}\n", portfolio.items.len());

                    for (i, item) in portfolio.items.iter().enumerate() {
                        let size_kb = (item.size as f64) / 1024.0;
                        let desc_str = item.description.as_deref().unwrap_or("-");
                        println!("#{}. {} | {:.1} KB | {}", i + 1, item.filename, size_kb, desc_str);
                    }
                }
            }

            PortfolioCommands::Extract {
                input,
                output,
                filename,
            } => {
                let doc = PdfDocument::load_from_file(&input)?;
                fs::create_dir_all(&output)?;

                if let Some(ref target_name) = filename {
                    if let Some(data) = extract_portfolio_item(&doc, target_name)? {
                        let target_path = output.join(target_name);
                        fs::write(&target_path, data)?;
                        println!("Extracted {} to {}", target_name, target_path.display());
                    } else {
                        eprintln!("Error: File '{}' not found in portfolio", target_name);
                        std::process::exit(1);
                    }
                } else {
                    let portfolio = get_portfolio(&doc)?;
                    let mut extracted_count = 0;
                    for item in &portfolio.items {
                        let target_path = output.join(&item.filename);
                        fs::write(&target_path, &item.data)?;
                        extracted_count += 1;
                    }
                    println!("Extracted all {} files to {}", extracted_count, output.display());
                }
            }
        },

        Commands::Optimize {
            input,
            output,
            linearize,
        } => {
            let mut doc = PdfDocument::load_from_file(&input)?;
            let stats = optimize_document(&mut doc)?;

            if linearize {
                let lin_bytes = linearize_document(&doc)?;
                fs::write(&output, lin_bytes)?;
                println!("Optimized and linearized PDF written to {}", output.display());
            } else {
                doc.save_to_file(&output)?;
                println!("Optimized PDF written to {}", output.display());
            }

            if cli.json {
                println!("{}", serde_json::to_string_pretty(&stats).unwrap());
            } else {
                println!("=== Optimization Summary ===");
                println!("Original Size: {} bytes", stats.original_bytes);
                println!("Optimized Size: {} bytes", stats.optimized_bytes);
                println!("Saved: {} bytes ({:.1}% reduction)", stats.saved_bytes(), stats.reduction_percent());
                println!("Streams Compressed: {}", stats.streams_compressed);
                println!("Dead Objects Pruned: {}", stats.objects_removed);
            }
        }

        Commands::Js {
            input,
            eval,
            calculate_all,
            output,
        } => {
            let mut doc = PdfDocument::load_from_file(&input)?;
            let mut ctx = JsExecutionContext::new();
            ctx.sync_from_document(&doc)?;

            // Run document-level scripts if any
            let doc_scripts = get_document_javascript(&doc)?;
            for (name, s) in doc_scripts {
                execute_script(&mut ctx, &s)?;
                if !cli.json {
                    println!("Executed document script '{}'", name);
                }
            }

            if let Some(code) = eval {
                let res = execute_script(&mut ctx, &code)?;
                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&res)?);
                } else {
                    println!("Result: {:?}", res);
                }
            }

            if calculate_all {
                let count = execute_all_field_calculations(&mut ctx)?;
                println!("Calculated {} form field(s)", count);
            }

            for msg in &ctx.console_log {
                println!("[JS Console] {}", msg);
            }

            if let Some(out_path) = output {
                ctx.sync_to_document(&mut doc)?;
                doc.save_to_file(&out_path)?;
                println!("Updated PDF saved to {}", out_path.display());
            }
        }

        Commands::ScanCleanup {
            inputs,
            output,
            no_deskew,
            no_crop,
            binarize,
        } => {
            let mut images = Vec::new();
            for p in &inputs {
                let img = image::open(p)?.to_rgba8();
                images.push(img);
            }

            let config = ScannerJobConfig {
                auto_deskew: !no_deskew,
                auto_crop: !no_crop,
                binarize,
                ..Default::default()
            };

            let doc = batch_scan_to_pdf(&images, &config)?;
            doc.save_to_file(&output)?;
            println!("Processed {} scanned page(s) -> {}", inputs.len(), output.display());
        }

        Commands::Print {
            input,
            output,
            pages,
            booklet,
            nup,
        } => {
            let doc = PdfDocument::load_from_file(&input)?;
            let layout_mode = if booklet {
                PrintLayoutMode::Booklet {
                    binding: BookletBinding::LeftToRight,
                    subset: ravpdf_tools::BookletSubset::BothSides,
                }
            } else if let Some(n) = nup {
                let nup_count = match n {
                    2 => NUpCount::TwoUp,
                    4 => NUpCount::FourUp,
                    6 => NUpCount::SixUp,
                    9 => NUpCount::NineUp,
                    16 => NUpCount::SixteenUp,
                    _ => NUpCount::TwoUp,
                };
                PrintLayoutMode::NUp {
                    count: nup_count,
                    order: NUpOrder::Horizontal,
                    draw_border: true,
                }
            } else {
                PrintLayoutMode::Standard
            };
            let config = PrintJobConfig {
                page_range_str: pages,
                layout_mode,
                ..Default::default()
            };

            let ps = generate_postscript(&doc, &config)?;
            fs::write(&output, ps)?;
            println!("PostScript print job written to {}", output.display());
        }

        Commands::Plugins {} => {
            let mut registry = PluginRegistry::new();
            let sample = Box::new(SampleWatermarkPlugin::new());
            let permissions = [
                PluginPermission::UiExtension,
                PluginPermission::DocumentTransform,
            ];
            registry.register_plugin(sample, &permissions)?;

            let plugins = registry.list_plugins();
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&plugins)?);
            } else {
                println!("=== Registered RavPDF Extensions ===");
                for p in plugins {
                    println!("ID: {}", p.manifest.id);
                    println!("Name: {}", p.manifest.name);
                    println!("Version: {} (Min RavPDF: {})", p.manifest.version, p.manifest.min_ravpdf_version);
                    println!("Author: {}", p.manifest.author);
                    println!("Description: {}", p.manifest.description);
                    println!("Enabled: {}", p.enabled);
                    println!("Permissions: {:?}", p.granted_permissions);
                    println!("------------------------------------");
                }
            }
        }

        Commands::Form { action } => match action {
            FormCommands::Export {
                input,
                output,
                format,
            } => {
                let doc = PdfDocument::load_from_file(&input)?;
                let fmt = parse_form_format(&format).unwrap_or(FormDataFormat::Json);
                let exported = export_form_data(&doc, fmt)?;
                fs::write(&output, exported)?;
                if cli.json {
                    println!("{}", serde_json::json!({
                        "status": "success",
                        "format": format!("{:?}", fmt),
                        "output": output.to_string_lossy(),
                    }));
                } else {
                    println!("Exported form data ({:?}) to {}", fmt, output.display());
                }
            }

            FormCommands::Import {
                input,
                data,
                output,
                format,
            } => {
                let mut doc = PdfDocument::load_from_file(&input)?;
                let raw_data = fs::read_to_string(&data)?;
                let fmt = format
                    .as_deref()
                    .and_then(parse_form_format)
                    .or_else(|| {
                        let ext = data.extension().and_then(|e| e.to_str()).unwrap_or("");
                        parse_form_format(ext)
                    })
                    .unwrap_or(FormDataFormat::Json);
                let updated = import_form_data(&mut doc, &raw_data, fmt)?;
                doc.save_to_file(&output)?;
                if cli.json {
                    println!("{}", serde_json::json!({
                        "status": "success",
                        "fields_updated": updated,
                        "format": format!("{:?}", fmt),
                        "output": output.to_string_lossy(),
                    }));
                } else {
                    println!("Imported {} form field(s) ({:?}) -> {}", updated, fmt, output.display());
                }
            }

            FormCommands::AutoDetect {
                input,
                output,
                page,
            } => {
                let mut doc = PdfDocument::load_from_file(&input)?;
                let total_pages = doc.page_count();
                let pages_to_check: Vec<usize> = if let Some(p) = page {
                    if p >= 1 && p <= total_pages {
                        vec![p - 1]
                    } else {
                        eprintln!("Error: page number out of range (1-{})", total_pages);
                        std::process::exit(1);
                    }
                } else {
                    (0..total_pages).collect()
                };

                let mut detected_count = 0;
                for p_idx in pages_to_check {
                    let detected = auto_detect_form_fields(&doc, p_idx)?;
                    let count = detected.len();
                    apply_auto_detected_fields(&mut doc, p_idx, &detected)?;
                    detected_count += count;
                }
                doc.save_to_file(&output)?;

                if cli.json {
                    println!("{}", serde_json::json!({
                        "status": "success",
                        "fields_detected": detected_count,
                        "output": output.to_string_lossy(),
                    }));
                } else {
                    println!("Auto-detected and applied {} form field(s) -> {}", detected_count, output.display());
                }
            }

            FormCommands::TabOrder {
                input,
                output,
                order,
                page,
            } => {
                let mut doc = PdfDocument::load_from_file(&input)?;
                let total_pages = doc.page_count();
                let tab_order = parse_tab_order(&order);
                let pages_to_check: Vec<usize> = if let Some(p) = page {
                    if p >= 1 && p <= total_pages {
                        vec![p - 1]
                    } else {
                        eprintln!("Error: page number out of range (1-{})", total_pages);
                        std::process::exit(1);
                    }
                } else {
                    (0..total_pages).collect()
                };

                for p_idx in pages_to_check {
                    apply_page_tab_order(&mut doc, p_idx, tab_order)?;
                }
                doc.save_to_file(&output)?;

                if cli.json {
                    println!("{}", serde_json::json!({
                        "status": "success",
                        "tab_order": format!("{:?}", tab_order),
                        "output": output.to_string_lossy(),
                    }));
                } else {
                    println!("Applied {:?} tab order and saved to {}", tab_order, output.display());
                }
            }
        },

        Commands::Media { action } => match action {
            MediaCommands::Embed {
                input,
                output,
                media,
                page,
                rect,
                autoplay,
                loop_playback,
                poster,
                title,
            } => {
                let mut doc = PdfDocument::load_from_file(&input)?;
                let p_idx = page.saturating_sub(1);
                let parsed_rect = parse_rect(&rect).unwrap_or([72.0, 400.0, 360.0, 600.0]);
                let poster_bytes = if let Some(p) = poster {
                    Some(fs::read(p)?)
                } else {
                    None
                };

                let content = if media.starts_with("http://") || media.starts_with("https://") {
                    let mime = if media.ends_with(".mp3") || media.ends_with(".aac") {
                        "audio/mpeg".to_string()
                    } else {
                        "video/mp4".to_string()
                    };
                    MediaContent::ExternalUrl {
                        url: media.clone(),
                        mime_type: mime,
                    }
                } else {
                    let media_path = PathBuf::from(&media);
                    let file_bytes = fs::read(&media_path)?;
                    let fname = media_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("media.mp4")
                        .to_string();
                    let mime = if fname.ends_with(".mp3") || fname.ends_with(".aac") {
                        "audio/mpeg".to_string()
                    } else {
                        "video/mp4".to_string()
                    };
                    MediaContent::Embedded {
                        data: file_bytes,
                        mime_type: mime,
                        filename: fname,
                    }
                };

                let config = RichMediaConfig {
                    title,
                    content,
                    rect: parsed_rect,
                    poster_image: poster_bytes,
                    auto_play: autoplay,
                    loop_playback,
                    show_controls: true,
                    volume: 1.0,
                    activation: if autoplay {
                        RichMediaActivation::PageVisible
                    } else {
                        RichMediaActivation::UserClick
                    },
                    deactivation: RichMediaDeactivation::PageClose,
                };

                let annot_id = add_rich_media_annotation(&mut doc, p_idx, &config)?;
                doc.save_to_file(&output)?;

                if cli.json {
                    println!("{}", serde_json::json!({
                        "status": "success",
                        "annotation_id": format!("{} {}", annot_id.number, annot_id.generation),
                        "page": page,
                        "output": output.to_string_lossy(),
                    }));
                } else {
                    println!(
                        "Embedded rich media annotation ({:?}) on page {} -> {}",
                        annot_id,
                        page,
                        output.display()
                    );
                }
            }

            MediaCommands::List { input, page } => {
                let doc = PdfDocument::load_from_file(&input)?;
                let total_pages = doc.page_count();
                let pages_to_check: Vec<usize> = if let Some(p) = page {
                    if p >= 1 && p <= total_pages {
                        vec![p - 1]
                    } else {
                        eprintln!("Error: page number out of range (1-{})", total_pages);
                        std::process::exit(1);
                    }
                } else {
                    (0..total_pages).collect()
                };

                let mut all_media = Vec::new();
                for p in pages_to_check {
                    let annots = get_rich_media_annotations(&doc, p)?;
                    for a in annots {
                        all_media.push((p + 1, a));
                    }
                }

                if cli.json {
                    let json: Vec<_> = all_media
                        .iter()
                        .map(|(p, a)| {
                            serde_json::json!({
                                "page": p,
                                "id": format!("{} {}", a.id.number, a.id.generation),
                                "title": a.title,
                                "rect": a.rect,
                                "is_embedded": a.is_embedded,
                                "external_url": a.external_url,
                                "mime_type": a.mime_type,
                                "auto_play": a.auto_play,
                                "loop": a.loop_playback,
                                "volume": a.volume,
                            })
                        })
                        .collect();
                    println!("{}", serde_json::to_string_pretty(&json)?);
                } else {
                    println!("=== Rich Media Annotations (Total: {}) ===", all_media.len());
                    for (p, a) in all_media {
                        let src = if a.is_embedded {
                            "Embedded Stream"
                        } else {
                            a.external_url.as_deref().unwrap_or("URL")
                        };
                        println!(
                            "Page {}: \"{}\" [{}] (ID {:?}) - {}",
                            p, a.title, a.mime_type, a.id, src
                        );
                    }
                }
            }

            MediaCommands::Extract {
                input,
                page,
                index,
                output,
            } => {
                let doc = PdfDocument::load_from_file(&input)?;
                let p_idx = page.saturating_sub(1);
                let annots = get_rich_media_annotations(&doc, p_idx)?;
                if let Some(target) = annots.get(index) {
                    if let Some(bytes) = extract_rich_media_content(&doc, target.id)? {
                        fs::write(&output, bytes)?;
                        println!("Extracted media stream to {}", output.display());
                    } else {
                        eprintln!("Error: Annotation does not contain an embedded stream (may be external URL)");
                        std::process::exit(1);
                    }
                } else {
                    eprintln!(
                        "Error: Annotation index {} out of range (found {} on page {})",
                        index,
                        annots.len(),
                        page
                    );
                    std::process::exit(1);
                }
            }
        },

        Commands::ThreeD { action } => match action {
            ThreeDCommands::Embed {
                input,
                output,
                model,
                page,
                rect,
                render_mode,
                lighting,
            } => {
                let mut doc = PdfDocument::load_from_file(&input)?;
                let p_idx = page.saturating_sub(1);
                let parsed_rect = parse_rect(&rect).unwrap_or([72.0, 300.0, 400.0, 600.0]);
                let model_bytes = fs::read(&model)?;
                let ext = model.extension().and_then(|e| e.to_str()).unwrap_or("");
                let fmt = if ext.eq_ignore_ascii_case("prc") {
                    ThreeDFormat::Prc
                } else {
                    ThreeDFormat::U3d
                };

                let render_m = parse_render_mode(&render_mode);
                let light_p = parse_lighting(&lighting);

                let view = ThreeDView {
                    render_mode: render_m,
                    lighting: light_p,
                    ..Default::default()
                };

                let config = ThreeDConfig {
                    format: fmt,
                    model_data: model_bytes,
                    rect: parsed_rect,
                    views: vec![view],
                    default_view_index: 0,
                    model_tree: None,
                };

                let annot_id = add_3d_annotation(&mut doc, p_idx, &config)?;
                doc.save_to_file(&output)?;

                if cli.json {
                    println!("{}", serde_json::json!({
                        "status": "success",
                        "annotation_id": format!("{} {}", annot_id.number, annot_id.generation),
                        "format": format!("{:?}", fmt),
                        "page": page,
                        "output": output.to_string_lossy(),
                    }));
                } else {
                    println!(
                        "Embedded 3D model ({:?}) annotation ({:?}) on page {} -> {}",
                        fmt,
                        annot_id,
                        page,
                        output.display()
                    );
                }
            }

            ThreeDCommands::List { input, page } => {
                let doc = PdfDocument::load_from_file(&input)?;
                let total_pages = doc.page_count();
                let pages_to_check: Vec<usize> = if let Some(p) = page {
                    if p >= 1 && p <= total_pages {
                        vec![p - 1]
                    } else {
                        eprintln!("Error: page number out of range (1-{})", total_pages);
                        std::process::exit(1);
                    }
                } else {
                    (0..total_pages).collect()
                };

                let mut all_3d = Vec::new();
                for p in pages_to_check {
                    let annots = get_3d_annotations(&doc, p)?;
                    for a in annots {
                        all_3d.push((p + 1, a));
                    }
                }

                if cli.json {
                    let json: Vec<_> = all_3d
                        .iter()
                        .map(|(p, a)| {
                            serde_json::json!({
                                "page": p,
                                "id": format!("{} {}", a.id.number, a.id.generation),
                                "format": format!("{:?}", a.format),
                                "rect": a.rect,
                                "views_count": a.views_count,
                                "stream_size_bytes": a.stream_size_bytes,
                            })
                        })
                        .collect();
                    println!("{}", serde_json::to_string_pretty(&json)?);
                } else {
                    println!("=== 3D Annotations (Total: {}) ===", all_3d.len());
                    for (p, a) in all_3d {
                        println!(
                            "Page {}: {:?} model [{} bytes] (ID {:?})",
                            p, a.format, a.stream_size_bytes, a.id
                        );
                    }
                }
            }

            ThreeDCommands::Extract {
                input,
                page,
                index,
                output,
            } => {
                let doc = PdfDocument::load_from_file(&input)?;
                let p_idx = page.saturating_sub(1);
                let annots = get_3d_annotations(&doc, p_idx)?;
                if let Some(target) = annots.get(index) {
                    if let Some(bytes) = extract_3d_stream(&doc, target.id)? {
                        fs::write(&output, &bytes)?;
                        println!(
                            "Extracted 3D stream ({} bytes) to {}",
                            bytes.len(),
                            output.display()
                        );
                    } else {
                        eprintln!("Error: 3D stream not found on annotation");
                        std::process::exit(1);
                    }
                } else {
                    eprintln!(
                        "Error: Annotation index {} out of range (found {} on page {})",
                        index,
                        annots.len(),
                        page
                    );
                    std::process::exit(1);
                }
            }
        },

        Commands::Batch { action } => match action {
            BatchCommands::Run {
                inputs,
                sequence,
                output,
            } => {
                let recipe_str = fs::read_to_string(&sequence)?;
                let seq: ActionSequence = serde_json::from_str(&recipe_str)
                    .map_err(|e| anyhow!("Failed to parse ActionSequence JSON: {}", e))?;
                fs::create_dir_all(&output)?;

                let job = BatchJob {
                    input_files: inputs.clone(),
                    output_directory: output.clone(),
                    actions: seq.actions,
                };

                if !cli.json {
                    println!("=== Action Wizard Batch Runner ===");
                    println!("Sequence: \"{}\"", seq.name);
                    println!("Description: {}", seq.description);
                    println!("Files: {} -> Destination: {}", inputs.len(), output.display());
                }

                let result = execute_batch_cancellable(
                    &job,
                    None,
                    Some(&|current, total, file| {
                        if !cli.json {
                            let fname = file.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                            println!("[{}/{}] Processing: {}", current, total, fname);
                        }
                    }),
                );

                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                } else {
                    println!("\n=== Execution Summary ===");
                    println!("Total Files: {}", result.total_files);
                    println!("Succeeded: {}", result.successful_files);
                    println!("Failed: {}", result.failed_files);
                    if result.cancelled {
                        println!("Status: CANCELLED");
                    }
                    for item in &result.details {
                        let fname = item.path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                        if item.success {
                            println!("  [OK] {}", fname);
                        } else {
                            println!(
                                "  [FAIL] {} - {}",
                                fname,
                                item.error_message.as_deref().unwrap_or("Error")
                            );
                        }
                    }
                }
            }

            BatchCommands::Template { name, output } => {
                let template = match name.to_lowercase().as_str() {
                    "archive" | "archiving" => ActionSequence::template_prepare_for_archiving(),
                    "web" | "web-optimize" => ActionSequence::template_optimize_for_web(),
                    _ => ActionSequence::template_prepare_for_distribution(),
                };
                let json = serde_json::to_string_pretty(&template)?;
                fs::write(&output, json)?;
                if cli.json {
                    println!("{}", serde_json::json!({
                        "status": "success",
                        "template": template.name,
                        "output": output.to_string_lossy(),
                    }));
                } else {
                    println!("Saved \"{}\" template recipe to {}", template.name, output.display());
                }
            }
        },

        Commands::Ocr {
            input,
            output,
            lang,
            mode,
            dpi,
            no_rotate,
            pages,
        } => {
            let mut doc = PdfDocument::load_from_file(&input)?;
            let page_indices: Option<Vec<usize>> = pages.map(|p_str| {
                p_str
                    .split(',')
                    .filter_map(|s| s.trim().parse::<usize>().ok().map(|n| n.saturating_sub(1)))
                    .collect()
            });

            let out_mode = match mode.to_lowercase().as_str() {
                "exact" | "searchable-image-exact" => OcrOutputMode::SearchableImageExact,
                "editable" | "editable-text" => OcrOutputMode::EditableText,
                _ => OcrOutputMode::SearchableImage,
            };

            let config = OcrConfig {
                language: lang.clone(),
                output_mode: out_mode,
                dpi,
                auto_rotate: !no_rotate,
                ..Default::default()
            };

            let result = perform_ocr_document(&mut doc, &config, page_indices.as_deref())?;
            doc.save_to_file(&output)?;

            if cli.json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("Successfully performed OCR on '{}'", input.display());
                println!("  Processed pages: {}", result.pages.len());
                println!("  Total words recognized: {}", result.total_words);
                println!("  Average confidence: {:.1}%", result.average_confidence * 100.0);
                println!("  Uncertain words: {}", result.total_uncertain_words);
                println!("  Saved searchable PDF to '{}'", output.display());
            }
        }

        Commands::Sign {
            input,
            output,
            signer,
            org,
            email,
            reason,
            location,
            page,
            visible,
            certify,
            permission,
            ltv,
        } => {
            let doc = PdfDocument::load_from_file(&input)?;
            let curr_page = page.saturating_sub(1);
            let rect = if visible {
                Some([72.0, 72.0, 260.0, 130.0])
            } else {
                None
            };
            let digital_id = DigitalId::generate(&signer, &org, &email, 365)?;

            let doc_mdp_perm = match permission.to_ascii_lowercase().as_str() {
                "form-filling" | "forms" => DocMdpPermission::FormFilling,
                "annotations" | "annot" => DocMdpPermission::AnnotationsAndFormFilling,
                _ => DocMdpPermission::NoChanges,
            };

            let opts = SigningOptions {
                field_name: "Signature1".to_string(),
                reason: reason.clone(),
                location: location.clone(),
                contact_info: Some(email.clone()),
                rect,
                is_certification: certify,
                certification_permission: if certify { Some(doc_mdp_perm) } else { None },
                embed_ltv: ltv,
            };

            let signed_bytes = sign_pdf_document_advanced(
                doc,
                curr_page,
                &digital_id,
                &opts,
            )?;
            fs::write(&output, &signed_bytes)?;

            if cli.json {
                let info = serde_json::json!({
                    "status": "success",
                    "signer": signer,
                    "organization": org,
                    "email": email,
                    "reason": reason,
                    "location": location,
                    "page": page,
                    "visible": visible,
                    "is_certification": certify,
                    "certification_permission": if certify { Some(doc_mdp_perm.description()) } else { None },
                    "has_ltv": ltv,
                    "output": output.display().to_string(),
                });
                println!("{}", serde_json::to_string_pretty(&info)?);
            } else {
                println!("Successfully digitally signed document '{}'", input.display());
                println!("  Signer: {}", signer);
                println!("  Organization: {}", org);
                println!("  Reason: {}", reason);
                println!("  Location: {}", location);
                println!("  Certification (DocMDP): {}", if certify { doc_mdp_perm.description() } else { "None (Approval Signature)" });
                println!("  Long-Term Validation (LTV /DSS): {}", if ltv { "Embedded" } else { "No" });
                println!("  Standard: ISO 32000-1 Section 12.8 / PKCS#7 detached");
                println!("  Saved signed PDF to '{}'", output.display());
            }
        }

        Commands::Preflight {
            input,
            profile,
            fix,
            output,
        } => {
            let mut doc = PdfDocument::load_from_file(&input)?;
            let p_enum = match profile.to_lowercase().as_str() {
                "print" | "printproduction" => PreflightProfile::PrintProduction,
                "quality" | "generalquality" => PreflightProfile::GeneralQuality,
                _ => PreflightProfile::PdfA1b,
            };

            let report = run_preflight(&doc, p_enum);

            let mut fixup_summary = None;
            if fix {
                let out_path = output.ok_or_else(|| anyhow!("--output path is required when --fix is specified"))?;
                let summary = execute_preflight_fixup(&mut doc, PreflightFixup::FixAll)?;
                doc.save_to_file(&out_path)?;
                fixup_summary = Some(summary);
            }

            if cli.json {
                let mut val = serde_json::to_value(&report)?;
                if let Some(ref s) = fixup_summary {
                    val["fixup_applied"] = serde_json::Value::String(s.clone());
                }
                println!("{}", serde_json::to_string_pretty(&val)?);
            } else {
                println!("Preflight Audit for '{}' [{}]", input.display(), p_enum.display_name());
                println!("  Status: {}", if report.is_compliant { "✓ COMPLIANT" } else { "✗ NON-COMPLIANT" });
                println!("  Passed: {}, Warnings: {}, Errors: {}", report.total_passed, report.total_warnings, report.total_errors);
                for c in &report.checks {
                    let mark = if c.passed { "✓" } else { "✗" };
                    println!("    {} [{}] {}: {}", mark, c.category, c.title, c.details);
                }
                if let Some(ref s) = fixup_summary {
                    println!("\n  Fixups applied: {}", s);
                }
            }
        }

        Commands::Color { action } => match action {
            ColorCommands::EmbedOutputIntent {
                input,
                output,
                profile,
                condition,
                registry,
            } => {
                let mut doc = PdfDocument::load_from_file(&input)?;
                let p_enum = match profile.to_lowercase().as_str() {
                    "srgb" => StandardIccProfile::SRgb,
                    "adobergb" | "adobe" => StandardIccProfile::AdobeRgb1998,
                    "p3" | "displayp3" => StandardIccProfile::DisplayP3,
                    "fogra39" => StandardIccProfile::FOGRA39,
                    "gracol" => StandardIccProfile::GRACoL2006,
                    "japancolor" | "japan" => StandardIccProfile::JapanColor2001Coated,
                    "dotgain20" | "dotgain" => StandardIccProfile::DotGain20,
                    _ => StandardIccProfile::USWebCoatedSWOP,
                };

                let oi_id = embed_output_intent(&mut doc, p_enum, &condition, &registry)?;
                doc.save_to_file(&output)?;

                if cli.json {
                    let info = serde_json::json!({
                        "status": "success",
                        "profile": p_enum.display_name(),
                        "condition": condition,
                        "registry": registry,
                        "output_intent_id": oi_id.to_string(),
                        "output": output.display().to_string(),
                    });
                    println!("{}", serde_json::to_string_pretty(&info)?);
                } else {
                    println!("Successfully embedded Output Intent in '{}'", input.display());
                    println!("  Profile: {}", p_enum.display_name());
                    println!("  Condition: {}", condition);
                    println!("  Registry: {}", registry);
                    println!("  Saved PDF to '{}'", output.display());
                }
            }

            ColorCommands::ListOutputIntents { input } => {
                let doc = PdfDocument::load_from_file(&input)?;
                let intents = get_output_intents(&doc)?;

                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&intents)?);
                } else {
                    println!("Output Intents in '{}':", input.display());
                    if intents.is_empty() {
                        println!("  (No output intents configured)");
                    } else {
                        for (i, oi) in intents.iter().enumerate() {
                            println!("  {}. Subtype: {}", i + 1, oi.subtype);
                            println!("     Condition: {}", oi.output_condition_identifier);
                            println!("     Registry: {}", oi.registry_name);
                            if let Some(ref inf) = oi.info {
                                println!("     Info: {}", inf);
                            }
                        }
                    }
                }
            }

            ColorCommands::Convert {
                input,
                output,
                target,
            } => {
                let mut doc = PdfDocument::load_from_file(&input)?;
                let modified = convert_document_colors(&mut doc, &target)?;
                doc.save_to_file(&output)?;

                if cli.json {
                    let info = serde_json::json!({
                        "status": "success",
                        "target_color_space": target,
                        "modified_streams": modified,
                        "output": output.display().to_string(),
                    });
                    println!("{}", serde_json::to_string_pretty(&info)?);
                } else {
                    println!("Converted document colors for '{}'", input.display());
                    println!("  Target Color Space: {}", target);
                    println!("  Modified Content Streams: {}", modified);
                    println!("  Saved converted PDF to '{}'", output.display());
                }
            }
        },

        Commands::Prepress { action } => match action {
            PrepressCommands::Marks {
                input,
                output,
                trim,
                bleed,
                registration,
                color_bars,
                info,
                slug_margin,
                bleed_offset,
            } => {
                let mut doc = PdfDocument::load_from_file(&input)?;
                let cfg = PrinterMarksConfig {
                    trim_marks: trim,
                    bleed_marks: bleed,
                    registration_marks: registration,
                    color_bars,
                    page_information: info,
                    slug_margin,
                    bleed_offset,
                    ..Default::default()
                };
                let pages_marked = add_printer_marks(&mut doc, &cfg)?;
                doc.save_to_file(&output)?;

                if cli.json {
                    let res = serde_json::json!({
                        "status": "success",
                        "pages_marked": pages_marked,
                        "slug_margin": slug_margin,
                        "bleed_offset": bleed_offset,
                        "output": output.display().to_string(),
                    });
                    println!("{}", serde_json::to_string_pretty(&res)?);
                } else {
                    println!("Added prepress printer marks to '{}'", input.display());
                    println!("  Pages Marked: {}", pages_marked);
                    println!("  Slug Margin: {} pt, Bleed Offset: {} pt", slug_margin, bleed_offset);
                    println!("  Saved marked PDF to '{}'", output.display());
                }
            }

            PrepressCommands::Inks { input } => {
                let doc = PdfDocument::load_from_file(&input)?;
                let inks = get_document_inks(&doc);

                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&inks)?);
                } else {
                    println!("Document Ink Plates in '{}':", input.display());
                    for (i, ink) in inks.iter().enumerate() {
                        let kind = if ink.is_spot { "Spot Plate" } else { "Process Plate" };
                        println!(
                            "  {}. {} [{}] CMYK: ({:.2}, {:.2}, {:.2}, {:.2})",
                            i + 1, ink.name, kind, ink.process_cmyk.0, ink.process_cmyk.1, ink.process_cmyk.2, ink.process_cmyk.3
                        );
                    }
                }
            }

            PrepressCommands::Flatten {
                input,
                output,
                balance,
            } => {
                let mut doc = PdfDocument::load_from_file(&input)?;
                let cfg = TransparencyFlattenConfig {
                    raster_vector_balance: balance,
                    ..Default::default()
                };
                let rep = flatten_document_transparency(&mut doc, &cfg)?;
                doc.save_to_file(&output)?;

                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&rep)?);
                } else {
                    println!("Flattened document transparency in '{}'", input.display());
                    println!("  ExtGState Flattened: {}", rep.extgstate_flattened);
                    println!("  Transparency Groups Removed: {}", rep.groups_removed);
                    println!("  Pages Processed: {}", rep.total_pages_processed);
                    println!("  Saved flattened PDF to '{}'", output.display());
                }
            }

            PrepressCommands::Trap {
                input,
                output,
                trap_width,
                black_width,
                name,
            } => {
                let mut doc = PdfDocument::load_from_file(&input)?;
                if let Some(out) = output {
                    let preset = TrapPreset {
                        name: name.unwrap_or_else(|| "Prepress Trap Preset".into()),
                        trap_width: trap_width.unwrap_or(0.25),
                        black_width: black_width.unwrap_or(0.50),
                        ..Default::default()
                    };
                    set_trap_presets(&mut doc, std::slice::from_ref(&preset))?;
                    doc.save_to_file(&out)?;

                    if cli.json {
                        let res = serde_json::json!({
                            "status": "success",
                            "preset": preset,
                            "output": out.display().to_string(),
                        });
                        println!("{}", serde_json::to_string_pretty(&res)?);
                    } else {
                        println!("Configured trap preset in '{}'", input.display());
                        println!("  Name: {}", preset.name);
                        println!("  Trap Width: {} pt, Black Width: {} pt", preset.trap_width, preset.black_width);
                        println!("  Saved PDF to '{}'", out.display());
                    }
                } else {
                    let presets = get_trap_presets(&doc);
                    if cli.json {
                        println!("{}", serde_json::to_string_pretty(&presets)?);
                    } else {
                        println!("Trap Presets in '{}':", input.display());
                        for (i, p) in presets.iter().enumerate() {
                            println!("  {}. '{}': trap_width={} pt, black_width={} pt", i + 1, p.name, p.trap_width, p.black_width);
                        }
                    }
                }
            }
        },

        Commands::Standards { action } => match action {
            StandardsCommands::Convert {
                input,
                output,
                format,
                icc,
            } => {
                let mut doc = PdfDocument::load_from_file(&input)?;
                let std_enum = match format.to_lowercase().replace(['-', '/'], "").as_str() {
                    "pdfa1b" | "a1b" => StandardFormat::PdfA1b,
                    "pdfa2b" | "a2b" => StandardFormat::PdfA2b,
                    "pdfa3b" | "a3b" => StandardFormat::PdfA3b,
                    "pdfx1a" | "x1a" => StandardFormat::PdfX1a,
                    "pdfx3" | "x3" => StandardFormat::PdfX3,
                    "pdfx4" | "x4" => StandardFormat::PdfX4,
                    "pdfe1" | "e1" => StandardFormat::PdfE1,
                    _ => StandardFormat::PdfA1b,
                };

                let icc_enum = match icc.to_lowercase().as_str() {
                    "swop" => StandardIccProfile::USWebCoatedSWOP,
                    "fogra39" | "fogra" => StandardIccProfile::FOGRA39,
                    "gracol" => StandardIccProfile::GRACoL2006,
                    "japancolor" | "japan" => StandardIccProfile::JapanColor2001Coated,
                    _ => StandardIccProfile::SRgb,
                };

                let cfg = StandardConversionConfig {
                    target: std_enum,
                    icc_profile: icc_enum,
                    ..Default::default()
                };

                let rep = convert_to_standard(&mut doc, &cfg)?;
                doc.save_to_file(&output)?;

                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&rep)?);
                } else {
                    println!("Converted document to standard '{}'", std_enum.display_name());
                    println!("  Version: %PDF-{}.{}", rep.target_version.0, rep.target_version.1);
                    for a in &rep.actions_applied {
                        println!("  • {}", a);
                    }
                    println!("  Saved compliant PDF to '{}'", output.display());
                }
            }

            StandardsCommands::Validate { input, format } => {
                let doc = PdfDocument::load_from_file(&input)?;
                let std_enum = match format.to_lowercase().replace(['-', '/'], "").as_str() {
                    "pdfa1b" | "a1b" => StandardFormat::PdfA1b,
                    "pdfa2b" | "a2b" => StandardFormat::PdfA2b,
                    "pdfa3b" | "a3b" => StandardFormat::PdfA3b,
                    "pdfx1a" | "x1a" => StandardFormat::PdfX1a,
                    "pdfx3" | "x3" => StandardFormat::PdfX3,
                    "pdfx4" | "x4" => StandardFormat::PdfX4,
                    "pdfe1" | "e1" => StandardFormat::PdfE1,
                    _ => StandardFormat::PdfA1b,
                };

                let rep = validate_standard_compliance(&doc, std_enum);

                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&rep)?);
                } else {
                    let status = if rep.is_compliant { "COMPLIANT" } else { "NON-COMPLIANT" };
                    println!("Standards Compliance Audit for '{}': {}", std_enum.display_name(), status);
                    for check in &rep.checks {
                        let sym = if check.passed { "✓ PASS" } else { "✗ FAIL" };
                        println!("  {} [{}]: {}", sym, check.code, check.description);
                        println!("     Details: {}", check.details);
                    }
                }

                if !rep.is_compliant {
                    std::process::exit(1);
                }
            }
        },

        Commands::TextSpans { input, page } => {
            let doc = PdfDocument::load_from_file(&input)?;
            let page_idx = page.saturating_sub(1);
            let spans = extract_page_text_spans(&doc, page_idx)?;

            if cli.json {
                let json_spans: Vec<_> = spans
                    .iter()
                    .map(|s| {
                        serde_json::json!({
                            "text": s.text,
                            "x": s.x,
                            "y": s.y,
                            "font_name": s.font_name,
                            "font_size": s.font_size,
                            "color": [s.color.0, s.color.1, s.color.2],
                            "char_spacing": s.char_spacing,
                            "word_spacing": s.word_spacing,
                        })
                    })
                    .collect();
                let json = serde_json::json!({
                    "path": input.to_string_lossy(),
                    "page": page,
                    "span_count": spans.len(),
                    "spans": json_spans,
                });
                println!("{}", serde_json::to_string_pretty(&json)?);
            } else {
                println!("=== Extracted Text Spans (Page {}) ===", page);
                println!("Found {} text spans:", spans.len());
                for (i, span) in spans.iter().enumerate() {
                    println!(
                        "[{}] \"{}\" at ({:.1}, {:.1}) | Font: {} {:.1}pt | RGB: ({:.2}, {:.2}, {:.2})",
                        i + 1, span.text, span.x, span.y, span.font_name, span.font_size, span.color.0, span.color.1, span.color.2
                    );
                }
            }
        }

        Commands::EditText {
            input,
            output,
            page,
            find,
            replace,
            font,
            font_size,
            color,
        } => {
            let mut doc = PdfDocument::load_from_file(&input)?;
            let page_idx = page.saturating_sub(1);

            let parsed_color = color.and_then(|c_str| {
                let parts: Vec<&str> = c_str.split(',').collect();
                if parts.len() == 3 {
                    if let (Ok(r), Ok(g), Ok(b)) = (
                        parts[0].trim().parse::<f32>(),
                        parts[1].trim().parse::<f32>(),
                        parts[2].trim().parse::<f32>(),
                    ) {
                        return Some((r, g, b));
                    }
                }
                None
            });

            let count = replace_text_span_in_page(
                &mut doc,
                page_idx,
                &find,
                &replace,
                font.as_deref(),
                font_size,
                parsed_color,
            )?;

            doc.save_to_file(&output)?;

            if cli.json {
                let json = serde_json::json!({
                    "input": input.to_string_lossy(),
                    "output": output.to_string_lossy(),
                    "page": page,
                    "find": find,
                    "replace": replace,
                    "font": font,
                    "replacements_count": count,
                });
                println!("{}", serde_json::to_string_pretty(&json)?);
            } else {
                println!("Replaced {} occurrence(s) of '{}' with '{}' on page {}.", count, find, replace, page);
                println!("Saved updated document to: {}", output.display());
            }
        }

        Commands::FromMarkdown {
            input,
            output,
            title,
            font_size,
            margin,
        } => {
            let md_content = fs::read_to_string(&input)?;
            let options = MarkdownPdfOptions {
                page_size: ravpdf_core::document::paper_sizes::A4,
                margin,
                base_font_size: font_size,
                line_spacing: font_size * 1.36,
                title,
                include_page_numbers: true,
            };

            let doc = create_pdf_from_markdown(&md_content, &options)?;
            let count = doc.page_count();
            doc.save_to_file(&output)?;

            if cli.json {
                let json = serde_json::json!({
                    "input": input.to_string_lossy(),
                    "output": output.to_string_lossy(),
                    "page_count": count,
                });
                println!("{}", serde_json::to_string_pretty(&json)?);
            } else {
                println!("Generated {} page(s) PDF document from Markdown: {}", count, output.display());
            }
        }
    }

    Ok(())
}

fn parse_rect(s: &str) -> Option<[f64; 4]> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() == 4 {
        if let (Ok(x1), Ok(y1), Ok(x2), Ok(y2)) = (
            parts[0].trim().parse::<f64>(),
            parts[1].trim().parse::<f64>(),
            parts[2].trim().parse::<f64>(),
            parts[3].trim().parse::<f64>(),
        ) {
            return Some([x1, y1, x2, y2]);
        }
    }
    None
}

fn parse_form_format(s: &str) -> Option<FormDataFormat> {
    match s.to_lowercase().as_str() {
        "fdf" => Some(FormDataFormat::Fdf),
        "xfdf" => Some(FormDataFormat::Xfdf),
        "csv" => Some(FormDataFormat::Csv),
        "xml" => Some(FormDataFormat::Xml),
        "json" => Some(FormDataFormat::Json),
        _ => None,
    }
}

fn parse_tab_order(s: &str) -> TabOrder {
    match s.to_lowercase().as_str() {
        "col" | "column" => TabOrder::Column,
        "struct" | "structure" => TabOrder::Structure,
        "manual" => TabOrder::Manual,
        _ => TabOrder::Row,
    }
}

fn parse_render_mode(s: &str) -> ThreeDRenderMode {
    match s.to_lowercase().as_str() {
        "wireframe" => ThreeDRenderMode::Wireframe,
        "solidwireframe" => ThreeDRenderMode::SolidWireframe,
        "transparent" => ThreeDRenderMode::Transparent,
        "transparentwireframe" => ThreeDRenderMode::TransparentWireframe,
        "shaded" | "shadedillustration" => ThreeDRenderMode::ShadedIllustration,
        "hiddenline" => ThreeDRenderMode::HiddenLine,
        "boundingbox" => ThreeDRenderMode::BoundingBox,
        _ => ThreeDRenderMode::Solid,
    }
}

fn parse_lighting(s: &str) -> ThreeDLighting {
    match s.to_lowercase().as_str() {
        "white" => ThreeDLighting::White,
        "night" => ThreeDLighting::Night,
        "hard" => ThreeDLighting::Hard,
        "primary" => ThreeDLighting::Primary,
        "blue" => ThreeDLighting::Blue,
        "red" => ThreeDLighting::Red,
        "cube" => ThreeDLighting::Cube,
        "cad" => ThreeDLighting::Cad,
        "headlamp" => ThreeDLighting::Headlamp,
        "none" => ThreeDLighting::None,
        _ => ThreeDLighting::Day,
    }
}
