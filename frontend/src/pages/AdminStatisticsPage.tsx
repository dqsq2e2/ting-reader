import React, { useEffect, useMemo, useState } from 'react';
import {
  Activity,
  BarChart3,
  BookOpen,
  Database,
  Headphones,
  Library,
  RefreshCw,
  TrendingUp,
  Users,
} from 'lucide-react';
import apiClient from '../api/client';
import BackButton from '../components/BackButton';
import type {
  AdminStatistics,
  BookActivityStatistics,
  LibraryStatistics,
  RecentActivityPoint,
  UserActivityStatistics,
} from '../types';

const AdminStatisticsPage: React.FC = () => {
  const [report, setReport] = useState<AdminStatistics | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchReport = async (silent = false) => {
    if (silent) {
      setRefreshing(true);
    } else {
      setLoading(true);
    }
    setError(null);

    try {
      const res = await apiClient.get('/api/system/statistics');
      setReport(res.data);
    } catch (err) {
      console.error('获取统计报表失败', err);
      setError('统计报表加载失败');
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  };

  useEffect(() => {
    fetchReport();
  }, []);

  const maxUserListen = useMemo(
    () => Math.max(1, ...(report?.userActivity || []).map(item => item.listenSeconds)),
    [report]
  );
  const maxBookHeat = useMemo(
    () => Math.max(1, ...(report?.topBooks || []).map(item => getBookHeatScore(item))),
    [report]
  );

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-primary-600"></div>
      </div>
    );
  }

  if (error || !report) {
    return (
      <div className="flex-1 min-h-full flex flex-col p-4 sm:p-6 md:p-8 animate-in fade-in duration-300">
        <div className="flex-1 space-y-6 max-w-7xl w-full mx-auto">
          <BackButton fallback="/mine" />
          <div className="rounded-2xl border border-red-100 dark:border-red-900/30 bg-red-50 dark:bg-red-950/20 p-6 text-red-600 dark:text-red-300">
            {error || '统计报表暂无数据'}
          </div>
        </div>
      </div>
    );
  }

  const { overview } = report;
  const totalLibraries = Math.max(1, overview.totalLibraries);
  const localPercent = Math.round((overview.localLibraries / totalLibraries) * 100);
  const webdavPercent = Math.round((overview.webdavLibraries / totalLibraries) * 100);
  const activeUserRate = overview.totalUsers > 0
    ? Math.round((overview.activeUsers / overview.totalUsers) * 100)
    : 0;

  return (
    <div className="flex-1 min-h-full flex flex-col p-4 sm:p-6 md:p-8 animate-in fade-in duration-300">
      <div className="flex-1 space-y-5 max-w-7xl w-full mx-auto">
        <BackButton fallback="/mine" />
        <div className="flex flex-col md:flex-row md:items-end justify-between gap-4">
          <div>
            <h1 className="text-2xl md:text-3xl font-black text-slate-900 dark:text-white flex items-center gap-3">
              <BarChart3 size={28} className="text-primary-600" />
              数据统计
            </h1>
            <p className="text-sm text-slate-500 mt-2">生成时间：{formatDateTime(report.generatedAt)}</p>
          </div>
          <button
            onClick={() => fetchReport(true)}
            disabled={refreshing}
            className="inline-flex items-center justify-center gap-2 px-4 py-2.5 rounded-xl bg-slate-900 dark:bg-white text-white dark:text-slate-900 text-sm font-bold hover:opacity-90 disabled:opacity-60 transition-opacity"
          >
            <RefreshCw size={17} className={refreshing ? 'animate-spin' : ''} />
            刷新报表
          </button>
        </div>

      <section className="grid grid-cols-2 xl:grid-cols-4 gap-3 md:gap-4">
        <MetricTile
          icon={<Library size={19} />}
          label="馆藏作品"
          value={formatNumber(overview.totalBooks)}
          detail={`${formatNumber(overview.totalChapters)} 章 · ${formatDuration(overview.totalDuration)}`}
          tone="text-sky-600 bg-sky-50 dark:bg-sky-900/20"
        />
        <MetricTile
          icon={<Headphones size={19} />}
          label="累计收听"
          value={formatDuration(overview.totalListenSeconds)}
          detail={`${formatNumber(overview.totalProgressRecords)} 条进度记录`}
          tone="text-emerald-600 bg-emerald-50 dark:bg-emerald-900/20"
        />
        <MetricTile
          icon={<Users size={19} />}
          label="活跃用户"
          value={`${formatNumber(overview.activeUsers)} / ${formatNumber(overview.totalUsers)}`}
          detail={`活跃率 ${activeUserRate}% · 管理员 ${formatNumber(overview.adminUsers)}`}
          tone="text-violet-600 bg-violet-50 dark:bg-violet-900/20"
        />
        <MetricTile
          icon={<Database size={19} />}
          label="媒体库"
          value={formatNumber(overview.totalLibraries)}
          detail={`本地 ${formatNumber(overview.localLibraries)} · WebDAV ${formatNumber(overview.webdavLibraries)}`}
          tone="text-amber-600 bg-amber-50 dark:bg-amber-900/20"
        />
      </section>

      <section className="grid grid-cols-1 2xl:grid-cols-[minmax(0,1.28fr)_minmax(380px,0.72fr)] gap-5">
        <Panel title="活跃趋势" icon={<TrendingUp size={19} />}>
          {report.recentActivity.length > 0 ? (
            <UsageTrend items={report.recentActivity} />
          ) : (
            <EmptyState icon={<TrendingUp size={30} />} text="暂无近期活跃记录" />
          )}
        </Panel>

        <Panel title="媒体库结构" icon={<Database size={19} />}>
          <LibraryMix
            total={overview.totalLibraries}
            local={overview.localLibraries}
            webdav={overview.webdavLibraries}
            localPercent={localPercent}
            webdavPercent={webdavPercent}
          />
        </Panel>
      </section>

      <section className="grid grid-cols-1 2xl:grid-cols-[minmax(0,1.12fr)_minmax(420px,0.88fr)] gap-5">
        <Panel title="馆藏数据" icon={<Library size={19} />}>
          {report.libraryBreakdown.length > 0 ? (
            <LibraryCards items={report.libraryBreakdown} />
          ) : (
            <EmptyState icon={<Library size={30} />} text="暂无媒体库数据" />
          )}
        </Panel>

        <Panel title="用户使用情况" icon={<Activity size={19} />}>
          {report.userActivity.length > 0 ? (
            <UserTable items={report.userActivity} maxListen={maxUserListen} />
          ) : (
            <EmptyState icon={<Users size={30} />} text="暂无用户活跃数据" />
          )}
        </Panel>
      </section>

      <Panel title="热门收听作品" icon={<BookOpen size={19} />}>
        {report.topBooks.length > 0 ? (
          <TopBookLeaderboard items={report.topBooks} maxHeat={maxBookHeat} />
        ) : (
          <EmptyState icon={<BookOpen size={30} />} text="暂无作品收听数据" />
        )}
      </Panel>
      </div>
    </div>
  );
};

