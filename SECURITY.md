# Security Policy

## Reporting a vulnerability

**Please don't open a public issue for security problems.**

Report privately through GitHub:
[Security > Report a vulnerability](https://github.com/Imperium42/echophrase/security/advisories/new).
Include the version, OS, reproduction steps and the impact you expect.

Echophrase is community-maintained, so there's no guaranteed response time. Reports are
read and handled on a best-effort basis, and fixed in the next release when confirmed.

## Supported versions

Only the latest release gets security fixes. The app has an in-app updater, so please
update before reporting.

## Scope

Echophrase is a local, offline-first desktop app. Audio is processed on your device and
never uploaded. In scope: the app, its updater, model downloads (integrity checks), and
build/release tooling in this repo. Out of scope: third-party models and their outputs,
and issues that need an already-compromised machine.
