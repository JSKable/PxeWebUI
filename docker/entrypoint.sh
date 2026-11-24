#!/usr/bin/env bash
set -e

echo "======================================================"
echo " Starting PxeWebUI Container Environment"
echo "======================================================"

DATA_DIR="${PXE_DATA_DIR:-/data}"
ISO_DIR="${PXE_ISO_DIR:-/isos}"
TFTP_DIR="${PXE_TFTP_DIR:-/tftp}"
BIND_ADDR="${PXE_BIND_ADDR:-0.0.0.0:8080}"
HTTP_PORT="${PXE_HTTP_PORT:-8080}"

mkdir -p "$DATA_DIR" "$ISO_DIR" "$TFTP_DIR"

# Download bootloaders if missing
if [ ! -f "$TFTP_DIR/ipxe.efi" ] || [ ! -f "$TFTP_DIR/undionly.kpxe" ]; then
    echo "Initial TFTP bootloaders not found. Downloading..."
    /app/tftp/get-bootloaders.sh "$TFTP_DIR"
fi

# Detect default network IP if not specified
if [ -z "$PXE_SERVER_IP" ]; then
    PXE_SERVER_IP=$(ip -4 route get 1.1.1.1 2>/dev/null | awk '{print $7}' | tr -d '\n')
    if [ -z "$PXE_SERVER_IP" ]; then
        PXE_SERVER_IP=$(hostname -I 2>/dev/null | awk '{print $1}')
    fi
    export PXE_SERVER_IP="${PXE_SERVER_IP:-127.0.0.1}"
fi

echo "PXE Server IP:   $PXE_SERVER_IP"
echo "HTTP Web & Boot: $HTTP_PORT"
echo "TFTP Port:       69/udp"
echo "ProxyDHCP Port:  67/udp"

DNSMASQ_CONF="$DATA_DIR/dnsmasq.conf"

# Generate dnsmasq configuration if not present
if [ ! -f "$DNSMASQ_CONF" ]; then
    cat <<EOF > "$DNSMASQ_CONF"
# Auto-generated dnsmasq.conf for PxeWebUI
port=0
log-dhcp
log-facility=-
dhcp-no-override
enable-tftp
tftp-root=$TFTP_DIR

# ProxyDHCP Mode: Coexists with your existing home/office router!
dhcp-range=$PXE_SERVER_IP,proxy

# Client Architecture Matching (RFC 4578)
dhcp-match=set:efi32,option:client-arch,6
dhcp-match=set:efi64,option:client-arch,7
dhcp-match=set:efi64,option:client-arch,9
dhcp-match=set:efiarm64,option:client-arch,11

# iPXE user class detection
dhcp-userclass=set:ipxe,iPXE

# PXE Prompt
pxe-prompt="Booting PxeWebUI...",1

# PXE Service Menu for ProxyDHCP
pxe-service=x86PC,"PxeWebUI (Legacy BIOS)",undionly.kpxe,$PXE_SERVER_IP
pxe-service=X86-64_EFI,"PxeWebUI (UEFI x64)",ipxe.efi,$PXE_SERVER_IP
pxe-service=BC_EFI,"PxeWebUI (UEFI BC)",ipxe.efi,$PXE_SERVER_IP
pxe-service=IA32_EFI,"PxeWebUI (UEFI 32-bit)",ipxe-i386.efi,$PXE_SERVER_IP
pxe-service=ARM64_EFI,"PxeWebUI (UEFI ARM64)",ipxe-arm64.efi,$PXE_SERVER_IP

# If client is already running iPXE, chainload directly to WebUI dynamic menu
dhcp-boot=tag:ipxe,http://$PXE_SERVER_IP:$HTTP_PORT/boot.ipxe

# Architecture-specific TFTP Bootloaders
dhcp-boot=tag:efi64,ipxe.efi,,$PXE_SERVER_IP
dhcp-boot=tag:efi32,ipxe-i386.efi,,$PXE_SERVER_IP
dhcp-boot=tag:efiarm64,ipxe-arm64.efi,,$PXE_SERVER_IP

# Default Fallback for Legacy BIOS
dhcp-boot=undionly.kpxe,,$PXE_SERVER_IP
EOF
fi

# Function to gracefully stop processes
cleanup() {
    echo "Stopping dnsmasq and PxeWebUI..."
    kill -TERM "$DNSMASQ_PID" 2>/dev/null || true
    kill -TERM "$APP_PID" 2>/dev/null || true
    wait
    exit 0
}

trap cleanup SIGINT SIGTERM

echo "Starting dnsmasq (ProxyDHCP & TFTP)..."
dnsmasq --conf-file="$DNSMASQ_CONF" --no-daemon &
DNSMASQ_PID=$!

echo "Starting PxeWebUI application..."
/app/pxewebui \
    --bind-addr "$BIND_ADDR" \
    --server-ip "$PXE_SERVER_IP" \
    --http-port "$HTTP_PORT" \
    --data-dir "$DATA_DIR" \
    --iso-dir "$ISO_DIR" \
    --tftp-dir "$TFTP_DIR" &
APP_PID=$!

wait -n "$APP_PID" "$DNSMASQ_PID"
cleanup
