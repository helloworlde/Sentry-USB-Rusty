// Drive GPS track normalisation.
//
// A drive's raw points come straight from the stitched dashcam clips and feed
// three consumers that each need slightly different guarantees: the map wants a
// geographically continuous line, while the scrubber and speed chart want a
// strictly increasing time axis. `monotonicTrack` produces one index-aligned
// sequence that satisfies both.

// A single GPS sample: [lat, lng, relativeMs, speedMps].
export type TrackPoint = [number, number, number, number]

// Largest believable distance between two consecutive GPS samples. The sampler
// runs at ~10-33 Hz, so real driving never moves a sample more than a few tens
// of metres (a 130 mph car covers ~6 m in 100 ms). A jump past this is either a
// genuine signal dropout — which we keep, and let the map break the line over —
// or a lone bad fix that teleports away and snaps back, which we drop. Kept in
// sync with DriveMap's GAP_M so the render layer agrees on what counts as "a
// jump".
const MAX_STEP_M = 300

function haversineM(
  lat1: number,
  lon1: number,
  lat2: number,
  lon2: number,
): number {
  const R = 6371000
  const r = Math.PI / 180
  const dLa = (lat2 - lat1) * r
  const dLo = (lon2 - lon1) * r
  const a =
    Math.sin(dLa / 2) ** 2 +
    Math.cos(lat1 * r) * Math.cos(lat2 * r) * Math.sin(dLo / 2) ** 2
  return 2 * R * Math.asin(Math.sqrt(a))
}

export interface NormalisedTrack {
  points: TrackPoint[]
  // Parallel to `points` when the input had a length-matched fsdStates array,
  // otherwise the original (possibly undefined) value is passed through so the
  // caller's coloring logic falls back exactly as before.
  fsdStates: number[] | undefined
}

/**
 * Normalise a drive's raw GPS track into one clean, index-aligned sequence that
 * is geographically continuous (for the map) AND strictly increasing in time
 * (for the scrubber and speed chart). A normal contiguous drive passes through
 * essentially unchanged.
 *
 * Why this exists: Tesla can stitch two OVERLAPPING dashcam clips into a single
 * drive — one clip gets truncated and the next starts a few seconds early. The
 * clips join up fine *geographically* (the car kept moving the whole time), but
 * the synthesised per-sample time jumps BACKWARD at the seam (…46, 47, then
 * back to 16, 17…). The previous logic read "time went backward" as duplicate
 * junk and DELETED every sample until the clock recovered. At highway speed
 * that silently discarded over a mile of real road, leaving a false gap on the
 * map at the seam (and an un-scrubbable stretch).
 *
 * Instead we KEEP every real position and only REPAIR the clock: each sample's
 * time is forced just after the previous one, preserving the true
 * sample-to-sample cadence where it is positive and nudging by 1 ms across a
 * backward or duplicate seam. The only samples dropped are genuine teleport
 * spikes — a single fix that jumps more than MAX_STEP_M away and immediately
 * snaps back — which would otherwise fling the scrubber across the map.
 *
 * fsdStates is filtered in lockstep so the route's FSD/manual coloring stays
 * aligned to the surviving points.
 */
export function monotonicTrack(
  points: TrackPoint[],
  fsdStates: number[] | undefined,
): NormalisedTrack {
  const hasFsd =
    Array.isArray(fsdStates) && fsdStates.length === points.length
  const outPoints: TrackPoint[] = []
  const outFsd: number[] = []

  let prevRawMs: number | null = null
  let prevMs: number | null = null
  let lastKept: TrackPoint | null = null

  for (let i = 0; i < points.length; i++) {
    const p = points[i]

    // Teleport spike: this sample is far from the last kept position AND the
    // next raw sample snaps back near that same anchor — a lone bad fix, not
    // real travel and not a genuine dropout (where the track would *continue*
    // from the new spot rather than return). Drop it without advancing the
    // anchor so the snap-back sample is measured against the same point.
    if (lastKept && haversineM(lastKept[0], lastKept[1], p[0], p[1]) > MAX_STEP_M) {
      const next = points[i + 1]
      const snapsBack =
        !!next && haversineM(lastKept[0], lastKept[1], next[0], next[1]) <= MAX_STEP_M
      if (snapsBack) continue
      // Otherwise it's a genuine dropout / start of a new track segment: keep
      // it; DriveMap breaks the polyline across the > MAX_STEP_M jump itself.
    }

    // Time repair: strictly increasing, cadence-preserving. A positive raw
    // delta is the real inter-sample gap and is kept as-is (so genuine pauses
    // and dropouts still read correctly); a zero/backward delta at a seam is
    // nudged to the minimum 1 ms.
    let ms: number
    if (prevMs === null || prevRawMs === null) {
      ms = p[2]
    } else {
      const rawDelta = p[2] - prevRawMs
      ms = prevMs + (rawDelta > 0 ? rawDelta : 1)
    }

    outPoints.push([p[0], p[1], ms, p[3]])
    if (hasFsd) outFsd.push(fsdStates![i])
    prevRawMs = p[2]
    prevMs = ms
    lastKept = p
  }

  return { points: outPoints, fsdStates: hasFsd ? outFsd : fsdStates }
}
