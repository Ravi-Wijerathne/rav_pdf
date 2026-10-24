//! Acrobat JavaScript Execution Engine and Sandboxed Runtime.
//!
//! Conforming to ISO 32000-1 Section 12.6.4.16 (JavaScript Actions), Section 7.7.4 (Name Trees),
//! and Adobe Acrobat JavaScript API Reference.
//!
//! Traceability:
//! - FR-12.1: Embedded JavaScript interpreter compatible with Acrobat's JavaScript API subset.
//! - FR-12.2: Document-level JavaScript (executed on document open/close/save/print events).
//! - FR-12.3: Field-level JavaScript (keystroke, validate, calculate, format, focus, blur).
//! - FR-12.4: Page-level JavaScript (page open/close events).
//! - FR-12.5: Acrobat JavaScript object model: `this` (Document), `app`, `event`, `console`, `color`, `util`, `global`.
//! - FR-12.6: `this.getField()` for accessing form field objects and their properties.
//! - FR-12.7: `app.alert()`, `app.response()`, `app.beep()` dialogs.
//! - FR-12.8: `this.mailDoc()` and `this.submitForm()` methods.
//! - FR-12.9: JavaScript console/debugger for testing scripts.
//! - FR-12.10: Sandboxed execution preventing file system access, network requests, or system command execution.
//! - FR-12.11: Enabling/disabling JavaScript execution globally via preferences.

use anyhow::{anyhow, Result};
use ravpdf_core::document::PdfDocument;
use ravpdf_core::object::{Object, PdfDictionary, PdfName, PdfString};
use ravpdf_forms::{get_form_fields, set_form_field_value};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Runtime value representation for JavaScript evaluations (FR-12.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum JsValue {
    Undefined,
    Null,
    Boolean(bool),
    Number(f64),
    String(String),
    Array(Vec<JsValue>),
    Object(HashMap<String, JsValue>),
}

impl JsValue {
    pub fn to_f64(&self) -> f64 {
        match self {
            JsValue::Number(n) => *n,
            JsValue::Boolean(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            JsValue::String(s) => s.trim().parse::<f64>().unwrap_or(0.0),
            JsValue::Array(arr) if arr.len() == 1 => arr[0].to_f64(),
            _ => 0.0,
        }
    }

    pub fn to_string_repr(&self) -> String {
        match self {
            JsValue::Undefined => "undefined".to_string(),
            JsValue::Null => "null".to_string(),
            JsValue::Boolean(b) => b.to_string(),
            JsValue::Number(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    format!("{:.0}", n)
                } else {
                    n.to_string()
                }
            }
            JsValue::String(s) => s.clone(),
            JsValue::Array(arr) => {
                let items: Vec<String> = arr.iter().map(|v| v.to_string_repr()).collect();
                items.join(",")
            }
            JsValue::Object(map) => {
                format!("[object Object] with {} keys", map.len())
            }
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            JsValue::Undefined | JsValue::Null => false,
            JsValue::Boolean(b) => *b,
            JsValue::Number(n) => *n != 0.0 && !n.is_nan(),
            JsValue::String(s) => !s.is_empty(),
            JsValue::Array(_) | JsValue::Object(_) => true,
        }
    }

    pub fn type_of(&self) -> &'static str {
        match self {
            JsValue::Undefined => "undefined",
            JsValue::Null | JsValue::Object(_) | JsValue::Array(_) => "object",
            JsValue::Boolean(_) => "boolean",
            JsValue::Number(_) => "number",
            JsValue::String(_) => "string",
        }
    }
}

/// Alert dialog invocation record (FR-12.7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsAlert {
    pub message: String,
    pub title: String,
    pub icon: u32,
    pub dialog_type: u32,
}

/// SubmitForm invocation record (FR-12.8, FR-12.10).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsSubmitFormAction {
    pub url: String,
    pub fdf: bool,
    pub empty_fields: bool,
}

/// MailDoc invocation record (FR-12.8, FR-12.10).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsMailDocAction {
    pub interactive: bool,
    pub to: String,
    pub cc: String,
    pub bcc: String,
    pub subject: String,
    pub message: String,
}

/// Event object state conforming to Acrobat JavaScript event model (FR-12.3, FR-12.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsEvent {
    pub name: String,
    pub event_type: String,
    pub target_name: String,
    pub value: JsValue,
    pub rc: bool,
    pub change: String,
    pub sel_start: usize,
    pub sel_end: usize,
}

impl Default for JsEvent {
    fn default() -> Self {
        Self {
            name: "Calculate".to_string(),
            event_type: "Field".to_string(),
            target_name: String::new(),
            value: JsValue::Undefined,
            rc: true,
            change: String::new(),
            sel_start: 0,
            sel_end: 0,
        }
    }
}

/// In-memory representation of an AcroForm field inside the JS runtime (FR-12.6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsFormField {
    pub name: String,
    pub value: JsValue,
    pub default_value: JsValue,
    pub read_only: bool,
    pub hidden: bool,
    pub required: bool,
    pub text_color: String,
    pub fill_color: String,
    pub text_size: f64,
    pub text_font: String,
    pub calculate_script: Option<String>,
    pub format_script: Option<String>,
    pub validate_script: Option<String>,
    pub keystroke_script: Option<String>,
}

impl JsFormField {
    pub fn new(name: impl Into<String>, initial_val: impl Into<String>) -> Self {
        let val_str = initial_val.into();
        Self {
            name: name.into(),
            value: JsValue::String(val_str.clone()),
            default_value: JsValue::String(val_str),
            read_only: false,
            hidden: false,
            required: false,
            text_color: "#000000".to_string(),
            fill_color: "#FFFFFF".to_string(),
            text_size: 10.0,
            text_font: "Helvetica".to_string(),
            calculate_script: None,
            format_script: None,
            validate_script: None,
            keystroke_script: None,
        }
    }
}

/// JavaScript sandboxed execution context conforming to FR-12.1 - FR-12.11.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsExecutionContext {
    pub js_enabled: bool,
    pub sandbox_strict: bool,
    pub num_pages: usize,
    pub page_num: usize,
    pub title: String,
    pub author: String,
    pub subject: String,
    pub event: JsEvent,
    pub fields: HashMap<String, JsFormField>,
    pub global_vars: HashMap<String, JsValue>,
    pub prompt_responses: HashMap<String, String>,
    pub console_log: Vec<String>,
    pub alerts: Vec<JsAlert>,
    pub submissions: Vec<JsSubmitFormAction>,
    pub mails: Vec<JsMailDocAction>,
    pub beeps: usize,
}

impl Default for JsExecutionContext {
    fn default() -> Self {
        Self::new()
    }
}

impl JsExecutionContext {
    pub fn new() -> Self {
        Self {
            js_enabled: true,
            sandbox_strict: true,
            num_pages: 1,
            page_num: 0,
            title: String::new(),
            author: String::new(),
            subject: String::new(),
            event: JsEvent::default(),
            fields: HashMap::new(),
            global_vars: HashMap::new(),
            prompt_responses: HashMap::new(),
            console_log: Vec::new(),
            alerts: Vec::new(),
            submissions: Vec::new(),
            mails: Vec::new(),
            beeps: 0,
        }
    }

    /// Register a form field into the JS runtime context (FR-12.6).
    pub fn register_field(&mut self, field: JsFormField) {
        self.fields.insert(field.name.clone(), field);
    }

    /// Access field by name (FR-12.6: `this.getField`).
    pub fn get_field(&self, name: &str) -> Option<&JsFormField> {
        self.fields.get(name)
    }

    /// Access mutable field by name.
    pub fn get_field_mut(&mut self, name: &str) -> Option<&mut JsFormField> {
        self.fields.get_mut(name)
    }

    /// Clear console output buffer (FR-12.9).
    pub fn console_clear(&mut self) {
        self.console_log.clear();
    }

