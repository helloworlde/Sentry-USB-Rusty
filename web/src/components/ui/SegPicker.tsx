import type { ReactNode } from "react"
import { cn } from "@/lib/utils"

interface SegOption<T extends string> {
  value: T
  label: ReactNode
}

interface SegPickerProps<T extends string> {
  options: SegOption<T>[]
  value: T
  onChange: (next: T) => void
  disabled?: boolean
}

export function SegPicker<T extends string>({
  options,
  value,
  onChange,
  disabled,
}: SegPickerProps<T>) {
  return (
    <div className="flex flex-wrap gap-1">
      {options.map((o) => {
        const active = o.value === value
        return (
          <button
            key={o.value}
            type="button"
            disabled={disabled}
            onClick={() => onChange(o.value)}
            aria-pressed={active}
            className={cn(
              "rounded-lg border px-2.5 py-1.5 text-xs font-medium transition-colors disabled:opacity-50",
              active
                ? "border-blue-500/40 bg-blue-500/10 text-blue-400"
                : "border-white/5 bg-white/[0.02] text-slate-400 hover:bg-white/[0.05]"
            )}
          >
            {o.label}
          </button>
        )
      })}
    </div>
  )
}
