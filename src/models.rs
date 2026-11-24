use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: String, // "admin", "operator", "viewer"
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub role: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUserRequest {
    pub password: Option<String>,
    pub role: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Iso {
    pub id: String,
    pub filename: String,
    pub title: String,
    pub description: String,
    pub category: String, // "linux", "windows", "hypervisor", "utility", "custom"
    pub file_size: i64,
    pub boot_mode: String, // "sanboot", "memdisk", "kernel_initrd", "wimboot", "custom"
    pub custom_ipxe: Option<String>,
    pub kernel_args: Option<String>,
    pub arch_bios: bool,
    pub arch_uefi64: bool,
    pub arch_uefi32: bool,
    pub arch_arm64: bool,
    pub secure_boot_supported: bool,
    pub is_default: bool,
    pub is_enabled: bool,
    pub checksum_sha256: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateIsoRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub boot_mode: Option<String>,
    pub custom_ipxe: Option<String>,
    pub kernel_args: Option<String>,
    pub arch_bios: Option<bool>,
    pub arch_uefi64: Option<bool>,
    pub arch_uefi32: Option<bool>,
    pub arch_arm64: Option<bool>,
    pub secure_boot_supported: Option<bool>,
    pub is_default: Option<bool>,
    pub is_enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartDownloadRequest {
    pub url: String,
    pub filename: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub boot_mode: Option<String>,
    pub secure_boot_supported: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadTask {
    pub id: String,
    pub url: String,
    pub filename: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub boot_mode: String,
    pub status: String, // "downloading", "completed", "failed", "cancelled"
    pub bytes_downloaded: u64,
    pub total_bytes: Option<u64>,
    pub speed_bps: u64,
    pub eta_seconds: Option<u64>,
    pub error_message: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsoPreset {
    pub name: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub url: String,
    pub boot_mode: String,
    pub secure_boot: bool,
    pub recommended_ram: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientLog {
    pub id: i64,
    pub client_ip: String,
    pub user_agent: String,
    pub requested_resource: String,
    pub boot_platform: Option<String>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub server_ip: String,
    pub http_port: u16,
    pub tftp_port: u16,
    pub boot_timeout: u32,
    pub default_boot_action: String, // "local_disk", "first_iso", "iso:<id>"
    pub proxydhcp_enabled: bool,
    pub menu_title: String,
    pub ipxe_banner: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStats {
    pub cpu_usage: f32,
    pub ram_used_mb: u64,
    pub ram_total_mb: u64,
    pub disk_free_gb: f64,
    pub disk_total_gb: f64,
    pub total_isos: usize,
    pub active_downloads: usize,
    pub server_ip: String,
    pub http_port: u16,
    pub uptime_seconds: u64,
}
