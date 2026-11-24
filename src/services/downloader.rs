use crate::db::Database;
use crate::models::{DownloadTask, Iso, IsoPreset, StartDownloadRequest};
use futures_util::StreamExt;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::broadcast;
use tracing::{error, info};

#[derive(Clone)]
pub struct DownloadManager {
    tasks: Arc<Mutex<HashMap<String, DownloadTask>>>,
    cancel_senders: Arc<Mutex<HashMap<String, broadcast::Sender<()>>>>,
    iso_dir: PathBuf,
    db: Database,
}

impl DownloadManager {
    pub fn new(iso_dir: PathBuf, db: Database) -> Self {
        std::fs::create_dir_all(&iso_dir).ok();
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
            cancel_senders: Arc::new(Mutex::new(HashMap::new())),
            iso_dir,
            db,
        }
    }

    pub fn list_tasks(&self) -> Vec<DownloadTask> {
        let tasks = self.tasks.lock().unwrap();
        tasks.values().cloned().collect()
    }

    pub fn get_task(&self, id: &str) -> Option<DownloadTask> {
        let tasks = self.tasks.lock().unwrap();
        tasks.get(id).cloned()
    }

    pub fn cancel_task(&self, id: &str) -> bool {
        let senders = self.cancel_senders.lock().unwrap();
        if let Some(tx) = senders.get(id) {
            let _ = tx.send(());
            let mut tasks = self.tasks.lock().unwrap();
            if let Some(task) = tasks.get_mut(id) {
                task.status = "cancelled".to_string();
                task.error_message = Some("Download cancelled by user".to_string());
            }
            return true;
        }
        false
    }

    pub fn start_download(&self, req: StartDownloadRequest) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let (cancel_tx, cancel_rx) = broadcast::channel(1);

        // Deduce filename
        let filename = req.filename.unwrap_or_else(|| {
            let url_path = req.url.split('?').next().unwrap_or(&req.url);
            let name = url_path.split('/').last().unwrap_or("image.iso");
            if name.is_empty() || !name.contains('.') {
                format!("{}.iso", uuid::Uuid::new_v4())
            } else {
                name.to_string()
            }
        });

        // Deduce title
        let title = req.title.unwrap_or_else(|| {
            filename
                .trim_end_matches(".iso")
                .trim_end_matches(".img")
                .replace(['-', '_'], " ")
        });

        let description = req.description.unwrap_or_default();
        let category = req.category.unwrap_or_else(|| "linux".to_string());
        let boot_mode = req.boot_mode.unwrap_or_else(|| "sanboot".to_string());
        let secure_boot = req.secure_boot_supported.unwrap_or(false);

        let task = DownloadTask {
            id: id.clone(),
            url: req.url.clone(),
            filename: filename.clone(),
            title: title.clone(),
            description: description.clone(),
            category: category.clone(),
            boot_mode: boot_mode.clone(),
            status: "downloading".to_string(),
            bytes_downloaded: 0,
            total_bytes: None,
            speed_bps: 0,
            eta_seconds: None,
            error_message: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        {
            let mut tasks = self.tasks.lock().unwrap();
            tasks.insert(id.clone(), task);
            let mut senders = self.cancel_senders.lock().unwrap();
            senders.insert(id.clone(), cancel_tx);
        }

        // Spawn background worker
        let mgr = self.clone();
        let task_id = id.clone();
        tokio::spawn(async move {
            mgr.download_worker(task_id, req.url, filename, title, description, category, boot_mode, secure_boot, cancel_rx).await;
        });

        id
    }

    async fn download_worker(
        &self,
        id: String,
        url: String,
        filename: String,
        title: String,
        description: String,
        category: String,
        boot_mode: String,
        secure_boot: bool,
        mut cancel_rx: broadcast::Receiver<()>,
    ) {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(3600))
            .build()
            .unwrap_or_default();

        let response = match client.get(&url).send().await {
            Ok(resp) => {
                if !resp.status().is_success() {
                    self.fail_task(&id, format!("HTTP error: {}", resp.status()));
                    return;
                }
                resp
            }
            Err(e) => {
                self.fail_task(&id, format!("Failed to connect: {}", e));
                return;
            }
        };

        let total_size = response.content_length();
        {
            let mut tasks = self.tasks.lock().unwrap();
            if let Some(task) = tasks.get_mut(&id) {
                task.total_bytes = total_size;
            }
        }

        let temp_filename = format!("{}.part", filename);
        let dest_path = self.iso_dir.join(&filename);
        let temp_path = self.iso_dir.join(&temp_filename);

        let mut file = match File::create(&temp_path).await {
            Ok(f) => f,
            Err(e) => {
                self.fail_task(&id, format!("Failed to create destination file: {}", e));
                return;
            }
        };

        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = 0;
        let start_time = Instant::now();
        let mut last_update = Instant::now();
        let mut last_bytes = 0u64;

        loop {
            tokio::select! {
                _ = cancel_rx.recv() => {
                    info!("Download task {} cancelled", id);
                    drop(file);
                    tokio::fs::remove_file(&temp_path).await.ok();
                    return;
                }
                chunk_opt = stream.next() => {
                    match chunk_opt {
                        Some(Ok(chunk)) => {
                            if let Err(e) = file.write_all(&chunk).await {
                                self.fail_task(&id, format!("Disk write error: {}", e));
                                drop(file);
                                tokio::fs::remove_file(&temp_path).await.ok();
                                return;
                            }
                            downloaded += chunk.len() as u64;

                            if last_update.elapsed() >= Duration::from_millis(500) {
                                let elapsed = last_update.elapsed().as_secs_f64();
                                let delta_bytes = downloaded.saturating_sub(last_bytes);
                                let speed = if elapsed > 0.0 { (delta_bytes as f64 / elapsed) as u64 } else { 0 };

                                let eta = if let Some(total) = total_size {
                                    if speed > 0 && total > downloaded {
                                        Some((total - downloaded) / speed)
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                };

                                {
                                    let mut tasks = self.tasks.lock().unwrap();
                                    if let Some(task) = tasks.get_mut(&id) {
                                        task.bytes_downloaded = downloaded;
                                        task.speed_bps = speed;
                                        task.eta_seconds = eta;
                                    }
                                }

                                last_update = Instant::now();
                                last_bytes = downloaded;
                            }
                        }
                        Some(Err(e)) => {
                            self.fail_task(&id, format!("Network stream error: {}", e));
                            drop(file);
                            tokio::fs::remove_file(&temp_path).await.ok();
                            return;
                        }
                        None => {
                            // Download complete!
                            break;
                        }
                    }
                }
            }
        }

        if let Err(e) = file.flush().await {
            self.fail_task(&id, format!("Failed to flush file: {}", e));
            tokio::fs::remove_file(&temp_path).await.ok();
            return;
        }
        drop(file);

        // Rename temp file to final filename
        if let Err(e) = tokio::fs::rename(&temp_path, &dest_path).await {
            self.fail_task(&id, format!("Failed to finalize file: {}", e));
            return;
        }

        info!("Download completed: {} ({} bytes)", filename, downloaded);

        // Register ISO in database
        let now = chrono::Utc::now().to_rfc3339();
        let new_iso = Iso {
            id: uuid::Uuid::new_v4().to_string(),
            filename: filename.clone(),
            title,
            description,
            category,
            file_size: downloaded as i64,
            boot_mode,
            custom_ipxe: None,
            kernel_args: None,
            arch_bios: true,
            arch_uefi64: true,
            arch_uefi32: false,
            arch_arm64: false,
            secure_boot_supported: secure_boot,
            is_default: false,
            is_enabled: true,
            checksum_sha256: None,
            created_at: now.clone(),
            updated_at: now,
        };

        if let Err(e) = self.db.insert_iso(&new_iso) {
            error!("Failed to register downloaded ISO in DB: {}", e);
        }

        // Update task state
        {
            let mut tasks = self.tasks.lock().unwrap();
            if let Some(task) = tasks.get_mut(&id) {
                task.status = "completed".to_string();
                task.bytes_downloaded = downloaded;
                task.speed_bps = 0;
                task.eta_seconds = Some(0);
            }
            let mut senders = self.cancel_senders.lock().unwrap();
            senders.remove(&id);
        }
    }

    fn fail_task(&self, id: &str, error: String) {
        error!("Download task {} failed: {}", id, error);
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(task) = tasks.get_mut(id) {
            task.status = "failed".to_string();
            task.error_message = Some(error);
        }
        let mut senders = self.cancel_senders.lock().unwrap();
        senders.remove(id);
    }

    pub fn get_presets() -> Vec<IsoPreset> {
        vec![
            IsoPreset {
                name: "ubuntu-server-24.04".to_string(),
                title: "Ubuntu Server 24.04 LTS (Noble Numbat)".to_string(),
                description: "Standard LTS server installer with cloud-init and automated autoinstall".to_string(),
                category: "linux".to_string(),
                url: "https://releases.ubuntu.com/24.04.1/ubuntu-24.04.1-live-server-amd64.iso".to_string(),
                boot_mode: "sanboot".to_string(),
                secure_boot: true,
                recommended_ram: "2 GB+".to_string(),
            },
            IsoPreset {
                name: "debian-12-netinst".to_string(),
                title: "Debian 12.8 (Bookworm) Netinst".to_string(),
                description: "Minimal network installer for universal Debian GNU/Linux".to_string(),
                category: "linux".to_string(),
                url: "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/debian-12.8.0-amd64-netinst.iso".to_string(),
                boot_mode: "sanboot".to_string(),
                secure_boot: true,
                recommended_ram: "1 GB".to_string(),
            },
            IsoPreset {
                name: "proxmox-ve-8".to_string(),
                title: "Proxmox VE 8.2 ISO Installer".to_string(),
                description: "Enterprise virtualization platform based on Debian and KVM/LXC".to_string(),
                category: "hypervisor".to_string(),
                url: "https://enterprise.proxmox.com/iso/proxmox-ve_8.2-1.iso".to_string(),
                boot_mode: "sanboot".to_string(),
                secure_boot: true,
                recommended_ram: "4 GB+".to_string(),
            },
            IsoPreset {
                name: "clonezilla-live".to_string(),
                title: "Clonezilla Live (amd64)".to_string(),
                description: "Partition and disk imaging/cloning solution for backup and deployment".to_string(),
                category: "utility".to_string(),
                url: "https://free.nchc.org.tw/clonezilla-live/alternative/stable/clonezilla-live-3.1.2-22-amd64.iso".to_string(),
                boot_mode: "sanboot".to_string(),
                secure_boot: true,
                recommended_ram: "1 GB".to_string(),
            },
            IsoPreset {
                name: "systemrescue-amd64".to_string(),
                title: "SystemRescue (Arch-based)".to_string(),
                description: "Rescue toolkit for repairing unbootable systems, partitions, and filesystems".to_string(),
                category: "utility".to_string(),
                url: "https://fastly-cdn.system-rescue.org/releases/11.02/systemrescue-11.02-amd64.iso".to_string(),
                boot_mode: "sanboot".to_string(),
                secure_boot: true,
                recommended_ram: "1 GB".to_string(),
            },
            IsoPreset {
                name: "archlinux-netboot".to_string(),
                title: "Arch Linux Network Boot / Live".to_string(),
                description: "Rolling-release lightweight, bleeding-edge x86_64 installation media".to_string(),
                category: "linux".to_string(),
                url: "https://geo.mirror.pkgbuild.com/iso/latest/archlinux-x86_64.iso".to_string(),
                boot_mode: "sanboot".to_string(),
                secure_boot: false,
                recommended_ram: "1 GB".to_string(),
            },
            IsoPreset {
                name: "alpine-standard".to_string(),
                title: "Alpine Linux 3.20 Standard (x86_64)".to_string(),
                description: "Ultra-small, security-oriented, lightweight musl/busybox Linux".to_string(),
                category: "linux".to_string(),
                url: "https://dl-cdn.alpinelinux.org/alpine/v3.20/releases/x86_64/alpine-standard-3.20.3-x86_64.iso".to_string(),
                boot_mode: "sanboot".to_string(),
                secure_boot: false,
                recommended_ram: "512 MB".to_string(),
            },
            IsoPreset {
                name: "fedora-server-netinst".to_string(),
                title: "Fedora 40 Server Netinst".to_string(),
                description: "Modern, feature-rich Linux platform for datacenter and homelab".to_string(),
                category: "linux".to_string(),
                url: "https://download.fedoraproject.org/pub/fedora/linux/releases/40/Server/x86_64/iso/Fedora-Server-netinst-x86_64-40-1.14.iso".to_string(),
                boot_mode: "sanboot".to_string(),
                secure_boot: true,
                recommended_ram: "2 GB".to_string(),
            },
        ]
    }
}
