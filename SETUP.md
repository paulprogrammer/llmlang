# llmlang Setup Guide

## 1. Prerequisites

1.  **Rust Toolchain:** You need the latest stable version of Rust and Cargo. Install via [rustup.rs](https://rustup.rs/).
2.  **LLVM 22:** The compiler targets LLVM 22. Ensure you have the LLVM development headers installed on your system.
    *   **Fedora/RedHat:** `sudo dnf install llvm-devel clang`
    *   **Note:** If your system uses a different version, you may need to update the `features` in `Cargo.toml`.

## 2. Building from Source

To compile the `llmlang` binary and set up the driver:

```bash
cargo build --release
chmod +x ./llm-clang
```

The resulting binary will be located at `target/release/llmlang`, and the wrapper `llm-clang` can be used for end-to-end builds.

## 3. Running Tests

To verify the build and run the integrated test suite:

```bash
cargo test
```

## 4. Installation

Select the option that best fits your environment.

### Option 1: Full System Availability
Installs `llmlang`, `llm-clang` (driver), and `llm-mcp` (server) globally. Ideal for workstations where `llmlang` is a primary tool.

```bash
# Deploy to standard binary path
sudo cp target/release/llmlang /usr/local/bin/
sudo cp target/release/llm-mcp /usr/local/bin/
sudo cp llm-clang /usr/local/bin/

# Set executable permissions
sudo chmod +x /usr/local/bin/llmlang /usr/local/bin/llm-mcp /usr/local/bin/llm-clang
```

### Option 2: User-Level (Standard PATH)
Deployment for the current user only. Prevents cluttering system directories and avoids `sudo`. Ensure `~/bin` or `~/.local/bin` is in your `$PATH`.

```bash
# Create local bin if required
mkdir -p ~/bin

# Deploy tools
cp target/release/llmlang ~/bin/
cp target/release/llm-mcp ~/bin/
cp llm-clang ~/bin/

# Set executable permissions
chmod +x ~/bin/llmlang ~/bin/llm-mcp ~/bin/llm-clang

# Note: Ensure ~/bin is exported in your .bashrc or .zshrc
```

### Option 3: Local Project Context
Encapsulates the toolchain within the project directory. Use this for CI pipelines, container builds, or when testing specific compiler versions against local source.

```bash
# Create project-local bin
mkdir -p ./bin

# Deploy tools
cp target/release/llmlang ./bin/
cp target/release/llm-mcp ./bin/
cp llm-clang ./bin/

# Set executable permissions
chmod +x ./bin/llmlang ./bin/llm-mcp ./bin/llm-clang

# Execution example:
# ./bin/llm-clang examples/hello.llm
```

## 5. Verification

Confirm the installation was successful by querying the compiler version:

```bash
llmlang --version
```

## 6. Troubleshooting

- **Linking Errors:** If the build fails to find LLVM, ensure `llvm-config` is in your PATH or set the `LLVM_SYS_221_PREFIX` environment variable to point to your LLVM installation.
- **Dependency Issues:** Run `cargo update` if there are conflicts with the `inkwell` git dependency.
