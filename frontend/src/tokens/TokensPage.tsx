import AddIcon from '@mui/icons-material/Add'
import BlockIcon from '@mui/icons-material/Block'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import Chip from '@mui/material/Chip'
import FormControlLabel from '@mui/material/FormControlLabel'
import IconButton from '@mui/material/IconButton'
import LinearProgress from '@mui/material/LinearProgress'
import Paper from '@mui/material/Paper'
import Switch from '@mui/material/Switch'
import Table from '@mui/material/Table'
import TableBody from '@mui/material/TableBody'
import TableCell from '@mui/material/TableCell'
import TableContainer from '@mui/material/TableContainer'
import TableHead from '@mui/material/TableHead'
import TableRow from '@mui/material/TableRow'
import Tooltip from '@mui/material/Tooltip'
import Typography from '@mui/material/Typography'
import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useSearchParams } from 'react-router'
import type { ApiToken, CreatedToken } from '../api/client'
import { EmptyState } from '../pages/EmptyState'
import { ErrorAlert } from '../pages/ErrorAlert'
import { PageHeader } from '../pages/PageHeader'
import { Time } from '../pages/Time'
import { visuallyHidden } from '../pages/visuallyHidden'
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
            variant="contained"
            startIcon={<AddIcon />}
            onClick={() => setCreating((c) => ({ open: true, key: c.key + 1 }))}
          >
            {t('tokens.create.action')}
          </Button>
        }
      />

      <Box sx={{ display: 'flex', mb: 1 }}>
        <FormControlLabel
          control={
            <Switch
              checked={view.revoked}
              onChange={(event) =>
                setSearchParams(
                  tokensViewToParams({ revoked: event.target.checked }),
                )
              }
            />
          }
          label={t('tokens.showRevoked')}
        />
      </Box>

      <Box sx={{ height: 4, mb: 1 }}>
        {tokens.isFetching && (
          <LinearProgress aria-label={t('common.loading')} />
        )}
      </Box>

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
          <Typography variant="body2" color="text.secondary" sx={{ my: 1.5 }}>
            {t('tokens.count', { count: shown.length })}
            {!view.revoked &&
              revokedCount > 0 &&
              ` · ${t('tokens.hiddenCount', { count: revokedCount })}`}
          </Typography>
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
    <TableContainer component={Paper} variant="outlined">
      <Table size="small" aria-label={t('nav.items.tokens')}>
        <TableHead>
          <TableRow>
            <TableCell>{t('tokens.columns.name')}</TableCell>
            <TableCell>{t('tokens.columns.scope')}</TableCell>
            <TableCell>{t('tokens.columns.created')}</TableCell>
            <TableCell>{t('tokens.columns.lastUsed')}</TableCell>
            <TableCell>{t('tokens.columns.status')}</TableCell>
            <TableCell align="right">
              <Box component="span" sx={visuallyHidden}>
                {t('tokens.columns.actions')}
              </Box>
            </TableCell>
          </TableRow>
        </TableHead>
        <TableBody>
          {rows.map((token) => (
            <TableRow key={token.id}>
              <TableCell
                sx={{
                  minWidth: 160,
                  overflowWrap: 'anywhere',
                  fontWeight: 500,
                  color: isRevoked(token) ? 'text.secondary' : undefined,
                }}
              >
                {token.name}
              </TableCell>
              <TableCell>
                {/* Describes the chip; as its label it would replace the
                    scope's name for screen readers. */}
                <Tooltip
                  describeChild
                  title={t(`tokens.scopeHelp.${token.scope}`)}
                >
                  <Chip
                    size="small"
                    variant="outlined"
                    label={t(`tokens.scope.${token.scope}`)}
                  />
                </Tooltip>
              </TableCell>
              <TableCell sx={{ whiteSpace: 'nowrap' }}>
                <Time iso={token.created_at} />
              </TableCell>
              <TableCell sx={{ whiteSpace: 'nowrap' }}>
                {token.last_used_at ? (
                  <Time iso={token.last_used_at} />
                ) : (
                  <Typography
                    component="span"
                    variant="body2"
                    color="text.secondary"
                  >
                    {t('tokens.neverUsed')}
                  </Typography>
                )}
              </TableCell>
              <TableCell sx={{ whiteSpace: 'nowrap' }}>
                {token.revoked_at ? (
                  <Box
                    sx={{ display: 'flex', alignItems: 'center', gap: 1 }}
                  >
                    <Chip size="small" label={t('tokens.status.revoked')} />
                    <Time iso={token.revoked_at} />
                  </Box>
                ) : (
                  <Chip
                    size="small"
                    variant="outlined"
                    color="success"
                    label={t('tokens.status.active')}
                  />
                )}
              </TableCell>
              <TableCell align="right" sx={{ whiteSpace: 'nowrap' }}>
                {!isRevoked(token) && (
                  <Tooltip title={t('tokens.revoke.action')}>
                    <IconButton
                      size="small"
                      aria-label={t('tokens.revoke.named', {
                        name: token.name,
                      })}
                      onClick={() => onRevoke(token)}
                    >
                      <BlockIcon fontSize="small" />
                    </IconButton>
                  </Tooltip>
                )}
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </TableContainer>
  )
}
