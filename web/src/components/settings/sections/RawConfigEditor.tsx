import { useState } from "react"
import { cn } from "@/lib/utils"
import { Modal } from "@/components/ui/Modal"

export interface RawConfigEntry {
  value: string
  active: boolean
}

interface Props {
  config: Record<string, RawConfigEntry>
  onClose: () => void
}

export function RawConfigEditor({ config, onClose }: Props) {
  const [entries, setEntries] = useState<Record<string, RawConfigEntry>>(() => {
    const e: Record<string, RawConfigEntry> = {}
    for (const [k, v] of Object.entries(config)) e[k] = { value: v.value, active: v.active }
    return e
  })
  const [saving, setSaving] = useState(false)
  const [saveMsg, setSaveMsg] = useState<string | null>(null)
  const [newKey, setNewKey] = useState("")
  const [newVal, setNewVal] = useState("")

  const sortedKeys = Object.keys(entries).sort()

  async function handleSave() {
    setSaving(true)
    setSaveMsg(null)
    try {
      const configData: Record<string, string> = {}
      for (const [k, v] of Object.entries(entries)) {
        if (v.active) configData[k] = v.value
      }
      const res = await fetch("/api/setup/config", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(configData),
      })
      if (!res.ok) throw new Error("Failed to save")
      setSaveMsg("Saved successfully")
      setTimeout(() => setSaveMsg(null), 3000)
    } catch (err) {
      setSaveMsg(err instanceof Error ? err.message : "Save failed")
    } finally {
      setSaving(false)
    }
  }

  function addEntry() {
    if (!newKey.trim()) return
    setEntries((prev) => ({
      ...prev,
      [newKey.trim()]: { value: newVal, active: true },
    }))
    setNewKey("")
    setNewVal("")
  }

  return (
    <Modal
      title="Raw Configuration"
      onClose={onClose}
      size="lg"
      footer={
        <div className="flex items-center justify-end gap-2">
          {saveMsg && (
            <span
              className={cn(
                "text-xs",
                saveMsg.includes("success") ? "text-emerald-400" : "text-red-400"
              )}
            >
              {saveMsg}
            </span>
          )}
          <button
            onClick={handleSave}
            disabled={saving}
            className="rounded-xl bg-blue-500 px-4 py-1.5 text-sm font-medium text-white hover:bg-blue-600 disabled:opacity-50"
          >
            {saving ? "Saving..." : "Save"}
          </button>
        </div>
      }
    >
      <div className="space-y-1">
        {sortedKeys.map((key) => (
          <div
            key={key}
            className="flex items-center gap-2 rounded-xl border border-white/5 bg-white/[0.02] px-3 py-2.5"
          >
            <input
              type="checkbox"
              checked={entries[key].active}
              onChange={(e) =>
                setEntries((prev) => ({
                  ...prev,
                  [key]: { ...prev[key], active: e.target.checked },
                }))
              }
              className="toggle-switch"
            />
            <span
              className={cn(
                "w-28 shrink-0 truncate font-mono text-xs sm:w-48",
                entries[key].active ? "text-blue-400" : "text-slate-600"
              )}
            >
              {key}
            </span>
            <input
              type="text"
              value={entries[key].value}
              onChange={(e) =>
                setEntries((prev) => ({
                  ...prev,
                  [key]: { ...prev[key], value: e.target.value },
                }))
              }
              className="flex-1 rounded-lg border border-white/10 bg-white/5 px-2.5 py-1.5 font-mono text-xs text-slate-200 outline-none focus:border-blue-500/50"
            />
            <button
              onClick={() =>
                setEntries((prev) => {
                  const n = { ...prev }
                  delete n[key]
                  return n
                })
              }
              className="text-xs text-slate-600 transition-colors hover:text-red-400"
            >
              ✕
            </button>
          </div>
        ))}
      </div>
      <div className="mt-4 flex items-center gap-2">
        <input
          type="text"
          value={newKey}
          onChange={(e) => setNewKey(e.target.value)}
          placeholder="NEW_KEY"
          className="w-48 rounded-lg border border-white/10 bg-white/5 px-2.5 py-1.5 font-mono text-xs text-slate-200 placeholder-slate-600 outline-none focus:border-blue-500/50"
        />
        <input
          type="text"
          value={newVal}
          onChange={(e) => setNewVal(e.target.value)}
          placeholder="value"
          className="flex-1 rounded-lg border border-white/10 bg-white/5 px-2.5 py-1.5 font-mono text-xs text-slate-200 placeholder-slate-600 outline-none focus:border-blue-500/50"
        />
        <button
          onClick={addEntry}
          className="rounded-lg bg-blue-500/20 px-3 py-1.5 text-xs font-medium text-blue-400 hover:bg-blue-500/30"
        >
          Add
        </button>
      </div>
    </Modal>
  )
}
