#!/bin/bash
# alpine_musl_test.sh - Musl build and integration test in Podman/Alpine

set -e

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$DIR"

echo "Building and running llmlang compiler in Alpine musl environment..."

# Write temporary Containerfile.alpine
cat << 'EOF' > Containerfile.alpine
FROM alpine:edge

# Install build dependencies
RUN apk add --no-cache \
    rust \
    cargo \
    llvm22-dev \
    clang-dev \
    clang \
    musl-dev \
    curl-dev \
    mbedtls2-dev \
    sqlite-dev \
    bash \
    zstd-dev \
    libffi-dev \
    gcc \
    g++ \
    make \
    git \
    python3

# Set environment variables for LLVM 22
ENV PATH="/usr/lib/llvm22/bin:${PATH}"
ENV LLVM_SYS_221_PREFIX="/usr/lib/llvm22"

WORKDIR /workspace
COPY . .

# Ensure no host-built object files or target artifacts are reused
RUN rm -rf target

# Run cargo test
RUN cargo test --verbose

# Run self-hosted tests
RUN ./llm-test
EOF

cleanup() {
    echo "Cleaning up temporary container files..."
    rm -f Containerfile.alpine
}
trap cleanup EXIT

# Build the container using podman
podman build -f Containerfile.alpine -t llmlang-alpine-test .

echo "Alpine musl build and test suite succeeded!"
