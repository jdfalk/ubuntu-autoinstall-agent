// file: src/utils/network.rs
// version: 1.0.0
// guid: y6z7a8b9-c0d1-2345-6789-012345678901

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

/// Network utilities for the installation agent
pub struct NetworkUtils;

impl NetworkUtils {
    /// Test network connectivity to a host
    pub async fn test_connectivity(host: &str, port: u16, timeout_secs: u64) -> Result<bool> {
        debug!("Testing connectivity to {}:{}", host, port);

        let timeout_duration = Duration::from_secs(timeout_secs);

        match timeout(timeout_duration, tokio::net::TcpStream::connect((host, port))).await {
            Ok(Ok(_)) => {
                debug!("Successfully connected to {}:{}", host, port);
                Ok(true)
            }
            Ok(Err(e)) => {
                debug!("Failed to connect to {}:{}: {}", host, port, e);
                Ok(false)
            }
            Err(_) => {
                debug!("Connection to {}:{} timed out after {} seconds", host, port, timeout_secs);
                Ok(false)
            }
        }
    }

    /// Test internet connectivity by pinging common hosts
    pub async fn test_internet_connectivity() -> Result<bool> {
        let test_hosts = [
            ("8.8.8.8", 53),      // Google DNS
            ("1.1.1.1", 53),      // Cloudflare DNS
            ("208.67.222.222", 53), // OpenDNS
        ];

        for (host, port) in &test_hosts {
            if Self::test_connectivity(host, *port, 5).await? {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Get all network interfaces
    pub async fn get_network_interfaces() -> Result<Vec<NetworkInterface>> {
        let output = tokio::process::Command::new("ip")
            .args(&["-j", "addr", "show"])
            .output()
            .await
            .context("Failed to execute ip command")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "ip command failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let output_str = String::from_utf8(output.stdout)
            .context("Failed to parse ip output as UTF-8")?;

        let interfaces: Vec<IpAddrShow> = serde_json::from_str(&output_str)
            .context("Failed to parse ip JSON output")?;

        let mut result = Vec::new();
        for iface in interfaces {
            let mut addresses = Vec::new();

            if let Some(addr_info) = iface.addr_info {
                for addr in addr_info {
                    if let Ok(ip) = IpAddr::from_str(&addr.local) {
                        addresses.push(IpAddress {
                            ip,
                            prefix_len: addr.prefixlen,
                            scope: addr.scope.unwrap_or_default(),
                        });
                    }
                }
            }

            result.push(NetworkInterface {
                name: iface.ifname,
                state: iface.operstate.unwrap_or("unknown".to_string()),
                mtu: iface.mtu,
                mac_address: iface.address,
                addresses,
                flags: iface.flags.unwrap_or_default(),
            });
        }

        Ok(result)
    }

    /// Get default route information
    pub async fn get_default_route() -> Result<Option<RouteInfo>> {
        let output = tokio::process::Command::new("ip")
            .args(&["-j", "route", "show", "default"])
            .output()
            .await
            .context("Failed to execute ip route command")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "ip route command failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let output_str = String::from_utf8(output.stdout)
            .context("Failed to parse ip route output as UTF-8")?;

        if output_str.trim().is_empty() {
            return Ok(None);
        }

        let routes: Vec<IpRoute> = serde_json::from_str(&output_str)
            .context("Failed to parse ip route JSON output")?;

        if let Some(route) = routes.first() {
            Ok(Some(RouteInfo {
                destination: route.dst.clone().unwrap_or("0.0.0.0/0".to_string()),
                gateway: route.gateway.clone(),
                interface: route.dev.clone().unwrap_or_default(),
                metric: route.metric,
            }))
        } else {
            Ok(None)
        }
    }

    /// Configure network interface
    pub async fn configure_interface(config: &InterfaceConfig) -> Result<()> {
        info!("Configuring network interface: {}", config.name);

        // Bring interface down first
        tokio::process::Command::new("ip")
            .args(&["link", "set", &config.name, "down"])
            .status()
            .await
            .context("Failed to bring interface down")?;

        // Configure IP addresses
        for addr in &config.addresses {
            let addr_str = format!("{}/{}", addr.ip, addr.prefix_len);

            let status = tokio::process::Command::new("ip")
                .args(&["addr", "add", &addr_str, "dev", &config.name])
                .status()
                .await
                .context("Failed to add IP address")?;

            if !status.success() {
                warn!("Failed to add address {} to interface {}", addr_str, config.name);
            }
        }

        // Set MTU if specified
        if let Some(mtu) = config.mtu {
            tokio::process::Command::new("ip")
                .args(&["link", "set", &config.name, "mtu", &mtu.to_string()])
                .status()
                .await
                .context("Failed to set MTU")?;
        }

        // Bring interface up
        let status = tokio::process::Command::new("ip")
            .args(&["link", "set", &config.name, "up"])
            .status()
            .await
            .context("Failed to bring interface up")?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to bring interface up"));
        }

        // Configure default gateway if specified
        if let Some(gateway) = &config.gateway {
            let status = tokio::process::Command::new("ip")
                .args(&["route", "add", "default", "via", gateway, "dev", &config.name])
                .status()
                .await
                .context("Failed to add default route")?;

            if !status.success() {
                warn!("Failed to add default route via {} on {}", gateway, config.name);
            }
        }

        info!("Network interface {} configured successfully", config.name);
        Ok(())
    }

    /// Configure DNS settings
    pub async fn configure_dns(dns_servers: &[String], search_domains: &[String]) -> Result<()> {
        info!("Configuring DNS settings");

        let mut resolv_conf = String::new();

        // Add search domains
        if !search_domains.is_empty() {
            resolv_conf.push_str(&format!("search {}\n", search_domains.join(" ")));
        }

        // Add nameservers
        for server in dns_servers {
            resolv_conf.push_str(&format!("nameserver {}\n", server));
        }

        // Write to resolv.conf
        tokio::fs::write("/etc/resolv.conf", resolv_conf).await
            .context("Failed to write /etc/resolv.conf")?;

        info!("DNS configuration updated successfully");
        Ok(())
    }

    /// Test DNS resolution
    pub async fn test_dns_resolution(hostname: &str) -> Result<Vec<IpAddr>> {
        debug!("Testing DNS resolution for: {}", hostname);

        let addresses = tokio::net::lookup_host(format!("{}:80", hostname)).await
            .context("Failed to resolve hostname")?;

        let ips: Vec<IpAddr> = addresses.map(|addr| addr.ip()).collect();

        if ips.is_empty() {
            return Err(anyhow::anyhow!("No IP addresses resolved for {}", hostname));
        }

        debug!("Resolved {} to {} addresses", hostname, ips.len());
        Ok(ips)
    }

    /// Get network statistics
    pub async fn get_network_stats() -> Result<HashMap<String, NetworkStats>> {
        let proc_net_dev = tokio::fs::read_to_string("/proc/net/dev").await
            .context("Failed to read /proc/net/dev")?;

        let mut stats = HashMap::new();

        for line in proc_net_dev.lines().skip(2) { // Skip header lines
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 17 {
                continue;
            }

            let interface = fields[0].trim_end_matches(':');

            let rx_bytes = fields[1].parse::<u64>().unwrap_or(0);
            let rx_packets = fields[2].parse::<u64>().unwrap_or(0);
            let rx_errors = fields[3].parse::<u64>().unwrap_or(0);
            let rx_dropped = fields[4].parse::<u64>().unwrap_or(0);

            let tx_bytes = fields[9].parse::<u64>().unwrap_or(0);
            let tx_packets = fields[10].parse::<u64>().unwrap_or(0);
            let tx_errors = fields[11].parse::<u64>().unwrap_or(0);
            let tx_dropped = fields[12].parse::<u64>().unwrap_or(0);

            stats.insert(interface.to_string(), NetworkStats {
                rx_bytes,
                rx_packets,
                rx_errors,
                rx_dropped,
                tx_bytes,
                tx_packets,
                tx_errors,
                tx_dropped,
            });
        }

        Ok(stats)
    }

    /// Validate IP address format
    pub fn validate_ip_address(ip: &str) -> Result<IpAddr> {
        IpAddr::from_str(ip)
            .with_context(|| format!("Invalid IP address format: {}", ip))
    }

    /// Validate network CIDR format
    pub fn validate_cidr(cidr: &str) -> Result<(IpAddr, u8)> {
        let parts: Vec<&str> = cidr.split('/').collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("Invalid CIDR format: {}", cidr));
        }

        let ip = Self::validate_ip_address(parts[0])?;
        let prefix_len = parts[1].parse::<u8>()
            .with_context(|| format!("Invalid prefix length: {}", parts[1]))?;

        // Validate prefix length based on IP version
        let max_prefix = match ip {
            IpAddr::V4(_) => 32,
            IpAddr::V6(_) => 128,
        };

        if prefix_len > max_prefix {
            return Err(anyhow::anyhow!(
                "Prefix length {} exceeds maximum {} for IP version",
                prefix_len, max_prefix
            ));
        }

        Ok((ip, prefix_len))
    }

