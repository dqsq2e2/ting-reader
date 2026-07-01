import { useEffect, useState } from 'react';
import apiClient from '../api/client';

export type Theme = 'light' | 'dark' | 'system';

export const normalizeTheme = (value: unknown): Theme => (
  value === 'light' || value === 'dark' || value === 'system' ? value : 'system'
);

export const useTheme = () => {
  const [theme, setTheme] = useState<Theme>(() => {
    return normalizeTheme(localStorage.getItem('theme'));
  });

  const syncThemeToDom = (value: unknown) => {
    const t = normalizeTheme(value);
    const root = window.document.documentElement;
    root.classList.remove('light', 'dark');

    let effectiveTheme = t;
    if (t === 'system') {
      effectiveTheme = window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
    }

    root.classList.add(effectiveTheme);
    localStorage.setItem('theme', t);
  };

  const applyTheme = (value: unknown) => {
    const nextTheme = normalizeTheme(value);
    syncThemeToDom(nextTheme);
    setTheme(nextTheme);
  };

  useEffect(() => {
    // Initial apply
    syncThemeToDom(theme);

    // Listen for system theme changes if set to system
    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    const handleChange = () => {
      if (theme === 'system') {
        syncThemeToDom('system');
      }
    };

    mediaQuery.addEventListener('change', handleChange);
    return () => mediaQuery.removeEventListener('change', handleChange);
  }, [theme]);

  const refreshTheme = async () => {
    try {
      const response = await apiClient.get('/api/settings');
      if (response.data.theme) {
        applyTheme(response.data.theme);
      }
    } catch (err) {
      console.error('从服务器刷新主题失败', err);
    }
  };

  return { theme, applyTheme, refreshTheme };
};
