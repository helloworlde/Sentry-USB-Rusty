import type { DriveSummary } from "@/types/drives"

// Single source of truth for window-scoped drive aggregates on the client.
// `useDrivesList` calls this for the Drives-tab summary strip; any future
// widget/export that needs "stats over a filtered set of drives" should
// import from here so the formula can't silently diverge again.
//
// Formula notes (must stay in sync with the Rust side):
//   - crates/drives/src/db.rs  (lifetime `drive_stats` cache → /api/drives/stats)
//   - crates/drives/src/grouper.rs::build_fsd_analytics  (/api/drives/fsd-analytics)
// Both Rust paths compute FSD% as distance-based and SEI-only. Earlier this
// helper computed it as engaged-ms / total-ms which produced numbers
// 10-15 points lower than the FSD Analytics page for the same window.
//
// Top-line totals (count, distance, duration) include every drive in the
// window. FSD/Autopilot ratios are SEI-only — Tessie's autopilot data is
// inferred rather than read from dashcam SEI telemetry, so mixing it would
// dilute the score.

export interface DrivesFilteredStats {
  count: number
  totalDistanceMi: number
  totalDistanceKm: number
  totalDurationMs: number
  fsdEngagedMs: number
  fsdDistanceMi: number
  fsdDistanceKm: number
  fsdPercent: number
  fsdDisengagements: number
  autopilotEngagedMs: number
  autopilotPercent: number
  tessieCount: number
}

export function computeFilteredStats(
  drives: DriveSummary[],
): DrivesFilteredStats {
  let totalDistanceMi = 0
  let totalDistanceKm = 0
  let totalDurationMs = 0
  let fsdEngagedMs = 0
  let fsdDistanceMi = 0
  let fsdDistanceKm = 0
  let fsdDisengagements = 0
  let autopilotEngagedMs = 0
  let tessieCount = 0
  // SEI-only denominators feed FSD% and Autopilot%. Top-line totals above
  // still sum every drive in the window.
  let seiTotalDistanceKm = 0
  let seiAutopilotDistanceKm = 0

  for (const d of drives) {
    totalDistanceMi += d.distanceMi
    totalDistanceKm += d.distanceKm
    totalDurationMs += d.durationMs
    fsdEngagedMs += d.fsdEngagedMs
    fsdDistanceMi += d.fsdDistanceMi
    fsdDistanceKm += d.fsdDistanceKm
    fsdDisengagements += d.fsdDisengagements
    autopilotEngagedMs += d.autosteerEngagedMs + d.taccEngagedMs
    if (d.source === "tessie") {
      tessieCount += 1
      continue
    }
    seiTotalDistanceKm += d.distanceKm
    seiAutopilotDistanceKm += d.autosteerDistanceKm + d.taccDistanceKm
  }

  const fsdPercent =
    seiTotalDistanceKm > 0 ? (fsdDistanceKm / seiTotalDistanceKm) * 100 : 0
  // "Autopilot" here means autosteer + TACC (non-FSD assist), kept as a
  // distinct stat from FSD. Backend's `assisted_percent` rolls FSD into
  // this bucket — we don't, so the strip can show FSD and AP separately.
  const autopilotPercent =
    seiTotalDistanceKm > 0
      ? (seiAutopilotDistanceKm / seiTotalDistanceKm) * 100
      : 0

  return {
    count: drives.length,
    totalDistanceMi,
    totalDistanceKm,
    totalDurationMs,
    fsdEngagedMs,
    fsdDistanceMi,
    fsdDistanceKm,
    fsdPercent,
    fsdDisengagements,
    autopilotEngagedMs,
    autopilotPercent,
    tessieCount,
  }
}
