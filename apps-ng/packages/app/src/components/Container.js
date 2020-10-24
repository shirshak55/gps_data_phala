import styled from 'styled-components'
import * as constants from '@/utils/style/constants'
import { useMediaQuery } from '@zeit-ui/react'

const Container = styled.div`
  display: flex;
  max-width: 1232px;
  min-width: ${constants.PAGE_MIN_WIDTH}px;
  ${({ isMobile }) => isMobile && `
    min-width: unset;
    max-width: unset;
  `}
  width: 100%;
  padding: 0 ${constants.CONTAINER_PADDING}px;
  margin: 0 auto;
  flex-flow: column nowrap;
`

export default (props) => {
  const isMobile = useMediaQuery('mobile')

  return <Container isMobile={isMobile} {...props} />
}
