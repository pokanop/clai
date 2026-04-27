# Security

## Reporting a vulnerability

If you believe you have found a security vulnerability in **clai**, please report it **privately** rather than using the public issue tracker, so we can coordinate a fix before wider disclosure.

- **Preferred:** use [GitHub Security Advisories](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing/privately-reporting-a-security-vulnerability) for this repository ( **Security** → **Report a vulnerability** ) if that feature is enabled for the org/repo.
- **Alternative:** contact the maintainers of [pokanop/clai](https://github.com/pokanop/clai) through a private channel you already use for the project.

Include: affected version or commit, steps to reproduce, and impact. We will treat valid reports seriously and work toward a fix and release where appropriate.

## What to know when using clai

**clai is designed to propose and run shell commands** based on model output, after policy checks. That implies inherent risk if misused.

- **Trust your environment:** only use models and configs you trust. Compromised or adversarial model output could attempt harmful commands; the **policy engine** is meant to block high-risk patterns, but it is not a guarantee against all misuse.
- **Interactive and automation modes** (`dry-run` / `confirm` / `auto`, `--yes`, cloud vs local) change how much you are prompted before execution. **Read** [`README.md`](README.md) for execution modes, exit codes, and migration notes for scripts.
- **Secrets:** do not commit `.env` or API keys. See [`.env.example`](.env.example) for the shape of optional environment variables; prefer OS keychains or secure env injection in production.
- **Dependencies:** pull GGUF files from known Hugging Face repos; verify when the registry provides checksums (`clai models pull --verify` where supported).

This document does not list unfixed issues; for general bugs and feature requests, use public issues when they are **not** security-sensitive.
