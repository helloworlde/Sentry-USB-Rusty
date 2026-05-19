import { Wifi, Cable } from "lucide-react"
import { PrefCard, PrefGrid } from "@/components/settings/PrefCard"
import { Row } from "@/components/ui/StatusTile"
import { Pill, LiveDot } from "@/components/ui/Pill"
import CloudPairingSection from "@/components/CloudPairingSection"
import { useAwayMode } from "@/hooks/useAwayMode"
import type { PiStatus } from "@/lib/api"

interface Props {
  status: PiStatus | null
}

export function NetworkTab({ status }: Props) {
  const { status: awayStatus } = useAwayMode()
  const wifiConnected = !!status?.wifi_ssid
  const ethConnected =
    !!status?.ether_speed && status.ether_speed !== "Unknown!"

  return (
    <PrefGrid>
      <PrefCard
        icon={<Wifi className="h-3.5 w-3.5" />}
        halo={wifiConnected ? "accent" : "slate"}
        title="WiFi"
        badge={wifiConnected ? <Pill kind="accent">Connected</Pill> : null}
      >
        {wifiConnected && status ? (
          <>
            <div className="t-md font-semibold">{status.wifi_ssid}</div>
            <Row label="IP" value={<span className="t-mono">{status.wifi_ip || "—"}</span>} />
            {status.wifi_strength && (
              <Row label="Signal" value={status.wifi_strength} />
            )}
          </>
        ) : (
          <p className="t-xs">
            No WiFi configured. Use the Setup Wizard to scan and connect.
          </p>
        )}
      </PrefCard>

      <PrefCard
        icon={<Cable className="h-3.5 w-3.5" />}
        halo={ethConnected ? "accent" : "slate"}
        title="Ethernet"
        badge={ethConnected && status ? <Pill kind="accent">{status.ether_speed}</Pill> : null}
      >
        {ethConnected && status ? (
          <>
            <Row label="IP" value={<span className="t-mono">{status.ether_ip || "—"}</span>} />
            <Row label="Link" value={status.ether_speed} />
          </>
        ) : (
          <p className="t-xs">No Ethernet link detected.</p>
        )}
      </PrefCard>

      {awayStatus.state === "active" && (
        <PrefCard
          icon={<Wifi className="h-3.5 w-3.5" />}
          halo="blue"
          title="Away Mode AP"
          badge={
            <Pill kind="sky">
              <LiveDot /> Live
            </Pill>
          }
        >
          {awayStatus.ap_ssid && <Row label="SSID" value={awayStatus.ap_ssid} />}
          {awayStatus.ap_ip && (
            <Row
              label="IP"
              value={<span className="t-mono">{awayStatus.ap_ip}</span>}
            />
          )}
          <p className="t-xs">
            Connect to this network to reach the UI while Away Mode is active.
          </p>
        </PrefCard>
      )}

      <div className="md:col-span-2">
        <CloudPairingSection />
      </div>
    </PrefGrid>
  )
}