const Panel = ({ title, icon, children }: { title: string; icon: React.ReactNode; children: React.ReactNode }) => (
  <section className="bg-white dark:bg-slate-900 rounded-2xl border border-slate-100 dark:border-slate-800 shadow-sm p-4 md:p-5 min-w-0">
    <div className="flex items-center gap-2.5 mb-4">
      <div className="w-9 h-9 rounded-xl bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-300 flex items-center justify-center">
        {icon}
      </div>
      <h2 className="text-base md:text-lg font-black text-slate-900 dark:text-white">{title}</h2>
    </div>
    {children}
  </section>
);

const MetricTile = ({
  icon,
  label,
  value,
  detail,
  tone,
}: {
  icon: React.ReactNode;
  label: string;
  value: string;
  detail: string;
  tone: string;
}) => (
  <div className="bg-white dark:bg-slate-900 rounded-2xl border border-slate-100 dark:border-slate-800 shadow-sm p-4 min-w-0">
    <div className="flex items-center justify-between gap-3">
      <div className={`w-10 h-10 rounded-xl flex items-center justify-center ${tone}`}>
        {icon}
      </div>
      <span className="text-[11px] font-bold text-slate-400 uppercase tracking-wide">{label}</span>
    </div>
    <p className="text-xl md:text-2xl font-black text-slate-900 dark:text-white truncate mt-4">{value}</p>
    <p className="text-xs text-slate-500 truncate mt-1">{detail}</p>
  </div>
);

