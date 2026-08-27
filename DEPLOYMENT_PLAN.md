# Deployment Plan

## Problem

Getting a fresh Raspberry Pi from a flashed SD card to a running dashboard
currently depends on a series of manual, undocumented steps performed
directly on this one Pi's filesystem. None of them are tracked in this repo,
so the only record of "how this machine got into its current working state"
is the machine itself. If this SD card dies, or a second unit needs to be
built, that knowledge has to be reconstructed from memory.

## Current state inventory

Surveyed directly on this Pi on 2026-07-29:

| #  | Item                                            | Where it lives now                                                               | In repo? | Scripted? |
|----|-------------------------------------------------|----------------------------------------------------------------------------------|----------|-----------|
| 1  | Dashboard autostart + crash-restart loop        | `niva-dashboard.service` (systemd unit) running `niva_dashboard/niva-dashboard-run.sh` | Yes      | Yes       |
| 2  | TTY1 autologin override                         | `/etc/systemd/system/getty@tty1.service.d/autologin.conf` -- kept as an idle login shell for local debugging; the service doesn't need TTY1 ownership, see decision below | No       | No        |
| 3  | `install-service.sh` + `niva-dashboard.service` | `niva_dashboard/install-service.sh` + `niva_dashboard/niva-dashboard.service`    | Yes      | Yes       |
| 4  | ADC udev rule                                   | `/etc/udev/rules.d/99-niva-adc.rules`                                            | No       | No        |
| 5  | GPS udev rule                                   | `/etc/udev/rules.d/99-niva-gps.rules`                                            | No       | No        |
| 6  | uhubctl sudoers entry                           | `/etc/sudoers.d/niva-uhubctl`                                                    | No       | No        |
| 7  | earlyoom package + config                       | apt package + `/etc/default/earlyoom`                                            | No       | No        |
| 8  | Boot-time service disables (6 units)            | applied via `systemctl disable`, described in `/home/user/boot-optimizations.md` | No (doc lives outside repo) | No |
| 9  | I2C bus enablement                              | `dtparam=i2c_arm=on`                                                             | No       | No        |
| 10 | `/etc/niva_dashboard/ui_style.json`             | `/etc/niva_dashboard/`                                                           | No       | Orphaned — load call is commented out at `main.rs:523`, so this file currently does nothing |
| 11 | Hardcoded absolute font/style paths             | baked into `niva_dashboard/src/graphics/ui_style.rs` (`/home/user/Work/Niva_Dashboard_Rpi/...`) | In repo, but wrong — ties to one dev machine's clone path | N/A |
| 12 | Boot splash (Plymouth)                          | apt packages `plymouth`/`plymouth-themes` + `/etc/plymouth/plymouthd.conf` + custom theme at `/usr/share/plymouth/themes/niva/` (`niva.plymouth`, `niva.script`, `splash.png`) + `splash plymouth.ignore-serial-consoles` on `cmdline.txt` + two masked units + a `getty@tty1` drop-in + a line in the `.profile` autostart block, described in `/home/user/splash-screen-fix.md` | No (doc lives outside repo) | No |
| 13 | Old `fbi`-based splash (`splashscreen.service`) | unit at `/etc/systemd/system/splashscreen.service`, image at `/opt/splash.png` | No | Superseded by item 12, currently `disabled` (not removed) — dead weight that should be deleted, not carried into `deploy/` |

Two more things worth flagging that fell out of this survey rather than being
part of the original ask:

- **Item 3 is resolved** (2026-08-27) — see "Decision: autostart mechanism"
  below. `install-service.sh` now installs a real `niva-dashboard.service`
  and the dashboard runs under systemd on this Pi.
