import { Download, Settings as SettingsIcon } from "lucide-react"
import { PrefCard, PrefGrid } from "@/components/settings/PrefCard"
import { ConfigBackupSection } from "@/components/settings/sections/ConfigBackupSection"

interface Props {
  onOpenRawConfig: () => void
  /** App version, written into the export file header. */
  version?: string | null
  /** Device hostname, written into the export file header. */
  hostname?: string | null
}

/**
 * Settings → Backups. Owns the single source of truth for both
 * "Export Config" and "Raw Configuration" — these used to be duplicated
 * as chips in the page-level ActionsRail, which produced two divergent
 * export code paths. The rich export (version/hostname header, Web-UI
 * preferences, bash-safe quote escaping) now lives here only.
 */
export function BackupsTab({ onOpenRawConfig, version, hostname }: Props) {
  // Export the device's full configuration as a bash-sourceable .conf
  // file. Active settings become `export KEY='value'` lines; defaults
  // become `# export KEY='value'` so the user can see what they didn't
  // change. Rusty-only Web-UI preferences (the JSON kv-store at
  // /mutable/.sentryusb_preferences.json) are appended as `# preference:`
  // comment lines for export completeness without polluting the bash
  // namespace if the file is ever sourced. Single quotes inside values
  // are escaped via the standard '\'' trick so the file stays valid bash.
  async function exportConfig(): Promise<void> {
    try {
      const [configRes, prefsRes] = await Promise.all([
        fetch("/api/setup/config"),
        fetch("/api/config/preference"),
      ])
      if (!configRes.ok) throw new Error("Failed to read config")
      const config = (await configRes.json()) as Record<
        string,
        { value: string; active: boolean }
      >
      const prefs = prefsRes.ok
        ? ((await prefsRes.json()) as Record<string, unknown>)
        : {}

      const now = new Date().toISOString()
      const ver = version || "unknown"
      const host = hostname || "sentryusb"
      const escape = (s: string) => (s ?? "").replace(/'/g, "'\\''")

      let content = ""
      content += `# sentryusb.conf — exported from Sentry USB UI\n`
      content += `# Exported:  ${now}\n`
      content += `# Hostname:  ${host}\n`
      content += `# Version:   ${ver}\n`
      content += `#\n`
      content += `# This file is bash-sourceable. Active settings are 'export' lines;\n`
      content += `# inactive/default values are commented out for reference.\n`
      content += `\n`
      content += `# === Setup configuration ===\n`

      // Sort for stable, diff-friendly output across exports.
      const keys = Object.keys(config).sort()
      for (const k of keys) {
        const e = config[k]
        const v = escape(e.value ?? "")
        if (e.active) {
          content += `export ${k}='${v}'\n`
        } else {
          content += `# export ${k}='${v}'\n`
        }
      }

      const prefKeys = Object.keys(prefs).sort()
      if (prefKeys.length > 0) {
        content += `\n`
        content += `# === Web UI preferences (Sentry USB Rusty) ===\n`
        content += `# Managed via the web UI; stored in /mutable/.sentryusb_preferences.json.\n`
        content += `# Listed here for export completeness — these are NOT sourced by bash.\n`
        for (const k of prefKeys) {
          const v = prefs[k]
          content += `# preference: ${k} = ${JSON.stringify(v)}\n`
        }
      }

      const blob = new Blob([content], { type: "text/plain" })
      const url = URL.createObjectURL(blob)
      const a = document.createElement("a")
      a.href = url
      a.download = "sentryusb.conf"
      a.click()
      URL.revokeObjectURL(url)
    } catch {
      /* ignore */
    }
  }

  return (
    <PrefGrid min={300}>
      <ConfigBackupSection />

      <PrefCard
        icon={<Download className="h-3.5 w-3.5" />}
        halo="slate"
        title="Export Config"
      >
        <p className="t-xs">
          Download every active configuration value as a single shell-format file — useful for
          migrating to a new Pi or sharing a recipe.
        </p>
        <button
          onClick={exportConfig}
          className="self-start rounded-lg border border-white/10 bg-white/5 px-3 py-1.5 text-xs font-medium text-slate-300 transition-colors hover:bg-white/10"
        >
          <Download className="mr-1.5 inline h-3.5 w-3.5" />
          Download sentryusb.conf
        </button>
      </PrefCard>

      <PrefCard
        icon={<SettingsIcon className="h-3.5 w-3.5" />}
        halo="slate"
        title="Raw Configuration"
      >
        <p className="t-xs">
          Edit any single key directly. Opens the raw editor with the full active +
          commented-out config.
        </p>
        <button
          onClick={onOpenRawConfig}
          className="self-start rounded-lg border border-white/10 bg-white/5 px-3 py-1.5 text-xs font-medium text-slate-300 transition-colors hover:bg-white/10"
        >
          Open editor
        </button>
      </PrefCard>
    </PrefGrid>
  )
}
