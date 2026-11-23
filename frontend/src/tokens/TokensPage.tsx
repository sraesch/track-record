import { cx, Switch } from '@graphite/react'
import { Ban, CircleCheck, Plus } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useSearchParams } from 'react-router'
import type { ApiToken, CreatedToken } from '../api/client'
import { EmptyState } from '../pages/EmptyState'
import { ErrorAlert } from '../pages/ErrorAlert'
import { PageHeader } from '../pages/PageHeader'
import { Time } from '../pages/Time'
import {
  Button,
  Chip,
  IconButton,
  ProgressBar,
  Table,
  Tooltip,
  VisuallyHidden,
} from '../ui'
import { useTokens } from './queries'
import {
  CreateTokenDialog,
  CreatedTokenDialog,
  RevokeTokenDialog,
} from './TokenDialogs'
import {
  isRevoked,
  parseTokensView,
  shownTokens,
  tokensViewToParams,
} from './tokenModel'
import './tokens.css'

/**
 * API tokens (NFR-2) for the machine clients: the harness and MCP clients.
 * They are created and revoked here and nowhere else. A secret is shown
 * once, right after its token is created; the server keeps a hash. Revoked
 * tokens are kept and listed on request, which is in the URL (FR-21).
 */
export function TokensPage() {
  const { t } = useTranslation()
  const [searchParams, setSearchParams] = useSearchParams()
  const view = useMemo(() => parseTokensView(searchParams), [searchParams])
  const tokens = useTokens()
  const all = useMemo(() => tokens.data ?? [], [tokens.data])
  const shown = useMemo(() => shownTokens(all, view), [all, view])
  const revokedCount = useMemo(() => all.filter(isRevoked).length, [all])

  // The key gives every opening of the dialog empty inputs.
  const [creating, setCreating] = useState({ open: false, key: 0 })
  // Both dialogs keep their token while they close. The created one holds
  // the secret, which is dropped once its dialog is gone.
  const [created, setCreated] = useState<{
    token: CreatedToken
    open: boolean
  }>()
  const [revoking, setRevoking] = useState<{ token: ApiToken; open: boolean }>()

  return (
    <>
      <PageHeader
        title={t('nav.items.tokens')}
        description={t('pages.tokens.description')}
        actions={
          <Button
            variant="primary"
            icon={Plus}
            onClick={() => setCreating((c) => ({ open: true, key: c.key + 1 }))}
          >
            {t('tokens.create.action')}
          </Button>
        }
      />

      <div className="tr-tokens-toolbar">
        <Switch
          label={t('tokens.showRevoked')}
          checked={view.revoked}
          onCheckedChange={(revoked) =>
            setSearchParams(tokensViewToParams({ revoked }))
          }
        />
      </div>

      <div className="tr-tokens-progress">
        {tokens.isFetching && <ProgressBar />}
      </div>

      {tokens.isError && !tokens.data ? (
        <ErrorAlert
          message={t('tokens.error', { message: tokens.error.message })}
          onRetry={() => tokens.refetch()}
        />
      ) : !tokens.data ? null : shown.length === 0 ? (
        <EmptyState>
          {all.length === 0
            ? t('tokens.empty')
            : t('tokens.noActive', { count: revokedCount })}
        </EmptyState>
      ) : (
        <>
          <TokenTable
            rows={shown}
            onRevoke={(token) => setRevoking({ token, open: true })}
          />
          <p className="ui-text ui-secondary tr-tokens-count">
            {t('tokens.count', { count: shown.length })}
            {!view.revoked &&
              revokedCount > 0 &&
              ` · ${t('tokens.hiddenCount', { count: revokedCount })}`}
          </p>
        </>
      )}

      <CreateTokenDialog
        key={creating.key}
        open={creating.open}
        tokens={all}
        onClose={() => setCreating((c) => ({ ...c, open: false }))}
        onCreated={(token) => {
          setCreating((c) => ({ ...c, open: false }))
          setCreated({ token, open: true })
        }}
      />
      {created && (
        <CreatedTokenDialog
          token={created.token}
          open={created.open}
          onClose={() => setCreated({ ...created, open: false })}
          onExited={() => setCreated(undefined)}
        />
      )}
      {revoking && (
        <RevokeTokenDialog
          token={revoking.token}
          open={revoking.open}
          onClose={() => setRevoking({ ...revoking, open: false })}
          onExited={() => setRevoking(undefined)}
        />
      )}
    </>
  )
}

type TableProps = {
  rows: readonly ApiToken[]
  onRevoke: (token: ApiToken) => void
}

function TokenTable({ rows, onRevoke }: TableProps) {
  const { t } = useTranslation()
  return (
    <Table framed aria-label={t('nav.items.tokens')}>
      <thead>
        <tr>
          <th>{t('tokens.columns.name')}</th>
          <th>{t('tokens.columns.scope')}</th>
          <th>{t('tokens.columns.created')}</th>
          <th>{t('tokens.columns.lastUsed')}</th>
          <th>{t('tokens.columns.status')}</th>
          <th className="g-r">
            <VisuallyHidden>{t('tokens.columns.actions')}</VisuallyHidden>
          </th>
        </tr>
      </thead>
      <tbody>
        {rows.map((token) => (
          <tr key={token.id}>
            <td
              className={cx(
                'ui-wrap-cell tr-tokens-name',
                isRevoked(token) && 'tr-tokens-revoked',
              )}
            >
              {token.name}
            </td>
            <td>
              {/* Describes the chip; as its label it would replace the
                  scope's name for screen readers. */}
              <Tooltip title={t(`tokens.scopeHelp.${token.scope}`)}>
                <Chip outlined>{t(`tokens.scope.${token.scope}`)}</Chip>
              </Tooltip>
            </td>
            <td>
              <Time iso={token.created_at} />
            </td>
            <td>
              {token.last_used_at ? (
                <Time iso={token.last_used_at} />
              ) : (
                <span className="ui-secondary">{t('tokens.neverUsed')}</span>
              )}
            </td>
            <td>
              {token.revoked_at ? (
                <span className="ui-inline-row tr-tokens-gap">
                  <Chip icon={Ban}>{t('tokens.status.revoked')}</Chip>
                  <Time iso={token.revoked_at} />
                </span>
              ) : (
                <Chip tone="success" icon={CircleCheck}>
                  {t('tokens.status.active')}
                </Chip>
              )}
            </td>
            <td className="g-r">
              {!isRevoked(token) && (
                <IconButton
                  size="sm"
                  icon={Ban}
                  label={t('tokens.revoke.named', { name: token.name })}
                  tooltip={t('tokens.revoke.action')}
                  onClick={() => onRevoke(token)}
                />
              )}
            </td>
          </tr>
        ))}
      </tbody>
    </Table>
  )
}
