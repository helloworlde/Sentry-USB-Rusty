# FAQ

## Does this void my Tesla warranty?

We can't speak for Tesla — read your warranty terms if it matters. The Pi connects to a regular Tesla USB port the same way any USB drive does. It writes to itself, not to your car.

## Does Tesla officially support this?

No. Sentry USB is a third-party open-source project, not affiliated with Tesla.

## Does it cost anything?

The Pi software itself is **free and open source** under the MIT license. You only pay for the hardware (Pi + SD card + cable).

The optional [Sentry Cloud](Sentry-Cloud) service has a paid tier — see [sentryusb.com](https://sentryusb.com) for details.

## Can I use it without internet?

Yes. Sentry USB only needs internet for:
- **First-time setup** (downloads the binary, installs system packages).
- **Updates** (auto-update checks).
- **Cloud sync** (if you use Sentry Cloud).
- Some **archive backends** (rclone to cloud storage).

All local archive methods (CIFS, NFS, rsync to a LAN server) work offline.

## How often does it archive? Can I trigger manually?

Sentry USB archives **whenever the Pi connects to a known WiFi network**. For most users, that means every time you park in your driveway or garage.

To trigger manually, open the web UI and click the **Archive Sync** action at the top of the **Settings** page.

## Can I run it on hardware other than a Raspberry Pi?

Officially supported: Raspberry Pi 4B, Pi 5, Pi Zero 2 W, Pi 3 (A+/B/B+).

Community-tested: Radxa Rock Pi 4C+, Radxa Zero 3W. These work but we don't actively test on them.

Anything else is uncharted — community help on [Discord](https://discord.gg/9QZEzVwdnt) is your best bet.

## What does Sentry Cloud see of my drives?

Nothing readable. Each route is encrypted on the Pi before it leaves your network. The cloud only ever sees ciphertext. Decryption happens in your browser when you sign in to view a drive — there's no key on the server.

See [Sentry Cloud](Sentry-Cloud) for the short version, or [sentryusb.com](https://sentryusb.com) for the full pitch.
