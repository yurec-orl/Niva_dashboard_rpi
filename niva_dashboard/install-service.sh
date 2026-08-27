#!/bin/bash
# Installs (or reinstalls) the niva-dashboard systemd service.
# Run once on the Pi; after that use `sudo systemctl restart niva-dashboard`
# to pick up a new binary.
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SERVICE_SRC="$SCRIPT_DIR/niva-dashboard.service"
SERVICE_DST="/etc/systemd/system/niva-dashboard.service"
RUN_SCRIPT="$SCRIPT_DIR/niva-dashboard-run.sh"

# getty@tty1 is left running (autologin, unchanged) -- the dashboard doesn't need TTY1
# ownership. DRM/GPIO/I2C/serial access comes from `user`'s group membership
# (video/render/gpio/i2c/dialout), not from an active login session on that seat, so the
# service and an idle tty1 shell coexist fine; whichever process actually calls
# drmModeSetCrtc (Plymouth, then the dashboard once Plymouth releases it) owns the
# display, independent of tty ownership.
chmod +x "$RUN_SCRIPT"

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