const UsageTrend = ({ items }: { items: RecentActivityPoint[] }) => {
  const width = 720;
  const height = 230;
  const paddingX = 28;
  const paddingY = 24;
  const maxUpdates = Math.max(1, ...items.map(item => item.progressUpdates));
  const points = items.map((item, index) => {
    const x = items.length === 1 ? width / 2 : paddingX + (index / (items.length - 1)) * (width - paddingX * 2);
    const y = height - paddingY - (item.progressUpdates / maxUpdates) * (height - paddingY * 2);
    return { x, y, item };
  });
  const line = points.map(point => `${point.x},${point.y}`).join(' ');
  const area = `${paddingX},${height - paddingY} ${line} ${width - paddingX},${height - paddingY}`;
  const totalUpdates = items.reduce((sum, item) => sum + item.progressUpdates, 0);
  const totalListen = items.reduce((sum, item) => sum + item.listenSeconds, 0);
  const activeUsers = Math.max(0, ...items.map(item => item.activeUsers));

  return (
    <div>
      <div className="grid grid-cols-3 gap-3 mb-4">
        <TrendStat label="更新次数" value={`${formatNumber(totalUpdates)} 次`} />
        <TrendStat label="活跃峰值" value={`${formatNumber(activeUsers)} 人`} />
        <TrendStat label="累计收听" value={formatDuration(totalListen)} />
      </div>
      <div className="relative h-72 rounded-2xl bg-slate-50 dark:bg-slate-950 border border-slate-100 dark:border-slate-800 overflow-hidden">
        <svg viewBox={`0 0 ${width} ${height}`} className="absolute inset-0 w-full h-full" preserveAspectRatio="none">
          <defs>
            <linearGradient id="statisticsTrendLine" x1="0" x2="1" y1="0" y2="0">
              <stop offset="0%" stopColor="#0284c7" />
              <stop offset="100%" stopColor="#059669" />
            </linearGradient>
            <linearGradient id="statisticsTrendArea" x1="0" x2="0" y1="0" y2="1">
              <stop offset="0%" stopColor="#0284c7" stopOpacity="0.18" />
              <stop offset="100%" stopColor="#059669" stopOpacity="0.02" />
            </linearGradient>
          </defs>
          {[0.25, 0.5, 0.75].map(mark => (
            <line
              key={mark}
              x1={paddingX}
              x2={width - paddingX}
              y1={height * mark}
              y2={height * mark}
              stroke="currentColor"
              className="text-slate-200 dark:text-slate-800"
              strokeWidth="1"
            />
          ))}
          <polygon points={area} fill="url(#statisticsTrendArea)" />
          <polyline points={line} fill="none" stroke="url(#statisticsTrendLine)" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" />
          {points.map(point => (
            <circle key={point.item.date} cx={point.x} cy={point.y} r="4" fill="#fff" stroke="#0284c7" strokeWidth="2.5" />
          ))}
        </svg>
        <div className="absolute inset-x-6 bottom-3 flex justify-between text-[10px] text-slate-400">
          {items.map(item => (
            <span key={item.date} className="truncate">{formatDay(item.date)}</span>
          ))}
        </div>
      </div>
    </div>
  );
};

const TrendStat = ({ label, value }: { label: string; value: string }) => (
  <div className="rounded-xl bg-slate-50 dark:bg-slate-800/70 border border-slate-100 dark:border-slate-800 p-3 min-w-0">
    <p className="text-[11px] text-slate-500 font-bold">{label}</p>
    <p className="font-black text-slate-900 dark:text-white truncate mt-1">{value}</p>
  </div>
);

const LibraryMix = ({
  total,
  local,
  webdav,
  localPercent,
  webdavPercent,
}: {
  total: number;
  local: number;
  webdav: number;
  localPercent: number;
  webdavPercent: number;
}) => (
  <div className="space-y-4">
    <div className="grid grid-cols-3 gap-3">
      <SmallStat label="总数" value={formatNumber(total)} />
      <SmallStat label="本地" value={formatNumber(local)} />
      <SmallStat label="WebDAV" value={formatNumber(webdav)} />
    </div>
    <div className="h-3 rounded-full bg-slate-100 dark:bg-slate-800 overflow-hidden flex">
      <div className="bg-sky-500" style={{ width: `${localPercent}%` }} />
      <div className="bg-violet-500" style={{ width: `${webdavPercent}%` }} />
    </div>
    <div className="space-y-3">
      <MixRow label="本地库" value={localPercent} color="bg-sky-500" />
      <MixRow label="WebDAV" value={webdavPercent} color="bg-violet-500" />
    </div>
  </div>
);

const SmallStat = ({ label, value }: { label: string; value: string }) => (
  <div className="rounded-xl bg-slate-50 dark:bg-slate-800/70 border border-slate-100 dark:border-slate-800 p-3">
    <p className="text-[11px] text-slate-500 font-bold">{label}</p>
    <p className="text-lg font-black text-slate-900 dark:text-white mt-1">{value}</p>
  </div>
);

