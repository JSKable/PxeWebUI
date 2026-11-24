#!/usr/bin/env bash
# ==============================================================================
# PxeWebUI - Proxmox VE Automated LXC & Container Installer
# ==============================================================================
# This script can be run:
# 1. Directly on the Proxmox VE Host to create a dedicated LXC container, OR
# 2. Inside an existing Debian/Ubuntu LXC container or VM to install PxeWebUI natively.
# ==============================================================================

set -e

CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${CYAN}"
echo "=========================================================="
echo "      PxeWebUI - Network Boot & ISO Management Server     "
echo "=========================================================="
echo -e "${NC}"

# Check if running on Proxmox VE Host (pveversion present)
if command -v pveversion >/dev/null 2>&1; then
    IS_PVE_HOST=true
    echo -e "${GREEN}[✔] Detected Proxmox VE Host environment.${NC}"
else
    IS_PVE_HOST=false
    echo -e "${YELLOW}[i] Running inside guest Linux system (LXC, VM or Bare-Metal).${NC}"
fi

if [ "$IS_PVE_HOST" = true ]; then
    echo ""
    echo -e "${CYAN}--- Proxmox VE Host LXC Container Creation Wizard ---${NC}"

    # Determine default CT ID
    NEXT_ID=$(pvesh get /cluster/nextid)
    read -rp "Enter Container ID [Default: $NEXT_ID]: " CT_ID
    CT_ID=${CT_ID:-$NEXT_ID}

    # Hostname
    read -rp "Enter Container Hostname [Default: pxewebui]: " CT_HOSTNAME
    CT_HOSTNAME=${CT_HOSTNAME:-pxewebui}

    # Target Storage
    DEF_STORAGE=$(pvesm status -content rootdir | awk 'NR>1 {print $1}' | head -n1)
    read -rp "Enter Target Storage [Default: $DEF_STORAGE]: " CT_STORAGE
    CT_STORAGE=${CT_STORAGE:-$DEF_STORAGE}

    # Disk Size in GB
    read -rp "Enter ISO & Root Disk Size in GB [Default: 40G]: " CT_DISK
    CT_DISK=${CT_DISK:-40G}

    # RAM in MB
    read -rp "Enter RAM in MB [Default: 1024]: " CT_RAM
    CT_RAM=${CT_RAM:-1024}

    # Network Bridge
    read -rp "Enter Network Bridge [Default: vmbr0]: " CT_BRIDGE
    CT_BRIDGE=${CT_BRIDGE:-vmbr0}

    # Password for root
    read -rsp "Enter Container root password [Default: PxeWebUIadmin]: " CT_PASSWORD
    echo ""
    CT_PASSWORD=${CT_PASSWORD:-PxeWebUIadmin}

    echo ""
    echo -e "${YELLOW}Downloading latest Debian 12 LXC template if needed...${NC}"
    pveam update >/dev/null 2>&1 || true
    TEMPLATE=$(pveam available -section system | grep "debian-12-standard" | awk '{print $2}' | tail -n1)
    if [ -z "$TEMPLATE" ]; then
        TEMPLATE=$(pveam available -section system | grep "debian-11-standard" | awk '{print $2}' | tail -n1)
    fi

    # Download template to storage if not already present
    TEMPLATE_STORAGE=$(pvesm status -content vztmpl | awk 'NR>1 {print $1}' | head -n1)
    if ! pveam list "$TEMPLATE_STORAGE" | grep -q "$TEMPLATE"; then
        echo -e "${CYAN}Downloading $TEMPLATE to $TEMPLATE_STORAGE...${NC}"
        pveam download "$TEMPLATE_STORAGE" "$TEMPLATE"
    fi
    FULL_TEMPLATE="$TEMPLATE_STORAGE:vztmpl/$TEMPLATE"

    echo -e "${GREEN}Creating LXC Container $CT_ID ($CT_HOSTNAME)...${NC}"
    pct create "$CT_ID" "$FULL_TEMPLATE" \
        --hostname "$CT_HOSTNAME" \
        --cores 2 \
        --memory "$CT_RAM" \
        --swap 512 \
        --rootfs "$CT_STORAGE:$CT_DISK" \
        --net0 "name=eth0,bridge=$CT_BRIDGE,ip=dhcp" \
        --ostype debian \
        --password "$CT_PASSWORD" \
        --unprivileged 0 \
        --features nesting=1 \
        --start 1

    echo -e "${YELLOW}Waiting for Container network initialization...${NC}"
    sleep 6

    # Get container IP
    CT_IP=$(pct exec "$CT_ID" -- ip -4 route get 1.1.1.1 2>/dev/null | awk '{print $7}' | tr -d '\n')
    if [ -z "$CT_IP" ]; then
        CT_IP=$(pct exec "$CT_ID" -- hostname -I | awk '{print $1}')
    fi

    echo -e "${GREEN}Container running with IP: $CT_IP${NC}"
    echo -e "${CYAN}Installing PxeWebUI inside LXC container $CT_ID...${NC}"

    # Push and run installation inside the container
    if [ -f "$0" ] && [ "$0" != "bash" ] && [ "$0" != "-bash" ]; then
        echo -e "${CYAN}Pushing installer script to container...${NC}"
        pct push "$CT_ID" "$0" /tmp/proxmox-install.sh
        pct exec "$CT_ID" -- bash /tmp/proxmox-install.sh
    else
        pct exec "$CT_ID" -- bash -c "$(curl -fsSL https://raw.githubusercontent.com/JSKable/PxeWebUI/main/proxmox-install.sh)"
    fi

    echo ""
    echo -e "${GREEN}==========================================================${NC}"
    echo -e "${GREEN}   PxeWebUI LXC Container Successfully Deployed!         ${NC}"
    echo -e "${GREEN}==========================================================${NC}"
    echo -e "Web Interface: ${CYAN}http://$CT_IP:8080${NC}"
    echo -e "Default Username: ${YELLOW}Admin${NC}"
    echo -e "Default Password: ${YELLOW}PxeWebUIadmin${NC}"
    echo -e "ProxyDHCP: ${GREEN}Enabled (Broadcasts on $CT_BRIDGE)${NC}"
    echo -e "iPXE Boot URL: ${CYAN}http://$CT_IP:8080/boot.ipxe${NC}"
    exit 0
