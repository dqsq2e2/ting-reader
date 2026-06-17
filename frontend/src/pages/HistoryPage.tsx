import React, { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import apiClient from '../api/client';
import type { Progress } from '../types';
import { getCoverUrl } from '../utils/image';
import { usePlayerStore } from '../store/playerStore';
import BackButton from '../components/BackButton';
import { ChevronRight, Clock, History, Play, Trash2 } from 'lucide-react';
import { getCoverAspectClass, useBookshelfCoverShape, type CoverShape } from '../hooks/useBookshelfCoverShape';

const HistoryPage: React.FC = () => {
  const currentChapter = usePlayerStore((state) => state.currentChapter);
  const coverShape = useBookshelfCoverShape();
  const [recentPlays, setRecentPlays] = useState<Progress[]>([]);
  const [loading, setLoading] = useState(true);
  const [clearing, setClearing] = useState(false);

  useEffect(() => {
    const fetchHistory = async () => {
      try {
        const res = await apiClient.get('/api/progress/recent');
        setRecentPlays(res.data || []);
      } catch (err) {
        console.error('获取收听历史失败', err);
      } finally {
        setLoading(false);
      }
    };

    fetchHistory();
    window.addEventListener('focus', fetchHistory);
    return () => window.removeEventListener('focus', fetchHistory);
  }, []);

  const handleClearHistory = async () => {
    if (recentPlays.length === 0 || clearing) return;
    if (!window.confirm('确定清空全部收听历史吗？')) return;

    setClearing(true);
    try {
      await apiClient.delete('/api/progress/recent');
      setRecentPlays([]);
    } catch (err) {
      console.error('清空收听历史失败', err);
      alert('清空历史失败，请稍后重试');
    } finally {
      setClearing(false);
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
      <div className="flex-1 space-y-6">
        <BackButton fallback="/mine" />

        <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
          <div>
            <h1 className="text-2xl md:text-3xl font-bold text-slate-900 dark:text-white flex items-center gap-3">
              <History className="text-primary-600" />
              收听历史
            </h1>
            <p className="text-sm md:text-base text-slate-500 dark:text-slate-400 mt-1">
              继续上次停下的位置，共 {recentPlays.length} 条记录。
            </p>
          </div>

          <button
            onClick={handleClearHistory}
            disabled={recentPlays.length === 0 || clearing}
            className="inline-flex items-center justify-center gap-2 px-4 py-2.5 rounded-xl bg-red-50 dark:bg-red-900/20 text-red-600 text-sm font-bold hover:bg-red-100 dark:hover:bg-red-900/30 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <Trash2 size={18} />
            {clearing ? '清空中...' : '清空历史'}
          </button>
        </div>

        {recentPlays.length > 0 ? (
          <div className="bg-white dark:bg-slate-900 rounded-3xl border border-slate-100 dark:border-slate-800 shadow-sm overflow-hidden">
            {recentPlays.map(progress => (
              <RecentPlayRow key={`${progress.bookId}-${progress.chapterId}`} progress={progress} coverShape={coverShape} />
            ))}
          </div>
        ) : (
          <div className="bg-white dark:bg-slate-900 rounded-3xl p-10 text-center border border-dashed border-slate-200 dark:border-slate-800">
            <div className="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-slate-100 dark:bg-slate-800 text-slate-400 mb-4">
              <Play size={30} />
            </div>
            <p className="text-slate-500">暂无收听历史，去书架开始第一本吧。</p>
            <Link to="/bookshelf" className="mt-4 inline-flex px-5 py-2.5 rounded-xl bg-primary-600 text-white text-sm font-bold">
              去书架
            </Link>
          </div>
        )}
      </div>

      <div
        className="shrink-0 transition-all duration-300"
        style={{ height: currentChapter ? 'var(--safe-bottom-with-player)' : 'var(--safe-bottom-base)' }}
      />
    </div>
  );
};

const RecentPlayRow = ({ progress, coverShape }: { progress: Progress; coverShape: CoverShape }) => {
  const percent = Math.min(100, Math.round((progress.position / (progress.chapterDuration || 1)) * 100));
  const lastListenedAt = formatLastListenedTime(progress.updatedAt);

  return (
    <Link
      to={`/book/${progress.bookId}`}
      className="flex items-center gap-3 md:gap-4 p-3 md:p-4 border-b border-slate-100 dark:border-slate-800 last:border-b-0 hover:bg-slate-50 dark:hover:bg-slate-800 transition-colors group"
    >
      <div className={`w-16 md:w-20 ${getCoverAspectClass(coverShape)} rounded-xl overflow-hidden shrink-0 shadow-sm`}>
        <img
          src={getCoverUrl(progress.coverUrl, progress.libraryId, progress.bookId)}
          alt={progress.bookTitle}
          referrerPolicy="no-referrer"
          className="w-full h-full object-cover group-hover:scale-105 transition-transform duration-300"
          onError={(event) => {
            (event.target as HTMLImageElement).src = 'https://placehold.co/300x400?text=No+Cover';
          }}
        />
      </div>
      <div className="flex-1 min-w-0 flex flex-col justify-between py-0.5">
        <div className="min-w-0">
          <p className="font-bold text-sm md:text-base dark:text-white group-hover:text-primary-600 transition-colors truncate">
            {progress.bookTitle || '未知书籍'}
          </p>
          <p className="text-xs text-slate-500 truncate mt-0.5">{progress.chapterTitle}</p>
          <p className="text-xs text-slate-400 truncate mt-1 flex items-center gap-1">
            <Clock size={13} className="shrink-0" />
            最后收听：{lastListenedAt}
          </p>
        </div>
        <div className="flex items-center justify-between mt-3">
          <div className="flex-1 h-1 bg-slate-100 dark:bg-slate-800 rounded-full mr-3 overflow-hidden">
            <div className="h-full bg-primary-500 rounded-full" style={{ width: `${percent}%` }} />
          </div>
          <span className="text-[10px] text-slate-400 shrink-0">{percent}%</span>
        </div>
      </div>
      <ChevronRight size={18} className="text-slate-300 shrink-0" />
    </Link>
  );
};

const formatLastListenedTime = (value?: string) => {
  if (!value) return '未知时间';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return '未知时间';

  const now = new Date();
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  const startOfDate = new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
  const dayDiff = Math.round((startOfToday - startOfDate) / 86400000);
  const time = date.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' });

  if (dayDiff === 0) return `今天 ${time}`;
  if (dayDiff === 1) return `昨天 ${time}`;
  if (dayDiff > 1 && dayDiff < 7) return `${dayDiff} 天前 ${time}`;

  return date.toLocaleString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
};

export default HistoryPage;