    /// Record message to console output (FR-12.5: `console.println`).
    pub fn console_println(&mut self, msg: impl Into<String>) {
        self.console_log.push(msg.into());
    }

    /// Record alert dialog (FR-12.7: `app.alert`).
    pub fn app_alert(&mut self, message: &str, title: &str, icon: u32, dialog_type: u32) -> u32 {
        self.alerts.push(JsAlert {
            message: message.to_string(),
            title: title.to_string(),
            icon,
            dialog_type,
        });
        1 // OK status code
    }

    /// Trigger beep (FR-12.7: `app.beep`).
    pub fn app_beep(&mut self) {
        self.beeps += 1;
    }

    /// Reset all form fields or specified fields to default values (FR-12.5).
    pub fn reset_form(&mut self, field_names: Option<&[String]>) {
        for (name, field) in self.fields.iter_mut() {
            if let Some(target_names) = field_names {
                if !target_names.contains(name) {
                    continue;
                }
            }
            field.value = field.default_value.clone();
        }
    }

    /// Synchronize form fields from a `PdfDocument` into this execution context.
    pub fn sync_from_document(&mut self, doc: &PdfDocument) -> Result<()> {
        self.num_pages = doc.page_count();
        let doc_fields = get_form_fields(doc)?;
        for f in doc_fields {
            let initial = f.value.clone().unwrap_or_default();
            let mut js_f = JsFormField::new(&f.name, initial);
            js_f.read_only = f.read_only;
            js_f.required = f.required;
            if let Some(def) = f.default_value {
                js_f.default_value = JsValue::String(def);
            }
            self.register_field(js_f);
        }
        Ok(())
    }

    /// Synchronize updated form field values from this execution context back to a `PdfDocument`.
    pub fn sync_to_document(&self, doc: &mut PdfDocument) -> Result<()> {
        let doc_fields = get_form_fields(doc)?;
        for f in doc_fields {
            if let Some(field_id) = f.id {
                if let Some(js_f) = self.get_field(&f.name) {
                    let val_str = js_f.value.to_string_repr();
                    set_form_field_value(doc, field_id, &val_str)?;
                }
            }
        }
        Ok(())
    }
}

// ============================================================================
// Sandboxing & Lexical / Expression Evaluation Engine (FR-12.1, FR-12.10)
// ============================================================================

/// Execute JavaScript code within the sandboxed environment (FR-12.1, FR-12.10, FR-12.11).
pub fn execute_script(ctx: &mut JsExecutionContext, script: &str) -> Result<JsValue> {
    if !ctx.js_enabled {
        return Ok(JsValue::Undefined);
    }

    let trimmed = script.trim();
    if trimmed.is_empty() {
        return Ok(JsValue::Undefined);
    }

    // 1. Sandbox security check (FR-12.10)
    if ctx.sandbox_strict {
        validate_sandbox_safety(trimmed)?;
    }

    // 2. Parse and execute statements
    let mut local_vars: HashMap<String, JsValue> = HashMap::new();
    let statements = split_statements(trimmed);
    let mut last_val = JsValue::Undefined;

    for stmt in statements {
        let stmt = stmt.trim();
        if stmt.is_empty() {
            continue;
        }

        // Return statement
        if stmt.starts_with("return ") || stmt == "return" {
            let expr = stmt.trim_start_matches("return").trim();
            return eval_expression(ctx, &mut local_vars, expr);
        }

        // If statement: if (condition) { ... } else { ... }
        if stmt.starts_with("if") && stmt.contains('(') {
            if let Some(handled) = handle_if_statement(ctx, &mut local_vars, stmt)? {
                last_val = handled;
                continue;
            }
        }

        // Variable declaration: var name = expr; or let/const
        if stmt.starts_with("var ") || stmt.starts_with("let ") || stmt.starts_with("const ") {
            let decl = stmt
                .trim_start_matches("var ")
                .trim_start_matches("let ")
                .trim_start_matches("const ")
                .trim();
            if let Some((name, expr)) = decl.split_once('=') {
                let name = name.trim().to_string();
                let val = eval_expression(ctx, &mut local_vars, expr.trim())?;
                local_vars.insert(name, val.clone());
                last_val = val;
            } else {
                local_vars.insert(decl.to_string(), JsValue::Undefined);
            }
            continue;
        }

        // Assignment: target = expr
        if let Some((target, expr)) = split_assignment(stmt) {
            let val = eval_expression(ctx, &mut local_vars, expr.trim())?;
            apply_assignment(ctx, &mut local_vars, target.trim(), val.clone())?;
            last_val = val;
            continue;
        }

        // Standard expression evaluation
        last_val = eval_expression(ctx, &mut local_vars, stmt)?;
    }

    Ok(last_val)
}

/// Validates that script does not contain dangerous forbidden primitives (FR-12.10).
fn validate_sandbox_safety(script: &str) -> Result<()> {
    const FORBIDDEN_TOKENS: &[&str] = &[
        "require(",
        "process.",
        "fs.",
        "child_process",
        "fetch(",
        "XMLHttpRequest",
        "WebSocket",
        "document.location",
        "window.",
        "eval(",
        "Function(",
        "__proto__",
        "localStorage",
        "sessionStorage",
    ];

    for forbidden in FORBIDDEN_TOKENS {
        if script.contains(forbidden) {
            return Err(anyhow!(
                "JavaScript Sandbox Security Violation (FR-12.10): Access to forbidden token '{}' is blocked",
                forbidden
            ));
        }
    }
    Ok(())
}

/// Splits script text into top-level statements respecting string literals and parentheses/braces.
fn split_statements(code: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut brace_depth = 0;
    let mut paren_depth = 0;
    for ch in code.chars() {
        match ch {
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
                current.push(ch);
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
                current.push(ch);
            }
            '{' if !in_single_quote && !in_double_quote => {
                brace_depth += 1;
                current.push(ch);
            }
            '}' if !in_single_quote && !in_double_quote => {
                brace_depth -= 1;
                current.push(ch);
                if brace_depth == 0 && paren_depth == 0 {
                    statements.push(current.clone());
                    current.clear();
                }
            }
            '(' if !in_single_quote && !in_double_quote => {
                paren_depth += 1;
                current.push(ch);
            }
            ')' if !in_single_quote && !in_double_quote => {
                paren_depth -= 1;
                current.push(ch);
            }
            ';' if !in_single_quote && !in_double_quote && brace_depth == 0 => {
                statements.push(current.clone());
                current.clear();
            }
            '\n' | '\r' if !in_single_quote && !in_double_quote && brace_depth == 0 => {
                if !current.trim().is_empty() && (current.ends_with('{') || current.ends_with('}'))
                {
                    statements.push(current.clone());
                    current.clear();
                } else {
                    current.push(' ');
                }
            }
            _ => {
                current.push(ch);
            }
        }
    }

    if !current.trim().is_empty() {
        statements.push(current);
    }

    statements
}

/// Splits assignment statement (e.g. `event.value = 123` or `x += 5`).
fn split_assignment(stmt: &str) -> Option<(&str, &str)> {
    let bytes = stmt.as_bytes();
    let mut in_single = false;
    let mut in_double = false;
    let mut paren_depth = 0;

    for i in 0..bytes.len() {
        let b = bytes[i];
        if b == b'\'' && !in_double {
            in_single = !in_single;
        } else if b == b'"' && !in_single {
            in_double = !in_double;
        } else if b == b'(' && !in_single && !in_double {
            paren_depth += 1;
        } else if b == b')' && !in_single && !in_double {
            paren_depth -= 1;
        } else if b == b'=' && !in_single && !in_double && paren_depth == 0 {
            // Ensure not == or === or != or <= or >=
            let prev = if i > 0 { bytes[i - 1] } else { 0 };
            let next = if i + 1 < bytes.len() {
                bytes[i + 1]
            } else {
                0
            };
            if prev != b'='
                && prev != b'!'
                && prev != b'<'
                && prev != b'>'
                && next != b'='
            {
                return Some((&stmt[..i], &stmt[i + 1..]));
            }
        }
    }
    None
}

