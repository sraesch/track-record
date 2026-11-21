import { useEffect } from 'react'
import { useTranslation } from 'react-i18next'

/** Sets the browser tab title to "<title> · Track Record". */
export function usePageTitle(title: string) {
  const { t } = useTranslation()
  const appName = t('app.name')
  useEffect(() => {
    document.title = title ? `${title} · ${appName}` : appName
  }, [title, appName])
}
