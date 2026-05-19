# Rusty Migration

If you got here from a "reinstall required" banner in your Sentry USB Settings screen: that's expected. Sentry USB was rewritten under the hood, and the new build can't be applied as an in-place update — the service unit and on-disk layout differ enough that auto-updating would brick the Pi.

You need to re-image the SD card once. Your config carries over.

## What you need to do

1. **Export your config.** In your current Sentry USB web UI, go to **Settings** → **Export Configuration** and save the `sentryusb.conf` file to your computer.
2. **Flash the new image** from the [latest release](https://github.com/Sentry-Six/Sentry-USB-Rusty/releases/latest) using Raspberry Pi Imager.
3. Boot the Pi and open `http://sentryusb.local`.
4. On the [Setup Wizard's](Setup-Wizard-Guide) Welcome step, click **Import Configuration** and pick the `sentryusb.conf` you saved in step 1. Every setting carries over — you can click through the wizard to confirm and apply.

## What's preserved

- **Every setting** in `sentryusb.conf`: archive credentials, notification URLs, WiFi, hostname, partition sizes, keep-awake configuration, the lot.
- **Archive destination** — your CIFS / rsync / rclone / NFS server keeps the same paths.
- **Clips already archived** to your NAS or cloud — untouched. Only the SD card is re-imaged.

## What's new

- Same web UI, faster server.
- Same Setup Wizard, same archive methods, same notification providers.
- Sentry Cloud integration out of the box.

## Why a rewrite, not an in-place update?

The old build's auto-updater overwrote the server binary in place using the existing systemd unit and storage layout. The new build ships with a different service unit and a different binary name, so swapping binaries underneath the old service would leave the system half-installed and unbootable. Re-imaging is the only safe path.

## Need help?

- **[Discord](https://discord.gg/9QZEzVwdnt)**
- **[Issues](https://github.com/Sentry-Six/Sentry-USB-Rusty/issues)**
