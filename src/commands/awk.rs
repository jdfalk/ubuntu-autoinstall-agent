// file: src/commands/awk.rs
// version: 1.0.0
// guid: 9b2c3d4e-5f6a-7b8c-9d0e-1f2a3b4c5d6e

use crate::executor::Executor;
use anyhow::{anyhow, Result};
use clap::{Arg, ArgMatches, Command};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

/// Build the awk command with comprehensive options
pub fn build_command() -> Command {
    Command::new("awk")
        .about("Pattern scanning and processing language (Rust implementation)")
        .arg(Arg::new("program")
            .help("AWK program text")
            .required(true))
        .arg(Arg::new("file")
            .help("Input files")
            .action(clap::ArgAction::Append))
        .arg(Arg::new("field-separator")
            .help("Field separator")
            .short('F')
            .long("field-separator")
            .value_name("FS"))
        .arg(Arg::new("assign")
            .help("Variable assignment")
            .short('v')
            .long("assign")
            .action(clap::ArgAction::Append)
            .value_name("VAR=VALUE"))
        .arg(Arg::new("file-program")
            .help("Read program from file")
            .short('f')
            .long("file")
            .value_name("PROGFILE"))
}

/// Execute awk commands with Rust-native implementation
pub async fn execute(matches: &ArgMatches, _executor: &Executor) -> Result<()> {
    let program_text = if let Some(prog_file) = matches.get_one::<String>("file-program") {
        fs::read_to_string(prog_file)?
    } else {
        matches.get_one::<String>("program")
            .ok_or_else(|| anyhow!("No AWK program provided"))?
            .clone()
    };

    let files: Vec<_> = matches.get_many::<String>("file")
        .map(|vals| vals.cloned().collect())
        .unwrap_or_default();

    let field_separator = matches.get_one::<String>("field-separator")
        .map(|s| s.clone())
        .unwrap_or_else(|| " ".to_string());

    let assignments = matches.get_many::<String>("assign")
        .map(|vals| vals.cloned().collect())
        .unwrap_or_default();

    // Parse the AWK program
    let program = parse_awk_program(&program_text)?;

    // Initialize AWK context
    let mut context = AwkContext::new(field_separator, assignments)?;

    if files.is_empty() {
        // Read from stdin
        process_input(Box::new(io::stdin().lock()), &program, &mut context)?;
    } else {
        // Process files
        for file_path in &files {
            context.filename = file_path.clone();
            context.fnr = 0; // Reset file line number

            let path = Path::new(file_path);
            if !path.exists() {
                eprintln!("awk: {}: No such file or directory", file_path);
                continue;
            }

            let file = fs::File::open(path)?;
            let reader = Box::new(BufReader::new(file));
            process_input(reader, &program, &mut context)?;
        }
    }

    // Execute END rules
    for rule in &program.end_rules {
        execute_action(&rule.action, &mut context, &[])?;
    }

    Ok(())
}

#[derive(Debug)]
struct AwkProgram {
    begin_rules: Vec<AwkRule>,
    pattern_rules: Vec<AwkRule>,
    end_rules: Vec<AwkRule>,
}

#[derive(Debug)]
struct AwkRule {
    pattern: Option<AwkPattern>,
    action: AwkAction,
}

#[derive(Debug)]
#[allow(dead_code)] // Part of complete AWK implementation
enum AwkPattern {
    Expression(String),
    Range(String, String),
}

#[derive(Debug)]
#[allow(dead_code)] // Part of complete AWK implementation
enum AwkAction {
    Block(Vec<AwkStatement>),
    Print(Option<String>),
    PrintF(String, Vec<String>),
}

#[derive(Debug)]
#[allow(dead_code)] // Part of complete AWK implementation
enum AwkStatement {
    Print(Option<String>),
    PrintF(String, Vec<String>),
    Assignment(String, String),
    If(String, Box<AwkStatement>, Option<Box<AwkStatement>>),
    For(String, String, String, Box<AwkStatement>),
    While(String, Box<AwkStatement>),
    Break,
    Continue,
    Next,
    Exit(Option<String>),
}

struct AwkContext {
    variables: HashMap<String, String>,
    fields: Vec<String>,
    nr: usize,  // Total record number
    fnr: usize, // File record number
    nf: usize,  // Number of fields
    filename: String,
    fs: String, // Field separator
    ofs: String, // Output field separator
    ors: String, // Output record separator
    rs: String,  // Record separator
}

impl AwkContext {
    fn new(field_separator: String, assignments: Vec<String>) -> Result<Self> {
        let mut variables = HashMap::new();

        // Process variable assignments
        for assignment in assignments {
            let parts: Vec<&str> = assignment.splitn(2, '=').collect();
            if parts.len() == 2 {
                variables.insert(parts[0].to_string(), parts[1].to_string());
            }
        }

        Ok(Self {
            variables,
            fields: Vec::new(),
            nr: 0,
            fnr: 0,
            nf: 0,
            filename: String::new(),
            fs: field_separator,
            ofs: " ".to_string(),
            ors: "\n".to_string(),
            rs: "\n".to_string(),
        })
    }

