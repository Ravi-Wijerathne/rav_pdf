<p align="center">
  <img src="assets/logo.png" alt="RavPDF Logo" width="160" height="160" />
</p>

<h1 align="center">RavPDF</h1>

<p align="center">
  <strong>High-Performance, Memory-Safe PDF Editor in Rust</strong>
</p>

<p align="center">
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/rust-1.80%2B-orange.svg?style=flat-square&logo=rust" alt="Rust" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg?style=flat-square" alt="License" /></a>
  <a href="docs/SRS.md"><img src="https://img.shields.io/badge/standards-ISO%2032000--1%20%7C%20ISO%2032000--2-emerald.svg?style=flat-square" alt="Standards" /></a>
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey.svg?style=flat-square" alt="Platform" />
  <img src="https://img.shields.io/badge/tests-passing-brightgreen.svg?style=flat-square" alt="Tests" />
</p>

**RavPDF** is a professional-grade, cross-platform PDF suite written entirely in **Rust**. Built from the ground up to achieve feature parity with **Adobe Acrobat Pro DC**, RavPDF delivers uncompromising performance, true 64-bit zero-cost abstractions, multi-threaded rendering, and the ironclad memory safety guarantees of the Rust language.

RavPDF provides both an interactive, hardware-accelerated **Desktop GUI** and a feature-complete headless **Command-Line Interface (CLI)** for scripting, cloud pipelines, and automated document processing.

---

## Table of Contents

