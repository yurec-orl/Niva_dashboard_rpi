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
| 1  | Dashboard autostart + crash-restart loop        | `~/.profile` (lines 30-60)                                                       | No       | No        |
| 2  | TTY1 autologin override                         | `/etc/systemd/system/getty@tty1.service.d/autologin.conf`                        | No       | No        |
| 3  | `install-service.sh` + `niva-dashboard.service` | `niva_dashboard/install-service.sh`                                              | Script yes, unit file **missing** | Broken |
| 4  | ADC udev rule                                   | `/etc/udev/rules.d/99-niva-adc.rules`                                            | No       | No        |
| 5  | GPS udev rule                                   | `/etc/udev/rules.d/99-niva-gps.rules`                                            | No       | No        |
| 6  | uhubctl sudoers entry                           | `/etc/sudoers.d/niva-uhubctl`                                                    | No       | No        |
| 7  | earlyoom package + config                       | apt package + `/etc/default/earlyoom`                                            | No       | No        |
| 8  | Boot-time service disables (6 units)            | applied via `systemctl disable`, described in `/home/user/boot-optimizations.md` | No (doc lives outside repo) | No |
| 9  | I2C bus enablement                              | `dtparam=i2c_arm=on`                                                             | No       | No        |
| 10 | `/etc/niva_dashboard/ui_style.json`             | `/etc/niva_dashboard/`                                                           | No       | Orphaned — load call is commented out at `main.rs:523`, so this file currently does nothing |
| 11 | Hardcoded absolute font/style paths             | baked into `niva_dashboard/src/graphics/ui_style.rs` (`/home/user/Work/Niva_Dashboard_Rpi/...`) | In repo, but wrong — ties to one dev machine's clone path | N/A |

Two more things worth flagging that fell out of this survey rather than being
part of the original ask:

- **Item 3 is actively misleading.** `install-service.sh` copies a
  `niva-dashboard.service` file that doesn't exist anywhere in the repo or
  its git history, and this Pi isn't running the dashboard via systemd at
  all — it's running via the `.profile` autologin loop (item 1). Anyone who
  runs `install-service.sh` today gets a hard failure. This needs to be
  either finished or removed, not left as-is.
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
├── 06-autostart.sh                         # installs systemd/autologin.conf + profile.d snippet
├── udev/
│   ├── 99-niva-adc.rules
│   └── 99-niva-gps.rules
├── sudoers/
│   └── niva-uhubctl
├── earlyoom/
│   └── earlyoom.default
├── systemd/
│   └── autologin.conf
└── profile.d/
    └── niva-dashboard-autostart.sh
```

Each numbered script should be safe to re-run (check-before-write, not
blind overwrite where it matters) and print what it changed or skipped.
`install.sh` runs them in order and ends with a summary of what still needs
a reboot to take effect (boot-config changes, autologin).

Boot-time doc: fold `/home/user/boot-optimizations.md`'s content into
`deploy/02-boot-config.sh` (as the executable source of truth) and a shorter
companion `deploy/BOOT_OPTIMIZATIONS.md` (as the rationale/measurements doc,
moved into the repo).

## Decision needed: autostart mechanism

Two competing mechanisms exist right now, and the plan shouldn't paper over
the conflict:

- **What's actually running**: getty@tty1 autologin → `.profile` loop, with
  specific behavior already load-bearing elsewhere — exit code 0 means
  "quit intentionally, drop to shell", exit code 42 means "rebuilt on disk,
  relaunch without counting it as a crash", anything else counts toward a
  5-strike crash limit. CLAUDE.md's earlyoom section already assumes this
  exact restart-on-crash behavior.
- **What's half-built and unused**: `install-service.sh` +
  `niva-dashboard.service` (missing).

Recommendation: **keep the `.profile`-based mechanism** — it already encodes
real, tested behavior (the exit-42 rebuild signal in particular) — but move
it into a repo-tracked script installed via `deploy/06-autostart.sh` instead
of a hand-edited dotfile. Delete or finish `install-service.sh` as a
follow-up; don't leave a broken script that looks authoritative. A systemd
unit would gain `journalctl` integration and `Restart=on-failure`, but
reproducing the exit-42 "rebuilt, not a crash" distinction under systemd
needs an `ExecStopPost` wrapper or similar — worth doing later, not blocking
this plan.

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
   (udev rules, sudoers entry, earlyoom config, boot-optimizations doc) into
   `deploy/` as inert reference files. Nothing on this Pi changes yet — this
   just stops the configuration living only on one machine's disk.
2. **Script it.** Write the numbered install scripts wrapping idempotent
   application of the captured files, plus package installs and boot-config
   changes. Run `deploy/install.sh` on *this* Pi and diff against current
   state to confirm it's a no-op.
3. **Fix the hardcoded paths** per above, so the binary isn't tied to one
   clone location.
4. **Resolve the install-service.sh / autostart conflict** per the decision
   above — track the real mechanism in `deploy/`, remove or finish the dead
   systemd path.
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
