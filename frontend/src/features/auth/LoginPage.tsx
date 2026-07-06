import React, { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import apiClient from '../../core/api/client';
import { useAuthStore } from '../../core/stores/authStore';
import { USER_AGREEMENT_URL, PRIVACY_POLICY_URL } from '../../core/constants/links';
import { safeStorage } from '../../core/utils/storage';
import { markSessionRestoreLogged } from '../../core/utils/sessionRestore';
import { Lock, Server, User } from 'lucide-react';

const LoginPage: React.FC = () => {
  const { t } = useTranslation();
  const [serverAddress, setServerAddress] = useState('');
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const [acceptedAgreement, setAcceptedAgreement] = useState(
    () => safeStorage.getItem('login_accept_agreement') === 'true',
  );
  const [rememberPassword, setRememberPassword] = useState(
    () => safeStorage.getItem('login_remember_password') !== 'false',
  );
  // const [resolving, setResolving] = useState(false);
  
  const navigate = useNavigate();
  const { setAuth, setServerUrl, setActiveUrl, serverUrl: storedServerUrl } = useAuthStore();
  
  // Check if running in Electron
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const isElectron = !!(window as any).electronAPI;

  useEffect(() => {
    if (storedServerUrl && isElectron) {
      setServerAddress(storedServerUrl);
    }
  }, [storedServerUrl, isElectron]);

  useEffect(() => {
    if (rememberPassword) {
      setUsername(safeStorage.getItem('login_username') || '');
      setPassword(safeStorage.getItem('login_password') || '');
    }
  }, [rememberPassword]);

  const resolveServerUrl = async (url: string) => {
    // Only for Electron
    if (!isElectron) return url;

    let finalUrl = url.replace(/\/$/, ''); // Remove trailing slash
    if (!finalUrl.startsWith('http')) {
      finalUrl = `http://${finalUrl}`;
    }

    try {
      // setResolving(true);
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const result = await (window as any).electronAPI.resolveRedirect(finalUrl);
      if (result && (result.finalUrl || result.statusCode === 401)) {
          return result.finalUrl || finalUrl;
      }
      return finalUrl;
    } catch (err) {
      console.warn(t('auth.urlResolveFailed'), err);
      return finalUrl;
    } finally {
      // setResolving(false);
    }
  };

  const handleLogin = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');

    if (!acceptedAgreement) {
      setError(t('auth.requireAgreement'));
      return;
    }

    setLoading(true);

    try {
      // 1. Resolve and set Server URL (Only in Electron)
      if (isElectron) {
        if (!serverAddress) {
          setError(t('auth.requireServerAddress'));
          setLoading(false);
          return;
        }
        const activeUrl = await resolveServerUrl(serverAddress);
        setServerUrl(serverAddress); // Always store the original input (Source of Truth)
        setActiveUrl(activeUrl);     // Store the resolved URL (Cache)
      }

      // 2. Login
      const response = await apiClient.post('/api/auth/login', { username, password });
      const { token, user } = response.data;
      safeStorage.setItem('login_accept_agreement', acceptedAgreement ? 'true' : 'false');
      safeStorage.setItem('login_remember_password', rememberPassword ? 'true' : 'false');
      if (rememberPassword) {
        safeStorage.setItem('login_username', username);
        safeStorage.setItem('login_password', password);
      } else {
        safeStorage.removeItem('login_username');
        safeStorage.removeItem('login_password');
      }
      setAuth(user, token);
      markSessionRestoreLogged(token);
      navigate('/');
    } catch (err: unknown) {
      console.error(t('auth.loginError'), err);
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const msg = (err as any)?.response?.data?.error || t('auth.loginFailed');
      setError(msg);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen flex flex-col items-center justify-center bg-slate-50 dark:bg-slate-950 p-4">
      <div className="flex-1 flex items-center justify-center w-full">
        <div className="w-full max-w-md bg-white dark:bg-slate-900 rounded-2xl shadow-xl p-8 space-y-8 border border-slate-200 dark:border-slate-800">
          <div className="text-center">
            <div className="inline-flex items-center justify-center w-20 h-20 mb-6">
              <img src="/logo.png" alt={t('common.logoAlt')} className="w-full h-full object-contain" />
            </div>
            <h1 className="text-2xl md:text-3xl font-bold text-slate-900 dark:text-white tracking-tight">Ting Reader</h1>
            <p className="text-sm text-slate-500 dark:text-slate-400 mt-2">{t('auth.tagline')}</p>
          </div>

          <form onSubmit={handleLogin} className="space-y-6">
            {isElectron && (
              <div className="space-y-2">
                <label className="text-sm font-medium text-slate-700 dark:text-slate-300">{t('auth.serverAddress')}</label>
                <div className="relative">
                  <span className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400">
                    <Server size={18} />
                  </span>
                  <input
                    type="text"
                    value={serverAddress}
                    onChange={(e) => setServerAddress(e.target.value)}
                    className="w-full pl-10 pr-4 py-2 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-lg focus:ring-2 focus:ring-primary-500 focus:border-transparent outline-none transition-all dark:text-white"
                    placeholder={t('auth.serverAddressPlaceholder')}
                    required={isElectron}
                  />
                </div>
                <p className="text-[10px] text-slate-400 px-1">
                  {t('auth.serverAddressHint')}
                </p>
              </div>
            )}

            <div className="space-y-2">
              <label className="text-sm font-medium text-slate-700 dark:text-slate-300">{t('auth.username')}</label>
              <div className="relative">
                <span className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400">
                  <User size={18} />
                </span>
                <input
                  type="text"
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  className="w-full pl-10 pr-4 py-2 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-lg focus:ring-2 focus:ring-primary-500 focus:border-transparent outline-none transition-all dark:text-white"
                  placeholder={t('auth.usernamePlaceholder')}
                  required
                />
              </div>
            </div>

            <div className="space-y-2">
              <label className="text-sm font-medium text-slate-700 dark:text-slate-300">{t('auth.password')}</label>
              <div className="relative">
                <span className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400">
                  <Lock size={18} />
                </span>
                <input
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  className="w-full pl-10 pr-4 py-2 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-lg focus:ring-2 focus:ring-primary-500 focus:border-transparent outline-none transition-all dark:text-white"
                  placeholder={t('auth.passwordPlaceholder')}
                  required
                />
              </div>
            </div>

            <div className="space-y-3">
              <label className="flex items-start gap-2 text-sm text-slate-600 dark:text-slate-300 leading-6">
                <input
                  type="checkbox"
                  checked={acceptedAgreement}
                  onChange={(e) => setAcceptedAgreement(e.target.checked)}
                  className="mt-1 h-4 w-4 rounded border-slate-300 text-primary-600 focus:ring-primary-500"
                />
                <span>
                  {t('auth.acceptAgreement')}
                  <a
                    href={USER_AGREEMENT_URL}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-primary-600 hover:text-primary-700 font-medium underline underline-offset-2 mx-1"
                  >
                    {t('auth.userAgreement')}
                  </a>
                  {t('auth.and')}
                  <a
                    href={PRIVACY_POLICY_URL}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-primary-600 hover:text-primary-700 font-medium underline underline-offset-2 mx-1"
                  >
                    {t('auth.privacyPolicy')}
                  </a>
                </span>
              </label>
              <label className="flex items-center gap-2 text-sm text-slate-600 dark:text-slate-300">
                <input
                  type="checkbox"
                  checked={rememberPassword}
                  onChange={(e) => {
                    const checked = e.target.checked;
                    setRememberPassword(checked);
                    safeStorage.setItem('login_remember_password', checked ? 'true' : 'false');
                    if (!checked) {
                      safeStorage.removeItem('login_username');
                      safeStorage.removeItem('login_password');
                    }
                  }}
                  className="h-4 w-4 rounded border-slate-300 text-primary-600 focus:ring-primary-500"
                />
                <span>{t('auth.rememberPassword')}</span>
              </label>
            </div>

            {error && (
              <div className="text-red-500 text-sm bg-red-50 dark:bg-red-900/20 p-3 rounded-lg border border-red-100 dark:border-red-900/30">
                {error}
              </div>
            )}

            <button
              type="submit"
              disabled={loading}
              className="w-full py-3 bg-primary-600 hover:bg-primary-700 text-white font-semibold rounded-lg shadow-md hover:shadow-lg transition-all disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {loading ? t('auth.loggingIn') : t('auth.login')}
            </button>
          </form>
        </div>
      </div>
      <div className="py-8 text-center text-slate-400 text-sm">
        <p>{t('auth.copyright')}</p>
      </div>
    </div>
  );
};

export default LoginPage;
