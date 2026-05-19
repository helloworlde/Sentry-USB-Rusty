import { Download, Settings as SettingsIcon } from "lucide-react"
import { PrefCard, PrefGrid } from "@/components/settings/PrefCard"
import { ConfigBackupSection } from "@/components/settings/sections/ConfigBackupSection"

interface Props {
  onOpenRawConfig: () => void
}

export function BackupsTab({ onOpenRawConfig }: Props) {
  async function exportConfig() {
    try {
      const res = await fetch("/api/setup/config")
      if (!res.ok) throw new Error("Failed")
      const data = await res.json()
      let content = "# sentryusb.conf - exported from Sentry USB UI\n"
      for (const [k, v] of Object.entries(data)) {
        const entry = v as { value: string; active: boolean }
        if (entry.active) {
          content += `export ${k}='${entry.value}'\n`
        } else {
          content += `# export ${k}='${entry.value}'\n`
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
