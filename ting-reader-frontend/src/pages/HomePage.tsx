import React, { useEffect, useState } from 'react';
import apiClient from '../api/client';
import type { Stats, Progress } from '../types';
import BookCard from '../components/BookCard';
import { 
  Play, 
  Clock, 
  Book as BookIcon, 
  Layers, 
  History,
  TrendingUp,
  Calendar
} from 'lucide-react';
import { Link } from 'react-router-dom';
import { getCoverUrl } from '../utils/image';
import ExpandableTitle from '../components/ExpandableTitle';
import { usePlayerStore } from '../store/playerStore';

const HomePage: React.FC = () => {
  const currentChapter = usePlayerStore((state) => state.currentChapter);
  const [stats, setStats] = useState<Stats | null>(null);
  const [recentPlays, setRecentPlays] = useState<Progress[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const fetchData = async () => {
      try {
        const [statsRes, recentRes] = await Promise.all([
          apiClient.get('/api/stats'),
          apiClient.get('/api/progress/recent')
        ]);
        setStats(statsRes.data);
        setRecentPlays(recentRes.data || []);
      } catch (err) {
        console.error('Failed to fetch home data', err);
      } finally {
        setLoading(false);
      }
    };
    fetchData();

    // Refresh data when window is refocused
    window.addEventListener('focus', fetchData);
    return () => window.removeEventListener('focus', fetchData);
  }, []);

  const formatDuration = (seconds: number) => {
    if (!seconds || seconds <= 0) return '0分钟';
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.round((seconds % 3600) / 60);
    
    if (hours > 0) {
      return `${hours}小时${minutes}分钟`;
    }
    return `${minutes}分钟`;
  };

  const getGreeting = () => {
    const hour = new Date().getHours();
    if (hour >= 5 && hour < 12) return '早上好';
    if (hour >= 12 && hour < 14) return '中午好';
    if (hour >= 14 && hour < 18) return '下午好';
    return '晚上好';
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
      <div className="flex-1 space-y-8">
        {/* Welcome Header */}
        <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
          <div>
            <h1 className="text-2xl md:text-3xl font-bold text-slate-900 dark:text-white">{getGreeting()}!</h1>
            <p className="text-sm md:text-base text-slate-500 dark:text-slate-400 mt-1">欢迎回来，继续您的听书之旅。</p>
          </div>
          <div className="flex items-center gap-2 text-sm text-slate-500 bg-white dark:bg-slate-900 px-4 py-2 rounded-xl shadow-sm border border-slate-100 dark:border-slate-800">
            <Calendar size={16} />
            <span>{new Date().toLocaleDateString('zh-CN', { weekday: 'long', year: 'numeric', month: 'long', day: 'numeric' })}</span>
          </div>
        </div>

      {/* Stats Overview */}
      <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
        <StatCard 
          icon={<BookIcon className="text-blue-500" size={20} />} 
          label="书籍总数" 
          value={stats?.total_books || 0} 
          unit="本"
        />
        <StatCard 
          icon={<Layers className="text-purple-500" size={20} />} 
          label="章节总数" 
          value={stats?.total_chapters || 0} 
          unit="章"
        />
        <StatCard 
          icon={<Clock className="text-orange-500" size={20} />} 
          label="总时长" 
          value={formatDuration(stats?.total_duration || 0)} 
        />
        <StatCard 
          icon={<TrendingUp className="text-green-500" size={20} />} 
          label="最近更新" 
          value={stats?.last_scan_time ? new Date(stats.last_scan_time).toLocaleDateString('zh-CN', { month: 'numeric', day: 'numeric' }) : '从未'} 
        />
      </div>

      {/* Recent Plays */}
      <section>
        <div className="flex items-center justify-between mb-6">
          <div className="flex items-center gap-2">
            <History size={24} className="text-primary-600" />
            <h2 className="text-2xl font-bold dark:text-white">最近播放</h2>
          </div>
          <Link to="/bookshelf" className="text-primary-600 hover:text-primary-700 font-medium text-sm">查看全部</Link>
        </div>
        
        {recentPlays.length > 0 ? (
          <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
            {recentPlays.map((progress) => (
              <RecentPlayCard key={progress.book_id} progress={progress} />
            ))}
          </div>
        ) : (
          <div className="bg-white dark:bg-slate-900 rounded-2xl p-12 text-center border border-dashed border-slate-200 dark:border-slate-800">
            <div className="inline-flex items-center justify-center w-16 h-16 rounded-full bg-slate-100 dark:bg-slate-800 text-slate-400 mb-4">
              <Play size={32} />
            </div>
            <p className="text-slate-500">暂无播放记录，快去书架看看吧！</p>
            <Link to="/bookshelf" className="mt-4 inline-block text-primary-600 font-medium">去书架</Link>
          </div>
        )}
      </section>
      </div>

      {/* Dynamic Safe Bottom Spacer */}
      <div 
        className="shrink-0 transition-all duration-300" 
        style={{ height: currentChapter ? 'var(--safe-bottom-with-player)' : 'var(--safe-bottom-base)' }} 
      />
    </div>
  );
};

