# GitHub Actions Workflows

This directory contains the CI/CD pipeline for the Hyperliquid Alerts bot.

## Workflow: `ci.yml`

**Triggers:** push and pull requests targeting `main`.

**Jobs:**

1. **`test`** (runs on GitHub-hosted `ubuntu-latest`)
   - `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`
   - Runs for every push and pull request.

2. **`deploy`** (runs on a **self-hosted runner** installed on the production VM)
   - Only runs for pushes to `main` (never for `pull_request` events, so
     untrusted forks can never execute code on the production host).
   - Builds the release binary directly on the target machine (no
     cross-compilation, no artifact transfer, no SSH keys/secrets needed).
   - Installs the binary to `~/apps/hyperliquid-alerts/hyperliquid-alerts` and
     restarts the `hyperliquid-alerts.service` systemd user unit.

## Production host setup (one-time, already done)

The production VM runs everything as the unprivileged `agent` Linux user
(no root, no passwordless sudo). Two persistent `systemd --user` services:

- `github-runner.service` — the self-hosted Actions runner
  (`~/actions-runner`), registered to this repo with labels
  `self-hosted, linux, x64, hl-alerts-vm`.
- `hyperliquid-alerts.service` — the bot itself, working directory
  `~/apps/hyperliquid-alerts` (holds the binary, `alerts.db`, and `.env`
  with `TELEGRAM_BOT_TOKEN` — never committed to git).

`loginctl enable-linger agent` is set so both user services start on boot
and keep running without an active login session.

Useful commands on the VM:

```bash
# Runner
systemctl --user status github-runner.service
journalctl --user -u github-runner.service -f

# Bot
systemctl --user status hyperliquid-alerts.service
journalctl --user -u hyperliquid-alerts.service -f
systemctl --user restart hyperliquid-alerts.service
```

If the runner ever needs re-registering (e.g. moved to a new host), generate
a new token from the repo's **Settings → Actions → Runners → New
self-hosted runner** page and re-run `./config.sh` in `~/actions-runner`.
