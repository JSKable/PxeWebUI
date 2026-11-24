# PxeWebUI

<p align="center">
  <img src="https://raw.githubusercontent.com/PxeWebUI/PxeWebUI/main/static/img/logo.svg" width="96" height="96" alt="PxeWebUI Logo" onerror="this.style.display='none'"/>
</p>

<h3 align="center">Modern & Sleek Web App & PXE Server for Network Booting ISOs</h3>

<p align="center">
  Built with high-performance <strong>Rust</strong> and a responsive <strong>Tailwind CSS</strong> interface.<br/>
  Optimized for hosting in a lightweight <strong>Proxmox VE LXC container</strong>, <strong>Docker</strong>, or <strong>VM</strong>.
</p>

---

## 🌟 Features

- **Sleek & Modern Web Interface**: Fast, responsive dark-mode UI styled with Tailwind CSS, featuring real-time telemetry gauges (CPU, RAM, and Disk space for ISO volume).
- **Zero-Conflict ProxyDHCP**: Operates in **ProxyDHCP** mode alongside your existing router/DHCP server. You **never** need to modify your home or enterprise router's DHCP options.
- **Easy ISO Management**:
  - Drag-and-drop file upload with live progress tracking (supports multi-gigabyte files).
  - Background downloader from external URLs with real-time speed, bytes downloaded, and ETA.
  - Curated quick presets for **Ubuntu 24.04 LTS**, **Debian 12 Netinst**, **Proxmox VE 8.2**, **Clonezilla Live**, **SystemRescue**, **Arch Linux**, **Alpine**, and **Fedora 40**.
- **Full ISO Customization**:
  - Customizable Display Titles, Descriptions, Categories, and Enabled/Disabled toggles.
  - Boot methods: `iPXE sanboot` (HTTP ISO emulation), Syslinux `memdisk` (BIOS RAM disk), `wimboot` (Windows PE / Setup), and custom iPXE scripts.
  - Architecture compatibility tags: Legacy BIOS (x86), UEFI 64-bit, UEFI 32-bit, UEFI ARM64, and Secure Boot.
  - Configurable Default Boot Item and countdown timeout.
- **Client Compatibility & Secure Boot**:
  - Detects client architecture via RFC 4578 (DHCP Option 93).
  - Legacy BIOS clients receive `undionly.kpxe`.
  - UEFI x86_64 clients receive `ipxe.efi`.
  - Secure Boot clients can chainload via signed shim & GRUB (`/boot/grub/grub.cfg`).
- **Security & User Management**:
  - HTTP Basic Auth & Session/Bearer Token authentication.
  - **Default Account**: Username: `Admin` | Password: `PxeWebUIadmin`.
  - Built-in User Management to create, update, and remove accounts with Admin, Operator, or Viewer roles.
- **Client Auditing & Diagnostics**:
  - Live client log auditing tracking client IPs, MACs, User-Agents, architecture platform, and requested resources.
  - Interactive PXE Boot Preview & CRT Terminal Simulator to preview the client screen before booting physical machines.

---

## 🚀 Quick Start on Proxmox VE

### Method 1: 1-Click Proxmox VE Host LXC Installer (Recommended)

Run this command directly in your **Proxmox VE Shell** (Node -> Shell):

```bash
bash -c "$(curl -fsSL https://raw.githubusercontent.com/PxeWebUI/PxeWebUI/main/proxmox-install.sh)"
```

The wizard will prompt you for:
- Container ID (e.g., `105`)
- Target Storage (e.g., `local-lvm` or `local-zfs`)
- Disk Size (e.g., `40G` for ISO storage)
- Network Bridge (e.g., `vmbr0`)

It automatically downloads the latest Debian template, creates a container with nesting enabled, installs `dnsmasq` and `pxewebui`, and launches the service!

---

### Method 2: Docker Compose (LXC or VM with Docker)

Clone this repository and launch with Docker Compose:

```bash
git clone https://github.com/PxeWebUI/PxeWebUI.git
cd PxeWebUI
docker compose up -d
```

> **Note on Network Mode**: `network_mode: host` is configured in `docker-compose.yml`. This is **required** so the container can receive broadcast UDP port 67 (DHCP discovery) packets from network clients.

---

### Method 3: Native Installation on Existing Linux (Debian / Ubuntu / Alpine)

