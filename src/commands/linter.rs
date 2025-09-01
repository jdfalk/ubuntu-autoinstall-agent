// file: src/commands/linter.rs
// version: 1.0.0
// guid: fa968456-1f5c-4092-a80d-58124e3660ee

use crate::executor::Executor;
use anyhow::Result;
use clap::{Arg, ArgMatches, Command};
use std::env;
use tracing::{debug, info};

/// Helper function to append additional arguments from environment variable
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

/// Build the linter command with various linting tools
pub fn build_command() -> Command {
    Command::new("linter")
        .about("Code linting and analysis tools")
        .subcommand(
            Command::new("buf")
                .about("Lint protocol buffer files (alias to buf lint)")
                .arg(
                    Arg::new("path")
                        .value_name("PATH")
                        .help("Path to lint (defaults to current directory)")
                        .default_value("."),
                )
                .arg(
                    Arg::new("config")
                        .long("config")
                        .short('c')
                        .value_name("CONFIG_FILE")
                        .help("Path to buf configuration file"),
                ),
        )
        .subcommand(
            Command::new("eslint")
                .about("Lint JavaScript/TypeScript files with ESLint")
                .arg(
                    Arg::new("path")
                        .value_name("PATH")
                        .help("Path to lint (defaults to current directory)")
                        .default_value("."),
                )
                .arg(
                    Arg::new("fix")
                        .long("fix")
                        .action(clap::ArgAction::SetTrue)
                        .help("Automatically fix problems"),
                )
                .arg(
                    Arg::new("config")
                        .long("config")
                        .short('c')
                        .value_name("CONFIG_FILE")
                        .help("Path to ESLint configuration file"),
                ),
        )
        .subcommand(
            Command::new("flake8")
                .about("Lint Python files with flake8")
                .arg(
                    Arg::new("path")
                        .value_name("PATH")
                        .help("Path to lint (defaults to current directory)")
                        .default_value("."),
                )
                .arg(
                    Arg::new("max-line-length")
                        .long("max-line-length")
                        .value_name("LENGTH")
                        .help("Maximum line length")
                        .default_value("88"),
                ),
        )
        .subcommand(
            Command::new("mypy")
                .about("Type check Python files with mypy")
                .arg(
                    Arg::new("path")
                        .value_name("PATH")
                        .help("Path to type check (defaults to current directory)")
                        .default_value("."),
                )
                .arg(
                    Arg::new("strict")
                        .long("strict")
                        .action(clap::ArgAction::SetTrue)
                        .help("Enable strict mode"),
                ),
        )
        .subcommand(
            Command::new("clippy")
                .about("Lint Rust code with Clippy")
                .arg(
                    Arg::new("all-targets")
                        .long("all-targets")
                        .action(clap::ArgAction::SetTrue)
                        .help("Check all targets"),
                )
                .arg(
                    Arg::new("all-features")
                        .long("all-features")
                        .action(clap::ArgAction::SetTrue)
                        .help("Check with all features enabled"),
                ),
        )
        .subcommand(
            Command::new("golangci-lint")
                .about("Lint Go code with golangci-lint")
                .arg(
                    Arg::new("path")
                        .value_name("PATH")
                        .help("Path to lint (defaults to current directory)")
                        .default_value("."),
                )
                .arg(
                    Arg::new("fix")
                        .long("fix")
                        .action(clap::ArgAction::SetTrue)
                        .help("Fix issues automatically"),
                ),
        )
        .subcommand(
            Command::new("shellcheck")
                .about("Lint shell scripts with ShellCheck")
                .arg(
                    Arg::new("path")
                        .value_name("PATH")
                        .help("Path to shell scripts")
                        .required(true),
                )
                .arg(
                    Arg::new("format")
                        .long("format")
                        .short('f')
                        .value_name("FORMAT")
                        .help("Output format (checkstyle, diff, gcc, json, json1, quiet, tty)")
                        .default_value("tty"),
                ),
        )
        .subcommand(
            Command::new("hadolint")
                .about("Lint Dockerfiles with Hadolint")
                .arg(
                    Arg::new("dockerfile")
                        .value_name("DOCKERFILE")
                        .help("Path to Dockerfile")
                        .default_value("Dockerfile"),
                ),
        )
        .subcommand(
            Command::new("yamllint")
                .about("Lint YAML files")
                .arg(
                    Arg::new("path")
                        .value_name("PATH")
                        .help("Path to YAML files")
                        .default_value("."),
                )
                .arg(
                    Arg::new("strict")
                        .long("strict")
                        .action(clap::ArgAction::SetTrue)
                        .help("Return non-zero exit code on warnings"),
                ),
        )
        .subcommand(
            Command::new("markdownlint")
                .about("Lint Markdown files")
                .arg(
                    Arg::new("path")
                        .value_name("PATH")
                        .help("Path to Markdown files")
                        .default_value("."),
                )
                .arg(
                    Arg::new("fix")
                        .long("fix")
                        .action(clap::ArgAction::SetTrue)
                        .help("Fix issues automatically"),
                ),
        )
        .subcommand(
            Command::new("all")
                .about("Run all applicable linters")
                .arg(
                    Arg::new("path")
                        .value_name("PATH")
                        .help("Path to lint (defaults to current directory)")
                        .default_value("."),
                )
                .arg(
                    Arg::new("fix")
                        .long("fix")
                        .action(clap::ArgAction::SetTrue)
                        .help("Fix issues automatically where possible"),
                ),
        )
}

