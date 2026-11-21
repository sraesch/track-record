// i18next setup (ADR 0001, ADR 0010). Every user-facing string comes from a
// locale file; components use `useTranslation()` and never hard-code text.
//
// Locales are bundled, so translations are ready synchronously and tests and
// the first render never see raw keys.
import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'
import enCommon from './locales/en/common.json'

export const defaultNS = 'common'

export const resources = {
  en: { common: enCommon },
} as const

export const supportedLanguages = Object.keys(resources) as Array<
  keyof typeof resources
>

void i18n.use(initReactI18next).init({
  resources,
  lng: 'en',
  fallbackLng: 'en',
  supportedLngs: supportedLanguages,
  defaultNS,
  ns: [defaultNS],
  interpolation: {
    // React already escapes rendered values.
    escapeValue: false,
  },
})

i18n.on('languageChanged', (lng) => {
  document.documentElement.lang = lng
})
document.documentElement.lang = i18n.language

export default i18n
