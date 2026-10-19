//! Batch Processing and Action Wizard Engine.
//!
//! Traceability:
//! - FR-30.1 (Custom multi-step action sequences combining tools in configurable order)
//! - FR-30.2 (Apply action sequences to selected files or directories)
//! - FR-30.4 (Supported automatable operations: Watermark, Bates, Flatten, Optimize, Redact, Encrypt)
//! - FR-30.8 (Progress indicators and execution logs)

use crate::bates::apply_bates_numbering;
use crate::optimize::optimize_document;
use crate::redact::redact_pattern;
use crate::watermark::apply_text_watermark;
use ravpdf_core::document::PdfDocument;
use ravpdf_crypto::{encrypt_document, Permissions};
use ravpdf_forms::flatten_form;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// An individual automatable step in a batch sequence (FR-30.4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BatchAction {
    Watermark {
        text: String,
        opacity: f32,
    },
    BatesNumbering {
        prefix: String,
        start_number: u32,
        digits: usize,
    },
    FlattenForms,
    Optimize,
    RedactPattern {
        pattern: String,
        replacement: String,
    },
    Encrypt {
        user_pass: String,
        owner_pass: String,
    },
}

impl BatchAction {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Watermark { .. } => "Add Watermark",
            Self::BatesNumbering { .. } => "Apply Bates Numbering",
            Self::FlattenForms => "Flatten Interactive Forms",
            Self::Optimize => "Optimize & Reduce Size",
            Self::RedactPattern { .. } => "Redact Pattern",
            Self::Encrypt { .. } => "Password Encrypt",
        }
    }
}

/// Configuration for a batch execution job (FR-30.1, FR-30.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchJob {
    pub input_files: Vec<PathBuf>,
    pub output_directory: PathBuf,
    pub actions: Vec<BatchAction>,
}

/// Execution outcome for a single processed file (FR-30.8).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchFileResult {
    pub path: PathBuf,
    pub success: bool,
    pub error_message: Option<String>,
}

/// Aggregate results from executing a batch sequence (FR-30.8).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BatchResult {
    pub total_files: usize,
    pub successful_files: usize,
    pub failed_files: usize,
    pub details: Vec<BatchFileResult>,
}

/// Executes a configured batch job over all target files (FR-30.1 - FR-30.4).
pub fn execute_batch(job: &BatchJob) -> BatchResult {
    let mut result = BatchResult {
        total_files: job.input_files.len(),
        successful_files: 0,
        failed_files: 0,
        details: Vec::new(),
    };

    if !job.output_directory.exists() {
        let _ = std::fs::create_dir_all(&job.output_directory);
    }

    for path in &job.input_files {
        match process_single_file(path, &job.output_directory, &job.actions) {
            Ok(_) => {
                result.successful_files += 1;
                result.details.push(BatchFileResult {
                    path: path.clone(),
                    success: true,
                    error_message: None,
                });
            }
            Err(e) => {
                result.failed_files += 1;
                result.details.push(BatchFileResult {
                    path: path.clone(),
                    success: false,
                    error_message: Some(e.to_string()),
                });
            }
        }
    }

    result
}

fn process_single_file(
    input_path: &Path,
    output_dir: &Path,
    actions: &[BatchAction],
) -> anyhow::Result<()> {
    let mut doc = PdfDocument::load_from_file(input_path)?;

    for action in actions {
        match action {
            BatchAction::Watermark { text, opacity } => {
                apply_text_watermark(&mut doc, text, *opacity, 45.0, 54.0)?;
            }
            BatchAction::BatesNumbering {
                prefix,
                start_number,
                digits,
            } => {
                apply_bates_numbering(&mut doc, prefix, *start_number, *digits, "")?;
            }
            BatchAction::FlattenForms => {
                flatten_form(&mut doc)?;
            }
            BatchAction::Optimize => {
                let _ = optimize_document(&mut doc)?;
            }
            BatchAction::RedactPattern {
                pattern,
                replacement,
            } => {
                let _ = redact_pattern(&mut doc, pattern, replacement)?;
            }
            BatchAction::Encrypt {
                user_pass,
                owner_pass,
            } => {
                encrypt_document(&mut doc, user_pass, owner_pass, Permissions::default())?;
            }
        }
    }

    let file_name = input_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("processed.pdf");
    let out_file_path = output_dir.join(file_name);
    doc.save_to_file(out_file_path)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_action_execution() {
        let temp_dir = std::env::temp_dir().join("ravpdf_batch_test");
        let input_dir = temp_dir.join("inputs");
        let output_dir = temp_dir.join("outputs");
        let _ = std::fs::create_dir_all(&input_dir);
        let _ = std::fs::create_dir_all(&output_dir);

        // Create 2 test input documents
        let file1 = input_dir.join("doc1.pdf");
        let file2 = input_dir.join("doc2.pdf");

        let mut doc1 = PdfDocument::new();
        doc1.add_blank_page(612.0, 792.0);
        doc1.save_to_file(&file1).expect("save doc1");

        let mut doc2 = PdfDocument::new();
        doc2.add_blank_page(612.0, 792.0);
        doc2.save_to_file(&file2).expect("save doc2");

        let job = BatchJob {
            input_files: vec![file1, file2],
            output_directory: output_dir.clone(),
            actions: vec![
                BatchAction::Watermark {
                    text: "BATCH APPROVED".into(),
                    opacity: 0.25,
                },
                BatchAction::Optimize,
            ],
        };

        let result = execute_batch(&job);
        assert_eq!(result.total_files, 2);
        assert_eq!(result.successful_files, 2);
        assert_eq!(result.failed_files, 0);

        // Verify outputs exist and are valid PDFs
        let out1 = output_dir.join("doc1.pdf");
        let out2 = output_dir.join("doc2.pdf");
        assert!(out1.exists());
        assert!(out2.exists());

        let loaded1 = PdfDocument::load_from_file(&out1).expect("load out1");
        assert_eq!(loaded1.page_count(), 1);

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
