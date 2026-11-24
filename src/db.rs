use crate::models::{ClientLog, Iso, Settings, User};
use rusqlite::{params, Connection, Result};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing::info;

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let conn = Connection::open(path)?;
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT UNIQUE NOT NULL,
                password_hash TEXT NOT NULL,
                role TEXT NOT NULL DEFAULT 'admin',
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS isos (
                id TEXT PRIMARY KEY,
                filename TEXT NOT NULL,
                title TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                category TEXT NOT NULL DEFAULT 'linux',
                file_size INTEGER NOT NULL DEFAULT 0,
                boot_mode TEXT NOT NULL DEFAULT 'sanboot',
                custom_ipxe TEXT,
                kernel_args TEXT,
                arch_bios INTEGER NOT NULL DEFAULT 1,
                arch_uefi64 INTEGER NOT NULL DEFAULT 1,
                arch_uefi32 INTEGER NOT NULL DEFAULT 0,
                arch_arm64 INTEGER NOT NULL DEFAULT 0,
                secure_boot_supported INTEGER NOT NULL DEFAULT 0,
                is_default INTEGER NOT NULL DEFAULT 0,
                is_enabled INTEGER NOT NULL DEFAULT 1,
                checksum_sha256 TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS client_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                client_ip TEXT NOT NULL,
                user_agent TEXT NOT NULL,
                requested_resource TEXT NOT NULL,
                boot_platform TEXT,
                timestamp TEXT NOT NULL
            );
            ",
        )?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        db.seed_defaults()?;
        Ok(db)
    }

    fn seed_defaults(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        // Check if admin user exists
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM users WHERE username = 'Admin'")?;
        let count: i64 = stmt.query_row([], |r| r.get(0))?;

        if count == 0 {
            info!("Seeding default Admin user...");
            // Hash 'PxeWebUIadmin'
            let hash = bcrypt::hash("PxeWebUIadmin", bcrypt::DEFAULT_COST)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            let now = chrono::Utc::now().to_rfc3339();

            conn.execute(
                "INSERT INTO users (username, password_hash, role, created_at) VALUES (?1, ?2, ?3, ?4)",
                params!["Admin", hash, "admin", now],
            )?;
            info!("Default user 'Admin' created successfully.");
        }

        // Seed default settings if empty
        let default_settings = [
            ("boot_timeout", "10"),
            ("default_boot_action", "local_disk"),
            ("proxydhcp_enabled", "true"),
            ("menu_title", "PxeWebUI - Network Boot Environment"),
            ("ipxe_banner", "Welcome to PxeWebUI Network Boot Server"),
        ];

        for (k, v) in default_settings {
            conn.execute(
                "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
                params![k, v],
            )?;
        }

        Ok(())
    }

    // --- Users ---
    pub fn get_user_by_username(&self, username: &str) -> Result<Option<User>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, username, password_hash, role, created_at FROM users WHERE username = ?1",
        )?;
        let mut rows = stmt.query(params![username])?;

        if let Some(row) = rows.next()? {
            Ok(Some(User {
                id: row.get(0)?,
                username: row.get(1)?,
                password_hash: row.get(2)?,
                role: row.get(3)?,
                created_at: row.get(4)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn list_users(&self) -> Result<Vec<User>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, username, password_hash, role, created_at FROM users ORDER BY id ASC",
        )?;
        let user_iter = stmt.query_map([], |row| {
            Ok(User {
                id: row.get(0)?,
                username: row.get(1)?,
                password_hash: row.get(2)?,
                role: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;

        let mut users = Vec::new();
        for user in user_iter {
            users.push(user?);
        }
        Ok(users)
    }

    pub fn create_user(&self, username: &str, password: &str, role: &str) -> Result<User> {
        let conn = self.conn.lock().unwrap();
        let hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO users (username, password_hash, role, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![username, hash, role, now],
        )?;

        let id = conn.last_insert_rowid();
        Ok(User {
            id,
            username: username.to_string(),
            password_hash: hash,
            role: role.to_string(),
            created_at: now,
        })
    }

    pub fn update_user(&self, id: i64, new_password: Option<&str>, new_role: Option<&str>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        if let Some(pw) = new_password {
            let hash = bcrypt::hash(pw, bcrypt::DEFAULT_COST)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            conn.execute(
                "UPDATE users SET password_hash = ?1 WHERE id = ?2",
                params![hash, id],
            )?;
        }
        if let Some(r) = new_role {
            conn.execute(
                "UPDATE users SET role = ?1 WHERE id = ?2",
                params![r, id],
            )?;
        }
        Ok(())
    }

    pub fn delete_user(&self, id: i64) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        // Prevent deleting the last admin
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM users WHERE role = 'admin'")?;
        let admin_count: i64 = stmt.query_row([], |r| r.get(0))?;

        let mut user_stmt = conn.prepare("SELECT role FROM users WHERE id = ?1")?;
        let user_role: Option<String> = user_stmt.query_row(params![id], |r| r.get(0)).ok();

        if user_role.as_deref() == Some("admin") && admin_count <= 1 {
            return Ok(false);
        }

        let rows = conn.execute("DELETE FROM users WHERE id = ?1", params![id])?;
        Ok(rows > 0)
    }

    // --- ISOs ---
    pub fn list_isos(&self) -> Result<Vec<Iso>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, filename, title, description, category, file_size, boot_mode,
                    custom_ipxe, kernel_args, arch_bios, arch_uefi64, arch_uefi32,
                    arch_arm64, secure_boot_supported, is_default, is_enabled,
                    checksum_sha256, created_at, updated_at
             FROM isos ORDER BY is_default DESC, title ASC",
        )?;

        let iso_iter = stmt.query_map([], |row| {
            Ok(Iso {
                id: row.get(0)?,
                filename: row.get(1)?,
                title: row.get(2)?,
                description: row.get(3)?,
                category: row.get(4)?,
                file_size: row.get(5)?,
                boot_mode: row.get(6)?,
                custom_ipxe: row.get(7)?,
                kernel_args: row.get(8)?,
                arch_bios: row.get::<_, i32>(9)? != 0,
                arch_uefi64: row.get::<_, i32>(10)? != 0,
                arch_uefi32: row.get::<_, i32>(11)? != 0,
                arch_arm64: row.get::<_, i32>(12)? != 0,
                secure_boot_supported: row.get::<_, i32>(13)? != 0,
                is_default: row.get::<_, i32>(14)? != 0,
                is_enabled: row.get::<_, i32>(15)? != 0,
                checksum_sha256: row.get(16)?,
                created_at: row.get(17)?,
                updated_at: row.get(18)?,
            })
        })?;

        let mut isos = Vec::new();
        for iso in iso_iter {
            isos.push(iso?);
        }
        Ok(isos)
    }

    pub fn get_iso(&self, id: &str) -> Result<Option<Iso>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, filename, title, description, category, file_size, boot_mode,
                    custom_ipxe, kernel_args, arch_bios, arch_uefi64, arch_uefi32,
                    arch_arm64, secure_boot_supported, is_default, is_enabled,
                    checksum_sha256, created_at, updated_at
             FROM isos WHERE id = ?1",
        )?;

        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Iso {
                id: row.get(0)?,
                filename: row.get(1)?,
                title: row.get(2)?,
                description: row.get(3)?,
                category: row.get(4)?,
                file_size: row.get(5)?,
                boot_mode: row.get(6)?,
                custom_ipxe: row.get(7)?,
                kernel_args: row.get(8)?,
                arch_bios: row.get::<_, i32>(9)? != 0,
                arch_uefi64: row.get::<_, i32>(10)? != 0,
                arch_uefi32: row.get::<_, i32>(11)? != 0,
                arch_arm64: row.get::<_, i32>(12)? != 0,
                secure_boot_supported: row.get::<_, i32>(13)? != 0,
                is_default: row.get::<_, i32>(14)? != 0,
                is_enabled: row.get::<_, i32>(15)? != 0,
                checksum_sha256: row.get(16)?,
                created_at: row.get(17)?,
                updated_at: row.get(18)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn insert_iso(&self, iso: &Iso) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO isos (
                id, filename, title, description, category, file_size, boot_mode,
                custom_ipxe, kernel_args, arch_bios, arch_uefi64, arch_uefi32,
                arch_arm64, secure_boot_supported, is_default, is_enabled,
                checksum_sha256, created_at, updated_at
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19
             )",
            params![
                iso.id,
                iso.filename,
                iso.title,
                iso.description,
                iso.category,
                iso.file_size,
                iso.boot_mode,
                iso.custom_ipxe,
                iso.kernel_args,
                if iso.arch_bios { 1 } else { 0 },
                if iso.arch_uefi64 { 1 } else { 0 },
                if iso.arch_uefi32 { 1 } else { 0 },
                if iso.arch_arm64 { 1 } else { 0 },
                if iso.secure_boot_supported { 1 } else { 0 },
                if iso.is_default { 1 } else { 0 },
                if iso.is_enabled { 1 } else { 0 },
                iso.checksum_sha256,
                iso.created_at,
                iso.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn update_iso(&self, iso: &Iso) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        // If this iso is marked as default, clear other defaults
        if iso.is_default {
            conn.execute("UPDATE isos SET is_default = 0 WHERE id != ?1", params![iso.id])?;
        }

        conn.execute(
            "UPDATE isos SET
                title = ?1,
                description = ?2,
                category = ?3,
                boot_mode = ?4,
                custom_ipxe = ?5,
                kernel_args = ?6,
                arch_bios = ?7,
                arch_uefi64 = ?8,
                arch_uefi32 = ?9,
                arch_arm64 = ?10,
                secure_boot_supported = ?11,
                is_default = ?12,
                is_enabled = ?13,
                updated_at = ?14
             WHERE id = ?15",
            params![
                iso.title,
                iso.description,
                iso.category,
                iso.boot_mode,
                iso.custom_ipxe,
                iso.kernel_args,
                if iso.arch_bios { 1 } else { 0 },
                if iso.arch_uefi64 { 1 } else { 0 },
                if iso.arch_uefi32 { 1 } else { 0 },
                if iso.arch_arm64 { 1 } else { 0 },
                if iso.secure_boot_supported { 1 } else { 0 },
                if iso.is_default { 1 } else { 0 },
                if iso.is_enabled { 1 } else { 0 },
                iso.updated_at,
                iso.id,
            ],
        )?;
        Ok(())
    }

    pub fn delete_iso(&self, id: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT filename FROM isos WHERE id = ?1")?;
        let filename: Option<String> = stmt.query_row(params![id], |r| r.get(0)).ok();

        if filename.is_some() {
            conn.execute("DELETE FROM isos WHERE id = ?1", params![id])?;
        }
        Ok(filename)
    }

    // --- Settings ---
    pub fn get_settings(&self, default_ip: &str, http_port: u16, tftp_port: u16) -> Result<Settings> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut server_ip = default_ip.to_string();
        let mut boot_timeout = 10u32;
        let mut default_boot_action = "local_disk".to_string();
        let mut proxydhcp_enabled = true;
        let mut menu_title = "PxeWebUI - Network Boot Environment".to_string();
        let mut ipxe_banner = "Welcome to PxeWebUI Network Boot Server".to_string();

        for item in rows {
            let (k, v) = item?;
            match k.as_str() {
                "server_ip" if !v.is_empty() => server_ip = v,
                "boot_timeout" => {
                    if let Ok(val) = v.parse::<u32>() {
                        boot_timeout = val;
                    }
                }
                "default_boot_action" => default_boot_action = v,
                "proxydhcp_enabled" => proxydhcp_enabled = v == "true" || v == "1",
                "menu_title" => menu_title = v,
                "ipxe_banner" => ipxe_banner = v,
                _ => {}
            }
        }

        Ok(Settings {
            server_ip,
            http_port,
            tftp_port,
            boot_timeout,
            default_boot_action,
            proxydhcp_enabled,
            menu_title,
            ipxe_banner,
        })
    }

    pub fn update_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = ?2",
            params![key, value],
        )?;
        Ok(())
    }

    // --- Client Logs ---
    pub fn log_client_access(
        &self,
        ip: &str,
        user_agent: &str,
        resource: &str,
        platform: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO client_logs (client_ip, user_agent, requested_resource, boot_platform, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![ip, user_agent, resource, platform, now],
        )?;

        // Prune logs older than 500 entries
        conn.execute(
            "DELETE FROM client_logs WHERE id NOT IN (SELECT id FROM client_logs ORDER BY id DESC LIMIT 500)",
            [],
        )?;
        Ok(())
    }

    pub fn list_recent_logs(&self, limit: usize) -> Result<Vec<ClientLog>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, client_ip, user_agent, requested_resource, boot_platform, timestamp
             FROM client_logs ORDER BY id DESC LIMIT ?1",
        )?;
        let log_iter = stmt.query_map(params![limit as i64], |row| {
            Ok(ClientLog {
                id: row.get(0)?,
                client_ip: row.get(1)?,
                user_agent: row.get(2)?,
                requested_resource: row.get(3)?,
                boot_platform: row.get(4)?,
                timestamp: row.get(5)?,
            })
        })?;

        let mut logs = Vec::new();
        for log in log_iter {
            logs.push(log?);
        }
        Ok(logs)
    }
}
