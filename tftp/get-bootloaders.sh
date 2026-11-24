#!/usr/bin/env bash
set -e

TFTP_DIR="${1:-$(dirname "$0")}"
mkdir -p "$TFTP_DIR"

echo "=== Downloading standard network bootloaders into $TFTP_DIR ==="

# 1. iPXE Legacy BIOS
echo "Downloading undionly.kpxe (BIOS)..."
curl -sSL "https://boot.ipxe.org/undionly.kpxe" -o "$TFTP_DIR/undionly.kpxe"

# 2. iPXE UEFI 64-bit
echo "Downloading ipxe.efi (UEFI x86_64)..."
curl -sSL "https://boot.ipxe.org/ipxe.efi" -o "$TFTP_DIR/ipxe.efi"

# 3. iPXE UEFI 32-bit (Optional)
echo "Downloading ipxe-i386.efi (UEFI 32-bit)..."
curl -sSL "https://boot.ipxe.org/ipxe-i386.efi" -o "$TFTP_DIR/ipxe-i386.efi" || true

# 4. iPXE UEFI ARM64
echo "Downloading ipxe-arm64.efi (UEFI AArch64)..."
curl -sSL "https://boot.ipxe.org/ipxe-arm64.efi" -o "$TFTP_DIR/ipxe-arm64.efi" || true

# 5. Wimboot (For Windows PE / Installer network booting)
echo "Downloading wimboot..."
curl -sSL "https://github.com/ipxe/wimboot/releases/latest/download/wimboot" -o "$TFTP_DIR/wimboot" || true

# 6. Memdisk (Syslinux 32-bit real-mode ISO bootloader)
if [ ! -f "$TFTP_DIR/memdisk" ]; then
    if [ -f "/usr/lib/syslinux/memdisk" ]; then
        cp "/usr/lib/syslinux/memdisk" "$TFTP_DIR/memdisk"
    elif [ -f "/usr/share/syslinux/memdisk" ]; then
        cp "/usr/share/syslinux/memdisk" "$TFTP_DIR/memdisk"
    else
        echo "Downloading memdisk from kernel.org mirror..."
        curl -sSL "https://mirrors.edge.kernel.org/pub/linux/utils/boot/syslinux/syslinux-6.03.tar.gz" | tar -xz -C "$TFTP_DIR" --wildcards '*/memdisk' --strip-components=2 || true
    fi
fi

chmod -R 644 "$TFTP_DIR"/* || true
echo "=== Bootloaders download finished successfully ==="