/// Applies assignment to local variable, event, or form field.
fn apply_assignment(
    ctx: &mut JsExecutionContext,
    locals: &mut HashMap<String, JsValue>,
    target: &str,
    val: JsValue,
) -> Result<()> {
    let target = target.trim();

    // event.value = ...
    if target == "event.value" {
        ctx.event.value = val;
        return Ok(());
    }

    // event.rc = ...
    if target == "event.rc" {
        ctx.event.rc = val.is_truthy();
        return Ok(());
    }

    // this.getField("Name").value = ...
    if target.starts_with("this.getField(") && target.ends_with(".value") {
        if let Some(inner) = target
            .strip_prefix("this.getField(")
            .and_then(|s| s.strip_suffix(".value"))
        {
            let field_name_expr = inner.trim_end_matches(')').trim();
            let field_name = clean_string_literal(field_name_expr);
            if let Some(f) = ctx.get_field_mut(&field_name) {
                f.value = val;
            }
            return Ok(());
        }
    }

    // global.x = ...
    if let Some(var_name) = target.strip_prefix("global.") {
        ctx.global_vars.insert(var_name.to_string(), val);
        return Ok(());
    }

    // Local variable
    locals.insert(target.to_string(), val);
    Ok(())
}

/// Handles conditional if (...) { ... } else { ... } statement.
fn handle_if_statement(
    ctx: &mut JsExecutionContext,
    locals: &mut HashMap<String, JsValue>,
    stmt: &str,
) -> Result<Option<JsValue>> {
    let open_paren = match stmt.find('(') {
        Some(pos) => pos,
        None => return Ok(None),
    };

    // Find matching closing paren
    let mut paren_depth = 0;
    let mut close_paren = 0;
    for (i, ch) in stmt[open_paren..].char_indices() {
        if ch == '(' {
            paren_depth += 1;
        } else if ch == ')' {
            paren_depth -= 1;
            if paren_depth == 0 {
                close_paren = open_paren + i;
                break;
            }
        }
    }

    if close_paren == 0 {
        return Ok(None);
    }

    let cond_expr = &stmt[open_paren + 1..close_paren];
    let cond_val = eval_expression(ctx, locals, cond_expr)?;
    let remainder = stmt[close_paren + 1..].trim();

    // Extract blocks
    let (then_block, else_block) = extract_then_else_blocks(remainder);

    if cond_val.is_truthy() {
        let res = execute_script_subcontext(ctx, locals, &then_block)?;
        Ok(Some(res))
    } else if let Some(else_code) = else_block {
        let res = execute_script_subcontext(ctx, locals, &else_code)?;
        Ok(Some(res))
    } else {
        Ok(Some(JsValue::Undefined))
    }
}

fn extract_then_else_blocks(code: &str) -> (String, Option<String>) {
    let trimmed = code.trim();
    if trimmed.starts_with('{') {
        let mut depth = 0;
        let mut end_then = 0;
        for (i, ch) in trimmed.char_indices() {
            if ch == '{' {
                depth += 1;
            } else if ch == '}' {
                depth -= 1;
                if depth == 0 {
                    end_then = i;
                    break;
                }
            }
        }

        let then_part = trimmed[1..end_then].to_string();
        let after_then = trimmed[end_then + 1..].trim();
        if let Some(else_after) = after_then.strip_prefix("else") {
            let else_trimmed = else_after.trim();
            if else_trimmed.starts_with('{') && else_trimmed.ends_with('}') {
                (
                    then_part,
                    Some(else_trimmed[1..else_trimmed.len() - 1].to_string()),
                )
            } else {
                (then_part, Some(else_trimmed.to_string()))
            }
        } else {
            (then_part, None)
        }
    } else {
        (trimmed.to_string(), None)
    }
}

fn execute_script_subcontext(
    ctx: &mut JsExecutionContext,
    locals: &mut HashMap<String, JsValue>,
    code: &str,
) -> Result<JsValue> {
    let statements = split_statements(code);
    let mut last = JsValue::Undefined;
    for stmt in statements {
        let stmt = stmt.trim();
        if stmt.is_empty() {
            continue;
        }
        if let Some((target, expr)) = split_assignment(stmt) {
            let val = eval_expression(ctx, locals, expr.trim())?;
            apply_assignment(ctx, locals, target.trim(), val.clone())?;
            last = val;
        } else {
            last = eval_expression(ctx, locals, stmt)?;
        }
    }
    Ok(last)
}

/// Evaluates a JavaScript expression to a `JsValue`.
pub fn eval_expression(
    ctx: &mut JsExecutionContext,
    locals: &mut HashMap<String, JsValue>,
    expr: &str,
) -> Result<JsValue> {
    let expr = expr.trim();
    if expr.is_empty() {
        return Ok(JsValue::Undefined);
    }

    // 1. Ternary operator: condition ? then_expr : else_expr
    if let Some((cond, then_e, else_e)) = split_ternary(expr) {
        let cond_val = eval_expression(ctx, locals, cond)?;
        return if cond_val.is_truthy() {
            eval_expression(ctx, locals, then_e)
        } else {
            eval_expression(ctx, locals, else_e)
        };
    }

    // 2. Logical OR (||)
    if let Some((left, right)) = split_binary_op(expr, "||") {
        let l_val = eval_expression(ctx, locals, left)?;
        if l_val.is_truthy() {
            return Ok(l_val);
        }
        return eval_expression(ctx, locals, right);
    }

    // 3. Logical AND (&&)
    if let Some((left, right)) = split_binary_op(expr, "&&") {
        let l_val = eval_expression(ctx, locals, left)?;
        if !l_val.is_truthy() {
            return Ok(l_val);
        }
        return eval_expression(ctx, locals, right);
    }

    // 4. Equality & Comparisons (==, !=, ===, !==, <=, >=, <, >)
    for op in &["===", "!==", "==", "!=", "<=", ">=", "<", ">"] {
        if let Some((left, right)) = split_binary_op(expr, op) {
            let l_val = eval_expression(ctx, locals, left)?;
            let r_val = eval_expression(ctx, locals, right)?;
            let cmp_res = match *op {
                "===" | "==" => l_val == r_val || l_val.to_string_repr() == r_val.to_string_repr(),
                "!==" | "!=" => l_val != r_val && l_val.to_string_repr() != r_val.to_string_repr(),
                "<" => l_val.to_f64() < r_val.to_f64(),
                "<=" => l_val.to_f64() <= r_val.to_f64(),
                ">" => l_val.to_f64() > r_val.to_f64(),
                ">=" => l_val.to_f64() >= r_val.to_f64(),
                _ => false,
            };
            return Ok(JsValue::Boolean(cmp_res));
        }
    }

    // 5. Additive (+, -)
    for op in &["+", "-"] {
        if let Some((left, right)) = split_additive_op(expr, op) {
            let l_val = eval_expression(ctx, locals, left)?;
            let r_val = eval_expression(ctx, locals, right)?;
            if *op == "+" {
                // If either is string, perform string concatenation
                if matches!(l_val, JsValue::String(_)) || matches!(r_val, JsValue::String(_)) {
                    return Ok(JsValue::String(format!(
                        "{}{}",
                        l_val.to_string_repr(),
                        r_val.to_string_repr()
                    )));
                } else {
                    return Ok(JsValue::Number(l_val.to_f64() + r_val.to_f64()));
                }
            } else {
                return Ok(JsValue::Number(l_val.to_f64() - r_val.to_f64()));
            }
        }
    }

    // 6. Multiplicative (*, /, %)
    for op in &["*", "/", "%"] {
        if let Some((left, right)) = split_binary_op(expr, op) {
            let l_val = eval_expression(ctx, locals, left)?;
            let r_val = eval_expression(ctx, locals, right)?;
            let r_num = r_val.to_f64();
            return match *op {
                "*" => Ok(JsValue::Number(l_val.to_f64() * r_num)),
                "/" => {
                    if r_num == 0.0 {
                        Ok(JsValue::Number(f64::NAN))
                    } else {
                        Ok(JsValue::Number(l_val.to_f64() / r_num))
                    }
                }
                "%" => Ok(JsValue::Number(l_val.to_f64() % r_num)),
                _ => unreachable!(),
            };
        }
    }

    // 7. Unary operators (!, -, +)
    if let Some(stripped) = expr.strip_prefix('!') {
        let val = eval_expression(ctx, locals, stripped)?;
        return Ok(JsValue::Boolean(!val.is_truthy()));
    }
    if let Some(stripped) = expr.strip_prefix('-') {
        if !stripped.starts_with(|c: char| c.is_ascii_digit()) {
            let val = eval_expression(ctx, locals, stripped)?;
            return Ok(JsValue::Number(-val.to_f64()));
        }
    }

    // 8. Parenthesized sub-expression
    if expr.starts_with('(')
        && expr.ends_with(')')
        && is_balanced_parentheses(&expr[1..expr.len() - 1])
    {
        return eval_expression(ctx, locals, &expr[1..expr.len() - 1]);
    }

    // 9. Function calls
    if expr.ends_with(')') && expr.contains('(') {
        if let Some(call_res) = handle_function_call(ctx, locals, expr)? {
            return Ok(call_res);
        }
    }

    // 10. Member property accesses (e.g. this.getField("Foo").value, event.value, color.red)
    if expr.contains('.') {
        if let Some(val) = handle_member_access(ctx, locals, expr)? {
            return Ok(val);
        }
    }

    // 11. Array literals: [a, b, c] or new Array(a, b, c)
    if expr.starts_with('[') && expr.ends_with(']') {
        let inner = &expr[1..expr.len() - 1];
        let items = parse_call_arguments(inner);
        let mut list = Vec::new();
        for item in items {
            list.push(eval_expression(ctx, locals, &item)?);
        }
        return Ok(JsValue::Array(list));
    }

    // 12. Literals and Identifier resolution
    eval_primitive_or_identifier(ctx, locals, expr)
}