fi

# ==============================================================================
# Guest Installation Mode (Running inside LXC container, VM, or bare-metal Linux)
# ==============================================================================
echo -e "${CYAN}--- Installing PxeWebUI System Services ---${NC}"

if [ "$(id -u)" -ne 0 ]; then
    echo -e "${RED}[!] Please run this installer as root (or with sudo).${NC}"
    exit 1
fi

echo -e "${YELLOW}Updating package repositories and installing dependencies...${NC}"
apt-get update -y
apt-get install -y --no-install-recommends \
    curl \
    wget \
    ca-certificates \
    dnsmasq \
    syslinux-common \
    iproute2 \
    tar \
    git \
    build-essential \
    pkg-config \
    libssl-dev

INSTALL_DIR="/opt/pxewebui"
DATA_DIR="/var/lib/pxewebui/data"
ISO_DIR="/var/lib/pxewebui/isos"
TFTP_DIR="/var/lib/pxewebui/tftp"

mkdir -p "$INSTALL_DIR" "$DATA_DIR" "$ISO_DIR" "$TFTP_DIR"

# Check if pre-built binary exists locally or download release
if [ -f "./target/release/pxewebui" ]; then
    echo -e "${GREEN}Copying local release binary...${NC}"
    cp "./target/release/pxewebui" "$INSTALL_DIR/pxewebui"
    cp -r "./tftp" "$INSTALL_DIR/"
    cp -r "./static" "$INSTALL_DIR/" 2>/dev/null || true
elif [ -f "./pxewebui" ]; then
    cp "./pxewebui" "$INSTALL_DIR/pxewebui"
    cp -r "./tftp" "$INSTALL_DIR/" 2>/dev/null || true
else
    echo -e "${YELLOW}Building or fetching PxeWebUI release binary...${NC}"
    # If not inside repository root, clone source from GitHub
    if [ ! -f "./Cargo.toml" ]; then
        echo -e "${CYAN}Cloning PxeWebUI source repository...${NC}"
        rm -rf /tmp/pxewebui-src
        git clone https://github.com/JSKable/PxeWebUI.git /tmp/pxewebui-src
        cd /tmp/pxewebui-src
    fi

    # If cargo is installed, build it
    if command -v cargo >/dev/null 2>&1; then
        cargo build --release
        cp "./target/release/pxewebui" "$INSTALL_DIR/pxewebui"
    else
        echo -e "${CYAN}Installing Rust toolchain for first-time compilation...${NC}"
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
        cargo build --release
        cp "./target/release/pxewebui" "$INSTALL_DIR/pxewebui"
    fi
fi

chmod +x "$INSTALL_DIR/pxewebui"

# Download initial bootloaders
echo -e "${YELLOW}Downloading standard TFTP network bootloaders...${NC}"
if [ -f "$INSTALL_DIR/tftp/get-bootloaders.sh" ]; then
    bash "$INSTALL_DIR/tftp/get-bootloaders.sh" "$TFTP_DIR"
