import React, { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import apiClient from '../../core/api/client';
import type { Book, Playlist, Progress } from '../../core/types';
import { useAuthStore } from '../../core/stores/authStore';
import { usePlayerStore } from '../../core/stores/playerStore';
import {
  ChevronRight,
  CheckCircle2,
  BarChart3,
  BellRing,
  Heart,
  History,
  Info,
  Key,
  Save,
  Settings,
  User,
} from 'lucide-react';

type UpdateInfo = {
  version: string;
  downloadUrl: string;
  size: string;
  date: string;
};

const MyPage: React.FC = () => {
  const user = useAuthStore(state => state.user);
  const setUser = useAuthStore(state => state.setUser);
  const currentChapter = usePlayerStore((state) => state.currentChapter);
  const [recentPlays, setRecentPlays] = useState<Progress[]>([]);
  const [favorites, setFavorites] = useState<Book[]>([]);
  const [playlistCount, setPlaylistCount] = useState(0);
  const [backendVersion, setBackendVersion] = useState('');
  const [showAbout, setShowAbout] = useState(false);
  const [backendUpdateInfo, setBackendUpdateInfo] = useState<UpdateInfo | null>(null);
  const [checkingBackendUpdate, setCheckingBackendUpdate] = useState(false);
  const [accountData, setAccountData] = useState({
    username: user?.username || '',
    password: '',
  });
  const [accountSaved, setAccountSaved] = useState(false);

  useEffect(() => {
    const fetchData = async () => {
      const [recentRes, favoritesRes, playlistsRes, healthRes] = await Promise.allSettled([
        apiClient.get('/api/progress/recent'),
        apiClient.get('/api/favorites'),
        apiClient.get('/api/playlists'),
        apiClient.get('/api/health'),
      ]);

      if (recentRes.status === 'fulfilled') {
        setRecentPlays(recentRes.value.data || []);
      }
      if (favoritesRes.status === 'fulfilled') {
        setFavorites(favoritesRes.value.data || []);
      }
      if (playlistsRes.status === 'fulfilled') {
        setPlaylistCount((playlistsRes.value.data as Playlist[] || []).length);
      }
      if (healthRes.status === 'fulfilled' && healthRes.value.data?.version) {
        setBackendVersion(healthRes.value.data.version);
      }
    };

    fetchData();
    window.addEventListener('focus', fetchData);
    return () => window.removeEventListener('focus', fetchData);
  }, []);

  useEffect(() => {
    setAccountData((current) => ({ ...current, username: user?.username || '' }));
  }, [user?.username]);

  const listenedMinutes = Math.round(
    recentPlays.reduce((total, progress) => total + Math.max(0, progress.position || 0), 0) / 60
  );
  const userInitial = user?.username?.charAt(0).toUpperCase() || 'U';

  const handleAccountUpdate = async (event: React.FormEvent) => {
    event.preventDefault();
    const nextUsername = accountData.username.trim();
    const nextPassword = accountData.password.trim();

    if (!nextUsername) {
      alert('用户名不能为空');
      return;
    }

    try {
      const updateData: Record<string, string> = {};
      if (nextUsername !== user?.username) {
        updateData.username = nextUsername;
      }
      if (nextPassword) {
        updateData.password = nextPassword;
      }

      if (Object.keys(updateData).length > 0) {
        await apiClient.patch('/api/me', updateData);
        if (updateData.username && user) {
          setUser({ ...user, username: updateData.username });
        }
      }

      setAccountData({ username: nextUsername, password: '' });
      setAccountSaved(true);
      setTimeout(() => setAccountSaved(false), 1800);
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : '更新失败';
      alert(message);
    }
  };

  const handleCheckBackendUpdate = async () => {
    if (checkingBackendUpdate || !backendVersion) return;
    setCheckingBackendUpdate(true);
    try {
      const { data } = await apiClient.get('/api/system/check-update');
      const remoteVersion = data.version.replace(/^v/, '');
      const currentVersion = backendVersion.replace(/^v/, '');

      if (remoteVersion !== currentVersion) {
        setBackendUpdateInfo(data);
      } else {
        alert('服务端已是最新版本');
      }
    } catch (error) {
      console.error('检查后端更新失败', error);
      alert('检查服务端更新失败，请稍后重试');
    } finally {
      setCheckingBackendUpdate(false);
    }
  };

  return (
    <div className="flex-1 min-h-full flex flex-col p-4 sm:p-6 md:p-8 animate-in fade-in duration-500">
      <div className="flex-1 space-y-6 max-w-5xl w-full mx-auto">
        <section className="bg-white dark:bg-slate-900 border border-slate-100 dark:border-slate-800 rounded-3xl p-5 md:p-6 shadow-sm">
          <div className="flex items-center gap-4">
            <div className="w-16 h-16 rounded-2xl bg-primary-100 dark:bg-primary-900/30 text-primary-600 flex items-center justify-center text-2xl font-bold shrink-0">
              {userInitial}
            </div>
            <div className="min-w-0">
              <p className="text-sm text-slate-500 dark:text-slate-400">我的</p>
              <h1 className="text-2xl md:text-3xl font-bold text-slate-900 dark:text-white truncate">
                {user?.username || '听书用户'}
              </h1>
              <p className="text-sm text-slate-500 mt-1">管理听书记录、收藏、书单和个人偏好。</p>
            </div>
          </div>

          <form
            onSubmit={handleAccountUpdate}
            className="mt-5 grid grid-cols-1 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto] gap-3 items-end"
          >
            <label className="block min-w-0">
              <span className="text-xs font-bold text-slate-500 dark:text-slate-400">用户名</span>
              <div className="relative mt-1.5">
                <User className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" size={17} />
                <input
                  type="text"
                  value={accountData.username}
                  onChange={event => setAccountData({ ...accountData, username: event.target.value })}
                  className="w-full pl-10 pr-4 py-2.5 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white text-sm"
                />
              </div>
            </label>
            <label className="block min-w-0">
              <span className="text-xs font-bold text-slate-500 dark:text-slate-400">修改密码</span>
              <div className="relative mt-1.5">
                <Key className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" size={17} />
                <input
                  type="password"
                  value={accountData.password}
                  onChange={event => setAccountData({ ...accountData, password: event.target.value })}
                  placeholder="留空则不修改"
                  className="w-full pl-10 pr-4 py-2.5 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white text-sm"
                />
              </div>
            </label>
            <div className="flex items-center gap-3">
              {accountSaved && (
                <span className="text-sm text-green-600 font-bold whitespace-nowrap">已更新</span>
              )}
              <button
                type="submit"
                className="inline-flex items-center justify-center gap-2 px-4 py-2.5 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-xl shadow-lg shadow-primary-500/20 transition-colors text-sm whitespace-nowrap"
              >
                <Save size={16} />
                保存
              </button>
            </div>
          </form>

          <div className="grid grid-cols-3 gap-3 mt-6">
            <SummaryCard label="最近" value={recentPlays.length} unit="本" />
            <SummaryCard label="收藏" value={favorites.length} unit="本" />
            <SummaryCard label="书单" value={playlistCount} unit="个" />
          </div>
        </section>

        <section className="space-y-3">
          <SectionHeader title="我的内容" />
          <div className="bg-white dark:bg-slate-900 border border-slate-100 dark:border-slate-800 rounded-3xl shadow-sm overflow-hidden">
            <EntryItem
              to="/history"
              icon={<History size={22} />}
              title="我的历史"
              description={recentPlays.length > 0 ? `最近听过 ${recentPlays.length} 本，约 ${listenedMinutes || 0} 分钟` : '查看图文收听记录'}
              tone="text-primary-600 bg-primary-50 dark:bg-primary-900/20"
            />
            <EntryItem
              to="/favorites"
              icon={<Heart size={22} />}
              title="我的收藏"
              description={`收藏夹里有 ${favorites.length} 部作品`}
              tone="text-red-500 bg-red-50 dark:bg-red-900/20"
            />
          </div>
        </section>

        <section className="space-y-3">
          <SectionHeader title="设置与管理" />
          <div className="bg-white dark:bg-slate-900 border border-slate-100 dark:border-slate-800 rounded-3xl shadow-sm overflow-hidden">
            <EntryItem
              to="/personalization"
              icon={<Settings size={22} />}
              title="个性化设置"
              description="外观展示与播放偏好"
              tone="text-blue-600 bg-blue-50 dark:bg-blue-900/20"
            />
            {user?.role === 'admin' && (
              <EntryItem
                to="/notifications"
                icon={<BellRing size={22} />}
                title="通知与事件"
                description="配置 Webhook 监听登录、播放、入库和删除"
                tone="text-emerald-600 bg-emerald-50 dark:bg-emerald-900/20"
              />
            )}
            {user?.role === 'admin' && (
              <EntryItem
                to="/statistics"
                icon={<BarChart3 size={22} />}
                title="数据统计"
                description="用户使用情况与馆藏报表"
                tone="text-violet-600 bg-violet-50 dark:bg-violet-900/20"
              />
            )}
          </div>
        </section>

        <div className="text-center text-slate-400 text-sm pt-2 pb-4">
          <button
            onClick={() => setShowAbout(true)}
            className="inline-flex items-center gap-2 text-slate-400 hover:text-primary-600 transition-colors text-sm font-bold underline decoration-slate-300 dark:decoration-slate-700 underline-offset-4"
          >
            <Info size={16} />
            关于 Ting Reader
          </button>
          <p className="mt-4 text-xs opacity-60">©2026 Ting Reader. 保留所有权利。</p>
        </div>
      </div>

      {showAbout && (
        <AboutModal
          backendVersion={backendVersion}
          checkingBackendUpdate={checkingBackendUpdate}
          backendUpdateInfo={backendUpdateInfo}
          onCheckUpdate={handleCheckBackendUpdate}
          onClose={() => setShowAbout(false)}
          onCloseUpdate={() => setBackendUpdateInfo(null)}
        />
      )}

      <div
        className="shrink-0 transition-all duration-300"
        style={{ height: currentChapter ? 'var(--safe-bottom-with-player)' : 'var(--safe-bottom-base)' }}
      />
    </div>
  );
};

const SummaryCard = ({ label, value, unit }: { label: string; value: number; unit: string }) => (
  <div className="rounded-2xl bg-slate-50 dark:bg-slate-800/70 p-3 text-center min-w-0">
    <p className="text-lg md:text-xl font-black text-slate-900 dark:text-white truncate">
      {value}
      <span className="text-xs font-bold text-slate-400 ml-1">{unit}</span>
    </p>
    <p className="text-xs text-slate-500 font-bold mt-1">{label}</p>
  </div>
);

const SectionHeader = ({ title }: { title: string }) => (
  <h2 className="px-1 text-sm font-black text-slate-500 dark:text-slate-400">{title}</h2>
);

const EntryItem = ({
  to,
  icon,
  title,
  description,
  tone,
}: {
  to: string;
  icon: React.ReactNode;
  title: string;
  description: string;
  tone: string;
}) => (
  <Link
    to={to}
    className="flex items-center justify-between gap-4 px-4 md:px-5 py-4 border-b border-slate-100 dark:border-slate-800 last:border-b-0 hover:bg-slate-50 dark:hover:bg-slate-800 transition-colors"
  >
    <div className="flex items-center gap-3 min-w-0">
      <div className={`w-11 h-11 rounded-2xl flex items-center justify-center shrink-0 ${tone}`}>
        {icon}
      </div>
      <div className="min-w-0">
        <p className="font-bold text-slate-900 dark:text-white truncate">{title}</p>
        <p className="text-sm text-slate-500 truncate">{description}</p>
      </div>
    </div>
    <ChevronRight size={18} className="text-slate-300 shrink-0" />
  </Link>
);

const AboutModal = ({
  backendVersion,
  checkingBackendUpdate,
  backendUpdateInfo,
  onCheckUpdate,
  onClose,
  onCloseUpdate,
}: {
  backendVersion: string;
  checkingBackendUpdate: boolean;
  backendUpdateInfo: UpdateInfo | null;
  onCheckUpdate: () => void;
  onClose: () => void;
  onCloseUpdate: () => void;
}) => (
  <>
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/50 backdrop-blur-sm animate-in fade-in duration-200">
      <div className="bg-white dark:bg-slate-900 rounded-3xl p-6 w-full max-w-sm shadow-2xl animate-in zoom-in-95 duration-200 border border-slate-100 dark:border-slate-800">
        <div className="text-center mb-6">
          <img src="/logo.png" alt="Ting Reader Logo" className="w-16 h-16 mx-auto mb-4 rounded-2xl shadow-sm object-contain p-1" />
          <h3 className="text-xl font-bold dark:text-white">关于 Ting Reader</h3>
        </div>

        <div className="space-y-4 mb-6">
          <div className="flex items-center justify-between p-3 bg-slate-50 dark:bg-slate-800 rounded-xl">
            <span className="text-sm font-bold text-slate-500">服务端版本</span>
            <div className="flex items-center gap-2">
              <span className="text-sm font-bold dark:text-white">v{backendVersion || 'Unknown'}</span>
              <button
                onClick={onCheckUpdate}
                disabled={checkingBackendUpdate || !backendVersion}
                className="text-xs bg-primary-50 dark:bg-primary-900/20 text-primary-600 px-2 py-1 rounded-lg font-bold hover:bg-primary-100 dark:hover:bg-primary-900/40 transition-colors disabled:opacity-50"
              >
                {checkingBackendUpdate ? '检查中...' : '检查更新'}
              </button>
            </div>
          </div>
        </div>

        <div className="text-center mb-6">
          <span className="text-sm text-slate-500 mr-2">官网地址</span>
          <a
            href="https://www.tingreader.cn"
            target="_blank"
            rel="noopener noreferrer"
            className="text-sm text-primary-600 hover:text-primary-700 font-bold"
          >
            www.tingreader.cn
          </a>
        </div>

        <button
          onClick={onClose}
          className="w-full py-3 bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-300 font-bold rounded-xl hover:bg-slate-200 dark:hover:bg-slate-700 transition-colors"
        >
          关闭
        </button>
      </div>
    </div>

    {backendUpdateInfo && (
      <div className="fixed inset-0 z-[60] flex items-center justify-center p-4 bg-black/50 backdrop-blur-sm animate-in fade-in duration-200">
        <div className="bg-white dark:bg-slate-900 rounded-3xl p-6 w-full max-w-sm shadow-2xl animate-in zoom-in-95 duration-200 border border-slate-100 dark:border-slate-800">
          <div className="text-center mb-6">
            <div className="w-16 h-16 bg-blue-100 dark:bg-blue-900/30 rounded-2xl flex items-center justify-center mx-auto mb-4 text-blue-600">
              <CheckCircle2 size={32} />
            </div>
            <h3 className="text-xl font-bold dark:text-white">发现服务端新版本 {backendUpdateInfo.version}</h3>
            <p className="text-sm text-slate-500 mt-2">
              发布时间: {new Date(backendUpdateInfo.date).toLocaleDateString()}
            </p>
          </div>

          <div className="flex gap-3">
            <button
              onClick={onCloseUpdate}
              className="flex-1 py-3 bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-300 font-bold rounded-xl hover:bg-slate-200 dark:hover:bg-slate-700 transition-colors"
            >
              暂不更新
            </button>
            <button
              onClick={() => {
                window.open('https://www.tingreader.cn/guide/update', '_blank');
                onCloseUpdate();
              }}
              className="flex-1 py-3 bg-blue-600 text-white font-bold rounded-xl hover:bg-blue-700 transition-colors shadow-lg shadow-blue-500/30"
            >
              前往官网更新
            </button>
          </div>
        </div>
      </div>
    )}
  </>
);

export default MyPage;