/// Execute linter commands
pub async fn execute(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    match matches.subcommand() {
        Some(("buf", sub_matches)) => execute_buf_lint(sub_matches, executor).await,
        Some(("eslint", sub_matches)) => execute_eslint(sub_matches, executor).await,
        Some(("flake8", sub_matches)) => execute_flake8(sub_matches, executor).await,
        Some(("mypy", sub_matches)) => execute_mypy(sub_matches, executor).await,
        Some(("clippy", sub_matches)) => execute_clippy(sub_matches, executor).await,
        Some(("golangci-lint", sub_matches)) => execute_golangci_lint(sub_matches, executor).await,
        Some(("shellcheck", sub_matches)) => execute_shellcheck(sub_matches, executor).await,
        Some(("hadolint", sub_matches)) => execute_hadolint(sub_matches, executor).await,
        Some(("yamllint", sub_matches)) => execute_yamllint(sub_matches, executor).await,
        Some(("markdownlint", sub_matches)) => execute_markdownlint(sub_matches, executor).await,
        Some(("all", sub_matches)) => execute_all_linters(sub_matches, executor).await,
        _ => {
            println!("No linter subcommand specified. Use 'linter --help' for usage information.");
            Ok(())
        }
    }
}

async fn execute_buf_lint(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let path = matches.get_one::<String>("path").unwrap();
    let mut args = vec!["buf".to_string(), "lint".to_string(), path.to_string()];

    if let Some(config) = matches.get_one::<String>("config") {
        args.push("--config".to_string());
        args.push(config.to_string());
    }

    // Append additional arguments from environment variable
    args = append_additional_args(args);

    info!("Running buf lint on: {}", path);
    match args.first() {

        Some(cmd) => executor.execute_secure(cmd, &args[1..]).await,

        None => anyhow::bail!("No command specified")

    }
}

async fn execute_eslint(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let path = matches.get_one::<String>("path").unwrap();
    let mut args = vec!["eslint".to_string(), path.to_string()];

    if matches.get_flag("fix") {
        args.push("--fix".to_string());
    }

    if let Some(config) = matches.get_one::<String>("config") {
        args.push("--config".to_string());
        args.push(config.to_string());
    }

    // Append additional arguments from environment variable
    args = append_additional_args(args);

    info!("Running ESLint on: {}", path);
    match args.first() {

        Some(cmd) => executor.execute_secure(cmd, &args[1..]).await,

        None => anyhow::bail!("No command specified")

    }
}

async fn execute_flake8(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let path = matches.get_one::<String>("path").unwrap();
    let max_line_length = matches.get_one::<String>("max-line-length").unwrap();

    let mut args = vec!["flake8".to_string(), "--max-line-length".to_string(), max_line_length.to_string(), path.to_string()];

    // Append additional arguments from environment variable
    args = append_additional_args(args);

    info!("Running flake8 on: {}", path);
    match args.first() {

        Some(cmd) => executor.execute_secure(cmd, &args[1..]).await,

        None => anyhow::bail!("No command specified")

    }
}

async fn execute_mypy(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let path = matches.get_one::<String>("path").unwrap();
    let mut args = vec!["mypy".to_string(), path.to_string()];

    if matches.get_flag("strict") {
        args.push("--strict".to_string());
    }

    // Append additional arguments from environment variable
    args = append_additional_args(args);

    info!("Running mypy on: {}", path);
    match args.first() {

        Some(cmd) => executor.execute_secure(cmd, &args[1..]).await,

        None => anyhow::bail!("No command specified")

    }
}

