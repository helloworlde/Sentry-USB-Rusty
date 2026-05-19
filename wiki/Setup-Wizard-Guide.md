# Setup Wizard Guide

The Setup Wizard runs the first time you open `http://sentryusb.local`. It walks you through 10 steps. You can re-run it anytime from **Settings** → **Re-run Setup Wizard**.

## 1. Welcome

Confirms the device is reachable and shows any existing configuration detected from a previous install (e.g., from a `sentryusb.conf` you dropped on the SD card's boot partition).

Click **Get Started**.

## 2. Network

- **Device Hostname** — defaults to `sentryusb`. Leave it unless you have a reason to change. The Pi is reachable at `http://<hostname>.local`.
- **WiFi Access Point** (optional) — broadcast a backup WiFi network from the Pi itself. Useful when you're away from home WiFi and want to reach the web UI from your phone.

## 3. Storage

Pick how much of the SD card each "virtual USB drive" gets. The remainder is used for snapshots (saved and sentry clips). Defaults are sensible — most users only touch **Dashcam Size**.

- **Dashcam Size** — the rolling dashcam partition. **40–60 GB is recommended.** Bigger isn't better: Tesla writes ~7–10 GB per hour, but it also needs free space to save Sentry clips. If the dashcam partition is too large, recent clips may fail to save.
- **Music** (optional) — separate partition for Tesla's music drive. Leave empty if you don't need it. If you set a size, you can also point it at a folder on your archive server to auto-sync.
- **LightShow** / **Boombox** (optional) — same idea, for custom light shows and boombox sounds.
- **External Data Drive** (optional) — point Sentry USB at a USB or NVMe drive instead of the SD card. Best for heavy users. **The selected drive will be wiped.**
- **Use ExFAT filesystem** — on by default. Leave it on unless you have a specific reason to use FAT32.

> **EU users:** Tesla's RecentClips retention is 10 minutes in the EU (vs 1 hour in North America). You'll need to lower the **Snapshot Interval** to **480 seconds** (8 minutes) on the [Advanced](#9-advanced) step, otherwise recent clips can roll off before they get archived.

## 4. Community

Toggle two optional features:

- **Community Wraps** — browse and apply community-made vehicle wrap previews.
- **Community Chimes** — replace the default lock chime with sounds from the community library.

Both are stored on the cam drive — no extra partition needed.

## 5. Archive

Pick where your clips get backed up.

| Option | What it is |
|--------|-----------|
| **CIFS / SMB** | Network share on a Windows PC, Mac, or NAS |
| **rsync** | SSH-based file sync — for Linux/Unix servers |
| **rclone** | Cloud storage (Google Drive, S3, Backblaze, Dropbox, etc.) |
| **NFS** | Network File System — common on Linux NAS devices |
| **None** | No archiving — clips stay on the SD card until overwritten |

See [Archive Methods](Archive-Methods) for setup details for each.

## 6. Keep Awake

Tesla's Sentry Mode shuts off after the car sleeps. Keep Awake holds the car awake so the Pi keeps getting power.

| Option | Requires |
|--------|----------|
| **Bluetooth LE** | Pair the Pi to your car once (free, no subscription) |
| **TeslaFi** | TeslaFi paid subscription |
| **Tessie** | Tessie paid subscription |
| **Webhook** | Your own service (e.g., Home Assistant) |
| **None** | Use the car's built-in Sentry/Camp modes manually |

## 7. Notifications

Pick one or more push notification providers. Sentry USB will notify you about archive failures, full drives, BLE pairing issues, etc.

See [Notifications](Notifications) for the full list of providers and how to get API keys for each.

## 8. Security

Set a **Web Username** and **Web Password** for the web UI.

Leave both empty to disable web auth entirely — only do this if your network is fully trusted.

## 9. Advanced

- **Timezone** — pick yours from the list (used for log timestamps and notification times).
- **Archive Delay (seconds)** — how long to wait after WiFi connects before archiving starts. Default 20 is fine.
- **Snapshot Interval (seconds)** — how often the Pi looks for new saved clips to archive. Default works for most users. **EU users: set this to 480** (8 min) because Tesla rotates RecentClips faster in the EU. Rule of thumb: set ~2 minutes shorter than the car's RecentClips retention.
- **Temperature Unit** — °C or °F for the temperature monitoring widget.

## 10. Review

Final summary of every choice. Click **Apply** to write the configuration and reboot. The Pi will come back up at the new hostname (`http://sentryusb.local` by default) in about a minute.