/// Evaluates literals, identifiers, or variables.
fn eval_primitive_or_identifier(
    ctx: &JsExecutionContext,
    locals: &HashMap<String, JsValue>,
    expr: &str,
) -> Result<JsValue> {
    let expr = expr.trim();

    // Undefined & Null
    if expr == "undefined" {
        return Ok(JsValue::Undefined);
    }
    if expr == "null" {
        return Ok(JsValue::Null);
    }

    // Booleans
    if expr == "true" {
        return Ok(JsValue::Boolean(true));
    }
    if expr == "false" {
        return Ok(JsValue::Boolean(false));
    }

    // String literal
    if (expr.starts_with('"') && expr.ends_with('"'))
        || (expr.starts_with('\'') && expr.ends_with('\''))
    {
        return Ok(JsValue::String(clean_string_literal(expr)));
    }

    // Number literal
    if let Ok(num) = expr.parse::<f64>() {
        return Ok(JsValue::Number(num));
    }

    // Local variable
    if let Some(val) = locals.get(expr) {
        return Ok(val.clone());
    }

    // Global variable
    if let Some(val) = ctx.global_vars.get(expr) {
        return Ok(val.clone());
    }

    // If identifier matches an AcroForm field name, resolve field value directly
    if let Some(field) = ctx.get_field(expr) {
        return Ok(field.value.clone());
    }

    // Fallback: return as String or Undefined
    Ok(JsValue::Undefined)
}

/// Handles property access: `this.numPages`, `event.value`, `color.red`, etc.
fn handle_member_access(
    ctx: &mut JsExecutionContext,
    locals: &mut HashMap<String, JsValue>,
    expr: &str,
) -> Result<Option<JsValue>> {
    // this.getField("X").value
    if expr.starts_with("this.getField(") && expr.ends_with(".value") {
        if let Some(inner) = expr
            .strip_prefix("this.getField(")
            .and_then(|s| s.strip_suffix(".value"))
        {
            let field_name_expr = inner.trim_end_matches(')').trim();
            let field_name = clean_string_literal(field_name_expr);
            let val = ctx
                .get_field(&field_name)
                .map(|f| f.value.clone())
                .unwrap_or(JsValue::Undefined);
            return Ok(Some(val));
        }
    }

    // this.getField("X").name
    if expr.starts_with("this.getField(") && expr.ends_with(".name") {
        if let Some(inner) = expr
            .strip_prefix("this.getField(")
            .and_then(|s| s.strip_suffix(".name"))
        {
            let field_name_expr = inner.trim_end_matches(')').trim();
            let field_name = clean_string_literal(field_name_expr);
            return Ok(Some(JsValue::String(field_name)));
        }
    }

    // event.*
    if expr == "event.value" {
        return Ok(Some(ctx.event.value.clone()));
    }
    if expr == "event.rc" {
        return Ok(Some(JsValue::Boolean(ctx.event.rc)));
    }
    if expr == "event.name" {
        return Ok(Some(JsValue::String(ctx.event.name.clone())));
    }
    if expr == "event.target" {
        return Ok(Some(JsValue::String(ctx.event.target_name.clone())));
    }

    // this.*
    if expr == "this.numPages" {
        return Ok(Some(JsValue::Number(ctx.num_pages as f64)));
    }
    if expr == "this.pageNum" {
        return Ok(Some(JsValue::Number(ctx.page_num as f64)));
    }
    if expr == "this.title" {
        return Ok(Some(JsValue::String(ctx.title.clone())));
    }
    if expr == "this.author" {
        return Ok(Some(JsValue::String(ctx.author.clone())));
    }

    // color.* (FR-12.5)
    if let Some(col_name) = expr.strip_prefix("color.") {
        let hex = match col_name {
            "black" => "#000000",
            "white" => "#FFFFFF",
            "red" => "#FF0000",
            "green" => "#00FF00",
            "blue" => "#0000FF",
            "cyan" => "#00FFFF",
            "magenta" => "#FF00FF",
            "yellow" => "#FFFF00",
            "gray" | "dkGray" => "#808080",
            "ltGray" => "#C0C0C0",
            "transparent" => "transparent",
            _ => "#000000",
        };
        return Ok(Some(JsValue::String(hex.to_string())));
    }

    // global.* (FR-12.5)
    if let Some(var_name) = expr.strip_prefix("global.") {
        return Ok(Some(
            ctx.global_vars
                .get(var_name)
                .cloned()
                .unwrap_or(JsValue::Undefined),
        ));
    }

    // Check locals or objects
    if let Some((obj_name, prop)) = expr.split_once('.') {
        if let Some(JsValue::Object(map)) = locals.get(obj_name) {
            return Ok(Some(map.get(prop).cloned().unwrap_or(JsValue::Undefined)));
        }
    }

    Ok(None)
}

