import React, { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import apiClient from '../api/client';
import { useAuthStore } from '../store/authStore';
import { Lock, User, Server } from 'lucide-react';

const LoginPage: React.FC = () => {
  const [serverAddress, setServerAddress] = useState('');
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const [resolving, setResolving] = useState(false);
  
  const navigate = useNavigate();
  const { setAuth, setServerUrl, setActiveUrl, serverUrl: storedServerUrl } = useAuthStore();
  
  // Check if running in Electron
  const isElectron = !!(window as any).electronAPI;

  useEffect(() => {
    if (storedServerUrl && isElectron) {
      setServerAddress(storedServerUrl);
    }
  }, [storedServerUrl, isElectron]);

  const resolveServerUrl = async (url: string) => {
    // Only for Electron
    if (!isElectron) return url;

    let finalUrl = url.replace(/\/$/, ''); // Remove trailing slash
    if (!finalUrl.startsWith('http')) {
      finalUrl = `http://${finalUrl}`;
    }

    try {
      setResolving(true);
      const result = await (window as any).electronAPI.resolveRedirect(finalUrl);
      if (result && (result.finalUrl || result.statusCode === 401)) {
          return result.finalUrl || finalUrl;
      }
      return finalUrl;
    } catch (err) {
      console.warn('URL resolution failed, using original', err);
      return finalUrl;
    } finally {
      setResolving(false);
    }
  };

  const handleLogin = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    setLoading(true);

    try {
      // 1. Resolve and set Server URL (Only in Electron)
      if (isElectron) {
        if (!serverAddress) {
          setError('请输入服务器地址');
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
      setAuth(user, token);
      navigate('/');
    } catch (err: any) {
      console.error('Login error:', err);
      setError(err.response?.data?.error || '登录失败，请检查用户名和密码');
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
              <img src="/logo.png" alt="Logo" className="w-full h-full object-contain" />
            </div>
            <h1 className="text-2xl md:text-3xl font-bold text-slate-900 dark:text-white tracking-tight">Ting Reader</h1>
            <p className="text-sm text-slate-500 dark:text-slate-400 mt-2">您的私有有声书馆</p>
          </div>

          <form onSubmit={handleLogin} className="space-y-6">
            {isElectron && (
              <div className="space-y-2">
                <label className="text-sm font-medium text-slate-700 dark:text-slate-300">服务器地址</label>
                <div className="relative">
                  <span className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400">
                    <Server size={18} />
                  </span>
                  <input
                    type="text"
                    value={serverAddress}
                    onChange={(e) => setServerAddress(e.target.value)}
                    className="w-full pl-10 pr-4 py-2 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-lg focus:ring-2 focus:ring-primary-500 focus:border-transparent outline-none transition-all dark:text-white"
                    placeholder="例如: http://192.168.1.10:3000"
                    required={isElectron}
                  />
                </div>
                <p className="text-[10px] text-slate-400 px-1">
                  请输入源地址，应用会自动处理重定向。
                </p>
              </div>
            )}

            <div className="space-y-2">
              <label className="text-sm font-medium text-slate-700 dark:text-slate-300">用户名</label>
              <div className="relative">
                <span className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400">
                  <User size={18} />
                </span>
                <input
                  type="text"
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  className="w-full pl-10 pr-4 py-2 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-lg focus:ring-2 focus:ring-primary-500 focus:border-transparent outline-none transition-all dark:text-white"
                  placeholder="请输入用户名"
                  required
                />
              </div>
            </div>

            <div className="space-y-2">
              <label className="text-sm font-medium text-slate-700 dark:text-slate-300">密码</label>
              <div className="relative">
                <span className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400">
                  <Lock size={18} />
                </span>
                <input
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  className="w-full pl-10 pr-4 py-2 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-lg focus:ring-2 focus:ring-primary-500 focus:border-transparent outline-none transition-all dark:text-white"
                  placeholder="请输入密码"
                  required
                />
              </div>
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
              {loading ? '正在登录...' : '登录'}
            </button>
          </form>
        </div>
      </div>
      <div className="py-8 text-center text-slate-400 text-sm">
        <p>©2026 Ting Reader.保留所有权利。</p>
      </div>
    </div>
  );
};

export default LoginPage;
