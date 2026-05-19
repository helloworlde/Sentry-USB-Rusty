import { cn } from "@/lib/utils"

interface TabBarProps<T extends string> {
  tabs: readonly T[]
  active: T
  onSelect: (next: T) => void
  /** Use the horizontal-scroll variant on narrow viewports (>= 4 tabs at <640px). */
  scrollable?: boolean
}

export function TabBar<T extends string>({
  tabs,
  active,
  onSelect,
  scrollable = false,
}: TabBarProps<T>) {
  const inner = (
    <div className={cn("tab-bar", scrollable && "tab-bar--scroll")} role="tablist">
      {tabs.map((t) => (
        <button
          key={t}
          type="button"
          role="tab"
          aria-selected={active === t}
          className={cn("tab-item", active === t && "active")}
          onClick={() => onSelect(t)}
        >
          {t}
        </button>
      ))}
    </div>
  )
  return scrollable ? <div className="tab-bar-wrap">{inner}</div> : inner
}
