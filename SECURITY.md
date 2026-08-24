# Security

DESKIO removes files. A bug in it costs someone their data, so security
reports are welcome and taken seriously.

## Reporting

Please report privately rather than opening a public issue:

- GitHub's [private vulnerability reporting](https://github.com/deskio/deskio/security/advisories/new)
- or email **benjamin dot biswas at gmail dot com**

Please include what you did, what happened, and which version and platform.
A proof of concept helps, but a clear description is enough.

## What counts

Anything that could make DESKIO remove, expose or damage something it
should not. Particularly:

- **A path that escapes the safety blocklist** — anything under a protected tree
  that `safety::check_removable` accepts.
- **A matching bug that mis-attributes files** — one app's data offered up when
  another is uninstalled.
- **Anything reached through the elevated path**, which runs as root.
- **A download or update that could come from somewhere other than GitHub.**
- **Data leaving the machine** other than the public version lookups described
  in the README.

## What the app already refuses

For context when judging severity, these are enforced and tested:

- Removal is always a move to the Trash, never a delete.
- The safety layer is checked immediately before each removal, not at scan time,
  and refuses system trees, the home directory itself, and Documents, Desktop
  and Downloads — the single exception being installer files directly in
  Downloads, which is deliberately narrow and covered by tests.
- Paths passed to the privileged helper go through `osascript` **argv** and are
  quoted with `quoted form of`; nothing is interpolated into a shell string.
- The download URL taken from the GitHub API must be exactly `github.com` or
  `objects.githubusercontent.com`, matched so a lookalike host fails. GitHub's
  own redirect to its asset CDN is followed from there.

## Supported versions

Only the latest release. This is a young project, and fixes go forward.
