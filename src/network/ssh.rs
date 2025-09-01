// file: src/network/ssh.rs
// version: 1.0.0
// guid: t0u1v2w3-x4y5-6789-0123-456789tuvwxy

//! SSH client for remote deployment operations

use std::net::TcpStream;
use ssh2::Session;
use crate::Result;
use tracing::{info, debug, error};

/// SSH client for remote operations
pub struct SshClient {
    session: Option<Session>,
    host: String,
}

impl SshClient {
    /// Create a new SSH client
    pub fn new() -> Self {
        Self {
            session: None,
            host: String::new(),
        }
    }

    /// Connect to remote host via SSH
    pub async fn connect(&mut self, host: &str, username: &str) -> Result<()> {
        info!("Connecting to {} as {}", host, username);

        let tcp = TcpStream::connect(format!("{}:22", host))
            .map_err(|e| crate::error::AutoInstallError::SshError(
                format!("Failed to connect to {}: {}", host, e)
            ))?;

        let mut session = Session::new()
            .map_err(|e| crate::error::AutoInstallError::SshError(
                format!("Failed to create SSH session: {}", e)
            ))?;

        session.set_tcp_stream(tcp);
        session.handshake()
            .map_err(|e| crate::error::AutoInstallError::SshError(
                format!("SSH handshake failed: {}", e)
            ))?;

        // Try key-based authentication first
        if let Err(_) = session.userauth_agent(username) {
            // Fall back to asking for password (in a real implementation)
            return Err(crate::error::AutoInstallError::SshError(
                "SSH authentication failed - no valid key found".to_string()
            ));
        }

        if !session.authenticated() {
            return Err(crate::error::AutoInstallError::SshError(
                "SSH authentication failed".to_string()
            ));
        }

        self.session = Some(session);
        self.host = host.to_string();

        info!("SSH connection established to {}", host);
        Ok(())
    }

    /// Execute command on remote host
    pub async fn execute(&mut self, command: &str) -> Result<()> {
        debug!("Executing command: {}", command);

        let session = self.session.as_mut()
            .ok_or_else(|| crate::error::AutoInstallError::SshError(
                "No active SSH session".to_string()
            ))?;

        let mut channel = session.channel_session()
            .map_err(|e| crate::error::AutoInstallError::SshError(
                format!("Failed to create SSH channel: {}", e)
            ))?;

        channel.exec(command)
            .map_err(|e| crate::error::AutoInstallError::SshError(
                format!("Failed to execute command: {}", e)
            ))?;

        let mut output = String::new();
        channel.read_to_string(&mut output)
            .map_err(|e| crate::error::AutoInstallError::SshError(
                format!("Failed to read command output: {}", e)
            ))?;

        channel.wait_close()
            .map_err(|e| crate::error::AutoInstallError::SshError(
                format!("Failed to close SSH channel: {}", e)
            ))?;

        let exit_status = channel.exit_status()
            .map_err(|e| crate::error::AutoInstallError::SshError(
                format!("Failed to get exit status: {}", e)
            ))?;

        if exit_status != 0 {
            error!("Command failed with exit code {}: {}", exit_status, output);
            return Err(crate::error::AutoInstallError::SshError(
                format!("Command failed with exit code {}: {}", exit_status, output)
            ));
        }

        debug!("Command executed successfully");
        Ok(())
    }

    /// Execute command and return output
    pub async fn execute_with_output(&mut self, command: &str) -> Result<String> {
        debug!("Executing command with output: {}", command);

        let session = self.session.as_mut()
            .ok_or_else(|| crate::error::AutoInstallError::SshError(
                "No active SSH session".to_string()
            ))?;

        let mut channel = session.channel_session()
            .map_err(|e| crate::error::AutoInstallError::SshError(
                format!("Failed to create SSH channel: {}", e)
            ))?;

        channel.exec(command)
            .map_err(|e| crate::error::AutoInstallError::SshError(
                format!("Failed to execute command: {}", e)
            ))?;

        let mut output = String::new();
        channel.read_to_string(&mut output)
            .map_err(|e| crate::error::AutoInstallError::SshError(
                format!("Failed to read command output: {}", e)
            ))?;

        channel.wait_close()
            .map_err(|e| crate::error::AutoInstallError::SshError(
                format!("Failed to close SSH channel: {}", e)
            ))?;

        let exit_status = channel.exit_status()
            .map_err(|e| crate::error::AutoInstallError::SshError(
                format!("Failed to get exit status: {}", e)
            ))?;

        if exit_status != 0 {
            error!("Command failed with exit code {}: {}", exit_status, output);
            return Err(crate::error::AutoInstallError::SshError(
                format!("Command failed with exit code {}: {}", exit_status, output)
            ));
        }

        debug!("Command executed successfully, output length: {}", output.len());
        Ok(output)
    }