    /// Calculate network and broadcast addresses
    pub fn calculate_network_info(ip: Ipv4Addr, prefix_len: u8) -> Result<NetworkInfo> {
        if prefix_len > 32 {
            return Err(anyhow::anyhow!("Invalid prefix length for IPv4: {}", prefix_len));
        }

        let ip_u32 = u32::from(ip);
        let mask_u32 = !((1u32 << (32 - prefix_len)) - 1);
        let network_u32 = ip_u32 & mask_u32;
        let broadcast_u32 = network_u32 | !mask_u32;

        let network = Ipv4Addr::from(network_u32);
        let broadcast = Ipv4Addr::from(broadcast_u32);
        let netmask = Ipv4Addr::from(mask_u32);

        // Calculate first and last usable addresses
        let first_usable = if prefix_len == 31 || prefix_len == 32 {
            network // For /31 and /32, use network address
        } else {
            Ipv4Addr::from(network_u32 + 1)
        };

        let last_usable = if prefix_len == 31 || prefix_len == 32 {
            broadcast // For /31 and /32, use broadcast address
        } else {
            Ipv4Addr::from(broadcast_u32 - 1)
        };

        let total_addresses = if prefix_len == 32 {
            1
        } else {
            2u64.pow(32 - prefix_len as u32)
        };

        let usable_addresses = if prefix_len >= 31 {
            total_addresses
        } else {
            total_addresses - 2 // Subtract network and broadcast
        };

        Ok(NetworkInfo {
            network,
            netmask,
            broadcast,
            first_usable,
            last_usable,
            total_addresses,
            usable_addresses,
            prefix_len,
        })
    }

