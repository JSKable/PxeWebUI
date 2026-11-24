use std::path::Path;
use tracing::info;

pub struct DnsmasqManager;

impl DnsmasqManager {
    /// Generates dnsmasq configuration for proxyDHCP and TFTP
    pub fn generate_config(
        server_ip: &str,
        tftp_dir: &Path,
        http_port: u16,
        proxy_dhcp: bool,
    ) -> String {
        let tftp_path = tftp_dir.to_string_lossy();
        let mut conf = String::new();

        conf.push_str("# Auto-generated dnsmasq.conf by PxeWebUI\n");
        conf.push_str("# Disables DNS functionality, acts as TFTP + ProxyDHCP\n\n");

        // Disable DNS
        conf.push_str("port=0\n\n");

        // Log DHCP queries for debugging and tracking
        conf.push_str("log-dhcp\n");
        conf.push_str("log-facility=-\n\n");

        // Enable TFTP Server
        conf.push_str("enable-tftp\n");
        conf.push_str(&format!("tftp-root={}\n\n", tftp_path));

        if proxy_dhcp {
            conf.push_str("# ProxyDHCP Mode: Co-exists with your main router/DHCP server\n");
            conf.push_str(&format!("dhcp-range={},proxy\n\n", server_ip));
        }

        // Tag client architectures (RFC 4578 / DHCP Option 93)
        conf.push_str("# Client Architecture Tags\n");
        conf.push_str("dhcp-match=set:bios,option:client-arch,0\n");
        conf.push_str("dhcp-match=set:efi32,option:client-arch,6\n");
        conf.push_str("dhcp-match=set:efi64,option:client-arch,7\n");
        conf.push_str("dhcp-match=set:efi64,option:client-arch,9\n");
        conf.push_str("dhcp-match=set:efiarm64,option:client-arch,11\n\n");

        // Tag iPXE requests (DHCP Option 175)
        conf.push_str("# Detect if client is already running iPXE\n");
        conf.push_str("dhcp-userclass=set:ipxe,iPXE\n\n");

        // Chainload to HTTP boot.ipxe if iPXE is running
        conf.push_str("# If client is already iPXE, point to PxeWebUI dynamic menu\n");
        conf.push_str(&format!(
            "dhcp-boot=tag:ipxe,http://{}:{}/boot.ipxe\n\n",
            server_ip, http_port
        ));

        // Otherwise deliver architecture-specific bootloader via TFTP
        conf.push_str("# Architecture-specific TFTP Bootloaders\n");
        conf.push_str("dhcp-boot=tag:bios,undionly.kpxe\n");
        conf.push_str("dhcp-boot=tag:efi32,ipxe-i386.efi\n");
        conf.push_str("dhcp-boot=tag:efi64,ipxe.efi\n");
        conf.push_str("dhcp-boot=tag:efiarm64,ipxe-arm64.efi\n\n");

        // PXE Service Menu entries for ProxyDHCP clients
        conf.push_str("# PXE Service Menu (for legacy/UEFI proxyDHCP discovery)\n");
        conf.push_str("pxe-service=tag:bios,x86PC,\"PxeWebUI (Legacy BIOS)\",undionly.kpxe\n");
        conf.push_str("pxe-service=tag:efi64,X86-64_EFI,\"PxeWebUI (UEFI x64)\",ipxe.efi\n");
        conf.push_str("pxe-service=tag:efi64,BC_EFI,\"PxeWebUI (UEFI BC)\",ipxe.efi\n");
        conf.push_str("pxe-service=tag:efi32,6,\"PxeWebUI (UEFI 32-bit)\",ipxe-i386.efi\n");
        conf.push_str("pxe-service=tag:efiarm64,11,\"PxeWebUI (UEFI ARM64)\",ipxe-arm64.efi\n");

        conf
    }

    pub fn write_config_file(
        dest_path: &Path,
        server_ip: &str,
        tftp_dir: &Path,
        http_port: u16,
        proxy_dhcp: bool,
    ) -> std::io::Result<()> {
        let conf = Self::generate_config(server_ip, tftp_dir, http_port, proxy_dhcp);
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(dest_path, conf)?;
        info!("Updated dnsmasq config at {:?}", dest_path);
        Ok(())
    }
}
