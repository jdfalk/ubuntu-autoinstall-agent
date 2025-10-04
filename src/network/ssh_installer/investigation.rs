// file: src/network/ssh_installer/investigation.rs
// version: 1.2.0
// guid: sshinv01-2345-6789-abcd-ef0123456789

//! System investigation capabilities for SSH installation

use super::config::SystemInfo;
use crate::Result;
use tracing::{info, warn};

pub struct SystemInvestigator<'a, T> {
    executor: &'a mut T,
}

impl<'a, T> SystemInvestigator<'a, T>
where
    T: crate::network::CommandExecutor,
{
    pub fn new(executor: &'a mut T) -> Self {
        Self { executor }
    }
}
