// file: src/cli/args.rs
// version: 1.0.0
// guid: f6g7h8i9-j0k1-2345-6789-012345fghijk

//! Command line argument definitions

use clap::{Parser, Subcommand};
use crate::config::Architecture;

#[derive(Parser)]
#[command(name = "ubuntu-autoinstall-agent")]
#[command(about = "Automated Ubuntu server deployment with golden images")]
#[command(version = env!("CARGO_PKG_VERSION"))]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[arg(short, long, global = true)]
    pub quiet: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create a golden Ubuntu image
    CreateImage {
        #[arg(short, long, value_enum, default_value = "amd64")]
        arch: ArchArg,

        #[arg(long, default_value = "24.04")]
        version: String,

        #[arg(short, long)]
        output: Option<String>,

        #[arg(short, long)]
        spec: Option<String>,
    },

    /// Deploy image to target machine
    Deploy {
        #[arg(short, long)]
        target: String,

        #[arg(short, long)]
        config: String,

        #[arg(short = 'i', long)]
        image: String,

        #[arg(long)]
        via_ssh: bool,

        #[arg(long)]
        dry_run: bool,
    },

    /// Validate image integrity
    Validate {
        #[arg(short, long)]
        image: String,
    },

    /// Check system prerequisites
    CheckPrereqs,

    /// List available images
    ListImages {
        #[arg(short, long)]
        filter_arch: Option<ArchArg>,

        #[arg(short, long)]
        json: bool,
    },

    /// Cleanup old images
    Cleanup {
        #[arg(long, default_value = "30")]
        older_than_days: u32,

        #[arg(long)]
        dry_run: bool,
    },
}

/// Architecture argument for CLI
#[derive(clap::ValueEnum, Clone, Debug)]
pub enum ArchArg {
    Amd64,
    Arm64,
}

impl From<ArchArg> for Architecture {
    fn from(arch: ArchArg) -> Self {
        match arch {
            ArchArg::Amd64 => Architecture::Amd64,
            ArchArg::Arm64 => Architecture::Arm64,
        }
    }
}

impl From<Architecture> for ArchArg {
    fn from(arch: Architecture) -> Self {
        match arch {
            Architecture::Amd64 => ArchArg::Amd64,
            Architecture::Arm64 => ArchArg::Arm64,
        }
    }
}
