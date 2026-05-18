# llmlang Build Guide

This guide details how to build the `llmlang` compiler from source.

## Prerequisites

1.  **Rust Toolchain:** You need the latest stable version of Rust and Cargo. Install via [rustup.rs](https://rustup.rs/).
2.  **LLVM 22:** The compiler targets LLVM 22. Ensure you have the LLVM development headers installed on your system.
    *   **Fedora/RedHat:** `sudo dnf install llvm-devel clang`
    *   **Note:** If your system uses a different version, you may need to update the `features` in `Cargo.toml`.

## Building the Compiler

To compile the `llmlang` binary:

```bash
cargo build --release
```

The resulting binary will be located at `target/release/llmlang`.

## Running Tests

To verify the build and run the integrated test suite:

```bash
cargo test
```

## Troubleshooting

- **Linking Errors:** If the build fails to find LLVM, ensure `llvm-config` is in your PATH or set the `LLVM_SYS_221_PREFIX` environment variable to point to your LLVM installation.
- **Dependency Issues:** Run `cargo update` if there are conflicts with the `inkwell` git dependency.