const StatCard = ({ icon, label, value, unit = '' }: { icon: React.ReactNode, label: string, value: string | number, unit?: string }) => (
  <div className="bg-white dark:bg-slate-900 p-4 md:p-6 rounded-2xl shadow-sm border border-slate-100 dark:border-slate-800 flex items-center gap-3 md:gap-4">
    <div className="w-10 h-10 md:w-12 md:h-12 rounded-xl bg-slate-50 dark:bg-slate-800 flex items-center justify-center shrink-0">
      {icon}
    </div>
    <div className="min-w-0">
      <p className="text-[10px] md:text-sm text-slate-500 dark:text-slate-400 font-bold uppercase tracking-tight truncate">{label}</p>
      <p className="text-lg md:text-xl font-bold dark:text-white truncate">
        {value}
        {unit && <span className="text-[10px] md:text-xs font-bold ml-0.5 opacity-50">{unit}</span>}
      </p>
    </div>
  </div>
);

const RecentPlayCard = ({ progress }: { progress: Progress }) => (
  <Link 
    to={`/book/${progress.book_id}`}
    className="bg-white dark:bg-slate-900 rounded-2xl p-3 md:p-4 shadow-sm border border-slate-100 dark:border-slate-800 flex gap-3 md:gap-4 hover:shadow-md transition-shadow group"
  >
    <div className="w-20 h-20 md:w-24 md:h-24 rounded-xl overflow-hidden shrink-0 shadow-sm">
      <img 
        src={getCoverUrl(progress.cover_url, progress.library_id, progress.book_id)} 
        alt={progress.book_title}
        crossOrigin="anonymous"
        className="w-full h-full object-cover group-hover:scale-105 transition-transform duration-300"
        onError={(e) => {
          (e.target as HTMLImageElement).src = 'https://placehold.co/300x400?text=No+Cover';
        }}
      />
    </div>
    <div className="flex-1 min-w-0 flex flex-col justify-between py-0.5">
      <div className="min-w-0">
        <ExpandableTitle 
          title={progress.book_title || ''} 
          className="font-bold text-sm md:text-base dark:text-white group-hover:text-primary-600 transition-colors" 
          maxLines={1} 
        />
        <p className="text-xs text-slate-500 truncate mt-0.5">正在播放: {progress.chapter_title}</p>
      </div>
      <div className="flex items-center justify-between mt-2">
        <div className="flex-1 h-1 bg-slate-100 dark:bg-slate-800 rounded-full mr-3 overflow-hidden">
          <div 
            className="h-full bg-primary-500 rounded-full" 
            style={{ width: `${Math.min(100, Math.round((progress.position / (progress.chapter_duration || 1)) * 100))}%` }}
          ></div>
        </div>
        <span className="text-[10px] text-slate-400 shrink-0">
          {Math.min(100, Math.round((progress.position / (progress.chapter_duration || 1)) * 100))}%
        </span>
      </div>
    </div>
  </Link>
);

export default HomePage;
