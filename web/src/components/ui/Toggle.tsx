import type { ReactNode } from "react"

interface ToggleProps {
  checked: boolean
  onChange: (next: boolean) => void
  label: ReactNode
  sub?: ReactNode
  disabled?: boolean
}

export function Toggle({ checked, onChange, label, sub, disabled }: ToggleProps) {
  return (
    <label className="flex items-center gap-3 cursor-pointer">
      <div className="flex-1 min-w-0">
        <div className="t-md">{label}</div>
        {sub && <div className="t-xs mt-0.5">{sub}</div>}
      </div>
      <input
        type="checkbox"
        className="toggle-switch"
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
      />
    </label>
  )
}