/// Handles function and method invocations for Acrobat JavaScript API subset (FR-12.5 - FR-12.8).
fn handle_function_call(
    ctx: &mut JsExecutionContext,
    locals: &mut HashMap<String, JsValue>,
    expr: &str,
) -> Result<Option<JsValue>> {
    let open_paren = match expr.find('(') {
        Some(pos) => pos,
        None => return Ok(None),
    };
    let fn_name = expr[..open_paren].trim();
    let args_str = &expr[open_paren + 1..expr.len() - 1];
    let raw_args = parse_call_arguments(args_str);

    // 1. app.alert(cMsg, nIcon, nType, cTitle) (FR-12.7)
    if fn_name == "app.alert" {
        let msg = raw_args
            .first()
            .map(|a| eval_expression(ctx, locals, a).unwrap_or(JsValue::Undefined).to_string_repr())
            .unwrap_or_default();
        let arg1_val = raw_args
            .get(1)
            .and_then(|a| eval_expression(ctx, locals, a).ok());
        let (icon, n_type, title) = match arg1_val {
            Some(JsValue::String(s)) => {
                let icon = raw_args
                    .get(2)
                    .and_then(|a| eval_expression(ctx, locals, a).ok())
                    .map(|v| v.to_f64() as u32)
                    .unwrap_or(0);
                let n_type = raw_args
                    .get(3)
                    .and_then(|a| eval_expression(ctx, locals, a).ok())
                    .map(|v| v.to_f64() as u32)
                    .unwrap_or(0);
                (icon, n_type, s)
            }
            Some(JsValue::Number(num)) => {
                let icon = num as u32;
                let n_type = raw_args
                    .get(2)
                    .and_then(|a| eval_expression(ctx, locals, a).ok())
                    .map(|v| v.to_f64() as u32)
                    .unwrap_or(0);
                let title = raw_args
                    .get(3)
                    .and_then(|a| eval_expression(ctx, locals, a).ok())
                    .map(|v| v.to_string_repr())
                    .unwrap_or_else(|| "Adobe Acrobat".to_string());
                (icon, n_type, title)
            }
            _ => (0, 0, "Adobe Acrobat".to_string()),
        };

        let res = ctx.app_alert(&msg, &title, icon, n_type);
        return Ok(Some(JsValue::Number(res as f64)));
    }

    // 2. app.beep() (FR-12.7)
    if fn_name == "app.beep" {
        ctx.app_beep();
        return Ok(Some(JsValue::Undefined));
    }

    // 3. app.response(cQuestion, cTitle, cDefault, bPassword) (FR-12.7)
    if fn_name == "app.response" {
        let question = raw_args
            .first()
            .map(|a| eval_expression(ctx, locals, a).unwrap_or(JsValue::Undefined).to_string_repr())
            .unwrap_or_default();
        let default_ans = raw_args
            .get(2)
            .map(|a| eval_expression(ctx, locals, a).unwrap_or(JsValue::Undefined).to_string_repr())
            .unwrap_or_default();
        let response = ctx
            .prompt_responses
            .get(&question)
            .cloned()
            .unwrap_or(default_ans);
        return Ok(Some(JsValue::String(response)));
    }

    // 4. console.println(...) and console.clear() (FR-12.5, FR-12.9)
    if fn_name == "console.println" || fn_name == "console.log" {
        let msg = raw_args
            .first()
            .map(|a| eval_expression(ctx, locals, a).unwrap_or(JsValue::Undefined).to_string_repr())
            .unwrap_or_default();
        ctx.console_println(msg);
        return Ok(Some(JsValue::Undefined));
    }
    if fn_name == "console.clear" {
        ctx.console_clear();
        return Ok(Some(JsValue::Undefined));
    }

    // 5. this.resetForm([aFields]) (FR-12.5)
    if fn_name == "this.resetForm" {
        if let Some(arg) = raw_args.first() {
            let val = eval_expression(ctx, locals, arg)?;
            if let JsValue::Array(items) = val {
                let names: Vec<String> = items.into_iter().map(|v| v.to_string_repr()).collect();
                ctx.reset_form(Some(&names));
            } else {
                ctx.reset_form(None);
            }
        } else {
            ctx.reset_form(None);
        }
        return Ok(Some(JsValue::Undefined));
    }

    // 6. this.mailDoc(bUI, cTo, cCc, cBcc, cSubject, cMsg) (FR-12.8)
    if fn_name == "this.mailDoc" {
        let interactive = raw_args
            .first()
            .and_then(|a| eval_expression(ctx, locals, a).ok())
            .map(|v| v.is_truthy())
            .unwrap_or(true);
        let to = raw_args
            .get(1)
            .and_then(|a| eval_expression(ctx, locals, a).ok())
            .map(|v| v.to_string_repr())
            .unwrap_or_default();
        let cc = raw_args
            .get(2)
            .and_then(|a| eval_expression(ctx, locals, a).ok())
            .map(|v| v.to_string_repr())
            .unwrap_or_default();
        let bcc = raw_args
            .get(3)
            .and_then(|a| eval_expression(ctx, locals, a).ok())
            .map(|v| v.to_string_repr())
            .unwrap_or_default();
        let subject = raw_args
            .get(4)
            .and_then(|a| eval_expression(ctx, locals, a).ok())
            .map(|v| v.to_string_repr())
            .unwrap_or_default();
        let message = raw_args
            .get(5)
            .and_then(|a| eval_expression(ctx, locals, a).ok())
            .map(|v| v.to_string_repr())
            .unwrap_or_default();

        ctx.mails.push(JsMailDocAction {
            interactive,
            to,
            cc,
            bcc,
            subject,
            message,
        });
        return Ok(Some(JsValue::Undefined));
    }

    // 7. this.submitForm(cURL, bFDF, bEmpty) (FR-12.8)
    if fn_name == "this.submitForm" {
        let url = raw_args
            .first()
            .and_then(|a| eval_expression(ctx, locals, a).ok())
            .map(|v| v.to_string_repr())
            .unwrap_or_default();
        let fdf = raw_args
            .get(1)
            .and_then(|a| eval_expression(ctx, locals, a).ok())
            .map(|v| v.is_truthy())
            .unwrap_or(true);
        let empty = raw_args
            .get(2)
            .and_then(|a| eval_expression(ctx, locals, a).ok())
            .map(|v| v.is_truthy())
            .unwrap_or(false);

        ctx.submissions.push(JsSubmitFormAction {
            url,
            fdf,
            empty_fields: empty,
        });
        return Ok(Some(JsValue::Undefined));
    }

    // 8. Acrobat Standard Calculation helper: AFSimple_Calculate(cFunction, cFields) (FR-11.2.4, FR-12.1)
    if fn_name == "AFSimple_Calculate" {
        let func_type = raw_args
            .first()
            .map(|a| eval_expression(ctx, locals, a).unwrap_or(JsValue::Undefined).to_string_repr())
            .unwrap_or_else(|| "SUM".to_string());

        let field_names = if let Some(arg) = raw_args.get(1) {
            let evaluated = eval_expression(ctx, locals, arg)?;
            match evaluated {
                JsValue::Array(items) => items.into_iter().map(|v| v.to_string_repr()).collect(),
                JsValue::String(s) => vec![s],
                _ => Vec::new(),
            }
        } else {
            Vec::new()
        };

        let mut values = Vec::new();
        for fname in field_names {
            if let Some(f) = ctx.get_field(&fname) {
                values.push(f.value.to_f64());
            }
        }

        let calc_result = match func_type.to_uppercase().as_str() {
            "SUM" => values.iter().sum::<f64>(),
            "PRD" => {
                if values.is_empty() {
                    0.0
                } else {
                    values.iter().product::<f64>()
                }
            }
            "AVG" => {
                if values.is_empty() {
                    0.0
                } else {
                    values.iter().sum::<f64>() / (values.len() as f64)
                }
            }
            "MIN" => values
                .iter()
                .copied()
                .fold(f64::INFINITY, |a, b| if a < b { a } else { b }),
            "MAX" => values
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, |a, b| if a > b { a } else { b }),
            _ => 0.0,
        };

        ctx.event.value = JsValue::Number(calc_result);
        return Ok(Some(JsValue::Number(calc_result)));
    }

    // 9. Acrobat Standard Formatting helper: AFNumber_Format(nDec, nSepStyle, nNegStyle, cCurrency, bCurrencyPrepend)
    if fn_name == "AFNumber_Format" {
        let dec = raw_args
            .first()
            .and_then(|a| eval_expression(ctx, locals, a).ok())
            .map(|v| v.to_f64() as usize)
            .unwrap_or(2);
        let curr = raw_args
            .get(3)
            .and_then(|a| eval_expression(ctx, locals, a).ok())
            .map(|v| v.to_string_repr())
            .unwrap_or_default();
        let curr_prepend = raw_args
            .get(4)
            .and_then(|a| eval_expression(ctx, locals, a).ok())
            .map(|v| v.is_truthy())
            .unwrap_or(true);

        let num = ctx.event.value.to_f64();
        let formatted_num = format!("{:.1$}", num, dec);
        let result = if curr.is_empty() {
            formatted_num
        } else if curr_prepend {
            format!("{}{}", curr, formatted_num)
        } else {
            format!("{} {}", formatted_num, curr)
        };
        ctx.event.value = JsValue::String(result.clone());
        return Ok(Some(JsValue::String(result)));
    }

    // 10. Acrobat Percent Formatting helper: AFPercent_Format(nDec, nSepStyle)
    if fn_name == "AFPercent_Format" {
        let dec = raw_args
            .first()
            .and_then(|a| eval_expression(ctx, locals, a).ok())
            .map(|v| v.to_f64() as usize)
            .unwrap_or(2);
        let pct = ctx.event.value.to_f64() * 100.0;
        let result = format!("{:.1$}%", pct, dec);
        ctx.event.value = JsValue::String(result.clone());
        return Ok(Some(JsValue::String(result)));
    }

    // 11. Acrobat Range Validation helper: AFRange_Validate(bGreaterThan, nLower, bLessThan, nUpper)
    if fn_name == "AFRange_Validate" {
        let b_lower = raw_args
            .first()
            .and_then(|a| eval_expression(ctx, locals, a).ok())
            .map(|v| v.is_truthy())
            .unwrap_or(false);
        let lower = raw_args
            .get(1)
            .and_then(|a| eval_expression(ctx, locals, a).ok())
            .map(|v| v.to_f64())
            .unwrap_or(0.0);
        let b_upper = raw_args
            .get(2)
            .and_then(|a| eval_expression(ctx, locals, a).ok())
            .map(|v| v.is_truthy())
            .unwrap_or(false);
        let upper = raw_args
            .get(3)
            .and_then(|a| eval_expression(ctx, locals, a).ok())
            .map(|v| v.to_f64())
            .unwrap_or(100.0);

        let val = ctx.event.value.to_f64();
        let mut valid = true;
        if b_lower && val < lower {
            valid = false;
        }
        if b_upper && val > upper {
            valid = false;
        }

        ctx.event.rc = valid;
        return Ok(Some(JsValue::Boolean(valid)));
    }

    // 12. util.printf(cFormat, ...) (FR-12.5)
    if fn_name == "util.printf" {
        let fmt = raw_args
            .first()
            .and_then(|a| eval_expression(ctx, locals, a).ok())
            .map(|v| v.to_string_repr())
            .unwrap_or_default();
        let val_arg = raw_args
            .get(1)
            .and_then(|a| eval_expression(ctx, locals, a).ok())
            .unwrap_or(JsValue::Undefined);

        let res = if fmt.contains("%.2f") {
            format!("{:.2}", val_arg.to_f64())
        } else if fmt.contains("%d") {
            format!("{:.0}", val_arg.to_f64())
        } else {
            val_arg.to_string_repr()
        };
        return Ok(Some(JsValue::String(res)));
    }

    // 13. Math library functions
    if let Some(mfn) = fn_name.strip_prefix("Math.") {
        let a = raw_args
            .first()
            .and_then(|x| eval_expression(ctx, locals, x).ok())
            .map(|v| v.to_f64())
            .unwrap_or(0.0);
        let b = raw_args
            .get(1)
            .and_then(|x| eval_expression(ctx, locals, x).ok())
            .map(|v| v.to_f64())
            .unwrap_or(0.0);

        let m_res = match mfn {
            "abs" => a.abs(),
            "round" => a.round(),
            "floor" => a.floor(),
            "ceil" => a.ceil(),
            "sqrt" => a.sqrt(),
            "min" => a.min(b),
            "max" => a.max(b),
            "pow" => a.powf(b),
            _ => a,
        };
        return Ok(Some(JsValue::Number(m_res)));
    }

    // 14. Standard conversions: parseInt, parseFloat, isNaN, Number, String, Boolean
    if fn_name == "parseFloat" || fn_name == "Number" {
        let val = raw_args
            .first()
            .and_then(|x| eval_expression(ctx, locals, x).ok())
            .map(|v| v.to_f64())
            .unwrap_or(0.0);
        return Ok(Some(JsValue::Number(val)));
    }
    if fn_name == "parseInt" {
        let val = raw_args
            .first()
            .and_then(|x| eval_expression(ctx, locals, x).ok())
            .map(|v| v.to_f64().floor())
            .unwrap_or(0.0);
        return Ok(Some(JsValue::Number(val)));
    }
    if fn_name == "isNaN" {
        let val = raw_args
            .first()
            .and_then(|x| eval_expression(ctx, locals, x).ok())
            .map(|v| v.to_f64().is_nan())
            .unwrap_or(false);
        return Ok(Some(JsValue::Boolean(val)));
    }
    if fn_name == "String" {
        let val = raw_args
            .first()
            .and_then(|x| eval_expression(ctx, locals, x).ok())
            .map(|v| v.to_string_repr())
            .unwrap_or_default();
        return Ok(Some(JsValue::String(val)));
    }
    if fn_name == "Boolean" {
        let val = raw_args
            .first()
            .and_then(|x| eval_expression(ctx, locals, x).ok())
            .map(|v| v.is_truthy())
            .unwrap_or(false);
        return Ok(Some(JsValue::Boolean(val)));
    }

    // 15. Array constructor: new Array(...)
    if fn_name == "new Array" || fn_name == "Array" {
        let mut list = Vec::new();
        for arg in raw_args {
            list.push(eval_expression(ctx, locals, &arg)?);
        }
        return Ok(Some(JsValue::Array(list)));
    }

    Ok(None)
}

