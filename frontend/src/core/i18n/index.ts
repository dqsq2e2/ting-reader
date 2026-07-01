import i18n from 'i18next';
import LanguageDetector from 'i18next-browser-languagedetector';
import { initReactI18next } from 'react-i18next';
import { defaultLanguage, languageStorageKey, normalizeLanguage, supportedLanguages } from './locales';
import enUS from './resources/en-US';
import zhCN from './resources/zh-CN';

const resources = {
  'zh-CN': { translation: zhCN },
  'en-US': { translation: enUS },
};

void i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources,
    fallbackLng: defaultLanguage,
    supportedLngs: [...supportedLanguages],
    interpolation: {
      escapeValue: false,
    },
    detection: {
      order: ['localStorage', 'navigator'],
      lookupLocalStorage: languageStorageKey,
      caches: ['localStorage'],
      convertDetectedLanguage: normalizeLanguage,
    },
  });

export default i18n;
