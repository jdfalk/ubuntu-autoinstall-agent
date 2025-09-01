// file: src/commands/prettier.rs
// version: 1.0.0
// guid: f3b23e72-ff46-4cdd-bba2-9f14cede3837

use crate::executor::Executor;
use anyhow::Result;
use clap::{Arg, ArgMatches, Command};
use std::env;
use tracing::{debug, info};

/// Helper function to append additional arguments from environment variable
#[allow(dead_code)]
fn append_additional_args(mut args: Vec<String>) -> Vec<String> {
    if let Ok(additional_args_str) = env::var("COPILOT_AGENT_ADDITIONAL_ARGS") {
        let additional_args: Vec<&str> = additional_args_str.lines().collect();
        for arg in additional_args {
            if !arg.trim().is_empty() {
                args.push(arg.to_string());
            }
        }
    }
    args
}

/// Build the prettier/formatter command with various formatting tools
pub fn build_command() -> Command {
    Command::new("prettier")
        .about("Code formatting and prettification tools")
        .subcommand(
            Command::new("prettier")
                .about("Format JavaScript/TypeScript/CSS/HTML/Markdown files")
                .arg(
                    Arg::new("path")
                        .value_name("PATH")
                        .help("Path to format (defaults to current directory)")
                        .default_value("."),
                )
                .arg(
                    Arg::new("write")
                        .long("write")
                        .short('w')
                        .action(clap::ArgAction::SetTrue)
                        .help("Write formatted output to files"),
                )
                .arg(
                    Arg::new("check")
                        .long("check")
                        .action(clap::ArgAction::SetTrue)
                        .help("Check if files are formatted"),
                )
                .arg(
                    Arg::new("config")
                        .long("config")
                        .short('c')
                        .value_name("CONFIG_FILE")
                        .help("Path to Prettier configuration file"),
                ),
        )
        .subcommand(
            Command::new("black")
                .about("Format Python files with Black")
                .arg(
                    Arg::new("path")
                        .value_name("PATH")
                        .help("Path to format (defaults to current directory)")
                        .default_value("."),
                )
                .arg(
                    Arg::new("check")
                        .long("check")
                        .action(clap::ArgAction::SetTrue)
                        .help("Don't write back, just check"),
                )
                .arg(
                    Arg::new("line-length")
                        .long("line-length")
                        .short('l')
                        .value_name("LENGTH")
                        .help("Line length")
                        .default_value("88"),
                ),
        )
        .subcommand(
            Command::new("isort")
                .about("Sort Python imports")
                .arg(
                    Arg::new("path")
                        .value_name("PATH")
                        .help("Path to format (defaults to current directory)")
                        .default_value("."),
                )
                .arg(
                    Arg::new("check")
                        .long("check")
                        .action(clap::ArgAction::SetTrue)
                        .help("Check import sorting without making changes"),
                )
                .arg(
                    Arg::new("diff")
                        .long("diff")
                        .action(clap::ArgAction::SetTrue)
                        .help("Show diff of changes"),
                ),
        )
        .subcommand(
            Command::new("rustfmt").about("Format Rust code").arg(
                Arg::new("check")
                    .long("check")
                    .action(clap::ArgAction::SetTrue)
                    .help("Check formatting without making changes"),
            ),
        )
        .subcommand(
            Command::new("gofmt")
                .about("Format Go code")
                .arg(
                    Arg::new("path")
                        .value_name("PATH")
                        .help("Path to format (defaults to current directory)")
                        .default_value("."),
                )
                .arg(
                    Arg::new("write")
                        .long("write")
                        .short('w')
                        .action(clap::ArgAction::SetTrue)
                        .help("Write result to file instead of stdout"),
                )
                .arg(
                    Arg::new("diff")
                        .long("diff")
                        .short('d')
                        .action(clap::ArgAction::SetTrue)
                        .help("Display diffs instead of rewriting files"),
                ),
        )
        .subcommand(
            Command::new("goimports")
                .about("Format Go code and manage imports")
                .arg(
                    Arg::new("path")
                        .value_name("PATH")
                        .help("Path to format (defaults to current directory)")
                        .default_value("."),
                )
                .arg(
                    Arg::new("write")
                        .long("write")
                        .short('w')
                        .action(clap::ArgAction::SetTrue)
                        .help("Write result to file instead of stdout"),
                ),
        )
        .subcommand(
            Command::new("buf-format")
                .about("Format protocol buffer files (alias to buf format)")
                .arg(
                    Arg::new("path")
                        .value_name("PATH")
                        .help("Path to format (defaults to current directory)")
                        .default_value("."),
                )
                .arg(
                    Arg::new("write")
                        .long("write")
                        .short('w')
                        .action(clap::ArgAction::SetTrue)
                        .help("Write formatted output to files"),
                ),
        )
        .subcommand(
            Command::new("shfmt")
                .about("Format shell scripts")
                .arg(
                    Arg::new("path")
                        .value_name("PATH")
                        .help("Path to shell scripts")
                        .required(true),
                )
                .arg(
                    Arg::new("write")
                        .long("write")
                        .short('w')
                        .action(clap::ArgAction::SetTrue)
                        .help("Write result to file instead of stdout"),
                )
                .arg(
                    Arg::new("indent")
                        .long("indent")
                        .short('i')
                        .value_name("SIZE")
                        .help("Indent size")
                        .default_value("2"),
                ),
        )
        .subcommand(
            Command::new("clang-format")
                .about("Format C/C++ code")
                .arg(
                    Arg::new("path")
                        .value_name("PATH")
                        .help("Path to format")
                        .required(true),
                )
                .arg(
                    Arg::new("in-place")
                        .long("in-place")
                        .short('i')
                        .action(clap::ArgAction::SetTrue)
                        .help("Format files in place"),
                )
                .arg(
                    Arg::new("style")
                        .long("style")
                        .value_name("STYLE")
                        .help("Coding style (LLVM, Google, Chromium, Mozilla, WebKit)")
                        .default_value("Google"),
                ),
        )
        .subcommand(
            Command::new("yaml-format")
                .about("Format YAML files")
                .arg(
                    Arg::new("path")
                        .value_name("PATH")
                        .help("Path to YAML files")
                        .default_value("."),
                )
                .arg(
                    Arg::new("indent")
                        .long("indent")
                        .short('i')
                        .value_name("SIZE")
                        .help("Indent size")
                        .default_value("2"),
                ),
        )
        .subcommand(
            Command::new("json-format")
                .about("Format JSON files")
                .arg(
                    Arg::new("path")
                        .value_name("PATH")
                        .help("Path to JSON files")
                        .required(true),
                )
                .arg(
                    Arg::new("indent")
                        .long("indent")
                        .short('i')
                        .value_name("SIZE")
                        .help("Indent size")
                        .default_value("2"),
                ),
        )
        .subcommand(
            Command::new("all")
                .about("Run all applicable formatters")
                .arg(
                    Arg::new("path")
                        .value_name("PATH")
                        .help("Path to format (defaults to current directory)")
                        .default_value("."),
                )
                .arg(
                    Arg::new("check")
                        .long("check")
                        .action(clap::ArgAction::SetTrue)
                        .help("Check formatting without making changes"),
                ),
        )
}