    /// Check if interface exists
    pub async fn interface_exists(name: &str) -> Result<bool> {
        let path = format!("/sys/class/net/{}", name);
        Ok(tokio::fs::metadata(path).await.is_ok())
    }

    /// Enable/disable interface
    pub async fn set_interface_state(name: &str, up: bool) -> Result<()> {
        let state = if up { "up" } else { "down" };

        let status = tokio::process::Command::new("ip")
            .args(&["link", "set", name, state])
            .status()
            .await
            .with_context(|| format!("Failed to set interface {} {}", name, state))?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to set interface {} {}", name, state));
        }

        Ok(())
    }

    /// Get interface MAC address
    pub async fn get_interface_mac(name: &str) -> Result<String> {
        let mac_path = format!("/sys/class/net/{}/address", name);
        let mac = tokio::fs::read_to_string(mac_path).await
            .with_context(|| format!("Failed to read MAC address for interface {}", name))?;

        Ok(mac.trim().to_string())
    }

    /// Validate MAC address format
    pub fn validate_mac_address(mac: &str) -> Result<String> {
        let mac = mac.to_lowercase();
        let parts: Vec<&str> = mac.split(':').collect();

        if parts.len() != 6 {
            return Err(anyhow::anyhow!("Invalid MAC address format: {}", mac));
        }

        for part in &parts {
            if part.len() != 2 {
                return Err(anyhow::anyhow!("Invalid MAC address format: {}", mac));
            }

            if !part.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(anyhow::anyhow!("Invalid MAC address format: {}", mac));
            }
        }

        Ok(mac)
    }
}

/// Network interface information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    pub name: String,
    pub state: String,
    pub mtu: Option<u32>,
    pub mac_address: Option<String>,
    pub addresses: Vec<IpAddress>,
    pub flags: Vec<String>,
}

/// IP address with prefix information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpAddress {
    pub ip: IpAddr,
    pub prefix_len: u8,
    pub scope: String,
}

/// Network interface configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceConfig {
    pub name: String,
    pub addresses: Vec<IpAddress>,
    pub gateway: Option<String>,
    pub mtu: Option<u32>,
    pub dns_servers: Option<Vec<String>>,
}

/// Route information
#[derive(Debug, Clone)]
pub struct RouteInfo {
    pub destination: String,
    pub gateway: Option<String>,
    pub interface: String,
    pub metric: Option<u32>,
}

/// Network statistics
#[derive(Debug, Clone)]
pub struct NetworkStats {
    pub rx_bytes: u64,
    pub rx_packets: u64,
    pub rx_errors: u64,
    pub rx_dropped: u64,
    pub tx_bytes: u64,
    pub tx_packets: u64,
    pub tx_errors: u64,
    pub tx_dropped: u64,
}

/// Network information calculated from IP and prefix
#[derive(Debug, Clone)]
pub struct NetworkInfo {
    pub network: Ipv4Addr,
    pub netmask: Ipv4Addr,
    pub broadcast: Ipv4Addr,
    pub first_usable: Ipv4Addr,
    pub last_usable: Ipv4Addr,
    pub total_addresses: u64,
    pub usable_addresses: u64,
    pub prefix_len: u8,
}

// Internal structures for parsing JSON output

