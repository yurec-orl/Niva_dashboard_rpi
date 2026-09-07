# PlatformIO pre-build hook: injects the git revision the firmware is built from as the
# FW_GIT_REV macro. main.cpp emits it at boot (and on a "$VER" query) as "$VER,<rev>" so a
# running module can be matched to a source revision.
#
# <rev> = short hash, with "-dirty" appended when the module's own working tree has
# uncommitted changes. Falls back to "unknown" when git isn't available or this isn't a
# checkout (e.g. a source tarball build).

Import("env")

import subprocess


def _git_rev():
    project_dir = env["PROJECT_DIR"]
    try:
        rev = subprocess.check_output(
            ["git", "rev-parse", "--short", "HEAD"],
            cwd=project_dir, stderr=subprocess.DEVNULL,
        ).decode().strip()
    except Exception:
        return "unknown"
    try:
        # Scope the dirty check to this module's subtree — the repo also holds the Rust app,
        # whose unrelated edits shouldn't mark the firmware build dirty.
        dirty = bool(subprocess.check_output(
            ["git", "status", "--porcelain", "--", "."],
            cwd=project_dir, stderr=subprocess.DEVNULL,
        ).decode().strip())
    except Exception:
        dirty = False
    return rev + ("-dirty" if dirty else "")


rev = _git_rev()
print("git_rev.py: FW_GIT_REV = %s" % rev)
env.Append(CPPDEFINES=[("FW_GIT_REV", '\\"%s\\"' % rev)])