- [Why RavPDF?](#why-ravpdf)
- [System Architecture](#system-architecture)
- [Workspace Crates](#workspace-crates)
- [Core Features & SRS Traceability](#core-features--srs-traceability)
- [Getting Started](#getting-started)
  - [Prerequisites](#prerequisites)
  - [Building from Source](#building-from-source)
- [Running the Desktop GUI (`ravpdf-gui`)](#running-the-desktop-gui-ravpdf-gui)
- [Using the Command-Line Interface (`ravpdf`)](#using-the-command-line-interface-ravpdf)
  - [Document Inspection](#document-inspection)
  - [Merging and Splitting](#merging-and-splitting)
  - [Security and Digital Signatures](#security-and-digital-signatures)
  - [Forms, Annotations, and Watermarks](#forms-annotations-and-watermarks)
  - [Prepress, Standards, and Conversion](#prepress-standards-and-conversion)
  - [OCR and Optimization](#ocr-and-optimization)
- [Standards & Conformance](#standards--conformance)
- [Testing & Quality Assurance](#testing--quality-assurance)
- [Contributing & License](#contributing--license)

---

## Why RavPDF?

- **Memory Safety Without Vulnerabilities**: C and C++ PDF implementations have historically suffered from critical memory vulnerabilities (buffer overflows, heap corruptions, use-after-free). RavPDF eliminates these classes of bugs by design through Rust's ownership model and strict safety audits.
- **ISO 32000-1 / ISO 32000-2 Conformance**: Parses, modifies, and serializes arbitrary PDF files—from legacy PDF 1.0 specifications up to modern PDF 2.0 object streams, cross-reference streams, and encryption handlers.
- **Deterministic Incremental Updates**: Supports non-destructive incremental saves to preserve existing digital signatures and byte integrity, as well as full-rewrite garbage-collected serialization.
- **Unified Engine**: The same modular engine powers the interactive desktop application and the headless command-line interface.

---

## System Architecture

RavPDF follows a strict layered architecture with clean separation between parser foundations, cryptographic primitives, rendering pipelines, editing services, and user interfaces:

```
┌────────────────────────────────────────────────────────────────────────┐
│                          PRESENTATION LAYER                            │
│   ┌───────────────────────────────┐  ┌──────────────────────────────┐  │
│   │   ravpdf-gui (Desktop GUI)    │  │     ravpdf-cli (CLI Utility) │  │
│   │   (eframe / egui multi-tab)   │  │     (clap subcommand suite)  │  │
│   └───────────────┬───────────────┘  └──────────────┬───────────────┘  │
├───────────────────┴─────────────────────────────────┴──────────────────┤
│                          APPLICATION SERVICES                          │
│   ┌─────────────────────────────────────────────────────────────────┐  │
│   │  ravpdf-tools                                                   │  │
│   │  (OCR, Preflight, Redaction, Standards, Bates, Compare, JS)     │  │
│   └──────────────────────┬──────────────────────────────────────────┘  │
├──────────────────────────┼─────────────────────────────────────────────┤
│                          DOMAIN ENGINES                                │
│   ┌───────────────────┐  │  ┌───────────────────┐  ┌────────────────┐  │
│   │   ravpdf-edit     │◄─┤  │   ravpdf-forms    │  │  ravpdf-annot  │  │
│   │ (Text, Page, Img) │  │  │ (AcroForms, XFA)  │  │ (Markup, Ink)  │  │
│   └─────────┬─────────┘  │  └─────────┬─────────┘  └────────┬───────┘  │
│             │            │            │                     │          │
│             ▼            ▼            ▼                     ▼          │
│   ┌───────────────────────────────────┐  ┌──────────────────────────┐  │
│   │          ravpdf-crypto            │  │      ravpdf-render       │  │
│   │ (AES-256, PAdES, CMS/PKCS#7, Cert)│  │ (TinySkia 2D Rasterizer) │  │
│   └─────────────────┬─────────────────┘  └──────────┬───────────────┘  │
├─────────────────────┴───────────────────────────────┴──────────────────┤
│                           CORE PDF ENGINE                              │
│   ┌─────────────────────────────────────────────────────────────────┐  │
│   │  ravpdf-core                                                    │  │
│   │  (Lexer, Parser, AST Object Graph, XRef, Streams, Serializer)   │  │
│   └─────────────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────────┘
```

---

## Workspace Crates

The project is organized into modular Cargo workspace crates:

| Crate | Directory | Purpose |
| :--- | :--- | :--- |
| **`ravpdf-core`** | [`crates/ravpdf-core`](crates/ravpdf-core) | Core PDF parser, lexer, AST objects, cross-reference tables/streams, filter pipelines (`Flate`, `ASCIIHex`, `ASCII85`, `RunLength`, `LZW`), CMaps, outlines, named destinations, and ISO 32000 writer. |
| **`ravpdf-render`** | [`crates/ravpdf-render`](crates/ravpdf-render) | 2D graphics rasterization pipeline powered by `tiny-skia`. Evaluates content stream operators, color spaces (`DeviceRGB`, `DeviceCMYK`, `DeviceGray`), transformations, and visual appearances. |
| **`ravpdf-edit`** | [`crates/ravpdf-edit`](crates/ravpdf-edit) | Content stream editing, text block reflow, font substitution, image extraction/transformation/Z-ordering, page geometry box manipulation (`MediaBox`, `CropBox`, `BleedBox`, `TrimBox`, `ArtBox`), page extraction/deletion/rotation, and PDF creation from Text, Markdown, or Images. |
| **`ravpdf-annot`** | [`crates/ravpdf-annot`](crates/ravpdf-annot) | Annotations and markups: text highlights, underlines, strikethroughs, squigglies, sticky notes, typewriter, callouts with leader lines, vector shapes (rectangles, ellipses, polygons), freehand ink drawings, and dynamic stamps. |
| **`ravpdf-forms`** | [`crates/ravpdf-forms`](crates/ravpdf-forms) | AcroForms and interactive form fields: text fields, checkboxes, radio groups, combo boxes, list boxes, signature fields, automatic form field detection from scans, tab ordering, data exchange (FDF, XFDF, JSON), and static form flattening. |
| **`ravpdf-crypto`** | [`crates/ravpdf-crypto`](crates/ravpdf-crypto) | Security handler: Standard encryption (RC4 40/128-bit, AES-128, AES-256 ISO 32000-2 R5/R6), granular permission masks, ASN.1 parsing, cryptographic digital signatures (PAdES / CAdES, PKCS#7 / CMS), and digital certificate management. |
| **`ravpdf-tools`** | [`crates/ravpdf-tools`](crates/ravpdf-tools) | Advanced document tools: True irreversible redaction, OCR text injection, visual & structural document diffing, headers/footers, Bates numbering, watermarks, metadata & XMP, full-text search, standards compliance (PDF/A, PDF/X, PDF/UA, PDF/E), preflight & print production, optimization/linearization, 3D & rich media, attachments & portfolios, sandboxed JavaScript engine, and batch action automation. |
| **`ravpdf-cli`** | [`crates/ravpdf-cli`](crates/ravpdf-cli) | Unified command-line interface (`ravpdf`) exposing all editing, security, conversion, and preflight tools. |
| **`ravpdf-gui`** | [`crates/ravpdf-gui`](crates/ravpdf-gui) | Multi-tab desktop application built with `eframe` and `egui` featuring continuous page scrolling, thumbnail navigation, on-canvas form creation, and rich editing toolbars. |

---

## Core Features & SRS Traceability

Every subsystem in RavPDF directly maps to specifications detailed in the [Software Requirements Specification (`docs/SRS.md`)](docs/SRS.md):

- **Viewing & Navigation (FR-01)**: Multi-tab workspace, continuous scrolling, Fit-Width/Fit-Page, outline/bookmark hierarchy, named destinations, and thumbnail grids.
- **Creation & Conversion (FR-02, FR-03)**: Compile PDFs from plain text, Markdown, images, and RTF; export to images (PNG, JPEG, WebP), plain text, HTML, CSV, and PostScript.
- **Content & Page Editing (FR-04, FR-05, FR-06, FR-07)**: In-place text replacement, alignment guides, font fallback suggestions, image cropping/scaling/opacity/Z-ordering, page rotation, insertion, deletion, reordering, and document split/merge workflows.
- **Annotations & Drawing (FR-08, FR-09, FR-10)**: Highlights, strikeout, free-text typewriter, callouts with elbow leader lines, vector geometry, freehand ink with stroke eraser, and dynamic date/time stamps.
- **Interactive Forms (FR-11, FR-12)**: Full AcroForms support, heuristic auto-detection of fields from flat scans, automatic tab order computation, FDF/XFDF/JSON data exchange, form flattening, and sandboxed JavaScript calculations.
- **Security & Digital Signatures (FR-13, FR-14, FR-15)**: Password encryption (AES-256), permission bitmasks, PAdES/CAdES digital signatures with LTV timestamping, and irreversible redaction that purges underlying text/vector data.
- **Document Integrity & Comparison (FR-16, FR-17, FR-18)**: Side-by-side visual difference highlighting, OCR recognition injection, headers/footers with dynamic tokens (`&[Page]`, `&[Date]`), watermarking, and legal Bates numbering.
- **Print Production & Standards (FR-21, FR-22, FR-23, FR-24)**: Preflight validation, ICC profile management, ink coverage & plate separations (CMYK + spot inks), trapping presets, printer crop/bleed marks, transparency flattening, and compliance validation for PDF/A (1a/1b/2b/3b), PDF/X, PDF/UA, and PDF/E.
- **Optimization & Linearization (FR-25)**: Content stream compression, duplicate resource stripping, unreferenced object removal, and Fast Web View (linearization) hint table generation.
- **Automation & Extensibility (FR-30, FR-36, FR-37)**: Action Wizard multi-stage batch automation, sandboxed plugin architecture, and complete CLI command coverage.

---

## Getting Started

### Prerequisites

- **Rust 1.80+** (Stable channel recommended)
- **Cargo** (Included with the Rust toolchain)
- Operating System:
  - **Windows**: Windows 10 or 11 (x64 / ARM64)
  - **macOS**: macOS 12+ (Apple Silicon or Intel)
  - **Linux**: Any modern distribution with `glibc 2.31+` and standard windowing libraries (X11 / Wayland)

### Building from Source

Clone the repository and compile the workspace in release mode:

```bash
# Clone the repository
git clone https://github.com/your-org/rav_pdf.git
cd rav_pdf

# Check workspace compilation
cargo check --workspace

# Build all binaries and crates with optimizations
cargo build --workspace --release
```

Compiled binaries will be available in the `target/release/` directory:
- `target/release/ravpdf` (CLI)
- `target/release/ravpdf-gui` (Desktop GUI)

---

## Running the Desktop GUI (`ravpdf-gui`)

Launch the cross-platform GUI application:

```bash
cargo run --release --bin ravpdf-gui
```

### Key GUI Shortcuts & Capabilities
- **File Management**: Open documents, save incremental updates, export flattened copies.
- **Navigation**: `Page Up` / `Page Down`, `Home` / `End`, direct "Go to Page" navigation (`Ctrl+G`), and interactive Bookmarks panel.
- **Zoom & Viewing**: Continuous scroll, `Ctrl + MouseWheel` zoom, Fit to Page (`Ctrl+0`), Fit to Width (`Ctrl+1`), and Actual Size 100% (`Ctrl+2`).
- **Search**: Interactive document search (`Ctrl+F`) with match count and jump-to-result navigation.
- **Annotation Tools**: Highlights, sticky notes, rectangles, ellipses, ink pens, typewriter, and stamps.
- **Page Management**: Reorder, rotate, crop, duplicate, or delete pages visually.
- **Forms**: On-canvas field placement, inspector panel for field naming and validation, and one-click form flattening.

---

## Using the Command-Line Interface (`ravpdf`)

RavPDF includes a powerful CLI utility (`ravpdf`) designed for automation, servers, and scripts.

```bash
cargo run --release --bin ravpdf -- --help
```

### Document Inspection
```bash
# Display metadata, page count, PDF version, and encryption status
ravpdf info input.pdf

# Output detailed inspection metrics in structured JSON
ravpdf --json info input.pdf
```

### Merging and Splitting
```bash
# Merge multiple documents into one
ravpdf merge doc1.pdf doc2.pdf doc3.pdf --output merged.pdf

# Split a document into individual single pages
ravpdf split input.pdf --pages --output-dir ./pages/

# Split a document by file size (e.g. 5 MB chunks)
ravpdf split input.pdf --size-mb 5 --output-dir ./chunks/

# Split document hierarchically based on top-level bookmarks
ravpdf split input.pdf --bookmarks --output-dir ./chapters/
```

### Security and Digital Signatures
```bash
# Encrypt a PDF with AES-256 and restrict printing/modifications
ravpdf encrypt input.pdf --output secure.pdf \
  --user-password "read123" \
  --owner-password "admin123" \
  --algorithm aes256 \
  --no-print --no-modify

# Decrypt a protected document
ravpdf decrypt secure.pdf --output plain.pdf --password "admin123"

# Digitally sign a document using a PKCS#12 Digital ID certificate
ravpdf sign document.pdf --output signed.pdf \
  --cert id.p12 --password "certpass" \
  --reason "Approval" --location "Headquarters"
```

### Forms, Annotations, and Watermarks
```bash
# Auto-detect form fields on scanned pages and save new form
ravpdf form autodetect scanned.pdf --output form_ready.pdf

# Export form data to JSON or XFDF
ravpdf form export filled.pdf --format json --output data.json

# Import data into form fields
ravpdf form import blank_form.pdf --data data.json --output populated.pdf

# Permanently flatten all interactive form fields into page content
ravpdf form flatten populated.pdf --output flat.pdf

# Apply diagonal watermark across all pages
ravpdf watermark input.pdf --output watermarked.pdf \
  --text "CONFIDENTIAL" --opacity 0.3 --rotation 45

# Apply legal Bates numbering
ravpdf bates input.pdf --output numbered.pdf \
  --prefix "CASE-2026-" --digits 6 --start 1 --position bottom-right
```

### Prepress, Standards, and Conversion
```bash
# Validate compliance against PDF/A-1b or PDF/X-1a
ravpdf standards validate document.pdf --standard pdf-a-1b

# Convert document to archival PDF/A-2b standard
ravpdf standards convert input.pdf --standard pdf-a-2b --output archival.pdf

# Run preflight inspection profile
ravpdf preflight input.pdf --profile "Magazine-Print"

# Generate color separations and print marks
ravpdf prepress marks input.pdf --output prepress.pdf --crop --bleed --slug
```

### OCR and Optimization
```bash
# Perform optical character recognition and generate searchable PDF
ravpdf ocr scanned.pdf --output searchable.pdf --mode searchable-image

# Optimize PDF size by stripping unneeded metadata and recompressing streams
ravpdf optimize input.pdf --output optimized.pdf --compress-streams --remove-unreferenced

# Linearize PDF for Fast Web View (byte-range streaming)
ravpdf linearize input.pdf --output web_ready.pdf
```

---

## Standards & Conformance

RavPDF is engineered strictly according to international document processing specifications:

- **ISO 32000-1:2008**: Document management — Portable document format (PDF 1.7)
- **ISO 32000-2:2020**: Document management — Portable document format (PDF 2.0)
- **ISO 19005 (PDF/A-1, PDF/A-2, PDF/A-3, PDF/A-4)**: Electronic document file format for long-term preservation
- **ISO 15930 (PDF/X-1a, PDF/X-3, PDF/X-4)**: Graphic technology — Prepress digital data exchange
- **ISO 14289 (PDF/UA-1)**: Universal accessibility in electronic document formats
- **ISO 24517 (PDF/E-1)**: Engineering document format using PDF
- **ETSI TS 103 172 / ETSI TS 101 733**: PAdES / CAdES Baseline Digital Signature profiles

---

## Testing & Quality Assurance

RavPDF includes a rigorous test suite covering unit tests, end-to-end integration workflows, and adversarial test cases (malformed object streams, broken xref tables, corrupt compression filters, circular references, and cryptographic tampering):

```bash
# Run the entire workspace test suite
cargo test --workspace

# Run CLI integration tests
cargo test -p ravpdf-cli --test cli_tests

# Run core security and adversarial parser tests
cargo test -p ravpdf-core --test adversarial_objstm_filters
cargo test -p ravpdf-crypto --test signature_incremental_adversarial
```

---

## Contributing & License

Contributions are welcome! Please refer to the [System Requirements Specification (`docs/SRS.md`)](docs/SRS.md) and project directives in [`AGENTS.md`](AGENTS.md) before submitting pull requests or proposing architectural modifications.

This project is dual-licensed under:
- **MIT License** ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
- **Apache License, Version 2.0** ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)

at your option.

