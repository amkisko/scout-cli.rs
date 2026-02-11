# SECURITY

## Reporting a Vulnerability

**Do NOT** open a public GitHub issue for security vulnerabilities.

Email security details to: **security@kiskolabs.com**

Include: description, steps to reproduce, potential impact, and suggested fix (if available).

Alternatively, report confidentially via **GitHub**: use the repository's *Security* tab → *Report a vulnerability*, or open a [private security advisory](https://github.com/amkisko/scout-cli.rs/security/advisories/new).

### Response Timeline

- We will acknowledge receipt of your report
- We will provide an initial assessment
- We will keep you informed of our progress and resolution timeline

### Disclosure Policy

- We will work with you to understand and resolve the issue
- We will credit you for the discovery (unless you prefer to remain anonymous)
- We will publish a security advisory after the vulnerability is patched
- We will coordinate public disclosure with you

## Automation Security

* **Context Isolation:** It is strictly forbidden to include production credentials, API keys, or Personally Identifiable Information (PII) in prompts sent to third-party LLMs or automation services.

* **Supply Chain:** All automated dependencies must be verified.

## API key handling (scout-cli)

**Plain-text API keys are not supported.** The CLI does not accept the API key via environment variables (`API_KEY`, `SCOUT_APM_API_KEY`) or `--api-key`. You must use a secret backend (1Password, Bitwarden, or KeePassXC) so the key is never on the command line, in shell history, or in process lists.

## Supported versions

We release patches for the latest minor version. Security updates are prioritized for the current stable release.
