import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import apiClient from '../api/client';
import { safeStorage } from '../utils/storage';
import { languageStorageKey, normalizeLanguage, type SupportedLanguage } from './locales';

export const useAppLanguage = () => {
  const { i18n } = useTranslation();

  const language = normalizeLanguage(i18n.resolvedLanguage || i18n.language);

  const setLanguage = useCallback(async (nextLanguage: SupportedLanguage, syncRemote = true) => {
    const normalizedLanguage = normalizeLanguage(nextLanguage);
    const currentLanguage = normalizeLanguage(i18n.resolvedLanguage || i18n.language);

    safeStorage.setItem(languageStorageKey, normalizedLanguage);
    document.documentElement.lang = normalizedLanguage;

    if (currentLanguage !== normalizedLanguage) {
      await i18n.changeLanguage(normalizedLanguage);
    }

    if (syncRemote) {
      await apiClient.post('/api/settings', { language: normalizedLanguage });
    }
  }, [i18n]);

  return { language, setLanguage };
};
