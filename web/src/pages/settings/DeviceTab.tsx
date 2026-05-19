import { PrefGrid } from "@/components/settings/PrefCard"
import { KeepAwakePreference } from "@/components/settings/sections/KeepAwakePreference"
import { AwayModeControl } from "@/components/settings/sections/AwayModeControl"
import { BlePairButton } from "@/components/settings/sections/BlePairButton"
import { DisplayUnitsSection } from "@/components/settings/sections/DisplayUnitsSection"

interface Props {
  /** Show BLE card only when this Pi configuration supports it. */
  usesBle: boolean
}

export function DeviceTab({ usesBle }: Props) {
  return (
    <PrefGrid>
      <KeepAwakePreference />
      <AwayModeControl />
      {usesBle && <BlePairButton />}
      <DisplayUnitsSection />
    </PrefGrid>
  )
}