/// Execute prettier/formatter commands
pub async fn execute(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    match matches.subcommand() {
        Some(("prettier", sub_matches)) => execute_prettier(sub_matches, executor).await,
        Some(("black", sub_matches)) => execute_black(sub_matches, executor).await,
        Some(("isort", sub_matches)) => execute_isort(sub_matches, executor).await,
        Some(("rustfmt", sub_matches)) => execute_rustfmt(sub_matches, executor).await,
        Some(("gofmt", sub_matches)) => execute_gofmt(sub_matches, executor).await,
        Some(("goimports", sub_matches)) => execute_goimports(sub_matches, executor).await,
        Some(("buf-format", sub_matches)) => execute_buf_format(sub_matches, executor).await,
        Some(("shfmt", sub_matches)) => execute_shfmt(sub_matches, executor).await,
        Some(("clang-format", sub_matches)) => execute_clang_format(sub_matches, executor).await,
        Some(("yaml-format", sub_matches)) => execute_yaml_format(sub_matches, executor).await,
        Some(("json-format", sub_matches)) => execute_json_format(sub_matches, executor).await,
        Some(("all", sub_matches)) => execute_all_formatters(sub_matches, executor).await,
        _ => {
            println!(
                "No prettier subcommand specified. Use 'prettier --help' for usage information."
            );
            Ok(())
        }
    }
}

async fn execute_prettier(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let path = matches.get_one::<String>("path").unwrap();
    let mut args = vec!["prettier"];

    if matches.get_flag("write") {
        args.push("--write");
    }

    if matches.get_flag("check") {
        args.push("--check");
    }

    if let Some(config) = matches.get_one::<String>("config") {
        args.extend(&["--config", config]);
    }

    args.push(path);

    info!("Running Prettier on: {}", path);
    match args.first() {
        Some(cmd) => executor.execute_secure(cmd, &args[1..]).await,
        None => anyhow::bail!("No command specified")
    }
}

async fn execute_black(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let path = matches.get_one::<String>("path").unwrap();
    let line_length = matches.get_one::<String>("line-length").unwrap();
    let mut args = vec!["black", "--line-length", line_length];

    if matches.get_flag("check") {
        args.push("--check");
    }

    args.push(path);

    info!("Running Black on: {}", path);
    match args.first() {

        Some(cmd) => executor.execute_secure(cmd, &args[1..]).await,

        None => anyhow::bail!("No command specified")

    }
}

async fn execute_isort(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let path = matches.get_one::<String>("path").unwrap();
    let mut args = vec!["isort"];

    if matches.get_flag("check") {
        args.push("--check-only");
    }

    if matches.get_flag("diff") {
        args.push("--diff");
    }

    args.push(path);

    info!("Running isort on: {}", path);
    match args.first() {

        Some(cmd) => executor.execute_secure(cmd, &args[1..]).await,

        None => anyhow::bail!("No command specified")

    }
}

