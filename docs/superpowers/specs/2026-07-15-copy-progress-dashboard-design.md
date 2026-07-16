# Copy Progress Dashboard Design

## Purpose

Show the active July 12 SentryUSB copy in a separate Terminal window without interrupting or modifying the resumable transfer.

## Display

Refresh every five seconds and show:

- a 40-character progress bar;
- copied and total decimal gigabytes;
- completion percentage;
- current transfer speed based on change between refreshes;
- estimated time remaining;
- finalized file count and partial-file state.

## Data flow

The dashboard reads the existing TSV manifest for expected filenames and sizes, then reads file sizes from `/Users/jhoan/Desktop/SentryUSB-RecentClips-2026-07-12`. Final files and the current `.part` file count toward copied bytes. It never opens the SSD, calls the reader, changes the transfer, or writes into the destination.

## Completion and errors

When copied bytes and finalized file count match the manifest and no `.part` file remains, show `COMPLETE — SSD SAFE TO UNPLUG`. If the manifest or destination is unavailable, show a clear waiting/error state and retry on the next refresh.

## Verification

Compare dashboard totals against the manifest total of 6,948 files and 308,626,088,727 bytes, and confirm the monitor process does not modify destination files.