- **Item 4's udev rule pins one specific STM32 unit's serial number**
  (`ATTRS{serial}=="8D8E416F4957"`), which CLAUDE.md's ADC section doesn't
  mention (it only calls out that the *GPS* rule can't match on serial).
  Swapping the physical ADC module for a spare will silently stop
  `/dev/niva_adc` from appearing until someone notices and edits the rule.
- **Item 9's `i2c_arm_baudrate=400000` is a BNO085 requirement, not a UPS HAT
  one** — the UPS HAT (INA219 + onboard MCU, see
  `BNO085_SHTP_PROTOCOL_RESEARCH.md`) works fine at the Pi's default 100 kHz.
  The BNO085 is meaningfully less error-prone at 400 kHz but still not fully
  stable at either speed (same doc). Since the baudrate applies to the whole
  shared `i2c-1` bus, this ties an otherwise-optional sensor's requirement to
  a config line that also affects the UPS HAT's communication — worth a
  one-line comment in `02-boot-config.sh` explaining why, so a future reader
  doesn't assume it's UPS-related and remove it if the BNO085 is unplugged.
- **Item 12 (Plymouth) has three non-obvious, easy-to-silently-break
  dependencies** — all discovered the hard way (see
  `/home/user/splash-screen-fix.md` for the full debugging trail), each with
  no error message pointing at the actual cause:
  - `cmdline.txt` must include `plymouth.ignore-serial-consoles`. This Pi's
    `console=serial0,115200 console=tty1` setup makes Plymouth's device
    manager detect a serial console and silently force the built-in
    text-only `details` plugin for *every* display, including tty1 —
    `plymouthd` runs, reports success, and never opens a DRM or fb device.
    No log output flags this unless `--debug` is explicitly enabled.
  - `plymouth-quit-wait.service` and `plymouth-quit.service` must both be
    masked (`systemctl mask`, not just `disable`). Left unmasked, the first
    makes `getty@tty1` block on Plymouth quitting — which can't happen here
    since it's `.profile` (launched *by* getty's autologin) that's supposed
    to quit Plymouth, a circular wait. The second is a stock unit that
    auto-quits Plymouth on its own shortly after boot, independent of
    whether the dashboard is actually ready.
  - The `.profile` autostart block's `plymouth quit --retain-splash` call
    **must be `sudo plymouth quit ...`**. `plymouthd`'s control socket is
    root-only; a bare `plymouth` call from the unprivileged autologin shell
    fails silently (non-zero exit, never checked by the script) and leaves
    `plymouthd` holding DRM master forever — `niva_dashboard` then opens the
    DRM device fine but its `drmModeSetCrtc` call fails with `-13`/`EACCES`
    and it silently falls back to invisible off-screen rendering (see
    `graphics/context.rs`'s warn-and-continue path on that error).
  - `getty@tty1` needs a scoped drop-in setting `TTYReset=no` (leaving
    `TTYVHangup=yes` and `TTYVTDisallocate=yes` at their stock values).
    `TTYReset` resets the VT to text mode as part of getty's normal startup,
    which wipes the visible splash within ~2s of boot regardless of whether
    Plymouth itself is still running — independent of the vhangup/DRM-master
    issues above. Disabling *all three* settings (an earlier attempt) breaks
    `agetty`'s ability to reclaim the tty at all when something uncooperative
    (the old `fbi`-based splash, item 13) is still on it; Plymouth is
    VT-handoff-aware enough that only `TTYReset` needs disabling.

## Goal

A new SD card should go from a stock PiOS flash to a fully running dashboard
through one documented, scripted, idempotent path — a `deploy/` directory in
this repo holding every system-level artifact as the source of truth, plus a
single top-level installer script. Anything that genuinely can't be scripted
(physical wiring, an initial `raspi-config` step that has no CLI equivalent)
gets a short manual checklist instead of tribal knowledge.

## Proposed structure

```
deploy/
├── install.sh                              # top-level idempotent installer, runs the below in order
├── 01-system-packages.sh                   # apt: build deps, earlyoom, uhubctl
├── 02-boot-config.sh                       # dtparam=i2c_arm=on + i2c_arm_baudrate=400000; the 6 boot-time systemctl disables
├── 03-udev-rules.sh                        # installs udev/*.rules, runs udevadm control --reload-rules
├── 04-sudoers.sh                           # installs sudoers/niva-uhubctl at mode 0440
├── 05-earlyoom.sh                          # installs earlyoom/earlyoom.default, restarts the service
├── 06-autostart.sh                         # installs systemd/autologin.conf (TTY1 idle debug shell) + niva-dashboard.service
│                                            #   + niva-dashboard-run.sh (wraps install-service.sh's steps)
├── 07-splash-screen.sh                     # apt install plymouth plymouth-themes; installs plymouth/ theme + plymouthd.conf;
│                                            #   appends splash + plymouth.ignore-serial-consoles to cmdline.txt; masks
│                                            #   plymouth-quit-wait/plymouth-quit; installs systemd/no-tty-reset.conf;
│                                            #   disables + deletes the old splashscreen.service/opt/splash.png (item 13);
│                                            #   ends with `update-initramfs -u`
├── udev/
│   ├── 99-niva-adc.rules
│   └── 99-niva-gps.rules
├── sudoers/
│   └── niva-uhubctl
├── earlyoom/
│   └── earlyoom.default
├── systemd/
│   ├── autologin.conf
│   ├── no-tty-reset.conf                   # getty@tty1.service.d drop-in: TTYReset=no
│   ├── niva-dashboard.service
│   └── niva-dashboard-run.sh
└── plymouth/
    ├── plymouthd.conf                      # Theme=niva, Renderer=drm
    └── themes/niva/
        ├── niva.plymouth
        ├── niva.script
        └── splash.png
```

Each numbered script should be safe to re-run (check-before-write, not
blind overwrite where it matters) and print what it changed or skipped.
`install.sh` runs them in order and ends with a summary of what still needs
a reboot to take effect (boot-config changes, autologin).

Boot-time doc: fold `/home/user/boot-optimizations.md`'s content into
`deploy/02-boot-config.sh` (as the executable source of truth) and a shorter
companion `deploy/BOOT_OPTIMIZATIONS.md` (as the rationale/measurements doc,
moved into the repo).

## Decision: autostart mechanism (resolved 2026-08-27)

Two competing mechanisms existed:

- getty@tty1 autologin → `.profile` loop, with specific behavior already
  load-bearing elsewhere — exit code 0 means "quit intentionally, drop to
  shell", exit code 42 means "rebuilt on disk, relaunch without counting it
  as a crash", anything else counts toward a 5-strike crash limit.
  CLAUDE.md's earlyoom section already assumes this exact restart-on-crash
  behavior.
- `install-service.sh` + `niva-dashboard.service` (unit file missing,
  broken).

This plan originally recommended keeping `.profile` and treating the
systemd path as a later nice-to-have (journalctl integration,
`Restart=on-failure`). That turned out to be load-bearing sooner than
expected: investigating why `HeadingFusionSensor`'s persist-on-`Drop`
wasn't saving on a diag-page-triggered reboot (but did save on a plain
dashboard restart) traced back to exactly this. Under the `.profile`
mechanism the dashboard is just an ordinary process launched from a login
shell, not a systemd unit — on `reboot`/`poweroff` it isn't covered by any
unit's ordered stop (SIGTERM + `TimeoutStopSec` grace period); it's only
caught by the terse, fixed-timeout sweep systemd uses for whatever's still
running at the very end of shutdown. A `UIEvent::Restart`-issued `sudo
reboot` races the app's own graceful exit against that sweep's SIGKILL, and
frequently lost — confirmed live by adding a log line to the SIGTERM
handler: it fired, but nothing logged after it, consistent with SIGKILL
arriving before the next event-loop tick.