async fn execute_rustfmt(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let mut args = vec!["cargo", "fmt"];

    if matches.get_flag("check") {
        args.push("--check");
    }

    info!("Running rustfmt");
    match args.first() {

        Some(cmd) => executor.execute_secure(cmd, &args[1..]).await,

        None => anyhow::bail!("No command specified")

    }
}

async fn execute_gofmt(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let path = matches.get_one::<String>("path").unwrap();
    let mut args = vec!["gofmt"];

    if matches.get_flag("write") {
        args.push("-w");
    }

    if matches.get_flag("diff") {
        args.push("-d");
    }

    args.push(path);

    info!("Running gofmt on: {}", path);
    match args.first() {

        Some(cmd) => executor.execute_secure(cmd, &args[1..]).await,

        None => anyhow::bail!("No command specified")

    }
}

async fn execute_goimports(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let path = matches.get_one::<String>("path").unwrap();
    let mut args = vec!["goimports"];

    if matches.get_flag("write") {
        args.push("-w");
    }

    args.push(path);

    info!("Running goimports on: {}", path);
    match args.first() {

        Some(cmd) => executor.execute_secure(cmd, &args[1..]).await,

        None => anyhow::bail!("No command specified")

    }
}

async fn execute_buf_format(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let path = matches.get_one::<String>("path").unwrap();
    let mut args = vec!["buf", "format"];

    if matches.get_flag("write") {
        args.push("--write");
    }

    args.push(path);

    info!("Running buf format on: {}", path);
    match args.first() {

        Some(cmd) => executor.execute_secure(cmd, &args[1..]).await,

        None => anyhow::bail!("No command specified")

    }
}

async fn execute_shfmt(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let path = matches.get_one::<String>("path").unwrap();
    let indent = matches.get_one::<String>("indent").unwrap();
    let mut args = vec!["shfmt", "-i", indent];

    if matches.get_flag("write") {
        args.push("-w");
    }

    args.push(path);

    info!("Running shfmt on: {}", path);
    match args.first() {

        Some(cmd) => executor.execute_secure(cmd, &args[1..]).await,

        None => anyhow::bail!("No command specified")

    }
}

async fn execute_clang_format(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let path = matches.get_one::<String>("path").unwrap();
    let style = matches.get_one::<String>("style").unwrap();
    let mut args = vec!["clang-format", "--style", style];

    if matches.get_flag("in-place") {
        args.push("-i");
    }

    args.push(path);

    info!("Running clang-format on: {}", path);
    match args.first() {

        Some(cmd) => executor.execute_secure(cmd, &args[1..]).await,

        None => anyhow::bail!("No command specified")

    }
}

async fn execute_yaml_format(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let path = matches.get_one::<String>("path").unwrap();
    let indent = matches.get_one::<String>("indent").unwrap();

    // Using yq for YAML formatting
    let args = vec!["yq", "eval", ".", "--indent", indent, path];

    info!("Running YAML format on: {}", path);
    match args.first() {

        Some(cmd) => executor.execute_secure(cmd, &args[1..]).await,

        None => anyhow::bail!("No command specified")

    }
}

async fn execute_json_format(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let path = matches.get_one::<String>("path").unwrap();
    let indent = matches.get_one::<String>("indent").unwrap();

    // Using jq for JSON formatting
    let args = vec!["jq", "--indent", indent, ".", path];

    info!("Running JSON format on: {}", path);
    match args.first() {

        Some(cmd) => executor.execute_secure(cmd, &args[1..]).await,

        None => anyhow::bail!("No command specified")

    }
}

async fn execute_all_formatters(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let path = matches.get_one::<String>("path").unwrap();
    let check = matches.get_flag("check");

    info!("Running all applicable formatters on: {}", path);

    // Run formatters based on detected file types
    let formatters = vec![
        (
            "prettier",
            if check {
                vec!["prettier", ".", "--check"]
            } else {
                vec!["prettier", ".", "--write"]
            },
        ),
        (
            "black",
            if check {
                vec!["black", ".", "--check"]
            } else {
                vec!["black", "."]
            },
        ),
        (
            "isort",
            if check {
                vec!["isort", ".", "--check-only"]
            } else {
                vec!["isort", "."]
            },
        ),
        (
            "rustfmt",
            if check {
                vec!["cargo", "fmt", "--", "--check"]
            } else {
                vec!["cargo", "fmt"]
            },
        ),
        ("gofmt", vec!["gofmt", "-w", "."]),
        (
            "buf format",
            if check {
                vec!["buf", "format", "."]
            } else {
                vec!["buf", "format", "--write", "."]
            },
        ),
    ];

    for (name, args) in formatters {
        info!("Running {}", name);
        match match args.first() {
     Some(cmd) => executor.execute_secure(cmd, &args[1..]).await,
     None => anyhow::bail!("No command specified")
 } {
            Ok(_) => info!("{}: ✅ Formatted", name),
            Err(e) => {
                debug!("{}: ❌ Failed: {}", name, e);
                // Continue with other formatters even if one fails
            }
        }
    }

    Ok(())
}
