# Security

## Reporting a vulnerability

Please report security problems privately, through GitHub's
[private vulnerability reporting](https://github.com/Curlybear/Gorgon/security/advisories/new),
not in a public issue. Say what is affected, how to reproduce it, and the
Gorgon version (`gorgon --version`) or commit.

You'll get an answer as soon as possible. Fixes go into a new release, and
the advisory is published once people can update.

## Supported versions

Only the latest release gets security fixes.

## Checking a download

Each release has a `SHA256SUMS` file. After downloading:

```sh
sha256sum -c --ignore-missing SHA256SUMS
```

Releases also carry a build provenance attestation, which the GitHub CLI can check:

```sh
gh attestation verify Gorgon-*.AppImage --repo Curlybear/Gorgon
```
