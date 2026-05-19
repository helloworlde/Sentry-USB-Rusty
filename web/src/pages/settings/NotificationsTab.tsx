import { PrefGrid } from "@/components/settings/PrefCard"
import { MobileNotificationsSection } from "@/components/settings/sections/MobileNotificationsSection"
import { CommunityFeaturesSection } from "@/components/settings/sections/CommunityFeaturesSection"

export function NotificationsTab() {
  return (
    <PrefGrid min={300}>
      <MobileNotificationsSection />
      <CommunityFeaturesSection />
    </PrefGrid>
  )
}
