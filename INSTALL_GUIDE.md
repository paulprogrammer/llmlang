# llmlang Installation Guide

Once you have built the compiler following the [Build Guide](BUILD_GUIDE.md), you can deploy the toolchain using one of the three standard mechanisms below. Select the option that best fits your environment.

## Option 1: Full System Availability
Installs `llmlang`, `llm-clang` (driver), and `llm-mcp` (server) globally. Ideal for workstations where `llmlang` is a primary tool.

```bash
# Deploy to standard binary path
sudo cp target/release/llmlang /usr/local/bin/
sudo cp target/release/llm-mcp /usr/local/bin/
sudo cp llm-clang /usr/local/bin/

# Set executable permissions
sudo chmod +x /usr/local/bin/llmlang /usr/local/bin/llm-mcp /usr/local/bin/llm-clang
```

## Option 2: User-Level (Standard PATH)
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

## Option 3: Local Project Context
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

## Verification
Confirm the installation was successful by querying the compiler version:

```bash
llmlang --version
```
