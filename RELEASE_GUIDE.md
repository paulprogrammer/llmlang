# llmlang Versioning & Release Guide

## 1. Versioning Strategy: SemVer
`llmlang` follows [Semantic Versioning 2.0.0](https://semver.org/).

- **MAJOR** version for incompatible API changes (e.g., changing AST structure or core tokens).
- **MINOR** version for adding functionality in a backwards-compatible manner (e.g., new operators, new MCP tools).
- **PATCH** version for backwards-compatible bug fixes.

Current Version: **v0.4.0**

## 2. Release Process

The release process is automated via GitHub Actions.

### Step 1: Update Version
Update the `version` field in `Cargo.toml`.

### Step 2: Tag the Release
Create and push a git tag following the `vX.Y.Z` format.

```bash
git tag -a v0.1.1 -m "Release v0.1.1"
git push origin v0.1.1
```

### Step 3: Automated Assets
The GitHub Action will trigger on the new tag and:
1. Build the `llmlang` compiler.
2. Build the `llm-mcp` server.
3. Package the binaries.
4. Create a GitHub Release and attach the binaries as assets.

## 3. Package Management Integration
Binary releases on GitHub are the primary distribution method for `llm-pkg`. When a project references a `# Git` dependency, it will prioritize fetching these release assets.