// ============================================================================
// Expression Splitting & Parsing Utilities
// ============================================================================

fn split_ternary(expr: &str) -> Option<(&str, &str, &str)> {
    let bytes = expr.as_bytes();
    let mut in_single = false;
    let mut in_double = false;
    let mut paren_depth = 0;
    let mut question_pos = None;

    for (i, &b) in bytes.iter().enumerate() {
        if b == b'\'' && !in_double {
            in_single = !in_single;
        } else if b == b'"' && !in_single {
            in_double = !in_double;
        } else if b == b'(' && !in_single && !in_double {
            paren_depth += 1;
        } else if b == b')' && !in_single && !in_double {
            paren_depth -= 1;
        } else if b == b'?' && !in_single && !in_double && paren_depth == 0 {
            question_pos = Some(i);
            break;
        }
    }

    if let Some(q_idx) = question_pos {
        let cond = &expr[..q_idx];
        let rest = &expr[q_idx + 1..];
        let mut colon_pos = None;
        let mut sub_depth = 0;
        let mut in_s = false;
        let mut in_d = false;

        for (i, b) in rest.bytes().enumerate() {
            if b == b'\'' && !in_d {
                in_s = !in_s;
            } else if b == b'"' && !in_s {
                in_d = !in_d;
            } else if b == b'(' && !in_s && !in_d {
                sub_depth += 1;
            } else if b == b')' && !in_s && !in_d {
                sub_depth -= 1;
            } else if b == b':' && !in_s && !in_d && sub_depth == 0 {
                colon_pos = Some(i);
                break;
            }
        }

        if let Some(c_idx) = colon_pos {
            let then_part = &rest[..c_idx];
            let else_part = &rest[c_idx + 1..];
            return Some((cond, then_part, else_part));
        }
    }
    None
}