const MixRow = ({ label, value, color }: { label: string; value: number; color: string }) => (
  <div className="flex items-center justify-between gap-4">
    <div className="flex items-center gap-2 text-sm font-bold text-slate-600 dark:text-slate-300">
      <span className={`w-2.5 h-2.5 rounded-full ${color}`} />
      {label}
    </div>
    <span className="text-sm font-black text-slate-900 dark:text-white">{value}%</span>
  </div>
);

const LibraryCards = ({ items }: { items: LibraryStatistics[] }) => (
  <div className="grid grid-cols-1 lg:grid-cols-3 gap-3">
    {items.map(item => (
      <article
        key={item.id}
        className="rounded-2xl border border-slate-100 dark:border-slate-800 bg-slate-50/70 dark:bg-slate-950/40 p-4"
      >
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <p className="font-black text-slate-900 dark:text-white truncate">{item.name}</p>
            <p className="text-xs text-slate-500 mt-1">{formatDuration(item.totalDuration)}</p>
          </div>
          <TypeBadge value={item.libraryType} />
        </div>

        <div className="grid grid-cols-3 gap-2 mt-4">
          <CompactStat label="作品" value={formatNumber(item.totalBooks)} />
          <CompactStat label="章节" value={formatNumber(item.totalChapters)} />
          <CompactStat label="时长" value={formatShortDuration(item.totalDuration)} />
        </div>

        <div className="flex items-center justify-between gap-3 mt-4 pt-4 border-t border-slate-200/70 dark:border-slate-800 text-xs">
          <span className="text-slate-400 font-bold">最近扫描</span>
          <span className="text-slate-600 dark:text-slate-300 font-bold truncate">{formatDateTime(item.lastScannedAt)}</span>
        </div>
      </article>
    ))}
  </div>
);

const UserTable = ({ items, maxListen }: { items: UserActivityStatistics[]; maxListen: number }) => (
  <div className="space-y-0 divide-y divide-slate-100 dark:divide-slate-800">
    {items.map(item => (
      <div key={item.id} className="py-3">
        <div className="flex items-center justify-between gap-4">
          <div className="flex items-center gap-3 min-w-0">
            <div className="w-9 h-9 rounded-full bg-primary-100 dark:bg-primary-900/30 text-primary-600 flex items-center justify-center font-black shrink-0">
              {item.username.charAt(0).toUpperCase()}
            </div>
            <div className="min-w-0">
              <p className="font-black text-slate-900 dark:text-white truncate">{item.username}</p>
              <p className="text-xs text-slate-500">{item.role === 'admin' ? '管理员' : '普通用户'} · {formatNumber(item.listenedBooks)} 本</p>
            </div>
          </div>
          <div className="text-right shrink-0">
            <p className="text-sm font-black text-slate-900 dark:text-white">{formatDuration(item.listenSeconds)}</p>
            <p className="text-[11px] text-slate-400">{formatDateTime(item.lastActiveAt)}</p>
          </div>
        </div>
        <div className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-3 mt-3">
          <ProgressLine value={item.listenSeconds} max={maxListen} color="bg-emerald-500" />
          <span className="text-[11px] text-slate-500 font-bold">{formatNumber(item.progressRecords)} 条</span>
        </div>
      </div>
    ))}
  </div>
);

const TopBookLeaderboard = ({ items, maxHeat }: { items: BookActivityStatistics[]; maxHeat: number }) => (
  <div className="grid grid-cols-1 xl:grid-cols-3 gap-3">
    {items.map((item, index) => {
      const heat = getBookHeatScore(item);
      const accent = getRankAccent(index);
      return (
        <article
          key={item.id}
          className={`rounded-2xl border p-4 min-w-0 ${accent.surface}`}
        >
          <div className="flex items-start gap-3">
            <div className={`w-9 h-9 rounded-xl flex items-center justify-center text-sm font-black shrink-0 ${accent.badge}`}>
              {index + 1}
            </div>
            <div className="min-w-0 flex-1">
              <p className="font-black text-slate-900 dark:text-white truncate">{item.title || '未知作品'}</p>
              <p className="text-xs text-slate-500 mt-1 truncate">
                {item.author || '未知作者'} · {item.libraryName || '未知媒体库'}
              </p>
            </div>
          </div>

          <div className="grid grid-cols-3 gap-2 mt-4">
            <CompactStat label="听众" value={formatNumber(item.listeners)} />
            <CompactStat label="记录" value={formatNumber(item.progressUpdates)} />
            <CompactStat label="收听" value={formatShortDuration(item.listenSeconds)} />
          </div>

          <div className="mt-4">
            <div className="flex items-center justify-between gap-3 mb-2 text-xs">
              <span className="text-slate-400 font-bold">综合热度</span>
              <span className="font-black text-slate-700 dark:text-slate-200">{formatNumber(heat)}</span>
            </div>
            <ProgressLine value={heat} max={maxHeat} color={accent.bar} />
          </div>
        </article>
      );
    })}
  </div>
);

