import { RadioGroup, TextField } from '@graphite/react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { ApiToken, CreatedToken, TokenScope } from '../api/client'
import { formatDateTime } from '../format'
import { ConfirmDialog } from '../pages/ConfirmDialog'
import { CopyButton } from '../pages/CopyButton'
import { Alert, Button, Dialog, Stack, TextArea } from '../ui'
import { useCreateToken, useRevokeToken } from './queries'
import { nameInUse, tokenScopes } from './tokenModel'
import './tokens.css'

type CreateProps = {
  open: boolean
  /** All tokens, to tell that a name is in use. */
  tokens: readonly ApiToken[]
  onClose: () => void
  /** Called with the new token and its secret; the caller shows it. */
  onCreated: (token: CreatedToken) => void
}

/** Asks for the name and the scope of a new token and creates it. */
export function CreateTokenDialog({
  open,
  tokens,
  onClose,
  onCreated,
}: CreateProps) {
  const { t } = useTranslation()
  const [name, setName] = useState('')
  // The least a token can do, unless more is chosen.
  const [scope, setScope] = useState<TokenScope>('read')
  const create = useCreateToken()

  return (
    <ConfirmDialog
      open={open}
      title={t('tokens.create.title')}
      confirmLabel={t('tokens.create.confirm')}
      canConfirm={name.trim() !== ''}
      pending={create.isPending}
      error={create.error}
      onClose={onClose}
      onConfirm={() =>
        create.mutate({ name: name.trim(), scope }, { onSuccess: onCreated })
      }
    >
      <TextField
        label={t('tokens.columns.name')}
        value={name}
        onChange={(event) => setName(event.target.value)}
        help={
          nameInUse(tokens, name)
            ? t('tokens.create.nameInUse')
            : t('tokens.create.nameHelp')
        }
        autoComplete="off"
      />
      <RadioGroup<TokenScope>
        label={t('tokens.columns.scope')}
        showLabel
        className="tr-tokens-scopes"
        value={scope}
        onChange={setScope}
        options={tokenScopes.map((value) => ({
          value,
          label: (
            <span className="tr-tokens-scope">
              <span>{t(`tokens.scope.${value}`)}</span>
              <span className="ui-secondary">
                {t(`tokens.scopeHelp.${value}`)}
              </span>
            </span>
          ),
        }))}
      />
      <p className="ui-text ui-secondary">{t('tokens.create.limits')}</p>
    </ConfirmDialog>
  )
}

type CreatedProps = {
  token: CreatedToken
  open: boolean
  onClose: () => void
  /** Called once the dialog has closed, to drop the secret. */
  onExited: () => void
}

/**
 * Shows the secret of a token that was just created. This is the only time
 * it can be read, so the dialog closes only with its button, not with
 * Escape or a click next to it.
 */
export function CreatedTokenDialog({
  token,
  open,
  onClose,
  onExited,
}: CreatedProps) {
  const { t } = useTranslation()
  return (
    // Without onClose, Escape and the backdrop do not close it. The secret
    // is the first field, so it gets the focus, which selects it: ready to
    // copy with the keyboard, also where the clipboard API is missing.
    <Dialog
      open={open}
      title={t('tokens.created.title', { name: token.name })}
      onExited={onExited}
      actions={
        <Button variant="primary" onClick={onClose}>
          {t('tokens.created.done')}
        </Button>
      }
    >
      <Alert tone="warning">{t('tokens.created.once')}</Alert>
      <Stack row gap={1} align="flex-end">
        <TextArea
          label={t('tokens.created.secret')}
          value={token.secret}
          readOnly
          spellCheck={false}
          rows={2}
          className="ui-grow ui-mono-input tr-tokens-secret"
          onFocus={(event) => event.currentTarget.select()}
        />
        <CopyButton text={token.secret} label={t('tokens.created.copy')} />
      </Stack>
      <p className="ui-text">
        {t('tokens.created.usage', {
          scope: t(`tokens.scope.${token.scope}`),
        })}{' '}
        <code className="ui-code">{t('tokens.created.header')}</code>
      </p>
    </Dialog>
  )
}

type RevokeProps = {
  token: ApiToken
  open: boolean
  onClose: () => void
  /** Called once the dialog has closed. */
  onExited: () => void
}

/** Asks before a token is revoked: it cannot be made valid again. */
export function RevokeTokenDialog({
  token,
  open,
  onClose,
  onExited,
}: RevokeProps) {
  const { t } = useTranslation()
  const revoke = useRevokeToken()
  return (
    <ConfirmDialog
      open={open}
      title={t('tokens.revoke.title', { name: token.name })}
      confirmLabel={t('tokens.revoke.confirm')}
      destructive
      pending={revoke.isPending}
      error={revoke.error}
      onClose={onClose}
      onExited={onExited}
      onConfirm={() => revoke.mutate(token.id, { onSuccess: onClose })}
    >
      <Stack gap={3}>
        <p className="ui-text">{t('tokens.revoke.text')}</p>
        <p className="ui-text ui-secondary">
          {token.last_used_at
            ? t('tokens.revoke.lastUsed', {
                time: formatDateTime(token.last_used_at),
              })
            : t('tokens.revoke.neverUsed')}
        </p>
      </Stack>
    </ConfirmDialog>
  )
}
