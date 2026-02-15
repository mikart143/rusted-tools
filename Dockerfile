# ---------- builder ----------
FROM almalinux/9-minimal:9.7 AS builder

WORKDIR /app

# Install build dependencies
RUN microdnf install -y \
    gcc \
    gcc-c++ \
    make \
    openssl-devel \
    pkg-config \
    && microdnf clean all

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src

# Build for release
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release && \
    cp /app/target/release/rusted-tools /app/rusted-tools


# ---------- final ----------
FROM almalinux/9-minimal:9.7 AS final

# Capture platform information
ARG TARGETPLATFORM
ARG TARGETOS
ARG TARGETARCH

# Copy the compiled binary
COPY --from=builder /app/rusted-tools /usr/local/bin/rusted-tools

# Install Node.js LTS, Python, and Docker (for dind)
RUN microdnf install -y \
    nodejs \
    python3 \
    docker \
    iptables \
    fuse-overlayfs \
    && microdnf clean all

# Copy uv from the official uv image
COPY --from=ghcr.io/astral-sh/uv:0.10.2 /uv /uvx /bin/

# Add entrypoint to start dockerd
COPY docker/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

# Create config directory
RUN mkdir -p /etc/rusted-tools

# Use non-root user
USER 65532:65532

# Expose default port
EXPOSE 3000

# Run the binary
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
CMD ["/usr/local/bin/rusted-tools", "--config", "/etc/rusted-tools/config.toml"]
