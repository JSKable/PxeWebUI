use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about = "PxeWebUI - Modern Web UI & PXE Server")]
pub struct AppConfig {
    #[arg(long, env = "PXE_BIND_ADDR", default_value = "0.0.0.0:8080")]
    pub bind_addr: String,

    #[arg(long, env = "PXE_SERVER_IP")]
    pub server_ip: Option<String>,

    #[arg(long, env = "PXE_HTTP_PORT", default_value = "8080")]
    pub http_port: u16,

    #[arg(long, env = "PXE_TFTP_PORT", default_value = "69")]
    pub tftp_port: u16,

    #[arg(long, env = "PXE_ISO_DIR", default_value = "./isos")]
    pub iso_dir: PathBuf,

    #[arg(long, env = "PXE_DATA_DIR", default_value = "./data")]
    pub data_dir: PathBuf,

    #[arg(long, env = "PXE_TFTP_DIR", default_value = "./tftp")]
    pub tftp_dir: PathBuf,
}

impl AppConfig {
    pub fn get_server_ip(&self) -> String {
        if let Some(ref ip) = self.server_ip {
            return ip.clone();
        }
        // Auto-detect local interface IP
        match local_ip_address::local_ip() {
            Ok(ip) => ip.to_string(),
            Err(_) => "127.0.0.1".to_string(),
        }
    }
}
