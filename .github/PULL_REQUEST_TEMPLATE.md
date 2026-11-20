## What and why

<!-- What does this change, and why? Link the issue if there is one: "Fixes #123". -->

## How it was tested

<!-- Platform(s), commands run, manual steps. -->

## Checklist

- [ ] `bun run check`, `bun run lint` and `bun run test:run` pass
- [ ] `cargo clippy --workspace --all-targets --no-default-features -- -D warnings` and `cargo test --workspace --no-default-features` pass
- [ ] User-facing strings are added to all three locales (`en`, `zh-CN`, `zh-TW`)
- [ ] User-visible changes have a short entry under `## [Unreleased]` in `CHANGELOG.md`
- [ ] New source files carry the SPDX header (`reuse lint` passes)
