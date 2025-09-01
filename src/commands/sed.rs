// file: src/commands/sed.rs
// version: 1.0.0
// guid: 8a1b2c3d-4e5f-6a7b-8c9d-0e1f2a3b4c5d

use crate::executor::Executor;
use anyhow::{anyhow, Result};
use clap::{Arg, ArgMatches, Command};
use regex::Regex;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

/// Build the sed command with comprehensive options
pub fn build_command() -> Command {
    Command::new("sed")
        .about("Stream editor for filtering and transforming text (Rust implementation)")
        .arg(Arg::new("expression")
            .help("Sed expression/script")
            .short('e')
            .long("expression")
            .action(clap::ArgAction::Append))
        .arg(Arg::new("file")
            .help("Input files")
            .action(clap::ArgAction::Append))
        .arg(Arg::new("in-place")
            .help("Edit files in place")
            .short('i')
            .long("in-place")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("backup")
            .help("Backup suffix for in-place editing")
            .long("backup")
            .value_name("SUFFIX"))
        .arg(Arg::new("quiet")
            .help("Suppress automatic printing of pattern space")
            .short('n')
            .long("quiet")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("extended-regexp")
            .help("Use extended regular expressions")
            .short('r')
            .long("extended-regexp")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("separate")
            .help("Consider files separately")
            .short('s')
            .long("separate")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("unbuffered")
            .help("Load minimal amounts of data and flush output buffers more often")
            .short('u')
            .long("unbuffered")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("null-data")
            .help("Separate lines by NUL characters")
            .short('z')
            .long("null-data")
            .action(clap::ArgAction::SetTrue))
}

