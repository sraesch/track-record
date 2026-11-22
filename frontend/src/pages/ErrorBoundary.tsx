import { Component, type ReactNode } from 'react'

type Props = {
  children: ReactNode
  /** Shown instead of the children once they failed to render. */
  fallback: (error: Error) => ReactNode
}

type State = { error: Error | null }

/**
 * Keeps a failing part of a page (e.g. a lazily loaded view whose code
 * could not be fetched) from taking the whole page down.
 */
export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null }

  static getDerivedStateFromError(error: unknown): State {
    return { error: error instanceof Error ? error : new Error(String(error)) }
  }

  render() {
    return this.state.error
      ? this.props.fallback(this.state.error)
      : this.props.children
  }
}
