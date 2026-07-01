export const supportedLanguages = ['zh-CN', 'en-US'] as const;

export type SupportedLanguage = (typeof supportedLanguages)[number];

export const defaultLanguage: SupportedLanguage = 'zh-CN';

export const languageStorageKey = 'language';

export const languageLabels: Record<SupportedLanguage, string> = {
  'zh-CN': '简体中文',
  'en-US': 'English',
};

export const languageNativeLabels: Record<SupportedLanguage, string> = {
  'zh-CN': '简体中文',
  'en-US': 'English',
};

export const normalizeLanguage = (value?: string | null): SupportedLanguage => {
  if (!value || value === 'system' || value === 'auto') return defaultLanguage;
  const normalized = value.replace('_', '-').toLowerCase();
  if (normalized.startsWith('zh')) return 'zh-CN';
  if (normalized.startsWith('en')) return 'en-US';
  return defaultLanguage;
};
