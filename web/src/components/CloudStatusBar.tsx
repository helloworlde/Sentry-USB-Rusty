import { useEffect, useState } from "react"
import { Link } from "react-router-dom"
import { Cloud, CloudOff, Upload, ChevronRight, AlertTriangle } from "lucide-react"
import { wsClient } from "@/lib/ws"
import { Pill, LiveDot } from "@/components/ui/Pill"

type CloudStatus = {
  paired: boolean
  pendingRouteCount: number
  totalUploadedRouteCount: number
  lastUploadAt: string | null
  lastUploadError: string | null
  pairingState: string
}

/**
 * Compact one-line Cloud status for the Dashboard. Clicking through goes to
 * Settings → Network where the full CloudPairingSection lives. Self-fetches
 * status (the full section also self-fetches; the cost is one extra poll).
 */
export function CloudStatusBar() {
  const [status, setStatus] = useState<CloudStatus | null>(null)

  useEffect(() => {
    let mounted = true
    let timer: ReturnType<typeof setTimeout> | null = null

    async function refetch() {
      try {
        const res = await fetch("/api/cloud/status")
        if (!res.ok) throw new Error()
        const data = await res.json()
        if (!mounted) return
        setStatus(data)
        const fast = data.paired && data.pendingRouteCount > 0
        if (timer) clearTimeout(timer)
        timer = setTimeout(refetch, fast ? 2000 : 30000)
      } catch {
        if (mounted) {
          if (timer) clearTimeout(timer)
          timer = setTimeout(refetch, 5000)
        }
      }
    }

    refetch()
    const unsubStatus = wsClient.subscribe("cloud_status_changed", () => {
      if (mounted) refetch()
    })
    const unsubUpload = wsClient.subscribe("cloud_upload", () => {
      if (mounted) refetch()
    })

    return () => {
      mounted = false
      if (timer) clearTimeout(timer)
      unsubStatus()
      unsubUpload()
    }
  }, [])

  if (!status) return null

  const linkTo = "/settings?tab=Car%20%26%20Network"

  // Unpaired — compact "Connect" prompt
  if (!status.paired) {
    return (
      <Link
        to={linkTo}
        className="glass-card glass-card-hover cloud-bar group transition-colors"
      >
        <span className="halo-blue inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-lg">
          <CloudOff className="h-4 w-4" />
        </span>
        <div className="min-w-0 flex-1">
          <div className="t-md">Connect SentryCloud</div>
          <div className="t-xs">
            Encrypted upload of drive data. Enter your 6-digit code in Settings.
          </div>
        </div>
        <ChevronRight className="h-4 w-4 text-slate-600 transition-transform group-hover:translate-x-0.5" />
      </Link>
    )
  }

  // Uploading — full progress strip
  if (status.pendingRouteCount > 0) {
    const total = status.pendingRouteCount + status.totalUploadedRouteCount
    const pct = total > 0 ? (status.totalUploadedRouteCount / total) * 100 : 0
    return (
      <Link
        to={linkTo}
        className="glass-card glass-card-hover flex flex-col gap-2 px-3.5 py-3 transition-colors"
      >
        <div className="flex items-center gap-3">
          <span className="halo-accent inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-lg">
            <Upload className="h-4 w-4" />
          </span>
          <div className="min-w-0 flex-1">
            <div className="t-md flex items-center gap-2">
              Uploading to SentryCloud
              <Pill kind="accent">
                <LiveDot /> {status.pendingRouteCount.toLocaleString()} pending
              </Pill>
            </div>
            <div className="t-xs">
              {status.totalUploadedRouteCount.toLocaleString()} uploaded · last{" "}
              {status.lastUploadAt
                ? new Date(status.lastUploadAt).toLocaleTimeString(undefined, {
                    hour: "2-digit",
                    minute: "2-digit",
                  })
                : "—"}
            </div>
          </div>
          <ChevronRight className="h-4 w-4 shrink-0 text-slate-600" />
        </div>
        <div className="bar">
          <div
            className="bg-gradient-to-r from-emerald-500 to-emerald-400"
            style={{ width: `${pct}%` }}
          />
        </div>
      </Link>
    )
  }

  // Error — error chip
  if (status.lastUploadError) {
    return (
      <Link
        to={linkTo}
        className="glass-card glass-card-hover cloud-bar group transition-colors"
      >
        <span className="halo-amber inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-lg">
          <AlertTriangle className="h-4 w-4" />
        </span>
        <div className="min-w-0 flex-1">
          <div className="t-md">SentryCloud — last upload failed</div>
          <div className="t-xs truncate">{status.lastUploadError}</div>
        </div>
        <ChevronRight className="h-4 w-4 text-slate-600" />
      </Link>
    )
  }

  // Paired + idle — compact summary
  return (
    <Link
      to={linkTo}
      className="glass-card glass-card-hover cloud-bar group transition-colors"
    >
      <span className="halo-accent inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-lg">
        <Cloud className="h-4 w-4" />
      </span>
      <div className="min-w-0 flex-1">
        <div className="t-md flex items-center gap-2">
          SentryCloud
          <Pill kind="accent">PAIRED</Pill>
        </div>
        <div className="t-xs">
          {status.totalUploadedRouteCount.toLocaleString()} routes uploaded
          {status.lastUploadAt && (
            <> · last {new Date(status.lastUploadAt).toLocaleString()}</>
          )}
        </div>
      </div>
      <ChevronRight className="h-4 w-4 text-slate-600 transition-transform group-hover:translate-x-0.5" />
    </Link>
  )
}
