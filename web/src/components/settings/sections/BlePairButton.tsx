import { useState, useEffect, useRef } from "react"
import { Bluetooth, CheckCircle, AlertCircle, Loader2 } from "lucide-react"
import { cn } from "@/lib/utils"
import { wsClient } from "@/lib/ws"
import { PrefCard } from "@/components/settings/PrefCard"
import { Pill } from "@/components/ui/Pill"

type BleState = "idle" | "initiating" | "waiting" | "polling" | "paired" | "error"

export function BlePairButton() {
  const [bleState, setBleState] = useState<BleState>("idle")
  const [bleMsg, setBleMsg] = useState("")
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => {
    fetch("/api/system/ble-status?quick=true")
      .then((r) => r.json())
      .then((data) => {
        if (data.status === "paired") {
          setBleState("paired")
          setBleMsg("Paired — click to re-pair")
        } else if (data.status === "keys_generated") {
          setBleState("idle")
          setBleMsg("")
          fetch("/api/system/ble-status")
            .then((r) => r.json())
            .then((d) => {
              if (d.status === "paired") {
                setBleState("paired")
                setBleMsg("Paired — click to re-pair")
              }
            })
            .catch(() => {})
        }
      })
      .catch(() => {})
  }, [])

  useEffect(() => {
    const unsub = wsClient.subscribe("ble_status", (data: unknown) => {
      const d = data as { status: string; error?: string; output?: string }
      if (d.status === "pairing") {
        setBleState("initiating")
        setBleMsg("Sending pairing request to car...")
      } else if (d.status === "error") {
        setBleState("error")
        const errMsg = d.error || "Unknown error"
        if (errMsg.includes("maximum number of BLE")) {
          setBleMsg(
            "Too many BLE devices active. Turn off Bluetooth on nearby phone keys and try again."
          )
        } else if (errMsg.includes("timed out")) {
          setBleMsg(
            "BLE connection timed out. Make sure the Pi is near the car and try again."
          )
        } else {
          setBleMsg(errMsg)
        }
        cleanup()
      } else if (d.status === "waiting") {
        setBleState("waiting")
        setBleMsg("Tap your keycard on the center console to confirm pairing.")
        startPolling()
      }
    })
    return () => {
      unsub()
      cleanup()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  function cleanup() {
    if (pollRef.current) {
      clearInterval(pollRef.current)
      pollRef.current = null
    }
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current)
      timeoutRef.current = null
    }
  }

  function startPolling() {
    cleanup()
    let count = 0
    pollRef.current = setInterval(async () => {
      count++
      try {
        const res = await fetch("/api/system/ble-status")
        if (res.ok) {
          const data = await res.json()
          if (data.status === "paired") {
            setBleState("paired")
            setBleMsg("Successfully paired with car!")
            cleanup()
            return
          }
        }
      } catch {
        /* ignore */
      }
      if (count >= 12) {
        setBleState("error")
        setBleMsg(
          "Pairing timed out. Make sure you tapped your keycard on the center console, then try again."
        )
        cleanup()
      }
    }, 5000)
    timeoutRef.current = setTimeout(() => {
      if (bleState !== "paired" && bleState !== "error") {
        setBleState("error")
        setBleMsg("Pairing timed out. Please try again.")
        cleanup()
      }
    }, 65000)
  }

  async function handlePair() {
    setBleState("initiating")
    setBleMsg("Sending pairing request...")
    try {
      const res = await fetch("/api/system/ble-pair", { method: "POST" })
      if (!res.ok) {
        const data = await res.json().catch(() => ({}))
        throw new Error(data.error || "Failed to initiate BLE pairing")
      }
    } catch (err) {
      setBleState("error")
      setBleMsg(err instanceof Error ? err.message : "Failed to initiate pairing")
    }
  }

  function handleReset() {
    cleanup()
    setBleState("idle")
    setBleMsg("")
  }

  const isActive =
    bleState !== "idle" && bleState !== "paired" && bleState !== "error"

  const halo =
    bleState === "paired"
      ? "accent"
      : bleState === "error"
      ? "red"
      : isActive
      ? "amber"
      : "blue"

  const icon = isActive ? (
    <Loader2 className="h-3.5 w-3.5 animate-spin" />
  ) : bleState === "paired" ? (
    <CheckCircle className="h-3.5 w-3.5" />
  ) : bleState === "error" ? (
    <AlertCircle className="h-3.5 w-3.5" />
  ) : (
    <Bluetooth className="h-3.5 w-3.5" />
  )

  return (
    <PrefCard
      icon={icon}
      halo={halo}
      title="BLE Pairing"
      badge={bleState === "paired" ? <Pill kind="accent">Paired</Pill> : null}
    >
      <p
        className={cn(
          "text-xs",
          bleState === "paired"
            ? "text-emerald-400"
            : bleState === "error"
            ? "text-red-400"
            : bleState === "waiting"
            ? "font-medium text-amber-400"
            : "text-slate-500"
        )}
      >
        {bleMsg || "Initiate Bluetooth Low Energy pairing with your car"}
      </p>
      <button
        onClick={
          bleState === "idle"
            ? handlePair
            : bleState === "paired"
            ? handlePair
            : bleState === "error"
            ? handleReset
            : undefined
        }
        disabled={isActive}
        className={cn(
          "self-start rounded-lg px-3 py-1.5 text-xs font-medium transition-colors disabled:opacity-50",
          bleState === "paired"
            ? "bg-white/5 text-slate-300 hover:bg-white/10"
            : bleState === "error"
            ? "bg-red-500/15 text-red-400 hover:bg-red-500/25"
            : "bg-blue-500/15 text-blue-400 hover:bg-blue-500/25"
        )}
      >
        {bleState === "paired"
          ? "Re-pair"
          : bleState === "error"
          ? "Retry"
          : isActive
          ? "Pairing..."
          : "Pair BLE"}
      </button>
    </PrefCard>
  )
}
