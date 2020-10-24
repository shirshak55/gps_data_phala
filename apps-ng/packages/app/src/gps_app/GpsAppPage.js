import React, { useEffect, useState, useMemo } from "react"
import styled from "styled-components"
import { observer } from "mobx-react"
import { Button, Input, Spacer, useInput, useToasts } from "@zeit-ui/react"
import { Plus as PlusIcon } from "@zeit-ui/react-icons"

import { useStore } from "@/store"
import Container from "@/components/Container"
import UnlockRequired from "@/components/accounts/UnlockRequired"
import PushCommandButton from "@/components/PushCommandButton"

import { CONTRACT_GPS_APP, createGpsStore } from "./AppStore"
import { reaction } from "mobx"

const ButtonWrapper = styled.div`
  margin-top: 5px;
  width: 200px;
`

const AppHeader = () => (
  <Container>
    <h1>GPS App!</h1>
  </Container>
)

const AppBody = observer(() => {
  const { appRuntime, gpsApp } = useStore()
  const [, setToast] = useToasts()
  const { state: latitude, bindings: latBindings } = useInput(0.0)
  const { state: longitude, bindings: longBindings } = useInput(0.0)

  async function getLocationDataFromStore() {
    console.log("getting location data from store")
    if (!gpsApp) {
      console.log("gps app instance not found")
      return
    }
    try {
      const response = await gpsApp.queryLocation(appRuntime)
      // Print the response in the original to the console
      console.log("Response::GetLocation", response)

      // Feeling lazy to destructure so not using modern js for following line :D
      gpsApp.setLocation(
        response.Location.latitude,
        response.Location.longitude,
      )
    } catch (err) {
      console.log("err", err)
      setToast(err.message, "error")
    }
  }

  return (
    <Container>
      <section>
        <div>PRuntime : {appRuntime ? "yes" : "no"}</div>
        <div>PRuntime ping: {appRuntime.latency || "+∞"}</div>
        <div>PRuntime connected: {appRuntime?.channelReady ? "yes" : "no"}</div>
      </section>
      <Spacer y={1} />

      <h3>GPS Location</h3>
      <section>
        <div>
          Location :{gpsApp.latitude === null ? "" : gpsApp.latitude}{" "}
          {gpsApp.longitude === null ? "" : gpsApp.longitude}
        </div>
        <div>
          <Button onClick={() => getLocationDataFromStore()}>
            Get Location
          </Button>
        </div>
      </section>
      <Spacer y={1} />

      <h3>
        Set GPS {latitude} {longitude}
      </h3>
      <section>
        <div>
          <div>Latitude</div>
          <Input  type="number" label="By" {...latBindings} />
        </div>
        <div>
          <div>Longitude</div>
          <Input  type="number" label="By" {...longBindings} />
        </div>

        <ButtonWrapper>
          <PushCommandButton
            // tx arguments
            contractId={6}
            payload={{
              SetLocation: {
                latitude: isNaN(latitude)? 0.0 : parseFloat(latitude),
                longitude: isNaN(longitude)? 0.0 : parseFloat(longitude),
              },
            }}
            // display messages
            modalTitle="GpsApp.SetLocation()"
            modalSubtitle={`Set the Gps location to ${latitude} ${longitude}`}
            onSuccessMsg="Tx succeeded"
            // button appearance
            buttonType="secondaryLight"
            icon={PlusIcon}
            name="Send"
          />
        </ButtonWrapper>
      </section>
    </Container>
  )
})

/**
 * Injects the mobx store to the global state once initialized
 */
const StoreInjector = observer(({ children }) => {
  const appStore = useStore()
  const [shouldRenderContent, setShouldRenderContent] = useState(false)

  useEffect(() => {
    if (!appStore || !appStore.appRuntime) return
    if (typeof appStore.gpsApp !== "undefined") return
    appStore.gpsApp = createGpsStore({
      appRuntime: appStore.appRuntime,
    })
  }, [appStore])

  useEffect(() =>
    reaction(
      () => appStore.gpsApp,
      () => {
        if (appStore.gpsApp && !shouldRenderContent) {
          setShouldRenderContent(true)
        }
      },
      { fireImmediately: true },
    ),
  )

  return shouldRenderContent && children
})

export default () => (
  <UnlockRequired>
    <StoreInjector>
      <AppHeader />
      <AppBody />
    </StoreInjector>
  </UnlockRequired>
)
