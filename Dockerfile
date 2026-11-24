# Multi-stage Dockerfile for PxeWebUI
# Stage 1: Build the Rust application
FROM rust:1.80-bullseye AS builder

WORKDIR /build

# Pre-install dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    curl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy dependency manifests
COPY Cargo.toml ./

# Dummy source to build dependencies and cache them
RUN mkdir src static tftp && \
    echo 'fn main() { println!("dummy"); }' > src/main.rs && \
    touch static/index.html && \
    cargo build --release && \
    rm -rf src target/release/deps/pxewebui*

# Copy real source code and static assets
COPY src ./src
COPY static ./static

# Build release binary
RUN cargo build --release

# Stage 2: Runtime environment
FROM debian:bullseye-slim

ENV DEBIAN_FRONTEND=noninteractive

# Install runtime packages: dnsmasq, syslinux (for memdisk), iproute2, curl, ca-certificates
RUN apt-get update && apt-get install -y --no-install-recommends \
    dnsmasq \
    syslinux-common \
    iproute2 \
    curl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary from builder
COPY --from=builder /build/target/release/pxewebui /app/pxewebui

# Copy scripts and TFTP helpers
COPY tftp /app/tftp
COPY docker/entrypoint.sh /app/entrypoint.sh

RUN chmod +x /app/pxewebui /app/entrypoint.sh /app/tftp/get-bootloaders.sh

# Create persistent storage mount points
RUN mkdir -p /data /isos /tftp

# Expose ports:
# 8080/tcp: Web UI & HTTP ISO server
# 69/udp: TFTP bootloader server
# 67/udp, 4011/udp: ProxyDHCP discovery
EXPOSE 8080/tcp 69/udp 67/udp 4011/udp

VOLUME ["/data", "/isos", "/tftp"]

ENTRYPOINT ["/app/entrypoint.sh"]
