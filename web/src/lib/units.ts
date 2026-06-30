import { useSyncExternalStore } from "react"

// Shared unit preferences, backed by /api/setup/config. The header's
// Metric/Imperial switch and the Display & Units toggles both read and
// write these, so a change in one reflects live in the other (one source
// of truth, no drift). Distance + dashboard temperature move together as
// "metric"/"imperial"; the System tile's CPU temperature stays independent
// (SYSTEM_TEMPERATURE_UNIT) per its own sub-toggle.
export type UnitState = {
  tempF: boolean // TEMPERATURE_UNIT === "F"
  systemTempF: boolean // SYSTEM_TEMPERATURE_UNIT === "F"
  km: boolean // DRIVE_MAP_UNIT === "km"
  pressureBar: boolean // PRESSURE_UNIT === "bar"
  loaded: boolean
}

// Defaults form a coherent metric set (°C → km, bar, °C system temp).
// Temperature is the anchor: see load(), where any unset key follows the
// temperature unit so a partial config never reads as a mixed system.
let state: UnitState = {
  tempF: false,
  systemTempF: false,
  km: true,
  pressureBar: true,
  loaded: false,
}
const listeners = new Set<() => void>()
let loading = false

function snapshot(): UnitState {
  return state
}

function set(patch: Partial<UnitState>) {
  state = { ...state, ...patch }
  for (const l of listeners) l()
}

function readActive(entry: unknown): string | null {
  if (entry == null) return null
  if (typeof entry === "string") return entry
  const e = entry as { value: string; active: boolean }
  return e.active ? e.value : null
}

async function load() {
  if (loading) return
  loading = true
  try {
    const res = await fetch("/api/setup/config")
    const cfg = res.ok ? await res.json() : {}
    const t = readActive(cfg.TEMPERATURE_UNIT)
    const s = readActive(cfg.SYSTEM_TEMPERATURE_UNIT)
    const d = readActive(cfg.DRIVE_MAP_UNIT)
    const p = readActive(cfg.PRESSURE_UNIT)
    // Temperature is the anchor (itself defaulting to °C/metric). Any unit
    // left unset inherits that system so the readouts stay coherent — metric
    // → km + bar + °C system temp; imperial → mi + psi + °F.
    const tempF = t != null ? t === "F" : state.tempF
    const metric = !tempF
    set({
      tempF,
      systemTempF: s != null ? s === "F" : tempF,
      km: d != null ? d === "km" : metric,
      pressureBar: p != null ? p === "bar" : metric,
      loaded: true,
    })
  } catch {
    set({ loaded: true })
  } finally {
    loading = false
  }
}

// Refetch each time the first consumer (re)mounts so navigating back to
// Settings picks up out-of-band edits (raw-config editor, setup wizard),
// while staying live-synced between mounted consumers in between.
function subscribe(cb: () => void): () => void {
  const wasEmpty = listeners.size === 0
  listeners.add(cb)
  if (wasEmpty) void load()
  return () => {
    listeners.delete(cb)
  }
}

// Read-modify-write the whole config with `updates` applied, then reflect
// them locally. Optimistic: state flips immediately, but reverts to the
// prior snapshot if the save fails so the UI never shows a value that
// didn't persist.
async function writeKeys(updates: Record<string, string>, optimistic: Partial<UnitState>) {
  const prev = state
  set(optimistic)
  try {
    const res = await fetch("/api/setup/config")
    const cfg = res.ok ? await res.json() : {}
    for (const [k, v] of Object.entries(updates)) cfg[k] = { value: v, active: true }
    const flat: Record<string, string> = {}
    for (const [k, v] of Object.entries(cfg)) {
      if (typeof v === "string") {
        flat[k] = v
      } else {
        const e = v as { value: string; active: boolean }
        if (e?.active) flat[k] = e.value
      }
    }
    const put = await fetch("/api/setup/config", {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(flat),
    })
    if (!put.ok) throw new Error("config save failed")
  } catch {
    set(prev)
  }
}

export function useUnits() {
  const s = useSyncExternalStore(subscribe, snapshot, snapshot)
  return {
    ...s,
    // Metric = Celsius + kilometres + bar; Imperial = Fahrenheit + miles +
    // psi. The master switch flips the defaults for all three at once.
    isMetric: !s.tempF,
    setMetric: (metric: boolean) =>
      writeKeys(
        {
          TEMPERATURE_UNIT: metric ? "C" : "F",
          DRIVE_MAP_UNIT: metric ? "km" : "mi",
          PRESSURE_UNIT: metric ? "bar" : "psi",
        },
        { tempF: !metric, km: metric, pressureBar: metric },
      ),
    setTempF: (f: boolean) =>
      writeKeys({ TEMPERATURE_UNIT: f ? "F" : "C" }, { tempF: f }),
    setKm: (km: boolean) =>
      writeKeys({ DRIVE_MAP_UNIT: km ? "km" : "mi" }, { km }),
    setSystemTempF: (f: boolean) =>
      writeKeys({ SYSTEM_TEMPERATURE_UNIT: f ? "F" : "C" }, { systemTempF: f }),
    setPressureBar: (bar: boolean) =>
      writeKeys({ PRESSURE_UNIT: bar ? "bar" : "psi" }, { pressureBar: bar }),
  }
}
