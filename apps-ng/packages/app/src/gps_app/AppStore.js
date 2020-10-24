import { types, flow } from "mobx-state-tree"

export const CONTRACT_GPS_APP = 6

export const createGpsStore = (defaultValue = {}, options = {}) => {
  const GpsAppStore = types
    .model("GpsAppStore", {
      longitude: types.maybeNull(types.number),
      latitude: types.maybeNull(types.number),
    })
    .actions((self) => ({
      setLocation(lat, long) {
        self.longitude = lat
        self.latitude = long
      },
      async queryLocation(runtime) {
        return await runtime.query(CONTRACT_GPS_APP, "GetLocation", () => ({
          account: defaultValue.appRuntime.accountIdHex,
        }))
      },
    }))

  return GpsAppStore.create(defaultValue)
}
