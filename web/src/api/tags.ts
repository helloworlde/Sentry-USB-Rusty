import { fetchChargeTags } from "./charging"

/** All distinct drive tag names (GET /api/drives/tags). */
export async function fetchDriveTags(): Promise<string[]> {
  const res = await fetch("/api/drives/tags")
  if (!res.ok) throw new Error(`drive tags: ${res.status}`)
  const data = await res.json()
  return Array.isArray(data) ? data : []
}

/**
 * Distinct union of every tag in use across drives + charges — the source
 * for "pick an existing tag instead of retyping" suggestions.
 *
 * It needs no separate bookkeeping: both backend lists are SELECT DISTINCT
 * over the per-item tag rows, so a tag appears here as soon as any drive or
 * charge uses it and disappears automatically once the last one drops it.
 */
export async function fetchAllTagNames(): Promise<string[]> {
  const [drive, charge] = await Promise.all([
    fetchDriveTags().catch(() => [] as string[]),
    fetchChargeTags().catch(() => [] as string[]),
  ])
  return Array.from(new Set([...drive, ...charge])).sort((a, b) =>
    a.localeCompare(b),
  )
}
