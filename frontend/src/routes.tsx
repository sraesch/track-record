import { Navigate, type RouteObject } from 'react-router'
import { AppLayout } from './layout/AppLayout'
import { homePath, navItems } from './navigation'
import { ErrorPage } from './pages/ErrorPage'
import { NotFoundPage } from './pages/NotFoundPage'
import { PlaceholderPage } from './pages/PlaceholderPage'
import { RunDetailPage } from './pages/RunDetailPage'

// All routes of the app. Every view has its own URL, and view state (filters,
// selections) will live in the URL's query string (FR-21).
export const routes: RouteObject[] = [
  {
    element: <AppLayout />,
    errorElement: <ErrorPage />,
    children: [
      { index: true, element: <Navigate to={homePath} replace /> },
      ...navItems.map(
        (item): RouteObject => ({
          path: item.path,
          element: <PlaceholderPage id={item.id} />,
        }),
      ),
      { path: '/runs/:runId', element: <RunDetailPage /> },
      { path: '*', element: <NotFoundPage /> },
    ],
  },
]
