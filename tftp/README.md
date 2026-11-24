# TFTP Network Bootloader Directory

This directory contains the initial stage network bootloader files served over TFTP to clients:

- **`undionly.kpxe`**: Universal Network Device Interface (UNDI) driver bootloader for Legacy BIOS (x86 16-bit).
- **`ipxe.efi`**: UEFI 64-bit (x86_64) iPXE network bootloader.
- **`ipxe-i386.efi`**: UEFI 32-bit iPXE bootloader (for older Intel Atom tablets).
- **`ipxe-arm64.efi`**: UEFI ARM64 iPXE bootloader (for Raspberry Pi 4/5, Ampere Altra, ARM servers).
- **`memdisk`**: Syslinux real-mode RAM disk loader for booting raw floppy and CD-ROM ISO images on Legacy BIOS.
- **`wimboot`**: iPXE Windows PE loader used to boot Windows installation environments over HTTP.

To download or refresh these files on Linux/Docker/Proxmox, run:
```bash
bash get-bootloaders.sh
```
