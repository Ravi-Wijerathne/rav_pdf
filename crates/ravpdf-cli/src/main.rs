//! RavPDF Command-Line Interface (CLI).
//!
//! Traceability: FR-37.1 - FR-37.6 (Complete CLI command suite).

use clap::{Parser as ClapParser, Subcommand};
use ravpdf_core::document::PdfDocument;
use anyhow::Result;
use ravpdf_crypto::{decrypt_document, encrypt_document, Permissions};
use ravpdf_edit::{merge_documents, rotate_page, split_document};
use ravpdf_forms::flatten_form;
use ravpdf_render::{pixmap_to_png, render_page, Color, RenderOptions};
use ravpdf_tools::{
    add_measurement_line_annotation, apply_bates_numbering, apply_text_watermark,
    audit_accessibility, calculate_distance, create_portfolio,
    export_comments_to_csv, export_page_to_image, export_to_html, export_to_plain_text,
    extract_all_text, extract_portfolio_item, get_document_comments, get_document_info,
    get_portfolio, linearize_document, optimize_document, redact_pattern, remediate_all_automatic,
    AccessibilityStatus, ExportImageFormat, LinearUnit, PortfolioItem, PortfolioLayout,
    ScaleRatio,
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
        } => {
            let mut doc = PdfDocument::load_from_file(&input)?;
            let owner = if owner_password.is_empty() {
                &password
            } else {
                &owner_password
            };
            encrypt_document(&mut doc, &password, owner, Permissions::all_permitted())?;
            doc.save_to_file(&output)?;
            println!("Document encrypted with AES-128: {}", output.display());
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
    }

    Ok(())
}
