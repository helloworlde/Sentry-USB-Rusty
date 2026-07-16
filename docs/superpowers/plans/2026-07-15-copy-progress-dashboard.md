# Copy Progress Dashboard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Open a live Terminal dashboard for the active July 12 SentryUSB copy with a progress bar, transferred size, speed, ETA, file count, and safe-to-unplug completion state.

**Architecture:** A standalone read-only shell monitor reads the existing TSV manifest and destination file sizes every five seconds. A separate Terminal window runs the monitor; it has no process-control or write access to the active copy.

**Tech Stack:** zsh, macOS `stat`, `awk`, AppleScript/Terminal.

## Global Constraints

- Do not stop, restart, or modify the active copy process.
- Read `/tmp/sentryusb-recentclips-2026-07-12.tsv` as the source of expected names and sizes.
- Read `/Users/jhoan/Desktop/SentryUSB-RecentClips-2026-07-12` for current final and `.part` file sizes.
- Refresh every five seconds.
- Completion requires 6,948 final files, 308,626,088,727 bytes, and zero `.part` files.

---

### Task 1: Create and open the live copy monitor

**Files:**
- Create: `/tmp/sentryusb-copy-progress.zsh`
- Read: `/tmp/sentryusb-recentclips-2026-07-12.tsv`
- Read: `/Users/jhoan/Desktop/SentryUSB-RecentClips-2026-07-12/*`

**Interfaces:**
- Consumes: TSV rows shaped as `filename<TAB>url_encoded_filename<TAB>expected_bytes`.
- Produces: a continuously refreshed Terminal display; `--once` prints one snapshot and exits for verification.

- [ ] **Step 1: Write a failing smoke check**

Run:

```bash
zsh /tmp/sentryusb-copy-progress.zsh --once
```

Expected: FAIL with `no such file or directory` before the script exists.

- [ ] **Step 2: Create the monitor**

Create `/tmp/sentryusb-copy-progress.zsh` with strict manifest/destination checks, a 40-character bar, decimal GB calculations, interval speed, ETA formatting, `.part` accounting, five-second refresh, and `--once` support. The monitor must only use read operations against the manifest and destination.

- [ ] **Step 3: Verify a one-shot snapshot**

Run:

```bash
zsh /tmp/sentryusb-copy-progress.zsh --once
```

Expected: exit 0 and output containing `SentryUSB July 12 Copy`, `/ 308.63 GB`, `files verified`, and `remaining` or `COMPLETE`.

- [ ] **Step 4: Verify read-only behavior**

Run destination file counts and byte totals immediately before and after the one-shot monitor. Expected: identical values; the monitor creates, removes, and renames no destination files.

- [ ] **Step 5: Open the live dashboard**

Run AppleScript to open a new Terminal window executing:

```bash
zsh /tmp/sentryusb-copy-progress.zsh
```

Expected: a visible Terminal window refreshes every five seconds while the existing copy continues.
