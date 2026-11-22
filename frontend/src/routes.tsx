import type { ReactElement } from 'react'
import { Navigate, type RouteObject } from 'react-router'
import { AboutPage } from './about/AboutPage'
import { AliasesPage } from './aliases/AliasesPage'
import { MetricsPage } from './metrics/MetricsPage'
import { AppLayout } from './layout/AppLayout'
import { aboutPath, homePath, type NavItemId, navItems } from './navigation'
import { AttachmentPage } from './attachments/AttachmentPage'
import { CaseExecutionPage } from './runs/case/CaseExecutionPage'
import { DerivedMetricsPage } from './derived/DerivedMetricsPage'
import { ErrorPage } from './pages/ErrorPage'
import { NotFoundPage } from './pages/NotFoundPage'
import { PlaceholderPage } from './pages/PlaceholderPage'
import { RunDetailPage } from './runs/detail/RunDetailPage'
import { RunListPage } from './runs/RunListPage'
import { SuitesPage } from './suites/SuitesPage'
import { TokensPage } from './tokens/TokensPage'

// The page of each view in the side menu; views without one show a
// placeholder until they are built.
const pages: Partial<Record<NavItemId, ReactElement>> = {
  runs: <RunListPage />,
  suites: <SuitesPage />,
  metrics: <MetricsPage />,
  aliases: <AliasesPage />,
  derivedMetrics: <DerivedMetricsPage />,
  tokens: <TokensPage />,
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
      { path: aboutPath, element: <AboutPage /> },
      { path: '*', element: <NotFoundPage /> },
    ],
  },
]
