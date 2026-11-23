# Security Policy

## Supported Versions

Security fixes are released on the latest stable minor series:

| Version | Supported |
| --- | --- |
| `1.3.x` | Yes |
| `< 1.3.0` | No |

## Reporting a Vulnerability

**Please do not report security vulnerabilities through public GitHub issues or pull requests.**

Instead, report them privately using GitHub's Private Vulnerability Reporting:

- [**Submit a Private Security Advisory**](https://github.com/howlil/otto-os/security/advisories/new)

Please include as much of the following as possible:

- Affected Otto OS version and Windows 11 build number (`winver`).
- Component involved (for example: in-app updater, installer, keyboard hook, Focus Shield UIA inspection, Background Cleanup, IPC/window handling, or landing API).
- Step-by-step reproduction instructions or proof of concept.
- Potential impact and any suggested mitigation.

### Response Expectations

- **Initial acknowledgment**: within 72 hours.
- **Triage & assessment**: within 7 days, including severity classification and expected remediation timeline.
- **Coordinated disclosure**: we aim to publish a fix and security advisory before public disclosure and will credit reporters unless anonymity is requested.

## Security Scope & Trust Boundaries

Because Otto OS is a native Windows desktop shell running in the user's session, the following boundaries are explicitly in scope for security reports:

- **In-app software updates (`crates/platform/src/control_center/updates.rs`)**: HTTPS downgrade/redirect attacks, URL allowlist bypasses, checksum verification flaws, path traversal during staging, or installer execution flaws.
- **Input & privacy boundaries**: accidental logging or disk persistence of clipboard contents, keystroke characters, Quick Capture tasks, or browser URLs beyond documented local files.
- **Process control (`Background Cleanup` & `Process Audit`)**: bypassing `SystemProtected` classification to throttle or terminate protected Windows/system/foreground processes.
- **Local persistence & recovery**: symlink/path-handling bugs in `%LOCALAPPDATA%\otto-os\` atomic file writes or installer `--quit` / `--restore-shell` flows.
- **External HTTP parsing**: malformed responses from GitHub Releases API or opt-in LRCLIB lyrics endpoints causing crashes, unbounded allocations, or memory safety issues.

### Out of Scope

- Windows Defender SmartScreen warnings while public binaries remain unsigned.
- Elevated (Administrator) application windows not receiving Otto Window Controls or Genie minimize — Windows UIPI enforces this boundary intentionally when Otto OS runs unprivileged.
- Issues requiring local Administrator privileges or physical access to an already unlocked session.
