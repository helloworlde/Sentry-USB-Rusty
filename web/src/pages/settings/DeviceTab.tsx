import { PrefGrid } from "@/components/settings/PrefCard"
import { KeepAwakePreference } from "@/components/settings/sections/KeepAwakePreference"
import { KeepAccessorySection } from "@/components/settings/sections/KeepAccessorySection"
import { DisplayUnitsSection } from "@/components/settings/sections/DisplayUnitsSection"
import { UpdateSection } from "@/components/settings/sections/UpdateSection"

export function DeviceTab() {
  return (
    <PrefGrid>
      <KeepAwakePreference />
      <DisplayUnitsSection />
      <KeepAccessorySection />
      <UpdateSection />
    </PrefGrid>
  )
}
