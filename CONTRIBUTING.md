# Contributing to DriftGlide

[English](CONTRIBUTING.md) | [Русский](CONTRIBUTING_RU.md)

Thank you for your interest in contributing to **DriftGlide**! We welcome bug reports, feature proposals, documentation improvements, and code contributions.

---

## 📋 Table of Contents

1. [Code of Conduct](#code-of-conduct)
2. [How Can I Contribute?](#how-can-i-contribute)
   - [Reporting Bugs](#reporting-bugs)
   - [Suggesting Enhancements](#suggesting-enhancements)
   - [Pull Requests](#pull-requests)
3. [Development Setup](#development-setup)
4. [Coding Guidelines](#coding-guidelines)
5. [Git Workflow & Commit Messages](#git-workflow--commit-messages)

---

## Code of Conduct

This project and everyone participating in it is governed by the [DriftGlide Code of Conduct](CODE_OF_CONDUCT.md). By participating, you are expected to uphold this code.

---

## How Can I Contribute?

### Reporting Bugs

Before creating a bug report, please check existing GitHub Issues to see if the problem has already been reported.

When filing a bug report, please include:
- A clear, descriptive title.
- Steps to reproduce the behavior.
- Expected vs actual behavior.
- Your desktop environment / Wayland compositor (e.g., driftwm, Sway, Hyprland).
- Relevant log output (run `RUST_LOG=driftglide=debug driftglide`).

### Suggesting Enhancements

Feature requests are welcome! When opening an issue for a feature proposal:
- Clearly explain the problem or motivation.
- Describe the proposed solution or design.
- Consider backwards compatibility and performance impact.

### Pull Requests

1. **Fork the repo** and create your branch from `main`.
2. Ensure the code compiles cleanly:
   ```bash
   cargo check
   cargo build --release
   ```
3. Run formatting and linter checks:
   ```bash
   cargo fmt --check 2>/dev/null || true
   cargo clippy 2>/dev/null || true
   ```
4. If you added code, please add tests or verify behavior interactively under Wayland.
5. Open a Pull Request with a clear description of your changes.

---

## Development Setup

### Prerequisites
- **Rust toolchain** (1.70 or newer)
- Wayland development libraries
- `wl-clipboard` (`wl-copy` and `wl-paste`)
- `fontdue` font dependencies (system fonts like Noto Sans, DejaVu Sans, or Liberation Sans)

### Running from source
```bash
cargo run --release
```

To run with verbose debug logs:
```bash
RUST_LOG=driftglide=debug cargo run
```

---

## Coding Guidelines

- **Idiomatic Rust**: Write clean, idiomatic Rust. Use `match`, `Option`, `Result`, and avoid unhandled panics (`.unwrap()` in production paths).
- **Thread Safety**: Mutex locks should be resilient to poisoning (`.unwrap_or_else(|e| e.into_inner())`).
- **Memory & Buffers**: Avoid redundant allocations in rendering loops. Use `DoubleBufferedShm` / `tiny-skia` buffers efficiently.
- **Wayland Protocol Discipline**: Follow Wayland client conventions. Always check buffer busy states and commit damaged regions properly.
- **Cross-Layout Keyboard**: Keyboard shortcuts should support both English and non-Latin (Cyrillic, etc.) XKB keysyms.

---

## Git Workflow & Commit Messages

We follow Conventional Commits formatting:

```
<type>(<scope>): <short description>
```

Types:
- `feat`: A new feature
- `fix`: A bug fix
- `docs`: Documentation changes
- `style`: Formatting, missing semicolons, etc.
- `refactor`: Code refactoring without behavioral change
- `perf`: Performance improvement
- `test`: Adding or updating tests
- `chore`: Build process, dependencies, auxiliary tools
