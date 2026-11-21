import CssBaseline from '@mui/material/CssBaseline'
import { ThemeProvider } from '@mui/material/styles'
import { type createBrowserRouter, RouterProvider } from 'react-router'
import { theme } from './theme'

type Props = {
  /** A browser router in the app, a memory router in tests. */
  router: ReturnType<typeof createBrowserRouter>
}

function App({ router }: Props) {
  return (
    <ThemeProvider theme={theme}>
      <CssBaseline enableColorScheme />
      <RouterProvider router={router} />
    </ThemeProvider>
  )
}

export default App
