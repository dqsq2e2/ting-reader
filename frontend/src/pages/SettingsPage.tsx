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
  Key,
  Code,
  ExternalLink,
  Copy,
  Download,
  ChevronRight
} from 'lucide-react';
import { useNavigate } from 'react-router-dom';
import { useAuthStore } from '../store/authStore';
import { usePlayerStore } from '../store/playerStore';

const SettingsPage: React.FC = () => {
  const navigate = useNavigate();
  const { user, setUser } = useAuthStore();
  const { applyTheme } = useTheme();
  const setPlaybackSpeed = usePlayerStore(state => state.setPlaybackSpeed);
  const [settings, setSettings] = useState({
    playback_speed: 1.0,
    sleep_timer_default: 0,
    auto_preload: false,
    auto_cache: false,
    theme: 'system' as 'light' | 'dark' | 'system',
    widget_css: ''
  });
  const [accountData, setAccountData] = useState({
    username: user?.username || '',
    password: ''
  });
  const [loading, setLoading] = useState(true);
  const [saved, setSaved] = useState(false);
  const [accountSaved, setAccountSaved] = useState(false);
  const [widgetEmbedType, setWidgetEmbedType] = useState<'private' | 'public'>('private');

  useEffect(() => {
    fetchSettings();
  }, []);

  const fetchSettings = async () => {
    try {
      const response = await apiClient.get('/api/settings');
      const fetchedSettings = {
        ...response.data,
        auto_preload: !!response.data.auto_preload,
        auto_cache: !!response.data.auto_cache
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
        <section className="bg-white dark:bg-slate-900 rounded-3xl p-4 md:p-6 border border-slate-100 dark:border-slate-800 shadow-sm">
          <h2 className="text-xl font-bold dark:text-white mb-6 flex items-center gap-2">
            <Monitor size={20} className="text-blue-500" />
            外观展示
          </h2>
          <div className="grid grid-cols-3 gap-2 md:gap-4">
            {[
              { id: 'light', icon: <Sun size={20} />, label: '浅色模式' },
              { id: 'dark', icon: <Moon size={20} />, label: '深色模式' },
              { id: 'system', icon: <Monitor size={20} />, label: '跟随系统' }
            ].map(theme => (
              <button
                key={theme.id}
                onClick={() => handleSave({ ...settings, theme: theme.id })}
                className={`flex flex-col items-center gap-2 md:gap-3 p-3 md:p-4 rounded-2xl border-2 transition-all ${
                  settings.theme === theme.id 
                    ? 'border-primary-600 bg-primary-50 dark:bg-primary-900/20 text-primary-600' 
                    : 'border-slate-100 dark:border-slate-800 text-slate-500 hover:bg-slate-50 dark:hover:bg-slate-800'
                }`}
              >
                {theme.icon}
                <span className="text-xs md:text-sm font-bold text-center leading-tight">{theme.label}</span>
              </button>
            ))}
          </div>
        </section>

        {/* Cache Management */}
        <section className="bg-white dark:bg-slate-900 rounded-3xl p-4 md:p-6 border border-slate-100 dark:border-slate-800 shadow-sm">
            <h2 className="text-xl font-bold dark:text-white mb-6 flex items-center gap-2">
              <Zap size={20} className="text-yellow-500" />
              服务端缓存管理
            </h2>
            
            <div 
                onClick={() => navigate('/downloads')}
                className="flex items-center justify-between p-4 bg-slate-50 dark:bg-slate-800 rounded-2xl cursor-pointer hover:bg-slate-100 dark:hover:bg-slate-700 transition-colors mb-6"
            >
                <div className="flex items-center gap-3">
                    <div className="w-10 h-10 bg-primary-50 dark:bg-primary-900/20 rounded-xl flex items-center justify-center text-primary-600">
                        <Download size={20} />
                    </div>
                    <div>
                        <div className="font-bold text-slate-900 dark:text-white">服务端缓存文件</div>
                        <div className="text-xs text-slate-500 font-medium">查看和管理服务器已缓存的音频文件</div>
                    </div>
                </div>
                <ChevronRight size={18} className="text-slate-400" />
            </div>
        </section>

        {/* Playback Settings */}
        <section className="bg-white dark:bg-slate-900 rounded-3xl p-4 md:p-6 border border-slate-100 dark:border-slate-800 shadow-sm">
          <h2 className="text-xl font-bold dark:text-white mb-6 flex items-center gap-2">
            <FastForward size={20} className="text-orange-500" />
            播放偏好
          </h2>
          <div className="space-y-6">
            <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
              <div>
                <p className="font-bold dark:text-white">默认播放倍速</p>
                <p className="text-xs md:text-sm text-slate-500">所有书籍开始播放时的初始倍速</p>
              </div>
              <div className="flex bg-slate-100 dark:bg-slate-800 p-1 rounded-xl self-start sm:self-auto w-full sm:w-auto">
                {[1.0, 1.25, 1.5, 2.0].map(speed => (
                  <button
                    key={speed}
                    onClick={() => handleSave({ ...settings, playback_speed: speed })}
                    className={`flex-1 sm:flex-none px-2 md:px-4 py-2 text-sm font-bold rounded-lg transition-all ${
                      settings.playback_speed === speed ? 'bg-white dark:bg-slate-700 shadow-sm text-primary-600' : 'text-slate-500'
                    }`}
                  >
                    {speed}x
                  </button>
                ))}
              </div>
            </div>

            <div className="flex items-center justify-between gap-4">
              <div className="flex-1 min-w-0">
                <p className="font-bold dark:text-white truncate">自动预加载下一章</p>
                <p className="text-xs md:text-sm text-slate-500 line-clamp-2">播放当前章节时，后台自动解密并缓冲下一章节</p>
              </div>
              <button
                onClick={() => handleSave({ ...settings, auto_preload: !settings.auto_preload })}
                className={`flex-shrink-0 w-12 md:w-14 h-7 md:h-8 rounded-full transition-all relative ${
                  settings.auto_preload ? 'bg-primary-600' : 'bg-slate-200 dark:bg-slate-700'
                }`}
              >
                <div className={`absolute top-1 w-5 md:w-6 h-5 md:h-6 bg-white rounded-full transition-all ${
                  settings.auto_preload ? 'left-6 md:left-7' : 'left-1'
                }`} />
              </button>
            </div>

            <div className="flex items-center justify-between gap-4 pt-4 border-t border-slate-100 dark:border-slate-800">
              <div className="flex-1 min-w-0">
                <p className="font-bold dark:text-white truncate">服务端自动缓存 (WebDAV)</p>
                <p className="text-xs md:text-sm text-slate-500 line-clamp-2">
                  播放当前章节时，通知服务器预先缓存下一章节 (仅适用于 WebDAV 库)
                </p>
              </div>
              <button
                onClick={() => handleSave({ ...settings, auto_cache: !settings.auto_cache })}
                className={`flex-shrink-0 w-12 md:w-14 h-7 md:h-8 rounded-full transition-all relative ${
                  settings.auto_cache ? 'bg-primary-600' : 'bg-slate-200 dark:bg-slate-700'
                }`}
              >
                <div className={`absolute top-1 w-5 md:w-6 h-5 md:h-6 bg-white rounded-full transition-all ${
                  settings.auto_cache ? 'left-6 md:left-7' : 'left-1'
                }`} />
              </button>
            </div>
          </div>
        </section>

        {/* Widget Settings */}
        <section className="bg-white dark:bg-slate-900 rounded-3xl p-4 md:p-6 border border-slate-100 dark:border-slate-800 shadow-sm">
          <h2 className="text-xl font-bold dark:text-white mb-6 flex items-center gap-2">
            <Code size={20} className="text-purple-500" />
            外挂组件 (Widget)
          </h2>
          <div className="space-y-6">
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <label className="text-sm font-bold text-slate-600 dark:text-slate-400">自定义 CSS 注入</label>
                <span className="text-[10px] text-slate-400 uppercase font-bold">针对 Widget 生效</span>
              </div>
              <textarea 
                value={settings.widget_css}
                onChange={e => setSettings({ ...settings, widget_css: e.target.value })}
                onBlur={() => handleSave(settings)}
                placeholder=".widget-mode { background: transparent !important; }"
                className="w-full h-32 px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white font-mono text-sm"
              />
            </div>

            <div className="p-4 bg-slate-50 dark:bg-slate-800 rounded-2xl border border-slate-100 dark:border-slate-700">
              <div className="flex items-center justify-between mb-4">
                <p className="text-xs font-bold text-slate-500 uppercase">嵌入代码 (Iframe)</p>
                
                <div className="flex bg-white dark:bg-slate-900 rounded-lg p-0.5 border border-slate-200 dark:border-slate-700">
                  <button
                    onClick={() => setWidgetEmbedType('private')}
                    className={`px-3 py-1 text-xs font-bold rounded-md transition-all ${
                      widgetEmbedType === 'private' 
                        ? 'bg-primary-50 dark:bg-primary-900/20 text-primary-600' 
                        : 'text-slate-500 hover:text-slate-700 dark:hover:text-slate-300'
                    }`}
                  >
                    免登录 (带 Token)
                  </button>
                  <button
                    onClick={() => setWidgetEmbedType('public')}
                    className={`px-3 py-1 text-xs font-bold rounded-md transition-all ${
                      widgetEmbedType === 'public' 
                        ? 'bg-primary-50 dark:bg-primary-900/20 text-primary-600' 
                        : 'text-slate-500 hover:text-slate-700 dark:hover:text-slate-300'
                    }`}
                  >
                    需登录 (公开)
                  </button>
                </div>
              </div>

              <div className="relative group">
                <code className="text-[10px] md:text-xs text-slate-600 dark:text-slate-400 break-all bg-white dark:bg-slate-950 p-3 rounded-xl block border border-slate-100 dark:border-slate-900 font-mono leading-relaxed">
                  {`<iframe src="${window.location.origin}/widget${widgetEmbedType === 'private' ? `?token=${useAuthStore.getState().token}` : ''}" width="100%" height="150" frameborder="0" allow="autoplay; fullscreen"></iframe>`}
                </code>
                <button 
                  onClick={() => {
                    const baseUrl = window.location.origin;
                    const token = widgetEmbedType === 'private' ? `?token=${useAuthStore.getState().token}` : '';
                    const embedCode = `<iframe src="${baseUrl}/widget${token}" width="100%" height="150" frameborder="0" allow="autoplay; fullscreen"></iframe>`;
                    navigator.clipboard.writeText(embedCode);
                    alert('已复制到剪贴板');
                  }}
                  className="absolute top-2 right-2 p-1.5 bg-slate-100 dark:bg-slate-800 hover:bg-primary-50 dark:hover:bg-primary-900/30 text-slate-500 hover:text-primary-600 rounded-lg transition-colors"
                  title="复制"
                >
                  <Copy size={14} />
                </button>
              </div>
              
              <div className="mt-3 flex gap-2">
                <div className="shrink-0 mt-0.5">
                  {widgetEmbedType === 'private' ? (
                    <Key size={12} className="text-orange-500" />
                  ) : (
                    <User size={12} className="text-blue-500" />
                  )}
                </div>
                <p className="text-[10px] text-slate-500 dark:text-slate-400 leading-tight">
                  {widgetEmbedType === 'private' ? (
                    <>
                      <span className="font-bold text-orange-500">注意安全：</span>
                      此代码包含您的访问凭证。请仅将其嵌入到您信任的私有页面（如个人 Dashboard）。任何访问该页面的人都将拥有您的播放权限。
                    </>
                  ) : (
                    <>
                      <span className="font-bold text-blue-500">公开模式：</span>
                      此代码不包含凭证，适合嵌入博客或公开网站。访客在首次使用时需要输入用户名和密码登录。
                    </>
                  )}
                </p>
              </div>

              <div className="mt-4 pt-4 border-t border-slate-100 dark:border-slate-700">
                <p className="text-xs font-bold text-slate-500 uppercase mb-2">布局代码参考 (直接复制)</p>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                  <div className="p-3 bg-white dark:bg-slate-950 rounded-xl border border-slate-100 dark:border-slate-900 group relative">
                    <p className="text-[10px] font-bold text-slate-400 mb-1">1. 吸底模式 (Fixed Bottom)</p>
                    <code className="text-[10px] text-slate-600 dark:text-slate-400 font-mono block whitespace-pre overflow-x-auto">
{`<div style="position: fixed; bottom: 0; left: 0; width: 100%; z-index: 9999;">
  <iframe src="${window.location.origin}/widget${widgetEmbedType === 'private' ? `?token=${useAuthStore.getState().token}` : ''}" width="100%" height="150" frameborder="0" allow="autoplay; fullscreen"></iframe>
</div>`}
                    </code>
                    <button 
                      onClick={() => {
                        const code = `<div style="position: fixed; bottom: 0; left: 0; width: 100%; z-index: 9999;">
  <iframe src="${window.location.origin}/widget${widgetEmbedType === 'private' ? `?token=${useAuthStore.getState().token}` : ''}" width="100%" height="150" frameborder="0" allow="autoplay; fullscreen"></iframe>
</div>`;
                        navigator.clipboard.writeText(code);
                        alert('已复制到剪贴板');
                      }}
                      className="absolute top-2 right-2 p-1.5 bg-slate-100 dark:bg-slate-800 hover:bg-primary-50 dark:hover:bg-primary-900/30 text-slate-500 hover:text-primary-600 rounded-lg transition-colors opacity-0 group-hover:opacity-100"
                      title="复制"
                    >
                      <Copy size={12} />
                    </button>
                  </div>
                  
                  <div className="p-3 bg-white dark:bg-slate-950 rounded-xl border border-slate-100 dark:border-slate-900 group relative">
                    <p className="text-[10px] font-bold text-slate-400 mb-1">2. 右下角悬浮 (Floating Right)</p>
                    <code className="text-[10px] text-slate-600 dark:text-slate-400 font-mono block whitespace-pre overflow-x-auto">
{`<div style="position: fixed; bottom: 20px; right: 20px; width: 350px; height: 150px; z-index: 9999; border-radius: 16px; overflow: hidden; box-shadow: 0 4px 20px rgba(0,0,0,0.15);">
  <iframe src="${window.location.origin}/widget${widgetEmbedType === 'private' ? `?token=${useAuthStore.getState().token}` : ''}" width="100%" height="100%" frameborder="0" allow="autoplay; fullscreen"></iframe>
</div>`}
                    </code>
                    <button 
                      onClick={() => {
                        const code = `<div style="position: fixed; bottom: 20px; right: 20px; width: 350px; height: 150px; z-index: 9999; border-radius: 16px; overflow: hidden; box-shadow: 0 4px 20px rgba(0,0,0,0.15);">
  <iframe src="${window.location.origin}/widget${widgetEmbedType === 'private' ? `?token=${useAuthStore.getState().token}` : ''}" width="100%" height="100%" frameborder="0" allow="autoplay; fullscreen"></iframe>
</div>`;
                        navigator.clipboard.writeText(code);
                        alert('已复制到剪贴板');
                      }}
                      className="absolute top-2 right-2 p-1.5 bg-slate-100 dark:bg-slate-800 hover:bg-primary-50 dark:hover:bg-primary-900/30 text-slate-500 hover:text-primary-600 rounded-lg transition-colors opacity-0 group-hover:opacity-100"
                      title="复制"
                    >
                      <Copy size={12} />
                    </button>
                  </div>
                </div>
              </div>
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
