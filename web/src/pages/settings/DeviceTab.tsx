import { PrefGrid } from "@/components/settings/PrefCard"
import { KeepAwakePreference } from "@/components/settings/sections/KeepAwakePreference"
import { KeepAccessorySection } from "@/components/settings/sections/KeepAccessorySection"
import { BleEnableToggle } from "@/components/settings/sections/BleEnableToggle"
import { BlePairButton } from "@/components/settings/sections/BlePairButton"
import { DisplayUnitsSection } from "@/components/settings/sections/DisplayUnitsSection"

export function DeviceTab() {
  return (
    <PrefGrid>
      <KeepAwakePreference />
      <BleEnableToggle />
      <BlePairButton />
      <DisplayUnitsSection />
      <KeepAccessorySection />
    </PrefGrid>
  )
}