#[derive(Debug, Deserialize)]
struct IpAddrShow {
    ifname: String,
    operstate: Option<String>,
    mtu: Option<u32>,
    address: Option<String>,
    addr_info: Option<Vec<AddrInfo>>,
    flags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct AddrInfo {
    family: String,
    local: String,
    prefixlen: u8,
    scope: Option<String>,
}

#[derive(Debug, Deserialize)]
struct IpRoute {
    dst: Option<String>,
    gateway: Option<String>,
    dev: Option<String>,
    metric: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_validate_ip_address() {
        assert!(NetworkUtils::validate_ip_address("192.168.1.1").is_ok());
        assert!(NetworkUtils::validate_ip_address("::1").is_ok());
        assert!(NetworkUtils::validate_ip_address("invalid").is_err());
        assert!(NetworkUtils::validate_ip_address("256.256.256.256").is_err());
    }

    #[test]
    fn test_validate_cidr() {
        assert!(NetworkUtils::validate_cidr("192.168.1.0/24").is_ok());
        assert!(NetworkUtils::validate_cidr("10.0.0.0/8").is_ok());
        assert!(NetworkUtils::validate_cidr("2001:db8::/32").is_ok());

        assert!(NetworkUtils::validate_cidr("192.168.1.0").is_err());
        assert!(NetworkUtils::validate_cidr("192.168.1.0/33").is_err());
        assert!(NetworkUtils::validate_cidr("invalid/24").is_err());
    }

    #[test]
    fn test_calculate_network_info() {
        let ip = Ipv4Addr::new(192, 168, 1, 100);
        let network_info = NetworkUtils::calculate_network_info(ip, 24).unwrap();

        assert_eq!(network_info.network, Ipv4Addr::new(192, 168, 1, 0));
        assert_eq!(network_info.broadcast, Ipv4Addr::new(192, 168, 1, 255));
        assert_eq!(network_info.netmask, Ipv4Addr::new(255, 255, 255, 0));
        assert_eq!(network_info.first_usable, Ipv4Addr::new(192, 168, 1, 1));
        assert_eq!(network_info.last_usable, Ipv4Addr::new(192, 168, 1, 254));
        assert_eq!(network_info.total_addresses, 256);
        assert_eq!(network_info.usable_addresses, 254);
        assert_eq!(network_info.prefix_len, 24);
    }

    #[test]
    fn test_calculate_network_info_point_to_point() {
        let ip = Ipv4Addr::new(10, 0, 0, 1);
        let network_info = NetworkUtils::calculate_network_info(ip, 31).unwrap();

        assert_eq!(network_info.network, Ipv4Addr::new(10, 0, 0, 0));
        assert_eq!(network_info.broadcast, Ipv4Addr::new(10, 0, 0, 1));
        assert_eq!(network_info.total_addresses, 2);
        assert_eq!(network_info.usable_addresses, 2);
    }

    #[test]
    fn test_validate_mac_address() {
        assert!(NetworkUtils::validate_mac_address("00:11:22:33:44:55").is_ok());
        assert!(NetworkUtils::validate_mac_address("AA:BB:CC:DD:EE:FF").is_ok());
        assert!(NetworkUtils::validate_mac_address("aa:bb:cc:dd:ee:ff").is_ok());

        assert!(NetworkUtils::validate_mac_address("00:11:22:33:44").is_err());
        assert!(NetworkUtils::validate_mac_address("00:11:22:33:44:GG").is_err());
        assert!(NetworkUtils::validate_mac_address("invalid").is_err());
    }

    #[tokio::test]
    async fn test_test_internet_connectivity() {
        // This test may fail in environments without internet access
        match NetworkUtils::test_internet_connectivity().await {
            Ok(connected) => {
                println!("Internet connectivity test result: {}", connected);
            }
            Err(e) => {
                println!("Internet connectivity test failed (this may be normal): {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_get_network_interfaces() {
        match NetworkUtils::get_network_interfaces().await {
            Ok(interfaces) => {
                assert!(!interfaces.is_empty());
                println!("Found {} network interfaces", interfaces.len());

                // Should have at least loopback interface
                let loopback = interfaces.iter().find(|iface| iface.name == "lo");
                assert!(loopback.is_some(), "Loopback interface not found");
            }
            Err(e) => {
                println!("Could not get network interfaces (this may be normal in some test environments): {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_dns_resolution() {
        match NetworkUtils::test_dns_resolution("google.com").await {
            Ok(ips) => {
                assert!(!ips.is_empty());
                println!("Resolved google.com to {} addresses", ips.len());
            }
            Err(e) => {
                println!("DNS resolution test failed (this may be normal without internet): {}", e);
            }
        }
    }
}