If you already have a Debian/Ubuntu LXC container or VM:

```bash
git clone https://github.com/PxeWebUI/PxeWebUI.git
cd PxeWebUI
sudo bash proxmox-install.sh
```

This installs dependencies (`dnsmasq`, `syslinux-common`), downloads TFTP bootloaders, and enables `pxewebui.service` and `pxewebui-dnsmasq.service` under systemd.

---

## 🔑 Default Credentials

| Field | Default Value | Notes |
| :--- | :--- | :--- |
| **Username** | `Admin` | Case-sensitive |
| **Password** | `PxeWebUIadmin` | Change immediately in User Management |
| **Web UI URL** | `http://<server-ip>:8080` | Also supports HTTP Basic Auth |

---

## 🖥️ How to Boot a Physical Machine via PXE

1. Ensure the client computer is connected to the same local network / VLAN as PxeWebUI via Ethernet.
2. Turn on the machine and immediately press the Boot Menu key:
   - **Dell**: `F12`
   - **HP**: `F9` or `Esc`
   - **Lenovo**: `F12`
   - **ASUS / Generic**: `F8` or `F11`
3. Select **Network Boot / Onboard NIC IPv4 / PXE**.
4. The client will obtain an IP from your router, discover PxeWebUI via ProxyDHCP, load iPXE, and present the PxeWebUI boot menu!

---

## ⚙️ Architecture & Port Mapping

| Port | Protocol | Purpose |
| :--- | :--- | :--- |
| **8080** (or 80) | TCP | Web Management UI, REST API, dynamic `/boot.ipxe`, and HTTP ISO streaming |
| **69** | UDP | TFTP Server: Serves initial `undionly.kpxe`, `ipxe.efi`, and `memdisk` |
| **67 / 4011** | UDP | ProxyDHCP: Listens for DHCP discovery and responds with bootloader filename |

---

## 📁 Repository Structure

```
PxeWebUI/
├── Cargo.toml               # Rust package & dependencies
├── Dockerfile               # Multi-stage release container build
├── docker-compose.yml       # Production host-networking compose definition
├── proxmox-install.sh       # Proxmox VE Host LXC & native Linux installer
├── src/
│   ├── main.rs              # Application entrypoint & HTTP routing
│   ├── auth.rs              # Basic Auth & Session/Bearer token engine
│   ├── config.rs            # CLI parameters & auto IP detection
│   ├── db.rs                # SQLite database migrations & queries
│   ├── models.rs            # Data structures & serialization schemas
│   ├── assets.rs            # Static frontend embedder (rust-embed)
│   ├── routes/              # Axum REST API handlers
│   │   ├── auth.rs          # Login, logout, profile
│   │   ├── iso.rs           # ISO upload, update, deletion
│   │   ├── download.rs      # Background URL downloader & presets
│   │   ├── pxe.rs           # Dynamic /boot.ipxe & HTTP ISO stream
│   │   ├── settings.rs      # Network & timeout configuration
│   │   ├── stats.rs         # Telemetry & client access logs
│   │   └── users.rs         # User account management
│   └── services/
│       ├── downloader.rs    # Streamed chunked downloader with speed & ETA
│       ├── pxe_generator.rs # Dynamic iPXE script & GRUB generator
│       └── dnsmasq.rs       # Automatic dnsmasq.conf generator
├── static/
│   ├── index.html           # Modern Tailwind CSS dashboard
│   └── js/app.js            # Frontend reactive controller
├── tftp/
│   ├── README.md            # TFTP bootloaders guide
│   └── get-bootloaders.sh   # Automated bootloader fetcher
└── docker/
    └── entrypoint.sh        # Container startup supervisor
```

---

## 🔒 Security Best Practices

1. **Change the Default Password**: Immediately navigate to **Users** after initial login and update the `Admin` password.
2. **Firewall Rules**: For untrusted networks, restrict access to port `8080` to management IPs, while leaving ports `67/udp` and `69/udp` open to the subnet being provisioned.
3. **Storage Allocation**: When deploying on Proxmox VE, consider mounting an NFS or ZFS pool to `/var/lib/pxewebui/isos` if you plan to store hundreds of gigabytes of ISOs.

---

## 📜 License

MIT License. Designed and crafted for high reliability and ease of use in homelabs, datacenters, and enterprise environments.
