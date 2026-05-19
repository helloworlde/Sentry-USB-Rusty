import type { ReactNode } from "react"
import { cn } from "@/lib/utils"
import type { Halo } from "@/components/ui/StatusTile"

interface PrefCardProps {
  icon: ReactNode
  halo?: Halo
  title: ReactNode
  badge?: ReactNode
  footer?: ReactNode
  className?: string
  children: ReactNode
}

export function PrefCard({
  icon,
  halo = "slate",
  title,
  badge,
  footer,
  className,
  children,
}: PrefCardProps) {
  return (
    <div className={cn("glass-card overflow-hidden", className)}>
      <div className="flex items-center gap-2.5 border-b border-white/5 px-3.5 py-2.5">
        <span
          className={cn("halo-" + halo, "inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-lg")}
        >
          {icon}
        </span>
        <span className="t-md font-semibold">{title}</span>
        {badge && <span className="ml-auto">{badge}</span>}
      </div>
      <div className="flex flex-col gap-2.5 p-3.5">{children}</div>
      {footer && (
        <div className="border-t border-white/5 px-3.5 py-2.5">{footer}</div>
      )}
    </div>
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