    fn split_record(&mut self, record: &str) {
        self.fields.clear();
        self.fields.push(record.to_string()); // $0 is the whole record

        // Split by field separator
        let parts: Vec<&str> = if self.fs == " " {
            // Special case: space means any whitespace
            record.split_whitespace().collect()
        } else {
            record.split(&self.fs).collect()
        };

        for part in parts {
            self.fields.push(part.to_string());
        }

        self.nf = self.fields.len() - 1; // Don't count $0
    }

    fn get_field(&self, index: usize) -> String {
        if index < self.fields.len() {
            self.fields[index].clone()
        } else {
            String::new()
        }
    }

    fn get_variable(&self, name: &str) -> String {
        match name {
            "NR" => self.nr.to_string(),
            "FNR" => self.fnr.to_string(),
            "NF" => self.nf.to_string(),
            "FILENAME" => self.filename.clone(),
            "FS" => self.fs.clone(),
            "OFS" => self.ofs.clone(),
            "ORS" => self.ors.clone(),
            "RS" => self.rs.clone(),
            _ => self.variables.get(name).cloned().unwrap_or_default(),
        }
    }

    #[allow(dead_code)]
    fn set_variable(&mut self, name: &str, value: &str) {
        match name {
            "FS" => self.fs = value.to_string(),
            "OFS" => self.ofs = value.to_string(),
            "ORS" => self.ors = value.to_string(),
            "RS" => self.rs = value.to_string(),
            _ => {
                self.variables.insert(name.to_string(), value.to_string());
            }
        }
    }
}

/// Parse AWK program text
fn parse_awk_program(program_text: &str) -> Result<AwkProgram> {
    let mut begin_rules = Vec::new();
    let mut pattern_rules = Vec::new();
    let mut end_rules = Vec::new();

    let lines: Vec<&str> = program_text.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        if line.is_empty() || line.starts_with('#') {
            i += 1;
            continue;
        }

        if line.starts_with("BEGIN") {
            let action_text = extract_action(&lines, &mut i)?;
            let action = parse_action(&action_text)?;
            begin_rules.push(AwkRule {
                pattern: None,
                action,
            });
        } else if line.starts_with("END") {
            let action_text = extract_action(&lines, &mut i)?;
            let action = parse_action(&action_text)?;
            end_rules.push(AwkRule {
                pattern: None,
                action,
            });
        } else {
            // Pattern-action rule or just action
            let (pattern, action_text) = if line.contains('{') {
                let brace_pos = line.find('{').unwrap();
                if brace_pos > 0 {
                    let pattern_text = line[..brace_pos].trim();
                    let pattern = if pattern_text.is_empty() {
                        None
                    } else {
                        Some(parse_pattern(pattern_text)?)
                    };
                    let action_text = extract_action(&lines, &mut i)?;
                    (pattern, action_text)
                } else {
                    let action_text = extract_action(&lines, &mut i)?;
                    (None, action_text)
                }
            } else {
                // Just a pattern, default action is print
                (Some(parse_pattern(line)?), "{ print }".to_string())
            };

            let action = parse_action(&action_text)?;
            pattern_rules.push(AwkRule { pattern, action });
        }

        i += 1;
    }

    Ok(AwkProgram {
        begin_rules,
        pattern_rules,
        end_rules,
    })
}

/// Extract action block from lines
fn extract_action(lines: &[&str], index: &mut usize) -> Result<String> {
    let mut action_text = String::new();
    let mut brace_count = 0;
    let mut found_start = false;

    while *index < lines.len() {
        let line = lines[*index];

        for ch in line.chars() {
            if ch == '{' {
                brace_count += 1;
                found_start = true;
            } else if ch == '}' {
                brace_count -= 1;
            }
        }

        if found_start {
            action_text.push_str(line);
            action_text.push('\n');
        }

        if found_start && brace_count == 0 {
            break;
        }

        *index += 1;
    }

    Ok(action_text)
}

/// Parse AWK pattern
fn parse_pattern(pattern_text: &str) -> Result<AwkPattern> {
    // For now, treat everything as an expression
    Ok(AwkPattern::Expression(pattern_text.to_string()))
}

/// Parse AWK action
fn parse_action(action_text: &str) -> Result<AwkAction> {
    let trimmed = action_text.trim();

    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        let inner = &trimmed[1..trimmed.len()-1].trim();

        // Simple parsing - just handle basic statements
        if inner.is_empty() || *inner == "print" {
            return Ok(AwkAction::Print(None));
        }

        if inner.starts_with("print ") {
            let expr = inner[6..].trim().to_string();
            return Ok(AwkAction::Print(Some(expr)));
        }

        if inner.starts_with("printf ") {
            // Simple printf parsing
            let args = inner[7..].trim();
            return Ok(AwkAction::PrintF(args.to_string(), Vec::new()));
        }

        // For now, treat complex blocks as print statements
        Ok(AwkAction::Print(None))
    } else {
        Ok(AwkAction::Print(None))
    }
}