/// Execute sed commands with Rust-native implementation
pub async fn execute(matches: &ArgMatches, _executor: &Executor) -> Result<()> {
    let expressions: Vec<_> = matches.get_many::<String>("expression")
        .map(|vals| vals.cloned().collect())
        .unwrap_or_default();

    let files: Vec<_> = matches.get_many::<String>("file")
        .map(|vals| vals.cloned().collect())
        .unwrap_or_default();

    let in_place = matches.get_flag("in-place");
    let backup_suffix = matches.get_one::<String>("backup");
    let quiet = matches.get_flag("quiet");
    let extended_regexp = matches.get_flag("extended-regexp");
    let _separate = matches.get_flag("separate");
    let unbuffered = matches.get_flag("unbuffered");
    let null_data = matches.get_flag("null-data");

    if expressions.is_empty() {
        return Err(anyhow!("No sed expression provided"));
    }

    // Compile sed expressions
    let mut sed_operations = Vec::new();
    for expr in &expressions {
        sed_operations.push(parse_sed_expression(expr, extended_regexp)?);
    }

    if files.is_empty() {
        // Read from stdin
        process_input(
            Box::new(io::stdin().lock()),
            &sed_operations,
            None,
            quiet,
            null_data,
            unbuffered,
        )?;
    } else {
        // Process files
        for file_path in &files {
            let path = Path::new(file_path);
            if !path.exists() {
                eprintln!("sed: can't read {}: No such file or directory", file_path);
                continue;
            }

            let file = fs::File::open(path)?;
            let reader = Box::new(BufReader::new(file));

            if in_place {
                // Create backup if requested
                if let Some(suffix) = backup_suffix {
                    let backup_path = format!("{}{}", file_path, suffix);
                    fs::copy(path, &backup_path)?;
                }

                // Process to memory first, then write back
                let mut output = Vec::new();
                {
                    let mut cursor = io::Cursor::new(&mut output);
                    process_input_to_writer(
                        reader,
                        &sed_operations,
                        &mut cursor,
                        quiet,
                        null_data,
                        unbuffered,
                    )?;
                }

                fs::write(path, output)?;
            } else {
                process_input(
                    reader,
                    &sed_operations,
                    Some(file_path),
                    quiet,
                    null_data,
                    unbuffered,
                )?;
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
enum SedOperation {
    Substitute {
        pattern: Regex,
        replacement: String,
        flags: SubstituteFlags,
    },
    Delete {
        pattern: Option<Regex>,
    },
    Print {
        pattern: Option<Regex>,
    },
    Append {
        text: String,
    },
    Insert {
        text: String,
    },
    Change {
        text: String,
    },
    Next,
    Quit,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // Part of complete sed implementation
struct SubstituteFlags {
    global: bool,
    print: bool,
    write_to: Option<String>, // For future 'w' flag support
    numeric: Option<usize>,
}

/// Parse a sed expression into operations
fn parse_sed_expression(expr: &str, extended_regexp: bool) -> Result<SedOperation> {
    let expr = expr.trim();

    if expr.is_empty() {
        return Err(anyhow!("Empty sed expression"));
    }

    // Handle substitute command (s/pattern/replacement/flags)
    if expr.starts_with('s') && expr.len() > 1 {
        return parse_substitute_command(expr, extended_regexp);
    }

    // Handle delete command
    if expr == "d" {
        return Ok(SedOperation::Delete { pattern: None });
    }

    // Handle print command
    if expr == "p" {
        return Ok(SedOperation::Print { pattern: None });
    }

    // Handle quit command
    if expr == "q" {
        return Ok(SedOperation::Quit);
    }

    // Handle next command
    if expr == "n" {
        return Ok(SedOperation::Next);
    }

    // Handle append command
    if expr.starts_with('a') && expr.len() > 1 {
        let text = expr[1..].trim_start_matches('\\').to_string();
        return Ok(SedOperation::Append { text });
    }

    // Handle insert command
    if expr.starts_with('i') && expr.len() > 1 {
        let text = expr[1..].trim_start_matches('\\').to_string();
        return Ok(SedOperation::Insert { text });
    }

    // Handle change command
    if expr.starts_with('c') && expr.len() > 1 {
        let text = expr[1..].trim_start_matches('\\').to_string();
        return Ok(SedOperation::Change { text });
    }

    Err(anyhow!("Unsupported sed expression: {}", expr))
}

/// Parse substitute command
fn parse_substitute_command(expr: &str, extended_regexp: bool) -> Result<SedOperation> {
    if expr.len() < 4 {
        return Err(anyhow!("Invalid substitute command: {}", expr));
    }

    let delimiter = expr.chars().nth(1).unwrap();
    let parts: Vec<&str> = expr[2..].split(delimiter).collect();

    if parts.len() < 2 {
        return Err(anyhow!("Invalid substitute command format: {}", expr));
    }

    let pattern_str = parts[0];
    let replacement = parts[1].to_string();
    let flags_str = if parts.len() > 2 { parts[2] } else { "" };

    // Create regex pattern
    let pattern = if extended_regexp {
        Regex::new(pattern_str)?
    } else {
        // Convert basic regex to extended regex (simplified)
        let extended_pattern = convert_basic_to_extended_regex(pattern_str);
        Regex::new(&extended_pattern)?
    };

    // Parse flags
    let mut flags = SubstituteFlags {
        global: false,
        print: false,
        write_to: None,
        numeric: None,
    };

    for c in flags_str.chars() {
        match c {
            'g' => flags.global = true,
            'p' => flags.print = true,
            '1'..='9' => {
                flags.numeric = Some(c.to_digit(10).unwrap() as usize);
            }
            _ => {} // Ignore unknown flags for now
        }
    }

    Ok(SedOperation::Substitute {
        pattern,
        replacement,
        flags,
    })
}

/// Convert basic regex to extended regex (simplified conversion)
fn convert_basic_to_extended_regex(basic: &str) -> String {
    // This is a simplified conversion - sed basic regex is quite complex
    basic.replace("\\(", "(")
         .replace("\\)", ")")
         .replace("\\+", "+")
         .replace("\\?", "?")
         .replace("\\{", "{")
         .replace("\\}", "}")
}

/// Process input with sed operations
fn process_input(
    reader: Box<dyn BufRead>,
    operations: &[SedOperation],
    _filename: Option<&str>,
    quiet: bool,
    null_data: bool,
    unbuffered: bool,
) -> Result<()> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    process_input_to_writer(reader, operations, &mut handle, quiet, null_data, unbuffered)
}

/// Process input to a writer
fn process_input_to_writer<W: Write>(
    mut reader: Box<dyn BufRead>,
    operations: &[SedOperation],
    writer: &mut W,
    quiet: bool,
    null_data: bool,
    unbuffered: bool,
) -> Result<()> {
    let separator = if null_data { b'\0' } else { b'\n' };

    loop {
        let mut line = Vec::new();
        let bytes_read = reader.read_until(separator, &mut line)?;

        if bytes_read == 0 {
            break; // EOF
        }

        // Remove the separator
        if line.last() == Some(&separator) {
            line.pop();
        }

        let mut line_str = String::from_utf8_lossy(&line).to_string();
        let mut should_print = !quiet;
        let mut should_continue = true;

        for operation in operations {
            if !should_continue {
                break;
            }

            match operation {
                SedOperation::Substitute { pattern, replacement, flags } => {
                    if flags.global {
                        line_str = pattern.replace_all(&line_str, replacement).to_string();
                    } else if let Some(n) = flags.numeric {
                        // Replace only the nth occurrence
                        let mut count = 0;
                        line_str = pattern.replace_all(&line_str, |_: &regex::Captures| {
                            count += 1;
                            if count == n {
                                replacement.clone()
                            } else {
                                return format!("{}", &line_str); // This is wrong, need to fix
                            }
                        }).to_string();
                    } else {
                        // Replace only first occurrence
                        line_str = pattern.replace(&line_str, replacement).to_string();
                    }

                    if flags.print {
                        should_print = true;
                    }
                }
                SedOperation::Delete { pattern: None } => {
                    should_print = false;
                    should_continue = false;
                }
                SedOperation::Print { pattern: None } => {
                    should_print = true;
                }
                SedOperation::Append { text } => {
                    if should_print {
                        writeln!(writer, "{}", line_str)?;
                    }
                    writeln!(writer, "{}", text)?;
                    should_print = false;
                    if unbuffered {
                        writer.flush()?;
                    }
                }
                SedOperation::Insert { text } => {
                    writeln!(writer, "{}", text)?;
                    if unbuffered {
                        writer.flush()?;
                    }
                }
                SedOperation::Change { text } => {
                    writeln!(writer, "{}", text)?;
                    should_print = false;
                    should_continue = false;
                    if unbuffered {
                        writer.flush()?;
                    }
                }
                SedOperation::Next => {
                    should_continue = false;
                }
                SedOperation::Quit => {
                    if should_print {
                        writeln!(writer, "{}", line_str)?;
                    }
                    return Ok(());
                }
                _ => {} // Pattern-based operations not implemented yet
            }
        }

        if should_print {
            writeln!(writer, "{}", line_str)?;
            if unbuffered {
                writer.flush()?;
            }
        }
    }

    Ok(())
}
