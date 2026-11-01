# Security

NiceDB holds the keys to production databases, so security reports get priority.

## Reporting a vulnerability

Please do not open a public issue. Report it privately through GitHub: on the repository page, open **Security**, then **Report a vulnerability** ([direct link](https://github.com/maxib0n/NiceDB/security/advisories/new)).

Include what you can of:

- the version (Settings, About) and the operating system;
- the engine and how the connection is made (TLS, SSH tunnel, proxy), if it matters;
- the steps to reproduce, and what an attacker gains.

Reports are answered in the same advisory. Once a fix is released, the advisory is published with credit to you, unless you prefer otherwise.

## Scope

In scope: the desktop app and its Rust crates, including how they store secrets (system keychain, encrypted `.nicedb` exports), guard writes to production connections, verify TLS and SSH host keys, and check the signature of updates.

Out of scope: vulnerabilities in the database servers themselves, and attacks that need someone who already controls your user account on the machine.

## Supported versions

Only the latest release receives security fixes.
