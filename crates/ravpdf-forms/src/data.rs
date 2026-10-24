//! Form Data Import and Export Engine (FDF, XFDF, CSV, XML, JSON).
//!
//! Traceability:
//! - FR-11.3.2 (Import and export form data as FDF, XFDF, XML, CSV, or TXT)
//! - ISO 32000-1 Clause 12.7.7 (Forms Data Format - FDF)
//! - ISO 19444-1 (XML Forms Data Format - XFDF)

use crate::field::{get_form_fields, set_form_field_value, FormFieldType};
use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Supported Form Data Interchange Formats (FR-11.3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormDataFormat {
    /// Adobe Forms Data Format (ISO 32000-1 Clause 12.7.7)
    Fdf,
    /// XML Forms Data Format (ISO 19444-1)
    Xfdf,
    /// Comma-Separated Values (Spreadsheet interchange)
    Csv,
    /// Structured XML Form Records
    Xml,
    /// JavaScript Object Notation
    Json,
}

/// Exports all interactive form field values into the specified format (FR-11.3.2).
pub fn export_form_data(doc: &PdfDocument, format: FormDataFormat) -> Result<String> {
    let fields = get_form_fields(doc)?;
    let mut field_map = Vec::new();

    for f in &fields {
        if f.no_export {
            continue;
        }
        let val = f.value.clone().unwrap_or_default();
        field_map.push((f.name.clone(), val));
    }

    match format {
        FormDataFormat::Fdf => Ok(generate_fdf(&field_map)),
        FormDataFormat::Xfdf => Ok(generate_xfdf(&field_map)),
        FormDataFormat::Csv => Ok(generate_csv(&field_map)),
        FormDataFormat::Xml => Ok(generate_xml(&field_map)),
        FormDataFormat::Json => Ok(generate_json(&field_map)),
    }
}

/// Imports form data into the document's interactive fields (FR-11.3.2).
///
/// Returns the number of fields whose values were successfully updated.
pub fn import_form_data(doc: &mut PdfDocument, data: &str, format: FormDataFormat) -> Result<usize> {
    let incoming_map = match format {
        FormDataFormat::Fdf => parse_fdf(data)?,
        FormDataFormat::Xfdf => parse_xfdf(data)?,
        FormDataFormat::Csv => parse_csv(data)?,
        FormDataFormat::Xml => parse_xml(data)?,
        FormDataFormat::Json => parse_json(data)?,
    };

    let existing_fields = get_form_fields(doc)?;
    let mut updated_count = 0;

    for f in &existing_fields {
        if let Some(target_id) = f.id {
            if let Some(new_val) = incoming_map.get(&f.name) {
                // If checkbox, normalize boolean-like values
                let final_val = if f.field_type == FormFieldType::Checkbox {
                    let lower = new_val.trim().to_lowercase();
                    if lower == "true" || lower == "yes" || lower == "1" || lower == "on" {
                        "Yes".to_string()
                    } else if lower == "false" || lower == "no" || lower == "0" || lower == "off" {
                        "Off".to_string()
                    } else {
                        new_val.clone()
                    }
                } else {
                    new_val.clone()
                };

                set_form_field_value(doc, target_id, &final_val)?;
                updated_count += 1;
            }
        }
    }

    Ok(updated_count)
}

// ---------------------------------------------------------------------------
// FDF Generation & Parsing (ISO 32000-1 Clause 12.7.7)
// ---------------------------------------------------------------------------

fn generate_fdf(entries: &[(String, String)]) -> String {
    let mut fdf = String::new();
    fdf.push_str("%FDF-1.2\n1 0 obj\n<<\n/FDF <<\n/Fields [\n");
    for (name, val) in entries {
        let safe_name = escape_pdf_string(name);
        let safe_val = escape_pdf_string(val);
        fdf.push_str(&format!("<< /T ({}) /V ({}) >>\n", safe_name, safe_val));
    }
    fdf.push_str("]\n>>\n>>\nendobj\ntrailer\n<< /Root 1 0 R >>\n%%EOF\n");
    fdf
}

fn parse_fdf(data: &str) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    let mut pos = 0;

    while let Some(dict_start) = data[pos..].find("<<") {
        let actual_start = pos + dict_start;
        if let Some(dict_end) = data[actual_start..].find(">>") {
            let actual_end = actual_start + dict_end;
            let chunk = &data[actual_start..actual_end];

            let t_val = extract_pdf_string(chunk, "/T");
            let v_val = extract_pdf_string(chunk, "/V");

            if let (Some(name), Some(val)) = (t_val, v_val) {
                map.insert(name, val);
            }
            pos = actual_end + 2;
        } else {
            break;
        }
    }

    Ok(map)
}