/// Process input with AWK program
fn process_input(
    mut reader: Box<dyn BufRead>,
    program: &AwkProgram,
    context: &mut AwkContext,
) -> Result<()> {
    // Execute BEGIN rules
    for rule in &program.begin_rules {
        execute_action(&rule.action, context, &[])?;
    }

    // Process each line
    let mut line = String::new();
    while reader.read_line(&mut line)? > 0 {
        // Remove trailing newline
        if line.ends_with('\n') {
            line.pop();
            if line.ends_with('\r') {
                line.pop();
            }
        }

        context.nr += 1;
        context.fnr += 1;
        context.split_record(&line);

        // Execute pattern-action rules
        for rule in &program.pattern_rules {
            if match_pattern(&rule.pattern, context, &line)? {
                execute_action(&rule.action, context, &context.fields.clone())?;
            }
        }

        line.clear();
    }

    Ok(())
}

/// Check if pattern matches current record
fn match_pattern(
    pattern: &Option<AwkPattern>,
    context: &AwkContext,
    line: &str,
) -> Result<bool> {
    match pattern {
        None => Ok(true), // No pattern means always match
        Some(AwkPattern::Expression(expr)) => {
            // Simple expression evaluation
            evaluate_expression(expr, context, line)
        }
        Some(AwkPattern::Range(_, _)) => {
            // Range patterns not implemented yet
            Ok(true)
        }
    }
}

/// Evaluate AWK expression (simplified)
fn evaluate_expression(expr: &str, context: &AwkContext, line: &str) -> Result<bool> {
    let expr = expr.trim();

    // Handle simple field comparisons
    if expr.contains("==") {
        let parts: Vec<&str> = expr.split("==").collect();
        if parts.len() == 2 {
            let left = evaluate_field_or_variable(parts[0].trim(), context)?;
            let right = parts[1].trim().trim_matches('"');
            return Ok(left == right);
        }
    }

    if expr.contains("~") {
        let parts: Vec<&str> = expr.split('~').collect();
        if parts.len() == 2 {
            let left = evaluate_field_or_variable(parts[0].trim(), context)?;
            let pattern = parts[1].trim().trim_matches('/');
            let regex = Regex::new(pattern)?;
            return Ok(regex.is_match(&left));
        }
    }

    // Handle regex patterns
    if expr.starts_with('/') && expr.ends_with('/') {
        let pattern = &expr[1..expr.len()-1];
        let regex = Regex::new(pattern)?;
        return Ok(regex.is_match(line));
    }

    // Handle simple field references
    if expr.starts_with('$') {
        let field_num_str = &expr[1..];
        if let Ok(field_num) = field_num_str.parse::<usize>() {
            let field_value = context.get_field(field_num);
            return Ok(!field_value.is_empty());
        }
    }

    // Default: treat as true if non-empty
    Ok(!expr.is_empty())
}

/// Evaluate field or variable reference
fn evaluate_field_or_variable(expr: &str, context: &AwkContext) -> Result<String> {
    let expr = expr.trim();

    if expr.starts_with('$') {
        let field_ref = &expr[1..];
        if let Ok(field_num) = field_ref.parse::<usize>() {
            Ok(context.get_field(field_num))
        } else {
            // $variable
            let var_value = context.get_variable(field_ref);
            if let Ok(field_num) = var_value.parse::<usize>() {
                Ok(context.get_field(field_num))
            } else {
                Ok(String::new())
            }
        }
    } else {
        // Variable reference
        Ok(context.get_variable(expr))
    }
}

/// Execute AWK action
fn execute_action(
    action: &AwkAction,
    context: &mut AwkContext,
    _fields: &[String],
) -> Result<()> {
    match action {
        AwkAction::Print(None) => {
            println!("{}", context.get_field(0));
        }
        AwkAction::Print(Some(expr)) => {
            let output = evaluate_print_expression(expr, context)?;
            println!("{}", output);
        }
        AwkAction::PrintF(format_str, _args) => {
            // Simple printf implementation
            let output = evaluate_print_expression(format_str, context)?;
            print!("{}", output);
        }
        AwkAction::Block(_statements) => {
            // Block execution not fully implemented
            println!("{}", context.get_field(0));
        }
    }

    Ok(())
}

/// Evaluate print expression
fn evaluate_print_expression(expr: &str, context: &AwkContext) -> Result<String> {
    let expr = expr.trim();

    // Handle field references
    if expr.starts_with('$') {
        return evaluate_field_or_variable(expr, context);
    }

    // Handle variable references
    if context.variables.contains_key(expr) || matches!(expr, "NR" | "FNR" | "NF" | "FILENAME") {
        return Ok(context.get_variable(expr));
    }

    // Handle string literals
    if expr.starts_with('"') && expr.ends_with('"') {
        return Ok(expr[1..expr.len()-1].to_string());
    }

    // Handle concatenation (simplified)
    if expr.contains(" ") {
        let parts: Vec<&str> = expr.split_whitespace().collect();
        let mut result = String::new();
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                result.push_str(&context.ofs);
            }
            result.push_str(&evaluate_print_expression(part, context)?);
        }
        return Ok(result);
    }

    // Default: return as-is
    Ok(expr.to_string())
}
