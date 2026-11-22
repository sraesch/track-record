import type { ReactElement } from 'react'
import { Navigate, type RouteObject } from 'react-router'
import { AliasesPage } from './aliases/AliasesPage'
import { AppLayout } from './layout/AppLayout'
import { homePath, type NavItemId, navItems } from './navigation'
import { AttachmentPage } from './attachments/AttachmentPage'
import { CaseExecutionPage } from './runs/case/CaseExecutionPage'
import { ErrorPage } from './pages/ErrorPage'
import { NotFoundPage } from './pages/NotFoundPage'
import { PlaceholderPage } from './pages/PlaceholderPage'
import { RunDetailPage } from './runs/detail/RunDetailPage'
import { RunListPage } from './runs/RunListPage'

// The page of each view in the side menu; views without one show a
// placeholder until they are built.
const pages: Partial<Record<NavItemId, ReactElement>> = {
  runs: <RunListPage />,
  aliases: <AliasesPage />,
}

// All routes of the app. Every view has its own URL, and view state (filters,
// selections) lives in the URL's query string (FR-21).
export const routes: RouteObject[] = [
  {
    element: <AppLayout />,
    errorElement: <ErrorPage />,
    children: [
      { index: true, element: <Navigate to={homePath} replace /> },
      ...navItems.map(
        (item): RouteObject => ({
          path: item.path,
          element: pages[item.id] ?? <PlaceholderPage id={item.id} />,
        }),
      ),
      { path: '/runs/:runId', element: <RunDetailPage /> },
      {
        path: '/runs/:runId/cases/:caseExecutionId',
        element: <CaseExecutionPage />,
      },
      {
        path: '/runs/:runId/attachments/:attachmentId',
        element: <AttachmentPage />,
      },
      { path: '*', element: <NotFoundPage /> },
    ],
  },
]
