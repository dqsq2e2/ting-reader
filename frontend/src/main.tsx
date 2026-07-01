import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { registerSW } from 'virtual:pwa-register'
import './index.css'
import i18n from './core/i18n'
import App from './App.tsx'

// 注册 Service Worker
const updateSW = registerSW({
  onNeedRefresh() {
    if (confirm(i18n.t('pwa.refreshPrompt'))) {
      updateSW(true)
    }
  },
  onOfflineReady() {
    console.log(i18n.t('pwa.offlineReady'))
  },
})

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