// ---------------------------------------------------------------------------
// XFDF Generation & Parsing (ISO 19444-1)
// ---------------------------------------------------------------------------

fn generate_xfdf(entries: &[(String, String)]) -> String {
    let mut xfdf = String::new();
    xfdf.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xfdf.push_str("<xfdf xmlns=\"http://ns.adobe.com/xfdf/\" xml:space=\"preserve\">\n");
    xfdf.push_str("  <fields>\n");
    for (name, val) in entries {
        xfdf.push_str(&format!(
            "    <field name=\"{}\">\n      <value>{}</value>\n    </field>\n",
            escape_xml(name),
            escape_xml(val)
        ));
    }
    xfdf.push_str("  </fields>\n</xfdf>\n");
    xfdf
}

fn parse_xfdf(data: &str) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    let mut pos = 0;

    while let Some(tag_start) = data[pos..].find("<field") {
        let actual_start = pos + tag_start;
        if let Some(tag_close) = data[actual_start..].find("</field>") {
            let actual_close = actual_start + tag_close;
            let block = &data[actual_start..actual_close];

            // Extract name="..."
            if let Some(name_idx) = block.find("name=\"") {
                let name_start = name_idx + 6;
                if let Some(quote_end) = block[name_start..].find('\"') {
                    let field_name = unescape_xml(&block[name_start..name_start + quote_end]);
                    // Extract <value>...</value>
                    if let Some(val_start_tag) = block.find("<value>") {
                        let val_start = val_start_tag + 7;
                        if let Some(val_end_tag) = block[val_start..].find("</value>") {
                            let field_val = unescape_xml(&block[val_start..val_start + val_end_tag]);
                            map.insert(field_name, field_val);
                        }
                    }
                }
            }
            pos = actual_close + 8;
        } else {
            break;
        }
    }

    Ok(map)
}

// ---------------------------------------------------------------------------
// CSV Generation & Parsing
// ---------------------------------------------------------------------------

fn generate_csv(entries: &[(String, String)]) -> String {
    let mut headers = Vec::new();
    let mut values = Vec::new();

    for (name, val) in entries {
        headers.push(format!("\"{}\"", escape_csv(name)));
        values.push(format!("\"{}\"", escape_csv(val)));
    }

    format!("{}\n{}\n", headers.join(","), values.join(","))
}

fn parse_csv(data: &str) -> Result<HashMap<String, String>> {
    let lines: Vec<&str> = data.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        return Ok(HashMap::new());
    }

    let headers = parse_csv_line(lines[0]);
    let mut map = HashMap::new();

    if lines.len() > 1 {
        let values = parse_csv_line(lines[1]);
        for (i, h) in headers.iter().enumerate() {
            if let Some(v) = values.get(i) {
                map.insert(h.clone(), v.clone());
            }
        }
    }

    Ok(map)
}

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '"' {
            if in_quotes && chars.peek() == Some(&'"') {
                current.push('"');
                chars.next();
            } else {
                in_quotes = !in_quotes;
            }
        } else if ch == ',' && !in_quotes {
            fields.push(current.trim().to_string());
            current = String::new();
        } else {
            current.push(ch);
        }
    }
    fields.push(current.trim().to_string());
    fields
}

// ---------------------------------------------------------------------------
// XML Generation & Parsing
// ---------------------------------------------------------------------------

fn generate_xml(entries: &[(String, String)]) -> String {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<form-data>\n");
    for (name, val) in entries {
        xml.push_str(&format!(
            "  <field name=\"{}\"><value>{}</value></field>\n",
            escape_xml(name),
            escape_xml(val)
        ));
    }
    xml.push_str("</form-data>\n");
    xml
}

fn parse_xml(data: &str) -> Result<HashMap<String, String>> {
    // Reuse XML field/value tag extractor
    parse_xfdf(data)
}

// ---------------------------------------------------------------------------
// JSON Generation & Parsing
// ---------------------------------------------------------------------------

fn generate_json(entries: &[(String, String)]) -> String {
    let mut map = serde_json::Map::new();
    for (name, val) in entries {
        map.insert(name.clone(), serde_json::Value::String(val.clone()));
    }
    serde_json::to_string_pretty(&serde_json::Value::Object(map))
        .unwrap_or_else(|_| "{}".to_string())
}