fn split_binary_op<'a>(expr: &'a str, op: &str) -> Option<(&'a str, &'a str)> {
    let bytes = expr.as_bytes();
    let op_bytes = op.as_bytes();
    let mut in_single = false;
    let mut in_double = false;
    let mut paren_depth = 0;

    let mut i = bytes.len();
    while i >= op_bytes.len() {
        i -= 1;
        let b = bytes[i];
        if b == b'\'' && !in_double {
            in_single = !in_single;
        } else if b == b'"' && !in_single {
            in_double = !in_double;
        } else if b == b')' && !in_single && !in_double {
            paren_depth += 1;
        } else if b == b'(' && !in_single && !in_double {
            paren_depth -= 1;
        } else if paren_depth == 0
            && !in_single
            && !in_double
            && i + op_bytes.len() <= bytes.len()
            && &bytes[i..i + op_bytes.len()] == op_bytes
        {
            let left = &expr[..i];
            let right = &expr[i + op_bytes.len()..];
            return Some((left, right));
        }
    }
    None
}

fn split_additive_op<'a>(expr: &'a str, op: &str) -> Option<(&'a str, &'a str)> {
    let bytes = expr.as_bytes();
    let op_char = op.as_bytes()[0];
    let mut in_single = false;
    let mut in_double = false;
    let mut paren_depth = 0;

    let mut i = bytes.len();
    while i > 0 {
        i -= 1;
        let b = bytes[i];
        if b == b'\'' && !in_double {
            in_single = !in_single;
        } else if b == b'"' && !in_single {
            in_double = !in_double;
        } else if b == b')' && !in_single && !in_double {
            paren_depth += 1;
        } else if b == b'(' && !in_single && !in_double {
            paren_depth -= 1;
        } else if b == op_char && paren_depth == 0 && !in_single && !in_double {
            if i == 0 {
                continue;
            }
            let prev = bytes[i - 1];
            // Ensure not unary or scientific notation (e.g. 1e-5)
            if prev == b'+'
                || prev == b'-'
                || prev == b'*'
                || prev == b'/'
                || prev == b'='
                || prev == b'e'
                || prev == b'E'
            {
                continue;
            }
            let left = &expr[..i];
            let right = &expr[i + 1..];
            return Some((left, right));
        }
    }
    None
}

fn parse_call_arguments(args_str: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut paren_depth = 0;
    let mut bracket_depth = 0;

    for ch in args_str.chars() {
        match ch {
            '\'' if !in_double => {
                in_single = !in_single;
                current.push(ch);
            }
            '"' if !in_single => {
                in_double = !in_double;
                current.push(ch);
            }
            '(' if !in_single && !in_double => {
                paren_depth += 1;
                current.push(ch);
            }
            ')' if !in_single && !in_double => {
                paren_depth -= 1;
                current.push(ch);
            }
            '[' if !in_single && !in_double => {
                bracket_depth += 1;
                current.push(ch);
            }
            ']' if !in_single && !in_double => {
                bracket_depth -= 1;
                current.push(ch);
            }
            ',' if !in_single && !in_double && paren_depth == 0 && bracket_depth == 0 => {
                args.push(current.trim().to_string());
                current.clear();
            }
            _ => {
                current.push(ch);
            }
        }
    }
    if !current.trim().is_empty() {
        args.push(current.trim().to_string());
    }
    args
}

fn is_balanced_parentheses(s: &str) -> bool {
    let mut depth = 0;
    for ch in s.chars() {
        if ch == '(' {
            depth += 1;
        } else if ch == ')' {
            depth -= 1;
            if depth < 0 {
                return false;
            }
        }
    }
    depth == 0
}