async fn execute_clippy(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let mut args = vec!["cargo".to_string(), "clippy".to_string()];

    if matches.get_flag("all-targets") {
        args.push("--all-targets".to_string());
    }

    if matches.get_flag("all-features") {
        args.push("--all-features".to_string());
    }

    args.extend(vec!["--".to_string(), "-D".to_string(), "warnings".to_string()]);

    // Append additional arguments from environment variable
    args = append_additional_args(args);

    info!("Running cargo clippy");
    match args.first() {

        Some(cmd) => executor.execute_secure(cmd, &args[1..]).await,

        None => anyhow::bail!("No command specified")

    }
}

async fn execute_golangci_lint(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let path = matches.get_one::<String>("path").unwrap();
    let mut args = vec!["golangci-lint".to_string(), "run".to_string(), path.to_string()];

    if matches.get_flag("fix") {
        args.push("--fix".to_string());
    }

    // Append additional arguments from environment variable
    args = append_additional_args(args);

    info!("Running golangci-lint on: {}", path);
    match args.first() {

        Some(cmd) => executor.execute_secure(cmd, &args[1..]).await,

        None => anyhow::bail!("No command specified")

    }
}

async fn execute_shellcheck(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let path = matches.get_one::<String>("path").unwrap();
    let format = matches.get_one::<String>("format").unwrap();

    let mut args = vec!["shellcheck".to_string(), "--format".to_string(), format.to_string(), path.to_string()];

    // Append additional arguments from environment variable
    args = append_additional_args(args);

    info!("Running ShellCheck on: {}", path);
    match args.first() {

        Some(cmd) => executor.execute_secure(cmd, &args[1..]).await,

        None => anyhow::bail!("No command specified")

    }
}

async fn execute_hadolint(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let dockerfile = matches.get_one::<String>("dockerfile").unwrap();
    let mut args = vec!["hadolint".to_string(), dockerfile.to_string()];

    // Append additional arguments from environment variable
    args = append_additional_args(args);

    info!("Running Hadolint on: {}", dockerfile);
    match args.first() {

        Some(cmd) => executor.execute_secure(cmd, &args[1..]).await,

        None => anyhow::bail!("No command specified")

    }
}

async fn execute_yamllint(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let path = matches.get_one::<String>("path").unwrap();
    let mut args = vec!["yamllint".to_string(), path.to_string()];

    if matches.get_flag("strict") {
        args.push("--strict".to_string());
    }

    // Append additional arguments from environment variable
    args = append_additional_args(args);

    info!("Running yamllint on: {}", path);
    match args.first() {

        Some(cmd) => executor.execute_secure(cmd, &args[1..]).await,

        None => anyhow::bail!("No command specified")

    }
}

async fn execute_markdownlint(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let path = matches.get_one::<String>("path").unwrap();
    let mut args = vec!["markdownlint".to_string(), path.to_string()];

    if matches.get_flag("fix") {
        args.push("--fix".to_string());
    }

    // Append additional arguments from environment variable
    args = append_additional_args(args);

    info!("Running markdownlint on: {}", path);
    match args.first() {

        Some(cmd) => executor.execute_secure(cmd, &args[1..]).await,

        None => anyhow::bail!("No command specified")

    }
}

async fn execute_all_linters(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let path = matches.get_one::<String>("path").unwrap();
    let fix = matches.get_flag("fix");

    info!("Running all applicable linters on: {}", path);

    // Check for different file types and run appropriate linters
    let linters = vec![
        ("buf", vec!["buf", "lint", "."]),
        (
            "eslint",
            if fix {
                vec!["eslint", ".", "--fix"]
            } else {
                vec!["eslint", "."]
            },
        ),
        ("flake8", vec!["flake8", "."]),
        (
            "clippy",
            vec!["cargo", "clippy", "--all-targets", "--", "-D", "warnings"],
        ),
        (
            "golangci-lint",
            if fix {
                vec!["golangci-lint", "run", ".", "--fix"]
            } else {
                vec!["golangci-lint", "run", "."]
            },
        ),
        ("yamllint", vec!["yamllint", "."]),
        (
            "markdownlint",
            if fix {
                vec!["markdownlint", ".", "--fix"]
            } else {
                vec!["markdownlint", "."]
            },
        ),
    ];

    for (name, args) in linters {
        info!("Running {}", name);
        match match args.first() {
     Some(cmd) => executor.execute_secure(cmd, &args[1..]).await,
     None => anyhow::bail!("No command specified")
 } {
            Ok(_) => info!("{}: ✅ Passed", name),
            Err(e) => {
                debug!("{}: ❌ Failed: {}", name, e);
                // Continue with other linters even if one fails
            }
        }
    }

    Ok(())
}