fn parse_json(data: &str) -> Result<HashMap<String, String>> {
    let parsed: serde_json::Value = serde_json::from_str(data)
        .map_err(|e| PdfError::InvalidStructure(format!("Invalid form JSON: {}", e)))?;
    let mut map = HashMap::new();

    if let serde_json::Value::Object(obj) = parsed {
        for (k, v) in obj {
            match v {
                serde_json::Value::String(s) => {
                    map.insert(k, s);
                }
                serde_json::Value::Number(n) => {
                    map.insert(k, n.to_string());
                }
                serde_json::Value::Bool(b) => {
                    map.insert(k, if b { "Yes".into() } else { "Off".into() });
                }
                _ => {}
            }
        }
    }

    Ok(map)
}

// ---------------------------------------------------------------------------
// Utility String Escaping & Parsing
// ---------------------------------------------------------------------------

fn escape_pdf_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

fn extract_pdf_string(chunk: &str, key: &str) -> Option<String> {
    if let Some(idx) = chunk.find(key) {
        let after = &chunk[idx + key.len()..];
        if let Some(start_paren) = after.find('(') {
            let inside = &after[start_paren + 1..];
            if let Some(end_paren) = inside.find(')') {
                return Some(
                    inside[..end_paren]
                        .replace("\\)", ")")
                        .replace("\\(", "(")
                        .replace("\\\\", "\\"),
                );
            }
        }
    }
    None
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\"', "&quot;")
        .replace('\'', "&apos;")
}

fn unescape_xml(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn escape_csv(s: &str) -> String {
    s.replace('\"', "\"\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::{add_form_field, FormField};
    use ravpdf_core::document::PdfDocument;

    #[test]
    fn test_export_import_fdf_and_xfdf() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        let mut f1 = FormField::new_text("ApplicantName", [100.0, 700.0, 300.0, 720.0]);
        f1.value = Some("Jane Doe".into());
        add_form_field(&mut doc, 0, &f1).expect("add f1");

        let mut f2 = FormField::new_checkbox("AgreeTerms", [100.0, 650.0, 120.0, 670.0], true);
        f2.value = Some("Yes".into());
        add_form_field(&mut doc, 0, &f2).expect("add f2");

        // 1. Export FDF
        let fdf_data = export_form_data(&doc, FormDataFormat::Fdf).expect("export fdf");
        assert!(fdf_data.contains("%FDF-1.2"));
        assert!(fdf_data.contains("/T (ApplicantName)"));
        assert!(fdf_data.contains("/V (Jane Doe)"));

        // 2. Export XFDF
        let xfdf_data = export_form_data(&doc, FormDataFormat::Xfdf).expect("export xfdf");
        assert!(xfdf_data.contains("<field name=\"ApplicantName\">"));
        assert!(xfdf_data.contains("<value>Jane Doe</value>"));

        // 3. Import modified XFDF
        let new_xfdf = r#"<?xml version="1.0" encoding="UTF-8"?>
<xfdf xmlns="http://ns.adobe.com/xfdf/" xml:space="preserve">
  <fields>
    <field name="ApplicantName"><value>Robert Smith</value></field>
    <field name="AgreeTerms"><value>Off</value></field>
  </fields>
</xfdf>"#;

        let updated = import_form_data(&mut doc, new_xfdf, FormDataFormat::Xfdf).expect("import xfdf");
        assert_eq!(updated, 2);

        let fields_after = get_form_fields(&doc).expect("get fields");
        let name_field = fields_after.iter().find(|f| f.name == "ApplicantName").unwrap();
        assert_eq!(name_field.value.as_deref(), Some("Robert Smith"));

        let check_field = fields_after.iter().find(|f| f.name == "AgreeTerms").unwrap();
        assert_eq!(check_field.value.as_deref(), Some("Off"));
    }

    #[test]
    fn test_export_import_csv_and_json() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        let mut f1 = FormField::new_text("City", [100.0, 500.0, 200.0, 520.0]);
        f1.value = Some("San Francisco".into());
        add_form_field(&mut doc, 0, &f1).expect("add city");

        // Export JSON
        let json_str = export_form_data(&doc, FormDataFormat::Json).expect("export json");
        assert!(json_str.contains("\"City\": \"San Francisco\""));

        // Import modified JSON
        let modified_json = r#"{ "City": "New York" }"#;
        let count = import_form_data(&mut doc, modified_json, FormDataFormat::Json).expect("import json");
        assert_eq!(count, 1);

        let fields = get_form_fields(&doc).expect("get fields");
        assert_eq!(fields[0].value.as_deref(), Some("New York"));

        // Export CSV
        let csv_str = export_form_data(&doc, FormDataFormat::Csv).expect("export csv");
        assert!(csv_str.contains("\"City\""));
        assert!(csv_str.contains("\"New York\""));
    }
}
