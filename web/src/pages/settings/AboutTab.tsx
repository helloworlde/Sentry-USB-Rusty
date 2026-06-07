import { Wand2 } from "lucide-react"
import { PrefCard, PrefGrid } from "@/components/settings/PrefCard"

interface Props {
  onOpenWizard: () => void
}

export function AboutTab({ onOpenWizard }: Props) {
  return (
    <PrefGrid min={300}>
      <PrefCard
        icon={<Wand2 className="h-3.5 w-3.5" />}
        halo="accent"
        title="Setup Wizard"
      >
        <p className="t-xs">
          Re-run the first-time setup wizard to reconfigure WiFi, drives, time zones and units.
          Safe to run any time — your existing config is the starting point.
        </p>
        <button
          onClick={onOpenWizard}
          className="self-start rounded-lg bg-blue-500/15 px-3 py-1.5 text-xs font-medium text-blue-400 transition-colors hover:bg-blue-500/25"
        >
          Launch Wizard
        </button>
        <div className="tile-divider" />
        <p className="section-label">Resources</p>
        <div className="flex flex-col gap-1">
          <a
            href="https://github.com/Sentry-Six/Sentry-USB-Rusty"
            target="_blank"
            rel="noopener noreferrer"
            className="t-sm text-blue-400 hover:text-blue-300"
          >
            GitHub repository ↗
          </a>
          <a
            href="https://discord.gg/9QZEzVwdnt"
            target="_blank"
            rel="noopener noreferrer"
            className="t-sm text-violet-400 hover:text-violet-300"
          >
            Discord community ↗
          </a>
        </div>
      </PrefCard>
    </PrefGrid>
  )
}
