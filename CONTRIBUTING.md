# Contributing to FerrousPanel

Thank you for your interest in contributing to **FerrousPanel**! As an open-source, high-performance Linux VPS & Server Control Panel powered by a native Rust Actix-Web core, we welcome bug reports, feature requests, documentation improvements, and pull requests.

## 🛠️ Development Setup

### Prerequisites
- **Operating System**: Linux (Ubuntu 22.04 / 24.04 LTS or Debian 11/12 recommended) or macOS
- **Node.js**: v20+ or v22+
- **Rust**: 1.83+ (`rustup default stable`)
- **Git**

### Clone & Install
```bash
git clone https://github.com/Ferrous-panel/Ferrous-Panel.git
cd Ferrous-Panel
npm install
```

### Running Locally
```bash
# Compiles the Rust backend core and launches the development server on port 3000
npm run dev
```

### Type Checking & Linting
```bash
npm run lint
```

## 📋 Pull Request Guidelines

1. **Branch Naming**: Use descriptive branch names: `feature/your-feature`, `fix/your-fix`, or `docs/improvement`.
2. **Commit Messages**: Write concise, conventional commit messages (`feat: ...`, `fix: ...`, `refactor: ...`).
3. **Rust Core Code**: Format all Rust code using `cargo fmt` and verify with `cargo check` inside `Ferrous-panel` and `rust-backend`.
4. **TypeScript / Frontend**: Maintain strict TypeScript typing (`npm run lint` must pass with 0 errors).
5. **Security First**: Ensure all privileged operations remain strictly guarded and sanitized through the compiled Rust binary gateway.

## 🐛 Reporting Bugs

Please open an issue on [GitHub Issues](https://github.com/Ferrous-panel/Ferrous-Panel/issues) with:
- Operating system and version (e.g., Ubuntu 24.04 LTS)
- Steps to reproduce
- Expected vs actual behavior
- Terminal logs or browser console screenshots