fn clean_string_literal(lit: &str) -> String {
    let trimmed = lit.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

// ============================================================================
// ISO 32000-1 Document / Names Tree JavaScript Extraction & Injection (FR-12.2)
// ============================================================================

/// Extract all document-level JavaScript scripts from ISO 32000-1 Section 7.7.4 Name Tree.
pub fn get_document_javascript(doc: &PdfDocument) -> Result<Vec<(String, String)>> {
    let mut scripts = Vec::new();

    let catalog_obj = doc
        .get_object(doc.root_id)
        .ok_or_else(|| anyhow!("Missing document catalog"))?;
    let catalog = catalog_obj
        .as_dict()
        .ok_or_else(|| anyhow!("Catalog is not a dictionary"))?;

    // 1. Check Document /OpenAction if it is a JavaScript Action (ISO 32000-1 Clause 12.6.4.16)
    if let Some(open_act_obj) = catalog.get("OpenAction") {
        if let Some(resolved) = doc.resolve(open_act_obj) {
            if let Some(dict) = resolved.as_dict() {
                if let Some(action_type) = dict.get("S") {
                    if action_type.as_name() == Some("JavaScript") {
                        if let Some(js_obj) = dict.get("JS") {
                            let js_text = extract_js_text(doc, js_obj)?;
                            scripts.push(("OpenAction".to_string(), js_text));
                        }
                    }
                }
            }
        }
    }

    // 2. Check /Names -> /JavaScript name tree (ISO 32000-1 Clause 7.7.4)
    let names_dict_obj = match catalog.get("Names").and_then(|obj| doc.resolve(obj)) {
        Some(resolved) => resolved,
        None => return Ok(scripts),
    };
    let names_dict = match names_dict_obj.as_dict() {
        Some(d) => d,
        None => return Ok(scripts),
    };

    let js_dict_obj = match names_dict.get("JavaScript").and_then(|obj| doc.resolve(obj)) {
        Some(resolved) => resolved,
        None => return Ok(scripts),
    };
    let js_dict = match js_dict_obj.as_dict() {
        Some(d) => d,
        None => return Ok(scripts),
    };

    if let Some(names_arr_obj) = js_dict.get("Names").and_then(|obj| doc.resolve(obj)) {
        if let Some(names_arr) = names_arr_obj.as_array() {
            let mut i = 0;
            while i + 1 < names_arr.len() {
                let name_val = match &names_arr[i] {
                    Object::String(s) => s.to_string_lossy(),
                    Object::Name(n) => n.0.clone(),
                    _ => format!("Script_{}", i / 2),
                };
                let action_obj = doc.resolve(&names_arr[i + 1]);
                if let Some(act_dict) = action_obj.and_then(|o| o.as_dict().cloned()) {
                    if let Some(js_obj) = act_dict.get("JS") {
                        let js_text = extract_js_text(doc, js_obj)?;
                        scripts.push((name_val, js_text));
                    }
                }
                i += 2;
            }
        }
    }

    Ok(scripts)
}

/// Embed document-level JavaScript conforming to ISO 32000-1 Clause 12.6.4.16 & Clause 7.7.4.
pub fn add_document_javascript(
    doc: &mut PdfDocument,
    script_name: &str,
    script_code: &str,
) -> Result<()> {
    // 1. Create JavaScript Action dictionary
    let action_id = doc.next_object_id();
    let mut js_action = PdfDictionary::new();
    js_action.insert("Type", PdfName::new("Action"));
    js_action.insert("S", PdfName::new("JavaScript"));
    js_action.insert("JS", PdfString::from_str(script_code));
    doc.objects.insert(action_id, Object::Dictionary(js_action));

    // 2. Resolve or create /Names dictionary in Catalog
    let catalog = doc
        .get_object_mut(doc.root_id)
        .and_then(|o| o.as_dict_mut())
        .ok_or_else(|| anyhow!("Invalid document catalog"))?;

    let names_id = if let Some(Object::Reference(id)) = catalog.get("Names") {
        *id
    } else {
        let nid = doc.next_object_id();
        let names_dict = PdfDictionary::new();
        doc.objects.insert(nid, Object::Dictionary(names_dict));
        let catalog = doc.get_object_mut(doc.root_id).and_then(|o| o.as_dict_mut()).unwrap();
        catalog.insert("Names", nid);
        nid
    };

    // 3. Resolve or create /JavaScript name tree node
    let names_dict = doc
        .get_object_mut(names_id)
        .and_then(|o| o.as_dict_mut())
        .ok_or_else(|| anyhow!("Invalid names dictionary"))?;

    let js_node_id = if let Some(Object::Reference(id)) = names_dict.get("JavaScript") {
        *id
    } else {
        let jid = doc.next_object_id();
        let mut js_tree = PdfDictionary::new();
        js_tree.insert("Names", vec![]);
        doc.objects.insert(jid, Object::Dictionary(js_tree));
        let names_dict = doc.get_object_mut(names_id).and_then(|o| o.as_dict_mut()).unwrap();
        names_dict.insert("JavaScript", jid);
        jid
    };

    // 4. Append script to Names array
    let js_tree = doc
        .get_object_mut(js_node_id)
        .and_then(|o| o.as_dict_mut())
        .ok_or_else(|| anyhow!("Invalid JavaScript names dictionary"))?;

    let mut names_vec: Vec<Object> = match js_tree.get("Names").and_then(|o| o.as_array()) {
        Some(arr) => arr.to_vec(),
        None => Vec::new(),
    };

    names_vec.push(Object::String(PdfString::from_str(script_name)));
    names_vec.push(Object::Reference(action_id));
    js_tree.insert("Names", names_vec);

    Ok(())
}

fn extract_js_text(doc: &PdfDocument, js_obj: &Object) -> Result<String> {
    match doc.resolve(js_obj) {
        Some(Object::String(s)) => Ok(s.to_string_lossy()),
        Some(Object::Stream(st)) => {
            let data = st.decode().unwrap_or_else(|_| st.raw_data.clone());
            Ok(String::from_utf8_lossy(&data).to_string())
        }
        _ => Err(anyhow!("Invalid JavaScript object representation in action")),
    }
}

// ============================================================================
// AcroForm Field Calculation & Formatting Execution Engine (FR-12.3, FR-11.2.4)
// ============================================================================

/// Execute calculation scripts for all form fields with attached calculation logic (FR-12.3, FR-11.2.4).
pub fn execute_all_field_calculations(ctx: &mut JsExecutionContext) -> Result<usize> {
    if !ctx.js_enabled {
        return Ok(0);
    }

    let field_names: Vec<String> = ctx.fields.keys().cloned().collect();
    let mut calculated_count = 0;

    for name in field_names {
        if let Some(calc_script) = ctx
            .get_field(&name)
            .and_then(|f| f.calculate_script.clone())
        {
            ctx.event.name = "Calculate".to_string();
            ctx.event.target_name = name.clone();
            ctx.event.value = ctx
                .get_field(&name)
                .map(|f| f.value.clone())
                .unwrap_or(JsValue::Undefined);

            execute_script(ctx, &calc_script)?;

            let new_val = ctx.event.value.clone();
            if let Some(f) = ctx.get_field_mut(&name) {
                f.value = new_val;
            }
            calculated_count += 1;
        }
    }

    Ok(calculated_count)
}

/// Execute formatting script for a specific field (FR-12.3, FR-11.2.5).
pub fn execute_field_formatting(ctx: &mut JsExecutionContext, field_name: &str) -> Result<String> {
    if !ctx.js_enabled {
        return Ok(ctx
            .get_field(field_name)
            .map(|f| f.value.to_string_repr())
            .unwrap_or_default());
    }

    let format_script = match ctx
        .get_field(field_name)
        .and_then(|f| f.format_script.clone())
    {
        Some(s) => s,
        None => {
            return Ok(ctx
                .get_field(field_name)
                .map(|f| f.value.to_string_repr())
                .unwrap_or_default())
        }
    };

    ctx.event.name = "Format".to_string();
    ctx.event.target_name = field_name.to_string();
    ctx.event.value = ctx
        .get_field(field_name)
        .map(|f| f.value.clone())
        .unwrap_or(JsValue::Undefined);

    execute_script(ctx, &format_script)?;
    Ok(ctx.event.value.to_string_repr())
}

/// Execute validation script for a specific field (FR-12.3, FR-11.2.3).
pub fn execute_field_validation(
    ctx: &mut JsExecutionContext,
    field_name: &str,
    new_value: JsValue,
) -> Result<bool> {
    if !ctx.js_enabled {
        return Ok(true);
    }

    let validate_script = match ctx
        .get_field(field_name)
        .and_then(|f| f.validate_script.clone())
    {
        Some(s) => s,
        None => return Ok(true),
    };

    ctx.event.name = "Validate".to_string();
    ctx.event.target_name = field_name.to_string();
    ctx.event.value = new_value;
    ctx.event.rc = true;

    execute_script(ctx, &validate_script)?;
    Ok(ctx.event.rc)
}

// ============================================================================
// Unit Tests (FR-12.1 - FR-12.11)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arithmetic_and_variable_evaluation() {
        let mut ctx = JsExecutionContext::new();
        let code = r#"
            var a = 10;
            var b = 25;
            var c = a * b + 50;
            c;
        "#;
        let res = execute_script(&mut ctx, code).expect("exec script");
        assert_eq!(res, JsValue::Number(300.0));
    }

    #[test]
    fn test_conditional_logic_and_string_concat() {
        let mut ctx = JsExecutionContext::new();
        let code = r#"
            var score = 85;
            var status = "";
            if (score >= 80) {
                status = "Passed with Honor";
            } else {
                status = "Regular Pass";
            }
            status;
        "#;
        let res = execute_script(&mut ctx, code).expect("exec if");
        assert_eq!(res, JsValue::String("Passed with Honor".to_string()));
    }

    #[test]
    fn test_field_calculate_and_acrobat_helpers() {
        let mut ctx = JsExecutionContext::new();
        ctx.register_field(JsFormField::new("subtotal", "100"));
        ctx.register_field(JsFormField::new("tax", "8.5"));
        ctx.register_field(JsFormField::new("total", "0"));

        // AFSimple_Calculate SUM
        let calc_script = r#"
            AFSimple_Calculate("SUM", new Array("subtotal", "tax"));
        "#;
        let res = execute_script(&mut ctx, calc_script).expect("calculate");
        assert_eq!(res, JsValue::Number(108.5));
        assert_eq!(ctx.event.value, JsValue::Number(108.5));

        // Format total as currency
        let fmt_script = r#"
            AFNumber_Format(2, 0, 0, "$", true);
        "#;
        let fmt_res = execute_script(&mut ctx, fmt_script).expect("format");
        assert_eq!(fmt_res, JsValue::String("$108.50".to_string()));
    }

    #[test]
    fn test_app_alert_console_and_dialogs() {
        let mut ctx = JsExecutionContext::new();
        let script = r#"
            console.println("Processing transaction...");
            app.alert("Invoice generated successfully!", "Success", 3, 0);
            app.beep();
        "#;
        execute_script(&mut ctx, script).expect("alert & beep");
        assert_eq!(ctx.console_log.len(), 1);
        assert_eq!(ctx.console_log[0], "Processing transaction...");
        assert_eq!(ctx.alerts.len(), 1);
        assert_eq!(ctx.alerts[0].message, "Invoice generated successfully!");
        assert_eq!(ctx.alerts[0].title, "Success");
        assert_eq!(ctx.beeps, 1);
    }

    #[test]
    fn test_document_javascript_name_tree_lifecycle() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        let script = "app.alert('Welcome to RavPDF!');";
        add_document_javascript(&mut doc, "InitDoc", script).expect("add doc js");

        let retrieved = get_document_javascript(&doc).expect("get doc js");
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].0, "InitDoc");
        assert_eq!(retrieved[0].1, script);
    }

    #[test]
    fn test_sandboxing_blocks_forbidden_primitives() {
        let mut ctx = JsExecutionContext::new();
        let dangerous_script = r#"
            var fs = require("fs");
            fs.readFileSync("/etc/passwd");
        "#;
        let err = execute_script(&mut ctx, dangerous_script);
        assert!(err.is_err());
        let err_msg = err.unwrap_err().to_string();
        assert!(err_msg.contains("JavaScript Sandbox Security Violation"));
    }
}