**Decision: switch to `niva-dashboard.service`.** As a real unit, the
dashboard now gets a proper ordered stop (SIGTERM, `TimeoutStopSec=15`)
on every path — `systemctl stop/restart`, and reboot/poweroff — with no
per-exit-path cleanup code needed in the Rust side. The exit-42 "rebuilt,
not a crash" distinction is preserved by keeping the *exact* restart-loop
logic from `.profile`, moved verbatim into `niva_dashboard/niva-dashboard-run.sh`
(git-tracked, executable), which is the unit's `ExecStart`. The wrapper
script owns exit-code interpretation and always exits 0 itself; systemd's
`Restart=on-failure` is purely a backstop for the script/binary dying in a
way that skips that logic entirely (e.g. OOM-kill), not the primary retry
mechanism.

Verified on this Pi: `sudo systemctl stop niva-dashboard` drives the same
SIGTERM path a reboot now takes, and the full graceful-exit-to-persisted-
heading cycle completes in ~240ms — comfortably inside the 15s window.

Two things this did *not* need, contrary to an earlier assumption baked
into the old (broken) `install-service.sh`:
- **Disabling getty@tty1.** DRM (`/dev/dri/card*`), GPIO, I2C, and the
  ADC/GPS serial devices are all accessible via `user`'s static group
  membership (`video`/`render`/`gpio`/`i2c`/`dialout`) — there's no
  logind/seat-ACL dependency on this Pi's setup, so the dashboard doesn't
  need to own TTY1 to open them. getty@tty1's autologin is left running
  unchanged, now just an idle debugging shell — `.profile` no longer
  launches anything from it.
