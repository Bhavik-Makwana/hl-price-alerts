# GitHub Actions Workflows

This directory contains GitHub Actions workflows for building and deploying the Hyperliquid Alerts Rust application.

## Workflows

### 1. Build and Release (`build-and-release.yml`)

**Triggers:**
- Push to version tags (e.g., `v1.0.0`)
- Manual workflow dispatch

**What it does:**
- Cross-compiles the Rust binary for Linux (x86_64-unknown-linux-gnu)
- Strips the binary to reduce size
- Creates a tar.gz archive
- Uploads artifacts for download
- Creates a GitHub release (on tag push)

**Usage:**
```bash
# Create and push a version tag
git tag v1.0.0
git push origin v1.0.0

# Or trigger manually from GitHub Actions tab
```

### 2. Deploy to Ubuntu (`deploy-to-ubuntu.yml`)

**Triggers:**
- After successful build workflow
- Manual workflow dispatch

**What it does:**
- Downloads the built binary
- Uploads to Ubuntu server via SSH
- Sets up systemd service
- Starts and enables the service

**Required Secrets:**
- `SSH_PRIVATE_KEY`: Private SSH key for server access

**Manual Inputs:**
- `server_host`: Ubuntu server hostname or IP
- `server_user`: SSH username (default: ubuntu)
- `deploy_path`: Deployment path (default: /opt/hyperliquid-alerts)

### 3. Simple Deploy (`simple-deploy.yml`)

**Triggers:**
- Manual workflow dispatch only

**What it does:**
- Downloads the built binary
- Uploads to Ubuntu server via SSH
- Sets basic permissions

**Required Secrets:**
- `SSH_PRIVATE_KEY`: Private SSH key for server access

**Manual Inputs:**
- `server_host`: Ubuntu server hostname or IP
- `server_user`: SSH username (default: ubuntu)
- `deploy_path`: Deployment path (default: /home/ubuntu/hyperliquid-alerts)

## Setup Instructions

### 1. Build and Release Setup

No additional setup required. The workflow will automatically:
- Install Rust toolchain
- Install cross-compilation dependencies
- Build the binary for Linux
- Create releases on tag push

### 2. Deployment Setup

#### Option A: Full Deployment with Systemd

1. **Generate SSH Key Pair:**
   ```bash
   ssh-keygen -t ed25519 -C "github-actions" -f ~/.ssh/github_actions_key
   ```

2. **Add Public Key to Ubuntu Server:**
   ```bash
   # On your Ubuntu server
   cat ~/.ssh/github_actions_key.pub >> ~/.ssh/authorized_keys
   chmod 600 ~/.ssh/authorized_keys
   ```

3. **Add Private Key to GitHub Secrets:**
   - Go to your repository → Settings → Secrets and variables → Actions
   - Add new secret: `SSH_PRIVATE_KEY`
   - Value: Contents of `~/.ssh/github_actions_key` (private key)

4. **Test Deployment:**
   - Go to Actions tab → "Deploy to Ubuntu" → Run workflow
   - Provide your server details

#### Option B: Simple File Upload

1. **Setup SSH access** (same as Option A)
2. **Run Simple Deploy workflow** with your server details

### 3. Running the Application

After deployment, you can run the application:

```bash
# For simple deployment
cd /home/ubuntu/hyperliquid-alerts
./hyperliquid-alerts

# For systemd deployment
sudo systemctl start hyperliquid-alerts
sudo systemctl status hyperliquid-alerts
sudo journalctl -u hyperliquid-alerts -f
```

## Environment Variables

The application expects these environment variables (create a `.env` file):

```bash
# Telegram Bot Token
TELEGRAM_BOT_TOKEN=your_bot_token

# Database path
DATABASE_URL=alerts.db

# Logging level
RUST_LOG=info
```

## Troubleshooting

### Build Issues
- Check that your `Cargo.toml` has the correct target configuration
- Ensure all dependencies are available for the target platform

### Deployment Issues
- Verify SSH key is correctly added to GitHub secrets
- Check that the server user has appropriate permissions
- Ensure the server is accessible from GitHub Actions runners

### Runtime Issues
- Check application logs: `sudo journalctl -u hyperliquid-alerts -f`
- Verify environment variables are set correctly
- Ensure the database file is accessible and writable
