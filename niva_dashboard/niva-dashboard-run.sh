#!/bin/bash
# Runs the Niva Dashboard binary in a restart loop, relaunching immediately on a
# rebuilt binary or an explicit restart request without counting it as a crash, and
# giving up after repeated crashes. This is niva-dashboard.service's ExecStart --
# moved here (verbatim restart-loop logic) from the old ~/.profile autostart block
# when the dashboard was switched from an autologin-shell launch to a proper systemd
# service, so that reboot/poweroff give the dashboard a real SIGTERM+timeout grace
# window instead of being swept up in the unmanaged-process shutdown sweep (see
# HeadingFusionSensor's persist-on-Drop, which depends on that window to save the
# last known heading).
#
# systemd's Restart=on-failure (see the unit) is a backstop for this script itself
# dying unexpectedly (e.g. OOM-killed) -- the exit-code-driven relaunch/give-up
# decisions below are handled here, not at the systemd level, so this script exits 0
# in both the deliberate-quit and gave-up-after-crashes cases.

DASHBOARD_DIR=/home/user/Work/Niva_Dashboard_Rpi/Niva_dashboard_rpi/niva_dashboard
DASHBOARD_BIN=$DASHBOARD_DIR/target/release/niva_dashboard
cd "$DASHBOARD_DIR" || exit 1

# Release the boot splash right before taking over the display -- retain-splash keeps
# its last frame frozen on screen so there's no gap until the dashboard's first frame.
# Must be sudo: plymouthd's control socket is root-only.
sudo plymouth quit --retain-splash

MAX_RESTARTS=5
restart_count=0
while true; do
    "$DASHBOARD_BIN"
    status=$?
    # Exit code 0 means the dashboard quit intentionally (e.g. 'q' pressed for
    # debugging) -- stop the loop rather than relaunching.
    # Exit code 42 means it wants an immediate relaunch -- either it detected a fresh
    # build on disk, or a restart was requested via SIGUSR1 (see restart_dashboard.sh)
    # -- relaunch right away without touching the crash count.
    # Any other exit code means it crashed/errored, so restart it after a short
    # delay, up to MAX_RESTARTS times before giving up.
    if [ "$status" -eq 0 ]; then
        break
    elif [ "$status" -eq 42 ]; then
        echo "Niva Dashboard restarting..."
        restart_count=0
        continue
    fi
    restart_count=$((restart_count + 1))
    if [ "$restart_count" -ge "$MAX_RESTARTS" ]; then
        echo "Niva Dashboard crashed $restart_count times in a row, giving up."
        break
    fi
    sleep 5
done
