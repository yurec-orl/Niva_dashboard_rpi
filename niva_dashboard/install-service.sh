#!/bin/bash
# Installs (or reinstalls) the niva-dashboard systemd service.
# Run once on the Pi; after that use `sudo systemctl restart niva-dashboard`
# to pick up a new binary.
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SERVICE_SRC="$SCRIPT_DIR/niva-dashboard.service"
SERVICE_DST="/etc/systemd/system/niva-dashboard.service"

# The dashboard takes exclusive ownership of TTY1; the login prompt must be removed.
echo "Disabling getty on TTY1..."
sudo systemctl disable --now getty@tty1.service 2>/dev/null || true

sudo cp "$SERVICE_SRC" "$SERVICE_DST"
sudo systemctl daemon-reload
sudo systemctl enable niva-dashboard

echo "Service installed and enabled."
echo ""
echo "Useful commands:"
echo "  sudo systemctl start niva-dashboard    – start now"
echo "  sudo systemctl stop niva-dashboard     – stop"
echo "  sudo systemctl restart niva-dashboard  – restart (use after recompile)"
echo "  journalctl -u niva-dashboard -f        – follow live logs (from SSH)"
echo ""
echo "SSH keyboard testing:"
echo "  sudo systemctl stop niva-dashboard && sudo ./target/release/niva_dashboard"
echo "  (SSH PTY counts as a TTY, crossterm raw mode works normally)"
