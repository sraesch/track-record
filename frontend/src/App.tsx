import { useTranslation } from 'react-i18next'

function App() {
  const { t } = useTranslation()

  return (
    <div className="app">
      <header className="app-header">
        <h1>{t('app.name')}</h1>
        <p>{t('app.tagline')}</p>
      </header>
      <main className="app-main">
        <p>{t('home.placeholder')}</p>
      </main>
    </div>
  )
}

export default App
