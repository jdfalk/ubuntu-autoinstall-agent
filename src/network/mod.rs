// file: src/network/mod.rs
// version: 1.0.0
// guid: s9t0u1v2-w3x4-5678-9012-345678stuvwx

//! Network operations module

pub mod download;
pub mod ssh;

pub use download::NetworkDownloader;
pub use ssh::SshClient;