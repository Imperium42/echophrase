# Contributing to Echophrase

Thanks for helping! Echophrase is **community-maintained**: there is no support team and no
guaranteed response time. Issues and PRs are reviewed on a best-effort basis. Clear, small,
well-tested PRs are the ones that get merged.

By participating you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md).
Security problems go through [private reporting](SECURITY.md), not public issues.

## Setup

See [Building from source](README.md#build-from-source). In short:

```bash
git clone https://github.com/<you>/echophrase.git
cd echophrase
bun install
bun run tauri:dev
```

Use **Bun** for JavaScript tooling (not npm/yarn). Architecture notes, conventions and gotchas
are in [CLAUDE.md](CLAUDE.md), which is written for human contributors and AI coding agents alike.

## Before you open a PR

1. **Open an issue first** for anything bigger than a bug fix, so we can agree on the approach.
2. Branch from `main` and keep the change focused.
3. Run the checks CI runs:
   ```bash
   bun run check && bun run lint && bun run test:run
   cargo clippy --workspace --all-targets --no-default-features -- -D warnings
   cargo test --workspace --no-default-features
   ```
4. Add tests for new behavior and bug fixes.
5. **User-facing strings** go in all three locales: `src/lib/locales/en.json`, `zh-CN.json`,
   `zh-TW.json` (a parity test fails otherwise). Use `t('key')` in templates.
6. **User-visible changes** get a short, plain-language entry under `## [Unreleased]` in
   [CHANGELOG.md](CHANGELOG.md).
7. **New source files** start with the SPDX header (`reuse lint` must pass):
   ```
   // SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
   // SPDX-License-Identifier: MPL-2.0
   ```
8. **New dependencies** must be permissively licensed. Rust crates must match the
   `accepted` list in `src-tauri/about.toml` (no GPL/AGPL/LGPL).

## Style

- Conventional commits for commits and PR titles: `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`.
- Rust: `cargo fmt`, idiomatic error handling, no `unwrap()` on user-reachable paths.
- Frontend: TypeScript + Svelte 5, Tailwind for styling, `bun run format` (Prettier).
- Use plain hyphens (`-`), not em or en dashes, in code, comments and copy.

## Platforms

- **Windows and macOS (Apple Silicon)** have official builds.
- **Linux** is community-supported and built from source. Linux fixes and packaging PRs are
  very welcome; please test them on a real Linux machine.

## License

By contributing, you agree that your contributions are licensed under the
[Mozilla Public License 2.0](LICENSE), the project's license. The "Echophrase" name and logo
are not covered by the code license; see [TRADEMARKS.md](TRADEMARKS.md).
