import React, { useEffect, useMemo, useState } from 'react';
import apiClient from '../api/client';
import { useTheme, type Theme } from '../hooks/useTheme';
import { usePlayerStore } from '../store/playerStore';
import { useAuthStore } from '../store/authStore';
import { DEFAULT_HOME_LAYOUT, normalizeHomeLayout, type HomeLayoutSettings } from '../utils/homeLayout';
import BackButton from '../components/BackButton';
import {
  CheckCircle2,
  Code,
  Copy,
  ExternalLink,
  FastForward,
  Home,
  Info,
  Key,
  Monitor,
  Moon,
  Settings,
  Sun,
  User,
} from 'lucide-react';

type SettingsState = {
  playbackSpeed: number;
  sleepTimerDefault: number;
  autoPreload: boolean;
  autoCache?: boolean;
  widgetCss?: string;
  theme: Theme;
  settingsJson?: Record<string, unknown>;
  homeLayout?: HomeLayoutSettings;
  userId?: string;
  updatedAt?: string;
  [key: string]: unknown;
};

const defaultSettings: SettingsState = {
  playbackSpeed: 1.0,
  sleepTimerDefault: 0,
  autoPreload: false,
  autoCache: false,
  widgetCss: '',
  theme: 'system',
  homeLayout: DEFAULT_HOME_LAYOUT,
};