else
    curl -sSL "https://boot.ipxe.org/undionly.kpxe" -o "$TFTP_DIR/undionly.kpxe"
    curl -sSL "https://boot.ipxe.org/ipxe.efi" -o "$TFTP_DIR/ipxe.efi"
    curl -sSL "https://github.com/ipxe/wimboot/releases/latest/download/wimboot" -o "$TFTP_DIR/wimboot" || true
    if [ -f "/usr/lib/syslinux/memdisk" ]; then
        cp "/usr/lib/syslinux/memdisk" "$TFTP_DIR/memdisk"
    fi
fi

# Detect server IP
HOST_IP=$(ip -4 route get 1.1.1.1 2>/dev/null | awk '{print $7}' | tr -d '\n')
if [ -z "$HOST_IP" ]; then
    HOST_IP=$(hostname -I | awk '{print $1}')
fi

# Generate initial dnsmasq configuration
DNSMASQ_CONF="$DATA_DIR/dnsmasq.conf"
cat <<EOF > "$DNSMASQ_CONF"
port=0
log-dhcp
log-facility=-
enable-tftp
tftp-root=$TFTP_DIR

# ProxyDHCP Mode
dhcp-range=$HOST_IP,proxy

# Client Architectures
dhcp-match=set:bios,option:client-arch,0
dhcp-match=set:efi32,option:client-arch,6
dhcp-match=set:efi64,option:client-arch,7
dhcp-match=set:efi64,option:client-arch,9
dhcp-match=set:efiarm64,option:client-arch,11

# iPXE userclass
dhcp-userclass=set:ipxe,iPXE

dhcp-boot=tag:ipxe,http://$HOST_IP:8080/boot.ipxe
dhcp-boot=tag:bios,undionly.kpxe
dhcp-boot=tag:efi32,ipxe-i386.efi
dhcp-boot=tag:efi64,ipxe.efi
dhcp-boot=tag:efiarm64,ipxe-arm64.efi

pxe-service=tag:bios,x86PC,"PxeWebUI (Legacy BIOS)",undionly.kpxe
pxe-service=tag:efi64,X86-64_EFI,"PxeWebUI (UEFI x64)",ipxe.efi
pxe-service=tag:efi64,BC_EFI,"PxeWebUI (UEFI BC)",ipxe.efi
EOF

# Disable default system dnsmasq to avoid conflict with our managed instance
systemctl stop dnsmasq 2>/dev/null || true
systemctl disable dnsmasq 2>/dev/null || true

# Create systemd service for dnsmasq proxy
cat <<EOF > /etc/systemd/system/pxewebui-dnsmasq.service
[Unit]
Description=PxeWebUI dnsmasq ProxyDHCP & TFTP Service
After=network.target

[Service]
Type=simple
ExecStart=/usr/sbin/dnsmasq --conf-file=$DNSMASQ_CONF --no-daemon
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
EOF

# Create systemd service for PxeWebUI
cat <<EOF > /etc/systemd/system/pxewebui.service
[Unit]
Description=PxeWebUI Web Server and ISO Manager
After=network.target pxewebui-dnsmasq.service

[Service]
Type=simple
WorkingDirectory=$INSTALL_DIR
ExecStart=$INSTALL_DIR/pxewebui \
    --bind-addr 0.0.0.0:8080 \
    --server-ip $HOST_IP \
    --http-port 8080 \
    --data-dir $DATA_DIR \
    --iso-dir $ISO_DIR \
    --tftp-dir $TFTP_DIR
Restart=always
RestartSec=3
Environment=RUST_LOG=info,pxewebui=debug

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable --now pxewebui-dnsmasq.service
systemctl enable --now pxewebui.service

echo ""
echo -e "${GREEN}==========================================================${NC}"
echo -e "${GREEN}   PxeWebUI Installation Completed Successfully!         ${NC}"
echo -e "${GREEN}==========================================================${NC}"
echo -e "Web Interface:   ${CYAN}http://$HOST_IP:8080${NC}"
echo -e "Default User:    ${YELLOW}Admin${NC}"
echo -e "Default Pass:    ${YELLOW}PxeWebUIadmin${NC}"
echo -e "ISO Storage:     ${CYAN}$ISO_DIR${NC}"
echo -e "iPXE Boot URL:   ${CYAN}http://$HOST_IP:8080/boot.ipxe${NC}"
echo -e "ProxyDHCP:       ${GREEN}Active on port 67/udp (Coexists with router)${NC}"
