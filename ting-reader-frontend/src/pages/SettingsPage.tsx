import React, { useEffect, useState } from 'react';
import apiClient from '../api/client';
import { useTheme } from '../hooks/useTheme';
import { 
  Settings as SettingsIcon, 
  Moon, 
  Sun, 
  Monitor, 
  Zap, 
  FastForward, 
  Timer,
  CheckCircle2,
  User,
  Key
} from 'lucide-react';
import { useAuthStore } from '../store/authStore';
import { usePlayerStore } from '../store/playerStore';

const SettingsPage: React.FC = () => {
  const { user, setUser } = useAuthStore();
  const { applyTheme } = useTheme();
  const setPlaybackSpeed = usePlayerStore(state => state.setPlaybackSpeed);
  const [settings, setSettings] = useState({
    playback_speed: 1.0,
    sleep_timer_default: 0,
    auto_preload: false,
    theme: 'system' as 'light' | 'dark' | 'system'
  });
  const [accountData, setAccountData] = useState({
    username: user?.username || '',
    password: ''
  });
  const [loading, setLoading] = useState(true);
  const [saved, setSaved] = useState(false);
  const [accountSaved, setAccountSaved] = useState(false);

  useEffect(() => {
    fetchSettings();
  }, []);

  const fetchSettings = async () => {
    try {
      const response = await apiClient.get('/api/settings');
      const fetchedSettings = {
        ...response.data,
        auto_preload: !!response.data.auto_preload
      };
      setSettings(fetchedSettings);
      // Ensure local theme matches server theme
      if (fetchedSettings.theme) {
        applyTheme(fetchedSettings.theme);
      }
    } catch (err) {
      console.error('Failed to fetch settings', err);
    } finally {
      setLoading(false);
    }
  };

  const handleSave = async (newSettings: any) => {
    try {
      await apiClient.post('/api/settings', newSettings);
      setSettings(newSettings);
      
      // Sync playback speed to player store immediately
      if (newSettings.playback_speed) {
        setPlaybackSpeed(newSettings.playback_speed);
      }
      
      // Apply theme immediately if it changed
      if (newSettings.theme) {
        applyTheme(newSettings.theme);
      }

      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (err) {
      alert('保存失败');
    }
  };

  const handleAccountUpdate = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      const updateData: any = {};
      if (accountData.username !== user?.username) {
        updateData.username = accountData.username;
      }
      if (accountData.password) {
        updateData.password = accountData.password;
      }

      if (Object.keys(updateData).length === 0) {
        setAccountSaved(true);
        setTimeout(() => setAccountSaved(false), 2000);
        return;
      }

      await apiClient.patch('/api/me', updateData);

      // Update local user store if username changed
      if (updateData.username && user) {
        setUser({ ...user, username: accountData.username });
      }

      setAccountData({ ...accountData, password: '' });
      setAccountSaved(true);
      setTimeout(() => setAccountSaved(false), 2000);
    } catch (err: any) {
      alert(err.response?.data?.error || '更新失败');
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-primary-600"></div>
      </div>
    );
  }

  return (
    <div className="w-full max-w-screen-2xl mx-auto p-4 sm:p-6 md:p-8 lg:p-10 space-y-8 animate-in fade-in duration-500">
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-6">
        <div className="text-center md:text-left">
          <h1 className="text-2xl md:text-3xl font-bold dark:text-white flex items-center justify-center md:justify-start gap-3">
            <SettingsIcon size={28} className="text-primary-600 md:w-8 md:h-8" />
            个性化设置
          </h1>
          <p className="text-sm md:text-base text-slate-500 mt-1">定制您的听书体验</p>
        </div>
        {saved && (
          <div className="flex items-center justify-center gap-2 text-green-600 font-bold bg-green-50 dark:bg-green-900/20 px-4 py-2 rounded-xl animate-in fade-in slide-in-from-right-4">
            <CheckCircle2 size={18} />
            已保存
          </div>
        )}
      </div>

      <div className="space-y-6">
        {/* Account Settings */}
        <section className="bg-white dark:bg-slate-900 rounded-3xl p-6 border border-slate-100 dark:border-slate-800 shadow-sm">
          <div className="flex items-center justify-between mb-6">
            <h2 className="text-xl font-bold dark:text-white flex items-center gap-2">
              <User size={20} className="text-primary-500" />
              账号信息
            </h2>
            {accountSaved && (
              <span className="text-sm text-green-600 font-bold flex items-center gap-1">
                <CheckCircle2 size={14} />
                更新成功
              </span>
            )}
          </div>
          <form onSubmit={handleAccountUpdate} className="space-y-4">
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <div className="space-y-2">
                <label className="text-sm font-bold text-slate-600 dark:text-slate-400">用户名</label>
                <div className="relative">
                  <User className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" size={18} />
                  <input 
                    type="text" 
                    value={accountData.username}
                    onChange={e => setAccountData({...accountData, username: e.target.value})}
                    className="w-full pl-10 pr-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                  />
                </div>
              </div>
              <div className="space-y-2">
                <label className="text-sm font-bold text-slate-600 dark:text-slate-400">修改密码 (留空则不修改)</label>
                <div className="relative">
                  <Key className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" size={18} />
                  <input 
                    type="password" 
                    value={accountData.password}
                    onChange={e => setAccountData({...accountData, password: e.target.value})}
                    placeholder="新密码"
                    className="w-full pl-10 pr-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                  />
                </div>
              </div>
            </div>
            <div className="flex justify-end">
              <button 
                type="submit"
                className="px-6 py-2.5 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-xl shadow-lg shadow-primary-500/30 transition-all text-sm"
              >
                更新账号信息
              </button>
            </div>
          </form>
        </section>

        {/* Appearance */}
        <section className="bg-white dark:bg-slate-900 rounded-3xl p-6 border border-slate-100 dark:border-slate-800 shadow-sm">
          <h2 className="text-xl font-bold dark:text-white mb-6 flex items-center gap-2">
            <Monitor size={20} className="text-blue-500" />
            外观展示
          </h2>
          <div className="grid grid-cols-3 gap-4">
            {[
              { id: 'light', icon: <Sun size={20} />, label: '浅色模式' },
              { id: 'dark', icon: <Moon size={20} />, label: '深色模式' },
              { id: 'system', icon: <Monitor size={20} />, label: '跟随系统' }
            ].map(theme => (
              <button
                key={theme.id}
                onClick={() => handleSave({ ...settings, theme: theme.id })}
                className={`flex flex-col items-center gap-3 p-4 rounded-2xl border-2 transition-all ${
                  settings.theme === theme.id 
                    ? 'border-primary-600 bg-primary-50 dark:bg-primary-900/20 text-primary-600' 
                    : 'border-slate-100 dark:border-slate-800 text-slate-500 hover:bg-slate-50 dark:hover:bg-slate-800'
                }`}
              >
                {theme.icon}
                <span className="text-sm font-bold">{theme.label}</span>
              </button>
            ))}
          </div>
        </section>

        {/* Playback Settings */}
        <section className="bg-white dark:bg-slate-900 rounded-3xl p-6 border border-slate-100 dark:border-slate-800 shadow-sm">
          <h2 className="text-xl font-bold dark:text-white mb-6 flex items-center gap-2">
            <FastForward size={20} className="text-orange-500" />
            播放偏好
          </h2>
          <div className="space-y-6">
            <div className="flex items-center justify-between">
              <div>
                <p className="font-bold dark:text-white">默认播放倍速</p>
                <p className="text-sm text-slate-500">所有书籍开始播放时的初始倍速</p>
              </div>
              <div className="flex bg-slate-100 dark:bg-slate-800 p-1 rounded-xl">
                {[1.0, 1.25, 1.5, 2.0].map(speed => (
                  <button
                    key={speed}
                    onClick={() => handleSave({ ...settings, playback_speed: speed })}
                    className={`px-4 py-2 text-sm font-bold rounded-lg transition-all ${
                      settings.playback_speed === speed ? 'bg-white dark:bg-slate-700 shadow-sm text-primary-600' : 'text-slate-500'
                    }`}
                  >
                    {speed}x
                  </button>
                ))}
              </div>
            </div>

            <div className="flex items-center justify-between">
              <div>
                <p className="font-bold dark:text-white">自动预加载下一章</p>
                <p className="text-sm text-slate-500">播放当前章节时，后台自动解密并缓冲下一章节</p>
              </div>
              <button
                onClick={() => handleSave({ ...settings, auto_preload: !settings.auto_preload })}
                className={`w-14 h-8 rounded-full transition-all relative ${
                  settings.auto_preload ? 'bg-primary-600' : 'bg-slate-200 dark:bg-slate-700'
                }`}
              >
                <div className={`absolute top-1 w-6 h-6 bg-white rounded-full transition-all ${
                  settings.auto_preload ? 'left-7' : 'left-1'
                }`} />
              </button>
            </div>
          </div>
        </section>
      </div>

      <div className="text-center text-slate-400 text-sm py-8">
        <p>©2026 Ting Reader.保留所有权利。</p>
      </div>
    </div>
  );
};

export default SettingsPage;