    /// Upload file to remote host
    pub async fn upload_file(&mut self, local_path: &str, remote_path: &str) -> Result<()> {
        info!("Uploading {} to {}:{}", local_path, self.host, remote_path);

        let session = self.session.as_mut()
            .ok_or_else(|| crate::error::AutoInstallError::SshError(
                "No active SSH session".to_string()
            ))?;

        // Get file size
        let metadata = std::fs::metadata(local_path)
            .map_err(|e| crate::error::AutoInstallError::IoError(e))?;

        let file_size = metadata.len();

        // Create SCP channel
        let mut remote_file = session.scp_send(
            std::path::Path::new(remote_path),
            0o644,
            file_size,
            None,
        ).map_err(|e| crate::error::AutoInstallError::SshError(
            format!("Failed to create SCP channel: {}", e)
        ))?;

        // Read and send file
        let file_content = std::fs::read(local_path)
            .map_err(|e| crate::error::AutoInstallError::IoError(e))?;

        remote_file.write_all(&file_content)
            .map_err(|e| crate::error::AutoInstallError::SshError(
                format!("Failed to write file data: {}", e)
            ))?;

        remote_file.send_eof()
            .map_err(|e| crate::error::AutoInstallError::SshError(
                format!("Failed to send EOF: {}", e)
            ))?;

        remote_file.wait_eof()
            .map_err(|e| crate::error::AutoInstallError::SshError(
                format!("Failed to wait for EOF: {}", e)
            ))?;

        remote_file.close()
            .map_err(|e| crate::error::AutoInstallError::SshError(
                format!("Failed to close remote file: {}", e)
            ))?;

        remote_file.wait_close()
            .map_err(|e| crate::error::AutoInstallError::SshError(
                format!("Failed to wait for close: {}", e)
            ))?;

        info!("File upload completed");
        Ok(())
    }

    /// Download file from remote host
    pub async fn download_file(&mut self, remote_path: &str, local_path: &str) -> Result<()> {
        info!("Downloading {}:{} to {}", self.host, remote_path, local_path);

        let session = self.session.as_mut()
            .ok_or_else(|| crate::error::AutoInstallError::SshError(
                "No active SSH session".to_string()
            ))?;

        let (mut remote_file, stat) = session.scp_recv(std::path::Path::new(remote_path))
            .map_err(|e| crate::error::AutoInstallError::SshError(
                format!("Failed to create SCP receive channel: {}", e)
            ))?;

        let mut contents = Vec::new();
        remote_file.read_to_end(&mut contents)
            .map_err(|e| crate::error::AutoInstallError::SshError(
                format!("Failed to read remote file: {}", e)
            ))?;

        // Verify file size
        if contents.len() != stat.size() as usize {
            return Err(crate::error::AutoInstallError::SshError(
                "File size mismatch during download".to_string()
            ));
        }

        std::fs::write(local_path, contents)
            .map_err(|e| crate::error::AutoInstallError::IoError(e))?;

        remote_file.send_eof()
            .map_err(|e| crate::error::AutoInstallError::SshError(
                format!("Failed to send EOF: {}", e)
            ))?;

        remote_file.wait_eof()
            .map_err(|e| crate::error::AutoInstallError::SshError(
                format!("Failed to wait for EOF: {}", e)
            ))?;

        remote_file.close()
            .map_err(|e| crate::error::AutoInstallError::SshError(
                format!("Failed to close remote file: {}", e)
            ))?;

        remote_file.wait_close()
            .map_err(|e| crate::error::AutoInstallError::SshError(
                format!("Failed to wait for close: {}", e)
            ))?;

        info!("File download completed");
        Ok(())
    }

    /// Disconnect SSH session
    pub fn disconnect(&mut self) {
        if let Some(session) = self.session.take() {
            let _ = session.disconnect(None, "", None);
            info!("SSH session disconnected");
        }
    }
}

impl Drop for SshClient {
    fn drop(&mut self) {
        self.disconnect();
    }
}

use std::io::{Read, Write};