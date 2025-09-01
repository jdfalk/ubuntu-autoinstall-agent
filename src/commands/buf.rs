// file: src/commands/buf.rs
// version: 1.1.0
// guid: 7e8f9a0b-1c2d-3e4f-5a6b-7c8d9e0f1a2b

use crate::executor::Executor;
use anyhow::Result;
use clap::{Arg, ArgMatches, Command};
use std::env;
use tracing::info;

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

/// Build the buf command with comprehensive subcommands
pub fn build_command() -> Command {
    Command::new("buf")
        .about("Protocol buffer operations with buf")
        .subcommand(
            Command::new("generate")
                .about("Generate code from protocol buffers")
                .arg(
                    Arg::new("module")
                        .long("module")
                        .short('m')
                        .value_name("MODULE")
                        .help("Generate for specific module"),
                )
                .arg(
                    Arg::new("path")
                        .long("path")
                        .short('p')
                        .value_name("PATH")
                        .help("Path to protocol buffer files"),
                )
                .arg(
                    Arg::new("output")
                        .long("output")
                        .short('o')
                        .value_name("OUTPUT_DIR")
                        .help("Output directory for generated code"),
                ),
        )
        .subcommand(
            Command::new("lint")
                .about("Lint protocol buffer files")
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
            Command::new("format")
                .about("Format protocol buffer files")
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
            Command::new("breaking")
                .about("Check for breaking changes")
                .arg(
                    Arg::new("against")
                        .long("against")
                        .value_name("REF")
                        .help("Git reference to compare against")
                        .default_value("main"),
                ),
        )
        .subcommand(
            Command::new("build")
                .about("Build protocol buffer modules")
                .arg(
                    Arg::new("path")
                        .value_name("PATH")
                        .help("Path to build (defaults to current directory)")
                        .default_value("."),
                ),
        )
        .subcommand(
            Command::new("push")
                .about("Push to Buf Schema Registry")
                .arg(
                    Arg::new("tag")
                        .long("tag")
                        .short('t')
                        .value_name("TAG")
                        .help("Tag for the push"),
                ),
        )
}

/// Execute buf commands
pub async fn execute(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    match matches.subcommand() {
        Some(("generate", sub_matches)) => execute_generate(sub_matches, executor).await,
        Some(("lint", sub_matches)) => execute_lint(sub_matches, executor).await,
        Some(("format", sub_matches)) => execute_format(sub_matches, executor).await,
        Some(("breaking", sub_matches)) => execute_breaking(sub_matches, executor).await,
        Some(("build", sub_matches)) => execute_build(sub_matches, executor).await,
        Some(("push", sub_matches)) => execute_push(sub_matches, executor).await,
        _ => {
            println!("No buf subcommand specified. Use 'buf --help' for usage information.");
            Ok(())
        }
    }
}

async fn execute_generate(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let mut args = vec!["buf".to_string(), "generate".to_string()];

    if let Some(module) = matches.get_one::<String>("module") {
        args.push("--path".to_string());
        args.push(format!("pkg/{}/proto", module));
    }

    if let Some(path) = matches.get_one::<String>("path") {
        args.push("--path".to_string());
        args.push(path.clone());
    }

    if let Some(output) = matches.get_one::<String>("output") {
        args.push("--output".to_string());
        args.push(output.clone());
    }

    // Append additional arguments from file
    args = append_additional_args(args);

    info!("Generating protocol buffers with args: {:?}", args);
    executor.execute_secure("buf", &args[1..]).await
}

async fn execute_lint(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let path = matches.get_one::<String>("path").unwrap();
    let mut args = vec!["buf".to_string(), "lint".to_string(), path.clone()];

    if let Some(config) = matches.get_one::<String>("config") {
        args.push("--config".to_string());
        args.push(config.clone());
    }

    // Append additional arguments from file
    args = append_additional_args(args);

    info!("Linting protocol buffers at path: {}", path);
    executor.execute_secure("buf", &args[1..]).await
}

async fn execute_format(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let path = matches.get_one::<String>("path").unwrap();
    let mut args = vec!["buf".to_string(), "format".to_string(), path.clone()];

    if matches.get_flag("write") {
        args.push("-w".to_string());
    }

    if let Some(config) = matches.get_one::<String>("config") {
        args.push("--config".to_string());
        args.push(config.clone());
    }

    info!("Formatting protocol buffers at path: {}", path);
    executor.execute_secure("buf", &args[1..]).await
}

async fn execute_breaking(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let against = matches.get_one::<String>("against").unwrap();
    let args = vec![
        "buf".to_string(),
        "breaking".to_string(),
        "--against".to_string(),
        against.clone(),
    ];

    info!("Checking for breaking changes against: {}", against);
    executor.execute_secure("buf", &args[1..]).await
}

async fn execute_build(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let path = matches.get_one::<String>("path").unwrap();
    let args = vec!["buf".to_string(), "build".to_string(), path.clone()];

    info!("Building protocol buffers at path: {}", path);
    executor.execute_secure("buf", &args[1..]).await
}

async fn execute_push(matches: &ArgMatches, executor: &Executor) -> Result<()> {
    let mut args = vec!["buf".to_string(), "push".to_string()];

    if let Some(tag) = matches.get_one::<String>("tag") {
        args.push("--tag".to_string());
        args.push(tag.clone());
    }

    info!("Pushing to Buf Schema Registry");
    executor.execute_secure("buf", &args[1..]).await
}
