import type { ReactNode } from "react"
import { cn } from "@/lib/utils"
import type { Halo } from "@/components/ui/StatusTile"

/**
 * Configures the "feature unavailable" overlay drawn over the card body when
 * `disabled` is set on a PrefCard. The header (icon + title + badge) keeps
 * rendering normally — only the body is blurred and inerted, so users can
 * still tell which card they're looking at.
 */
export interface DisabledConfig {
  /** Centered text explaining what the user needs to enable. */
  reason: string
  /** Optional "go enable this" affordance. */
  cta?: {
    label: string
    onClick?: () => void
    href?: string
  }
}

interface PrefCardProps {
  icon: ReactNode
  halo?: Halo
  title: ReactNode
  badge?: ReactNode
  footer?: ReactNode
  className?: string
  /**
   * When set, the card body is rendered behind a blurred, non-interactive
   * overlay with the supplied reason + optional CTA. Children stay mounted
   * (no state loss on re-enable). The body wrapper carries the `inert`
   * attribute, which removes it from the focus order and from assistive
   * tech — `pointer-events: none` alone wouldn't block keyboard Tab.
   */
  disabled?: DisabledConfig
  children: ReactNode
}

export function PrefCard({
  icon,
  halo = "slate",
  title,
  badge,
  footer,
  className,
  disabled,
  children,
}: PrefCardProps) {
  return (
    <div
      className={cn("glass-card overflow-hidden", className)}
      aria-disabled={disabled ? true : undefined}
      data-disabled={disabled ? "true" : undefined}
    >
      <div className="flex items-center gap-2.5 border-b border-white/5 px-3.5 py-2.5">
        <span
          className={cn("halo-" + halo, "inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-lg")}
        >
          {icon}
        </span>
        <span className="t-md font-semibold">{title}</span>
        {badge && <span className="ml-auto">{badge}</span>}
      </div>
      {disabled ? (
        <div className="relative">
          <div
            inert
            className="flex flex-col gap-2.5 p-3.5 blur-[2px] opacity-40 select-none transition-all"
          >
            {children}
          </div>
          <div className="absolute inset-0 flex flex-col items-center justify-center gap-2 p-4 text-center">
            <p className="max-w-[28ch] text-xs text-slate-300">{disabled.reason}</p>
            {disabled.cta && <DisabledCta cta={disabled.cta} />}
          </div>
        </div>
      ) : (
        <div className="flex flex-col gap-2.5 p-3.5">{children}</div>
      )}
      {footer && (
        <div className="border-t border-white/5 px-3.5 py-2.5">{footer}</div>
      )}
    </div>
  )
}

function DisabledCta({ cta }: { cta: NonNullable<DisabledConfig["cta"]> }) {
  const cls =
    "rounded-lg bg-blue-500/15 px-3 py-1.5 text-xs font-medium text-blue-400 transition-colors hover:bg-blue-500/25"
  if (cta.href) {
    return (
      <a href={cta.href} className={cls}>
        {cta.label}
      </a>
    )
  }
  return (
    <button type="button" onClick={cta.onClick} className={cls}>
      {cta.label}
    </button>
  )
}

/** Grid wrapper used by every settings tab — auto-fit so cards reflow when hidden. */
export function PrefGrid({ children, min = 280 }: { children: ReactNode; min?: number }) {
  return (
    <div
      className="grid items-start gap-2.5"
      style={{ gridTemplateColumns: `repeat(auto-fit, minmax(${min}px, 1fr))` }}
    >
      {children}
    </div>
  )
}
