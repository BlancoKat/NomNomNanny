# Contributing to NomNom Nanny

Thank you for your interest in contributing to NomNom Nanny! This is a personal project that is being prepared for the FOSS community.

## How to Contribute

### Reporting Issues
- Use the GitHub issue tracker.
- Provide as much detail as possible: OS, steps to reproduce, screenshots if relevant.
- For bugs in the USDA integration, please include the exact food description if possible.

### Development Setup
See the main [README.md](README.md) for prerequisites and how to run the app in development mode.

### Pull Requests
- Fork the repository and create a branch for your changes.
- Make sure `npm run tauri build` succeeds on your platform (or at least `npm run check`).
- Keep changes focused and include a clear description.
- We are still early, so major architectural changes should be discussed first via an issue.

### Code Style
- Follow existing patterns.
- Rust: `cargo fmt` and `cargo clippy`.
- TypeScript/Svelte: Run `npm run check`.

## License
By contributing, you agree that your contributions will be licensed under the MIT License.

## Questions?
Feel free to open an issue or discussion.
