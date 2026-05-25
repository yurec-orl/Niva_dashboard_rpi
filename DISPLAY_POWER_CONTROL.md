# Display Power Control via USB

## Goal
Prevent the greyish glow and color artifacts on the display during Pi startup by cutting USB power to the display until the dashboard application is ready.

## Hardware Setup
- **Display video**: HDMI
- **Display power**: USB (connected to a Pi USB port)
- **Control tool**: `uhubctl`

## Implementation Plan

### Step 1 — Install uhubctl
```bash
sudo apt install uhubctl
```

Verify that your Pi and USB port support software power switching:
```bash
sudo uhubctl
```
Note the hub location (`-l`) and port number (`-p`) values for the display's USB port.

### Step 2 — Create a service to cut display power during early boot
Create `/etc/systemd/system/display-power-off.service`:
```ini
[Unit]
Description=Cut display USB power during boot
DefaultDependencies=no
Before=sysinit.target

[Service]
Type=oneshot
ExecStart=/usr/sbin/uhubctl -l 1-1 -p 2 -a 0
RemainAfterExit=yes

[Install]
WantedBy=sysinit.target
```

### Step 3 — Create a service to restore display power when dashboard is ready
Create `/etc/systemd/system/display-power-on.service`:
```ini
[Unit]
Description=Power on display USB port after dashboard starts
After=niva-dashboard.service

[Service]
Type=oneshot
ExecStart=/usr/sbin/uhubctl -l 1-1 -p 2 -a 1

[Install]
WantedBy=multi-user.target
```

> Replace `-l 1-1 -p 2` with the actual hub location and port number from `uhubctl` output.

### Step 4 — Enable both services
```bash
sudo systemctl enable display-power-off.service
sudo systemctl enable display-power-on.service
```

## Notes
- Display data (video) is carried over HDMI — USB is power only
- Cutting USB power fully removes power from the display with no data side effects
- The `uhubctl` command requires root/sudo
