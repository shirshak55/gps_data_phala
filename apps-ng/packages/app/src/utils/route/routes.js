import PhalaWalletPage from '@phala/wallet'
import AppSettingsPage from '@/components/SettingsPage'
import HelloWorldAppPage from '@phala/helloworld-app'
import GpsAppPage from '@/gps_app/GpsAppPage'

export const COMPONENT_ROUTES = {
  wallet: PhalaWalletPage,
  settings: AppSettingsPage,
  helloworldapp: HelloWorldAppPage,
  gpsapp: GpsAppPage
}

export const MENU_ROUTES = {
  WALLET: '/wallet',
  SETTINGS: '/settings',
  HELLOWORLDAPP: '/helloworldapp',
  GPSAPP: '/gpsapp',
}