const PersonalizationPage: React.FC = () => {
  const user = useAuthStore(state => state.user);
  const token = useAuthStore(state => state.token);
  const currentChapter = usePlayerStore((state) => state.currentChapter);
  const setPlaybackSpeed = usePlayerStore((state) => state.setPlaybackSpeed);
  const { applyTheme } = useTheme();
  const [settings, setSettings] = useState<SettingsState>(defaultSettings);
  const [loading, setLoading] = useState(true);
  const [saved, setSaved] = useState(false);
  const [widgetEmbedType, setWidgetEmbedType] = useState<'private' | 'public'>('private');

  useEffect(() => {
    const fetchSettings = async () => {
      try {
        const response = await apiClient.get('/api/settings');
        const data = response.data;
        const fetchedSettings: SettingsState = {
          ...defaultSettings,
          ...data,
          playbackSpeed: data.playbackSpeed ?? defaultSettings.playbackSpeed,
          sleepTimerDefault: data.sleepTimerDefault ?? defaultSettings.sleepTimerDefault,
          autoPreload: data.autoPreload ?? defaultSettings.autoPreload,
          autoCache: !!data.autoCache,
          widgetCss: data.widgetCss ?? '',
          theme: data.theme ?? defaultSettings.theme,
          homeLayout: normalizeHomeLayout(data.settingsJson?.homeLayout ?? data.homeLayout),
        };
        setSettings(fetchedSettings);
        applyTheme(fetchedSettings.theme);
      } catch (err) {
        console.error('获取设置失败', err);
      } finally {
        setLoading(false);
      }
    };

    fetchSettings();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const sanitizeSettings = (data: SettingsState) => {
    const cleanSettings = { ...data };
    delete cleanSettings.settingsJson;
    delete cleanSettings.userId;
    delete cleanSettings.updatedAt;
    if (user?.role !== 'admin') {
      delete cleanSettings.autoCache;
      delete cleanSettings.widgetCss;
    }
    return cleanSettings;
  };

  const handleSaveSettings = async (patch: Partial<SettingsState>) => {
    const nextSettings = { ...settings, ...patch };

    try {
      await apiClient.post('/api/settings', sanitizeSettings(nextSettings));
      setSettings(nextSettings);

      if (typeof patch.playbackSpeed === 'number') {
        setPlaybackSpeed(patch.playbackSpeed);
      }
      if (patch.theme) {
        applyTheme(patch.theme);
      }

      setSaved(true);
      setTimeout(() => setSaved(false), 1800);
    } catch {
      alert('保存失败');
    }
  };

  const handleSaveHomeLayout = async (patch: Partial<HomeLayoutSettings>) => {
    const currentLayout = normalizeHomeLayout(settings.homeLayout);
    const nextHomeLayout = { ...currentLayout, ...patch };
    await handleSaveSettings({ homeLayout: nextHomeLayout });
  };

  const widgetToken = widgetEmbedType === 'private' && token ? `?token=${token}` : '';
  const widgetUrl = `${window.location.origin}/widget${widgetToken}`;
  const embedCode = `<iframe src="${widgetUrl}" width="100%" height="150" frameborder="0" allow="autoplay; fullscreen"></iframe>`;
  const homeLayout = normalizeHomeLayout(settings.homeLayout);
  const layoutCodes = useMemo(() => ({
    fixedBottom: `<div style="position: fixed; bottom: 0; left: 0; width: 100%; z-index: 9999;">
  <iframe src="${widgetUrl}" width="100%" height="150" frameborder="0" allow="autoplay; fullscreen"></iframe>
</div>`,
    floatingRight: `<div style="position: fixed; bottom: 20px; right: 20px; width: 350px; height: 150px; z-index: 9999; border-radius: 16px; overflow: hidden; box-shadow: 0 4px 20px rgba(0,0,0,0.15);">
  <iframe src="${widgetUrl}" width="100%" height="100%" frameborder="0" allow="autoplay; fullscreen"></iframe>
</div>`,
  }), [widgetUrl]);

  const handleCopy = async (text: string) => {
    try {
      if (navigator.clipboard && navigator.clipboard.writeText) {
        await navigator.clipboard.writeText(text);
      } else {
        const textArea = document.createElement('textarea');
        textArea.value = text;
        textArea.style.position = 'fixed';
        textArea.style.left = '-9999px';
        textArea.style.top = '0';
        document.body.appendChild(textArea);
        textArea.focus();
        textArea.select();
        document.execCommand('copy');
        document.body.removeChild(textArea);
      }
      alert('已复制到剪贴板');
    } catch (err) {
      console.error('复制失败:', err);
      alert('复制失败，请手动复制');
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
    <div className="flex-1 min-h-full flex flex-col p-4 sm:p-6 md:p-8 animate-in fade-in duration-500">
      <div className="flex-1 space-y-6 max-w-5xl w-full mx-auto">
        <BackButton fallback="/mine" />

        <div className="flex items-center justify-between gap-4">
          <div>
            <h1 className="text-2xl md:text-3xl font-bold text-slate-900 dark:text-white flex items-center gap-3">
              <Settings className="text-primary-600" />
              个性化设置
            </h1>
            <p className="text-sm md:text-base text-slate-500 dark:text-slate-400 mt-1">外观展示与播放偏好。</p>
          </div>
          {saved && (
            <span className="text-sm text-green-600 font-bold flex items-center gap-1">
              <CheckCircle2 size={14} />
              已保存
            </span>
          )}
        </div>

        <section className="bg-white dark:bg-slate-900 rounded-3xl p-5 md:p-6 border border-slate-100 dark:border-slate-800 shadow-sm">
          <h2 className="text-xl font-bold dark:text-white mb-6 flex items-center gap-2">
            <Monitor size={20} className="text-blue-500" />
            外观展示
          </h2>
          <div className="grid grid-cols-3 gap-2 md:gap-4">
            {[
              { id: 'light' as Theme, icon: <Sun size={20} />, label: '浅色模式' },
              { id: 'dark' as Theme, icon: <Moon size={20} />, label: '深色模式' },
              { id: 'system' as Theme, icon: <Monitor size={20} />, label: '跟随系统' },
            ].map(theme => (
              <button
                key={theme.id}
                onClick={() => handleSaveSettings({ theme: theme.id })}
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

        <section className="bg-white dark:bg-slate-900 rounded-3xl p-5 md:p-6 border border-slate-100 dark:border-slate-800 shadow-sm">
          <h2 className="text-xl font-bold dark:text-white mb-6 flex items-center gap-2">
            <Home size={20} className="text-emerald-500" />
            首页调整
          </h2>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            <HomeLayoutToggle
              title="顶部推荐"
              description="展示继续收听和可点击切换的大封面 Hero"
              checked={homeLayout.showHero}
              onChange={() => handleSaveHomeLayout({ showHero: !homeLayout.showHero })}
            />
            <HomeLayoutToggle
              title="听书数据"
              description="展示最近已听、收藏、书单和当前播放"
              checked={homeLayout.showStats}
              onChange={() => handleSaveHomeLayout({ showStats: !homeLayout.showStats })}
            />
            <HomeLayoutToggle
              title="为你推荐"
              description="展示收藏、最近收听和最近上新的综合推荐"
              checked={homeLayout.showRecommended}
              onChange={() => handleSaveHomeLayout({ showRecommended: !homeLayout.showRecommended })}
            />
            <HomeLayoutToggle
              title="最近收听"
              description="展示首页内的最近收听卡片"
              checked={homeLayout.showRecent}
              onChange={() => handleSaveHomeLayout({ showRecent: !homeLayout.showRecent })}
            />
            <HomeLayoutToggle
              title="最近上新"
              description="展示最新加入馆藏的作品列表"
              checked={homeLayout.showRecentlyAdded}
              onChange={() => handleSaveHomeLayout({ showRecentlyAdded: !homeLayout.showRecentlyAdded })}
            />
            <HomeLayoutToggle
              title="书单与系列"
              description="展示我的书单和系列入口"
              checked={homeLayout.showCollections}
              onChange={() => handleSaveHomeLayout({ showCollections: !homeLayout.showCollections })}
            />
          </div>
        </section>

        <section className="bg-white dark:bg-slate-900 rounded-3xl p-5 md:p-6 border border-slate-100 dark:border-slate-800 shadow-sm">
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
                    onClick={() => handleSaveSettings({ playbackSpeed: speed })}
                    className={`flex-1 sm:flex-none px-2 md:px-4 py-2 text-sm font-bold rounded-lg transition-all ${
                      settings.playbackSpeed === speed
                        ? 'bg-white dark:bg-slate-700 shadow-sm text-primary-600'
                        : 'text-slate-500'
                    }`}
                  >
                    {speed}x
                  </button>
                ))}
              </div>
            </div>

            <ToggleRow
              title="自动预加载下一章"
              description="播放当前章节时，后台自动缓冲下一章节"
              checked={settings.autoPreload}
              onChange={() => handleSaveSettings({ autoPreload: !settings.autoPreload })}
            />
            {user?.role === 'admin' && (
              <ToggleRow
                title="服务端自动缓存"
                description="播放当前章节时，通知服务器预先缓存下一章节"
                checked={!!settings.autoCache}
                onChange={() => handleSaveSettings({ autoCache: !settings.autoCache })}
              />
            )}
          </div>
        </section>

        {user?.role === 'admin' && (
          <section className="bg-white dark:bg-slate-900 rounded-3xl p-5 md:p-6 border border-slate-100 dark:border-slate-800 shadow-sm">
            <div className="flex flex-col lg:flex-row lg:items-center lg:justify-between gap-4 mb-6">
              <h2 className="text-xl font-bold dark:text-white flex items-center gap-2">
                <Code size={20} className="text-violet-500" />
                Widget 设置
              </h2>
              <a
                href="/widget"
                target="_blank"
                rel="noopener noreferrer"
                className="inline-flex items-center justify-center gap-2 px-3 py-2 rounded-xl bg-slate-100 dark:bg-slate-800 text-sm font-bold text-slate-600 dark:text-slate-300 hover:text-primary-600 transition-colors"
              >
                <ExternalLink size={16} />
                打开 Widget
              </a>
            </div>

            <div className="space-y-5">
              <div>
                <div className="flex items-center justify-between gap-3 mb-2">
                  <p className="font-bold dark:text-white">自定义 CSS</p>
                  <span className="text-[10px] text-slate-400 uppercase font-bold">Widget only</span>
                </div>
                <textarea
                  value={settings.widgetCss || ''}
                  onChange={event => setSettings(prev => ({ ...prev, widgetCss: event.target.value }))}
                  onBlur={() => handleSaveSettings({ widgetCss: settings.widgetCss || '' })}
                  placeholder=".widget-mode { background: transparent !important; }"
                  className="w-full h-32 px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-2xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white font-mono text-sm resize-none"
                />
              </div>

              <div className="rounded-2xl bg-slate-50 dark:bg-slate-800/70 border border-slate-100 dark:border-slate-800 p-4 space-y-4">
                <div className="flex items-center justify-between gap-3">
                  <p className="font-bold dark:text-white">嵌入代码</p>
                  <div className="flex bg-white dark:bg-slate-900 rounded-xl p-1 border border-slate-100 dark:border-slate-700">
                    <button
                      onClick={() => setWidgetEmbedType('private')}
                      className={`flex items-center gap-1.5 px-3 py-1.5 text-xs font-bold rounded-lg transition-all ${
                        widgetEmbedType === 'private'
                          ? 'bg-primary-50 dark:bg-primary-900/20 text-primary-600'
                          : 'text-slate-500'
                      }`}
                    >
                      <Key size={13} />
                      免登录
                    </button>
                    <button
                      onClick={() => setWidgetEmbedType('public')}
                      className={`flex items-center gap-1.5 px-3 py-1.5 text-xs font-bold rounded-lg transition-all ${
                        widgetEmbedType === 'public'
                          ? 'bg-primary-50 dark:bg-primary-900/20 text-primary-600'
                          : 'text-slate-500'
                      }`}
                    >
                      <User size={13} />
                      公开
                    </button>
                  </div>
                </div>

                <CopyBlock code={embedCode} onCopy={handleCopy} />

                <div className="grid grid-cols-1 lg:grid-cols-2 gap-3">
                  <EmbedSnippetCard title="吸底模式" code={layoutCodes.fixedBottom} onCopy={handleCopy} />
                  <EmbedSnippetCard title="右下悬浮" code={layoutCodes.floatingRight} onCopy={handleCopy} />
                </div>

                <div className="flex gap-2 text-[11px] text-slate-500 dark:text-slate-400">
                  <Info size={14} className="shrink-0 mt-0.5" />
                  <span>{widgetEmbedType === 'private' ? '免登录代码包含访问凭证，请只放在可信私有页面。' : '公开模式不包含凭证，访客首次使用时需要登录。'}</span>
                </div>
              </div>
            </div>
          </section>
        )}
      </div>

      <div
        className="shrink-0 transition-all duration-300"
        style={{ height: currentChapter ? 'var(--safe-bottom-with-player)' : 'var(--safe-bottom-base)' }}
      />
    </div>
  );
};

const ToggleRow = ({
  title,
  description,
  checked,
  onChange,
}: {
  title: string;
  description: string;
  checked: boolean;
  onChange: () => void;
}) => (
  <div className="flex items-center justify-between gap-4 pt-4 border-t border-slate-100 dark:border-slate-800">
    <div className="flex-1 min-w-0">
      <p className="font-bold dark:text-white truncate">{title}</p>
      <p className="text-xs md:text-sm text-slate-500 line-clamp-2">{description}</p>
    </div>
    <button
      onClick={onChange}
      className={`flex-shrink-0 w-12 md:w-14 h-7 md:h-8 rounded-full transition-all relative ${
        checked ? 'bg-primary-600' : 'bg-slate-200 dark:bg-slate-700'
      }`}
    >
      <div className={`absolute top-1 w-5 md:w-6 h-5 md:h-6 bg-white rounded-full transition-all ${
        checked ? 'left-6 md:left-7' : 'left-1'
      }`} />
    </button>
  </div>
);

const HomeLayoutToggle = ({
  title,
  description,
  checked,
  onChange,
}: {
  title: string;
  description: string;
  checked: boolean;
  onChange: () => void;
}) => (
  <button
    type="button"
    onClick={onChange}
    className={`text-left rounded-2xl border p-4 transition-all ${
      checked
        ? 'border-primary-200 dark:border-primary-900/50 bg-primary-50/80 dark:bg-primary-900/20'
        : 'border-slate-100 dark:border-slate-800 bg-slate-50 dark:bg-slate-800/50 hover:bg-slate-100 dark:hover:bg-slate-800'
    }`}
  >
    <div className="flex items-start justify-between gap-4">
      <div className="min-w-0">
        <p className="font-bold text-slate-900 dark:text-white">{title}</p>
        <p className="text-xs text-slate-500 line-clamp-2 mt-1">{description}</p>
      </div>
      <span className={`shrink-0 w-11 h-6 rounded-full transition-all relative ${
        checked ? 'bg-primary-600' : 'bg-slate-200 dark:bg-slate-700'
      }`}>
        <span className={`absolute top-1 w-4 h-4 bg-white rounded-full transition-all ${
          checked ? 'left-6' : 'left-1'
        }`} />
      </span>
    </div>
  </button>
);

const CopyBlock = ({ code, onCopy }: { code: string; onCopy: (code: string) => void }) => (
  <div className="relative">
    <code className="text-[10px] md:text-xs text-slate-600 dark:text-slate-400 break-all bg-white dark:bg-slate-950 p-3 pr-11 rounded-xl block border border-slate-100 dark:border-slate-900 font-mono leading-relaxed">
      {code}
    </code>
    <button
      onClick={() => onCopy(code)}
      className="absolute top-2 right-2 p-1.5 bg-slate-100 dark:bg-slate-800 hover:bg-primary-50 dark:hover:bg-primary-900/30 text-slate-500 hover:text-primary-600 rounded-lg transition-colors"
      title="复制"
    >
      <Copy size={14} />
    </button>
  </div>
);

const EmbedSnippetCard = ({
  title,
  code,
  onCopy,
}: {
  title: string;
  code: string;
  onCopy: (code: string) => void;
}) => (
  <div className="relative bg-white dark:bg-slate-900 rounded-xl border border-slate-100 dark:border-slate-700 p-3">
    <p className="text-xs font-bold text-slate-500 mb-2">{title}</p>
    <code className="text-[10px] text-slate-600 dark:text-slate-400 font-mono block whitespace-pre overflow-x-auto pr-9">
      {code}
    </code>
    <button
      onClick={() => onCopy(code)}
      className="absolute top-3 right-3 p-1.5 bg-slate-100 dark:bg-slate-800 hover:bg-primary-50 dark:hover:bg-primary-900/30 text-slate-500 hover:text-primary-600 rounded-lg transition-colors"
      title="复制"
    >
      <Copy size={14} />
    </button>
  </div>
);

export default PersonalizationPage;