- **`PAMName=`/`TTYPath=` on the unit.** Same reasoning — no seat session
  needed. DRM master handoff still works the same way it already did
  (Plymouth releases it via `plymouth quit --retain-splash`, now called
  from the top of `niva-dashboard-run.sh` instead of `.profile`; the
  dashboard acquires it once Plymouth lets go).

`install-service.sh` no longer disables getty@tty1 (that step is gone) and
now installs a real unit file instead of failing on a missing one. `deploy/`
item 3's "Broken" status above is resolved as a side effect.

## Fixing the hardcoded install paths

`ui_style.rs` bakes in `/home/user/Work/Niva_Dashboard_Rpi/...` at over a
dozen call sites (already flagged in CLAUDE.md's TODO list). Pick one fixed
install root (e.g. `/opt/niva_dashboard`, with `fonts/` and config under it)
and either:
- resolve font/style paths at runtime relative to
  `std::env::current_exe()`'s parent, or
- read an install-root override from an env var / small config file, falling
  back to the fixed install root.

This is a prerequisite for `deploy/install.sh` being able to place the repo
anywhere other than this exact developer clone path.

## Phasing

1. **Capture, no behavior change.** Copy the current manual system state
   (udev rules, sudoers entry, earlyoom config, boot-optimizations doc,
   Plymouth theme + `plymouthd.conf` + `splash-screen-fix.md`) into
   `deploy/` as inert reference files. Nothing on this Pi changes yet — this
   just stops the configuration living only on one machine's disk.
2. **Script it.** Write the numbered install scripts wrapping idempotent
   application of the captured files, plus package installs and boot-config
   changes. Run `deploy/install.sh` on *this* Pi and diff against current
   state to confirm it's a no-op.
3. **Fix the hardcoded paths** per above, so the binary isn't tied to one
   clone location.
4. **~~Resolve the install-service.sh / autostart conflict~~ — done.** systemd
   was chosen (see decision above) and is running on this Pi directly via
   `niva_dashboard/install-service.sh`. Still outstanding: fold
   `niva-dashboard.service`/`niva-dashboard-run.sh` into the `deploy/` tree
   alongside the rest of item 1-13's captured files, once that tree exists.
5. **Validate end-to-end on a second SD card** — flash stock PiOS, clone the
   repo, run `deploy/install.sh`, reboot, confirm the dashboard comes up with
   no manual steps beyond flashing and cloning. This is the real test; steps
   1-4 are just groundwork for it.

## Out of scope for this plan

- UPS HAT *physical* wiring/pogo-pin connection — hardware assembly, not
  software provisioning. Worth a one-line checklist item, not a script.
- The `default_style.json` / `ui_style.json` dead-code cleanup already
  tracked in CLAUDE.md's TODO — related (item 10 above touches the same
  file) but a separate cleanup, not a deployment-scripting task.