const CompactStat = ({ label, value }: { label: string; value: string }) => (
  <div className="rounded-xl bg-white dark:bg-slate-900 border border-slate-100 dark:border-slate-800 px-3 py-2 min-w-0">
    <p className="text-[10px] text-slate-400 font-bold">{label}</p>
    <p className="text-sm font-black text-slate-900 dark:text-white truncate mt-0.5">{value}</p>
  </div>
);

const TypeBadge = ({ value }: { value: string }) => {
  const isWebdav = value.toLowerCase() === 'webdav';
  return (
    <span className={`inline-flex items-center px-2.5 py-1 rounded-lg text-xs font-black ${
      isWebdav
        ? 'bg-violet-50 dark:bg-violet-900/20 text-violet-600'
        : 'bg-sky-50 dark:bg-sky-900/20 text-sky-600'
    }`}>
      {value.toUpperCase()}
    </span>
  );
};

const ProgressLine = ({ value, max, color }: { value: number; max: number; color: string }) => {
  const width = value > 0 ? Math.max(4, Math.round((value / max) * 100)) : 0;
  return (
    <div className="h-2 rounded-full bg-slate-100 dark:bg-slate-800 overflow-hidden">
      <div className={`h-full rounded-full ${color}`} style={{ width: `${width}%` }} />
    </div>
  );
};

const EmptyState = ({ icon, text }: { icon: React.ReactNode; text: string }) => (
  <div className="h-44 flex flex-col items-center justify-center rounded-2xl border border-dashed border-slate-200 dark:border-slate-800 text-slate-400">
    {icon}
    <p className="text-sm mt-3">{text}</p>
  </div>
);

const formatNumber = (value: number) => new Intl.NumberFormat('zh-CN').format(Math.round(value || 0));

const getBookHeatScore = (item: BookActivityStatistics) => (
  item.listeners * 20 + item.progressUpdates * 6 + Math.ceil((item.listenSeconds || 0) / 60)
);

const getRankAccent = (index: number) => {
  if (index === 0) {
    return {
      surface: 'border-violet-100 dark:border-violet-900/30 bg-violet-50/70 dark:bg-violet-950/20',
      badge: 'bg-violet-600 text-white shadow-sm',
      bar: 'bg-gradient-to-r from-violet-500 to-fuchsia-500',
    };
  }
  if (index === 1) {
    return {
      surface: 'border-sky-100 dark:border-sky-900/30 bg-sky-50/70 dark:bg-sky-950/20',
      badge: 'bg-sky-500 text-white shadow-sm',
      bar: 'bg-gradient-to-r from-sky-500 to-cyan-500',
    };
  }
  return {
    surface: 'border-slate-100 dark:border-slate-800 bg-slate-50/70 dark:bg-slate-950/40',
    badge: 'bg-slate-200 dark:bg-slate-800 text-slate-600 dark:text-slate-300',
    bar: 'bg-gradient-to-r from-slate-400 to-slate-500',
  };
};

const formatDuration = (seconds?: number) => {
  const safeSeconds = Math.max(0, Math.round(seconds || 0));
  const hours = Math.floor(safeSeconds / 3600);
  const minutes = Math.round((safeSeconds % 3600) / 60);
  if (hours > 0) return `${hours} 小时 ${minutes} 分钟`;
  return `${minutes} 分钟`;
};

const formatShortDuration = (seconds?: number) => {
  const safeSeconds = Math.max(0, Math.round(seconds || 0));
  const hours = Math.floor(safeSeconds / 3600);
  const minutes = Math.round((safeSeconds % 3600) / 60);
  if (hours > 0) return `${hours}h ${minutes}m`;
  return `${minutes}m`;
};

const formatDateTime = (value?: string) => {
  if (!value) return '暂无记录';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
};

const formatDay = (value: string) => {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value.slice(5);
  return date.toLocaleDateString('zh-CN', { month: '2-digit', day: '2-digit' });
};

export default AdminStatisticsPage;
