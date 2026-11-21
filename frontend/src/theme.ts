import { createTheme } from '@mui/material/styles'

// Light and dark color schemes as CSS variables. The scheme is selected by a
// data attribute on <html>, so the user can override the system preference
// (ColorModeMenu); MUI stores the choice in localStorage.
export const theme = createTheme({
  cssVariables: { colorSchemeSelector: 'data-mui-color-scheme' },
  colorSchemes: {
    light: {
      palette: {
        primary: { main: '#1f5fa8' },
        background: { default: '#f6f7f9' },
      },
    },
    dark: {
      palette: {
        primary: { main: '#7fb0ea' },
      },
    },
  },
  typography: {
    fontFamily: [
      'system-ui',
      '-apple-system',
      '"Segoe UI"',
      'Roboto',
      'sans-serif',
    ].join(','),
  },
  shape: { borderRadius: 8 },
})

export const drawerWidth = 248
