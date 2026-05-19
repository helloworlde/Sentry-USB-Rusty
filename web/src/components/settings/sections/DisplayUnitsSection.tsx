import { useEffect, useState } from "react"
import { Thermometer } from "lucide-react"
import { PrefCard } from "@/components/settings/PrefCard"
import { Toggle } from "@/components/ui/Toggle"

type ConfigVal = { value: string; active: boolean } | string

function readActive(entry: ConfigVal | undefined): string | null {
  if (entry == null) return null
  if (typeof entry === "string") return entry
  return entry.active ? entry.value : null
}

async function writeKey(key: string, value: string) {
  const res = await fetch("/api/setup/config")
  const cfg = res.ok ? await res.json() : {}
  cfg[key] = { value, active: true }
  const flat: Record<string, string> = {}
  for (const [k, v] of Object.entries(cfg)) {
    const entry = v as { value: string; active: boolean }
    if (entry.active) flat[k] = entry.value
  }
  await fetch("/api/setup/config", {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(flat),
  })
}

export function DisplayUnitsSection() {
  const [useFahrenheit, setUseFahrenheit] = useState(false)
  const [useKm, setUseKm] = useState(false)

  useEffect(() => {
    fetch("/api/setup/config")
      .then((r) => r.json())
      .then((cfg) => {
        const temp = readActive(cfg.TEMPERATURE_UNIT)
        if (temp != null) setUseFahrenheit(temp === "F")
        const dist = readActive(cfg.DRIVE_MAP_UNIT)
        if (dist != null) setUseKm(dist === "km")
      })
      .catch(() => {})
  }, [])

  return (
    <PrefCard
      icon={<Thermometer className="h-3.5 w-3.5" />}
      halo="violet"
      title="Display & Units"
    >
      <Toggle
        checked={useFahrenheit}
        onChange={async (next) => {
          setUseFahrenheit(next)
          await writeKey("TEMPERATURE_UNIT", next ? "F" : "C").catch(() => {})
        }}
        label="Use Fahrenheit (°F)"
        sub="Apply to dashboard temperature readouts"
      />
      <Toggle
        checked={useKm}
        onChange={async (next) => {
          setUseKm(next)
          await writeKey("DRIVE_MAP_UNIT", next ? "km" : "mi").catch(() => {})
        }}
        label="Distance in kilometres"
        sub="Default uses miles for en-US"
      />
    </PrefCard>
  )
}
