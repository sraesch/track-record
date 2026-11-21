import FilterAltOffIcon from '@mui/icons-material/FilterAltOff'
import Autocomplete from '@mui/material/Autocomplete'
import Box from '@mui/material/Box'
import Button from '@mui/material/Button'
import Checkbox from '@mui/material/Checkbox'
import Chip from '@mui/material/Chip'
import ListItemText from '@mui/material/ListItemText'
import MenuItem from '@mui/material/MenuItem'
import TextField from '@mui/material/TextField'
import ToggleButton from '@mui/material/ToggleButton'
import ToggleButtonGroup from '@mui/material/ToggleButtonGroup'
import { useTranslation } from 'react-i18next'
import type { RunStatus, RunTrigger, Validity } from '../api/client'
import { useBranches, useClientDeviceNames, useSuites } from './queries'
import {
  hasActiveFilters,
  type RunFilters,
  runStatuses,
  runTriggers,
  validities,
} from './runFilters'

type Props = {
  filters: RunFilters
  onChange: (filters: RunFilters) => void
}

const fieldSx = { minWidth: 160, flex: '1 1 160px', maxWidth: 260 }

/** Filters of the run list (FR-11); every change goes to the URL. */
export function RunFilterBar({ filters, onChange }: Props) {
  const { t } = useTranslation()
  const suites = useSuites()
  const devices = useClientDeviceNames()
  const branches = useBranches()
  const set = (patch: Partial<RunFilters>) => onChange({ ...filters, ...patch })

  return (
    <Box
      component="search"
      aria-label={t('runs.filters.label')}
      sx={{ display: 'flex', flexWrap: 'wrap', gap: 1.5, mb: 2 }}
    >
      <Autocomplete
        size="small"
        sx={fieldSx}
        options={suites.data ?? []}
        loading={suites.isPending}
        value={filters.suite ?? null}
        onChange={(_, value) => set({ suite: value ?? undefined })}
        renderInput={(params) => (
          <TextField {...params} label={t('runs.filters.suite')} />
        )}
      />

      <TextField
        select
        size="small"
        sx={fieldSx}
        label={t('runs.filters.status')}
        value={filters.status}
        slotProps={{
          select: {
            multiple: true,
            renderValue: (selected) =>
              (selected as RunStatus[])
                .map((s) => t(`runs.status.${s}`))
                .join(', '),
          },
        }}
        onChange={(event) =>
          set({ status: event.target.value as unknown as RunStatus[] })
        }
      >
        {runStatuses.map((status) => (
          <MenuItem key={status} value={status}>
            <Checkbox
              size="small"
              checked={filters.status.includes(status)}
              sx={{ p: 0, mr: 1 }}
            />
            <ListItemText primary={t(`runs.status.${status}`)} />
          </MenuItem>
        ))}
      </TextField>

      <TextField
        select
        size="small"
        sx={fieldSx}
        label={t('runs.filters.trigger')}
        value={filters.trigger ?? ''}
        onChange={(event) =>
          set({
            trigger: (event.target.value || undefined) as
              | RunTrigger
              | undefined,
          })
        }
      >
        <MenuItem value="">
          <em>{t('runs.filters.any')}</em>
        </MenuItem>
        {runTriggers.map((trigger) => (
          <MenuItem key={trigger} value={trigger}>
            {t(`runs.trigger.${trigger}`)}
          </MenuItem>
        ))}
      </TextField>

      <Autocomplete
        size="small"
        sx={fieldSx}
        options={branches.data ?? []}
        loading={branches.isPending}
        value={filters.branch ?? null}
        onChange={(_, value) => set({ branch: value ?? undefined })}
        renderInput={(params) => (
          <TextField {...params} label={t('runs.filters.branch')} />
        )}
      />

      <Autocomplete
        size="small"
        sx={fieldSx}
        options={devices.data ?? []}
        loading={devices.isPending}
        value={filters.device ?? null}
        onChange={(_, value) => set({ device: value ?? undefined })}
        renderInput={(params) => (
          <TextField {...params} label={t('runs.filters.device')} />
        )}
      />

      <Autocomplete
        multiple
        freeSolo
        size="small"
        sx={{ ...fieldSx, maxWidth: 360 }}
        options={[]}
        value={filters.tags}
        onChange={(_, value) =>
          set({
            tags: [...new Set(value.map((v) => v.trim()).filter(Boolean))],
          })
        }
        renderValue={(value, getItemProps) =>
          value.map((tag, index) => {
            const { key, ...itemProps } = getItemProps({ index })
            return <Chip key={key} size="small" label={tag} {...itemProps} />
          })
        }
        renderInput={(params) => (
          <TextField
            {...params}
            label={t('runs.filters.tags')}
            placeholder={
              filters.tags.length === 0 ? t('runs.filters.tagsHint') : undefined
            }
          />
        )}
      />

      <TextField
        type="date"
        size="small"
        sx={{ ...fieldSx, maxWidth: 190 }}
        label={t('runs.filters.from')}
        value={filters.from ?? ''}
        onChange={(event) => set({ from: event.target.value || undefined })}
        slotProps={{
          inputLabel: { shrink: true },
          htmlInput: { max: filters.to },
        }}
      />
      <TextField
        type="date"
        size="small"
        sx={{ ...fieldSx, maxWidth: 190 }}
        label={t('runs.filters.to')}
        value={filters.to ?? ''}
        onChange={(event) => set({ to: event.target.value || undefined })}
        slotProps={{
          inputLabel: { shrink: true },
          htmlInput: { min: filters.from },
        }}
      />

      <ToggleButtonGroup
        size="small"
        exclusive
        aria-label={t('runs.filters.validity')}
        value={filters.validity}
        onChange={(_, value: Validity | null) => {
          if (value) set({ validity: value })
        }}
      >
        {validities.map((validity) => (
          <ToggleButton key={validity} value={validity} sx={{ px: 1.5 }}>
            {t(`runs.validity.${validity}`)}
          </ToggleButton>
        ))}
      </ToggleButtonGroup>

      {hasActiveFilters(filters) && (
        <Button
          size="small"
          startIcon={<FilterAltOffIcon />}
          onClick={() =>
            onChange({ status: [], tags: [], validity: 'valid' })
          }
        >
          {t('runs.filters.clear')}
        </Button>
      )}
    </Box>
  )
}
