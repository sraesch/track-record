import Alert from '@mui/material/Alert'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import Dialog from '@mui/material/Dialog'
import DialogActions from '@mui/material/DialogActions'
import DialogContent from '@mui/material/DialogContent'
import DialogTitle from '@mui/material/DialogTitle'
import FormControl from '@mui/material/FormControl'
import FormControlLabel from '@mui/material/FormControlLabel'
import FormLabel from '@mui/material/FormLabel'
import InputAdornment from '@mui/material/InputAdornment'
import Radio from '@mui/material/Radio'
import RadioGroup from '@mui/material/RadioGroup'
import TextField from '@mui/material/TextField'
import Typography from '@mui/material/Typography'
import { useId, useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { ApiToken, CreatedToken, TokenScope } from '../api/client'
import { formatDateTime } from '../format'
import { ConfirmDialog } from '../pages/ConfirmDialog'
import { CopyButton } from '../pages/CopyButton'
import { codeSx } from '../pages/styles'
import { useCreateToken, useRevokeToken } from './queries'
import { nameInUse, tokenScopes } from './tokenModel'

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
  const scopeId = useId()
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
      <Box sx={{ display: 'grid', gap: 2, pt: 1 }}>
        <TextField
          label={t('tokens.columns.name')}
          value={name}
          onChange={(event) => setName(event.target.value)}
          helperText={
            nameInUse(tokens, name)
              ? t('tokens.create.nameInUse')
              : t('tokens.create.nameHelp')
          }
          autoComplete="off"
        />
        <FormControl>
          <FormLabel id={scopeId}>{t('tokens.columns.scope')}</FormLabel>
          <RadioGroup
            aria-labelledby={scopeId}
            value={scope}
            onChange={(event) => setScope(event.target.value as TokenScope)}
          >
            {tokenScopes.map((value) => (
              <FormControlLabel
                key={value}
                value={value}
                control={<Radio />}
                sx={{ alignItems: 'flex-start', mt: 0.5 }}
                label={
                  <Box sx={{ pt: 1 }}>
                    <Typography>{t(`tokens.scope.${value}`)}</Typography>
                    <Typography variant="body2" color="text.secondary">
                      {t(`tokens.scopeHelp.${value}`)}
                    </Typography>
                  </Box>
                }
              />
            ))}
          </RadioGroup>
        </FormControl>
        <Typography variant="body2" color="text.secondary">
          {t('tokens.create.limits')}
        </Typography>
      </Box>
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
  const titleId = useId()
  return (
    <Dialog
      open={open}
      aria-labelledby={titleId}
      fullWidth
      maxWidth="sm"
      slotProps={{
        transition: {
          // The secret gets the focus, which selects it: ready to copy
          // with the keyboard, also where the clipboard API is missing.
          onEntered: (node: HTMLElement) =>
            node.querySelector<HTMLElement>('textarea')?.focus(),
          onExited,
        },
      }}
    >
      <DialogTitle id={titleId}>
        {t('tokens.created.title', { name: token.name })}
      </DialogTitle>
      <DialogContent>
        <Box sx={{ display: 'grid', gap: 2 }}>
          <Alert severity="warning">{t('tokens.created.once')}</Alert>
          <TextField
            label={t('tokens.created.secret')}
            value={token.secret}
            multiline
            fullWidth
            slotProps={{
              htmlInput: {
                readOnly: true,
                spellCheck: false,
                sx: codeSx,
                onFocus: (event: React.FocusEvent<HTMLTextAreaElement>) =>
                  event.currentTarget.select(),
              },
              input: {
                endAdornment: (
                  <InputAdornment position="end">
                    <CopyButton
                      text={token.secret}
                      label={t('tokens.created.copy')}
                    />
                  </InputAdornment>
                ),
              },
            }}
          />
          <Typography variant="body2">
            {t('tokens.created.usage', {
              scope: t(`tokens.scope.${token.scope}`),
            })}{' '}
            <Box component="code" sx={codeSx}>
              {t('tokens.created.header')}
            </Box>
          </Typography>
        </Box>
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button variant="contained" onClick={onClose}>
          {t('tokens.created.done')}
        </Button>
      </DialogActions>
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
      <Box sx={{ display: 'grid', gap: 1.5 }}>
        <Typography>{t('tokens.revoke.text')}</Typography>
        <Typography variant="body2" color="text.secondary">
          {token.last_used_at
            ? t('tokens.revoke.lastUsed', {
                time: formatDateTime(token.last_used_at),
              })
            : t('tokens.revoke.neverUsed')}
        </Typography>
      </Box>
    </ConfirmDialog>
  )
}
