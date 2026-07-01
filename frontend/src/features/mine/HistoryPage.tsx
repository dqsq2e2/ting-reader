import React, { useEffect, useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import apiClient from '../../core/api/client';
import type { Progress } from '../../core/types';
import { getCoverUrl } from '../../core/utils/image';
import { usePlayerStore } from '../../core/stores/playerStore';
import BackButton from '../../shared/widgets/BackButton';
import {
  CheckSquare,
  ChevronDown,
  ChevronRight,
  Clock,
  History,
  Play,
  Square,
  Trash2,
  X,
} from 'lucide-react';
import {
  getCoverAspectClass,
  useBookshelfCoverShape,
  type CoverShape,
} from '../../core/hooks/useBookshelfCoverShape';
import LoadingSpinner from '../../shared/ui/LoadingSpinner';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';

interface HistoryBookGroup {
  book_id: string;
  book_title: string;
  cover_url?: string;
  library_id?: string;
  latest: Progress;
  chapters: Progress[];
}

const progressKey = (progress: Progress) =>
  progress.id || `${progress.book_id}:${progress.chapter_id}`;

const HistoryPage: React.FC = () => {
  const { t } = useTranslation();
  const currentChapter = usePlayerStore((state) => state.currentChapter);
  const coverShape = useBookshelfCoverShape();
  const [recentPlays, setRecentPlays] = useState<Progress[]>([]);
  const [expandedBookIds, setExpandedBookIds] = useState<Set<string>>(new Set());
  const [selectionMode, setSelectionMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(true);
  const [deleting, setDeleting] = useState(false);

  const fetchHistory = async () => {
    try {
      const res = await apiClient.get('/api/progress/recent');
      setRecentPlays(res.data || []);
    } catch (err) {
      console.error('获取我的历史失败', err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchHistory();
    window.addEventListener('focus', fetchHistory);
    return () => window.removeEventListener('focus', fetchHistory);
  }, []);

  const groups = useMemo(() => {
    const map = new Map<string, HistoryBookGroup>();
    for (const item of recentPlays) {
      if (!item.chapter_id) continue;
      const existing = map.get(item.book_id);
      if (existing) {
        existing.chapters.push(item);
        if (new Date(item.updated_at).getTime() > new Date(existing.latest.updated_at).getTime()) {
          existing.latest = item;
        }
        continue;
      }
      map.set(item.book_id, {
        book_id: item.book_id,
        book_title: item.book_title || t('historyPage.unknownBook'),
        cover_url: item.cover_url,
        library_id: item.library_id,
        latest: item,
        chapters: [item],
      });
    }

    return Array.from(map.values())
      .map((group) => ({
        ...group,
        chapters: [...group.chapters].sort(
          (a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime()
        ),
      }))
      .sort(
        (a, b) => new Date(b.latest.updated_at).getTime() - new Date(a.latest.updated_at).getTime()
      );
  }, [recentPlays, t]);

  const allIds = useMemo(() => recentPlays.map(progressKey), [recentPlays]);
  const allSelected = allIds.length > 0 && allIds.every((id) => selectedIds.has(id));

  const enterSelectionMode = () => {
    setSelectionMode(true);
    setSelectedIds(new Set());
  };

  const exitSelectionMode = () => {
    setSelectionMode(false);
    setSelectedIds(new Set());
  };

  const toggleExpanded = (bookId: string) => {
    setExpandedBookIds((current) => {
      const next = new Set(current);
      if (next.has(bookId)) {
        next.delete(bookId);
      } else {
        next.add(bookId);
      }
      return next;
    });
  };

  const toggleProgress = (progress: Progress) => {
    const id = progressKey(progress);
    setSelectedIds((current) => {
      const next = new Set(current);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const toggleBook = (group: HistoryBookGroup) => {
    const ids = group.chapters.map(progressKey);
    const selected = ids.every((id) => selectedIds.has(id));
    setSelectedIds((current) => {
      const next = new Set(current);
      for (const id of ids) {
        if (selected) {
          next.delete(id);
        } else {
          next.add(id);
        }
      }
      return next;
    });
  };

  const toggleAll = () => {
    setSelectedIds(allSelected ? new Set() : new Set(allIds));
  };

  const deleteSelected = async () => {
    if (selectedIds.size === 0 || deleting) return;
    setDeleting(true);
    try {
      const selected = recentPlays.filter((item) => selectedIds.has(progressKey(item)));
      await apiClient.post('/api/progress/recent/delete', {
        progress_ids: selected.map((item) => item.id).filter(Boolean),
        chapter_ids: selected.filter((item) => !item.id).map((item) => item.chapter_id),
      });
      setRecentPlays((current) => current.filter((item) => !selectedIds.has(progressKey(item))));
      exitSelectionMode();
    } catch (err) {
      console.error('删除我的历史失败', err);
      alert(t('historyPage.deleteFailed'));
    } finally {
      setDeleting(false);
    }
  };

  if (loading) {
    return <LoadingSpinner />;
  }

  return (
    <div className="flex-1 min-h-full flex flex-col p-4 sm:p-6 md:p-8 animate-in fade-in duration-500">
      <div className="flex-1 space-y-6">
        <BackButton fallback="/mine" />

        <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
          <div>
            <h1 className="text-2xl md:text-3xl font-bold text-slate-900 dark:text-white flex items-center gap-3">
              <History className="text-primary-600" />
              {t('historyPage.title')}
            </h1>
            <p className="text-sm md:text-base text-slate-500 dark:text-slate-400 mt-1">
              {t('historyPage.subtitle', { books: groups.length, chapters: recentPlays.length })}
            </p>
          </div>

          {selectionMode ? (
            <div className="flex flex-wrap items-center gap-2">
              <button
                onClick={toggleAll}
                disabled={recentPlays.length === 0 || deleting}
                className="inline-flex items-center justify-center gap-2 px-4 py-2.5 rounded-xl bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-200 text-sm font-bold hover:bg-slate-200 dark:hover:bg-slate-700 transition-colors disabled:opacity-50"
              >
                {allSelected ? <CheckSquare size={18} /> : <Square size={18} />}
                {t('historyPage.selectAll')}
              </button>
              <button
                onClick={deleteSelected}
                disabled={selectedIds.size === 0 || deleting}
                className="inline-flex items-center justify-center gap-2 px-4 py-2.5 rounded-xl bg-red-50 dark:bg-red-900/20 text-red-600 text-sm font-bold hover:bg-red-100 dark:hover:bg-red-900/30 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                <Trash2 size={18} />
                {deleting ? (
                  t('historyPage.deleting')
                ) : (
                  <>
                    <span className="sm:hidden">{t('historyPage.delete')}</span>
                    <span className="hidden sm:inline">
                      {t('historyPage.deleteSelected', { count: selectedIds.size })}
                    </span>
                  </>
                )}
              </button>
              <button
                onClick={exitSelectionMode}
                disabled={deleting}
                className="inline-flex items-center justify-center gap-2 px-4 py-2.5 rounded-xl bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-700 text-slate-600 dark:text-slate-300 text-sm font-bold hover:bg-slate-50 dark:hover:bg-slate-800 transition-colors"
              >
                <X size={18} />
                {t('historyPage.cancel')}
              </button>
            </div>
          ) : (
            <button
              onClick={enterSelectionMode}
              disabled={recentPlays.length === 0}
              className="inline-flex items-center justify-center gap-2 px-4 py-2.5 rounded-xl bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-700 text-slate-700 dark:text-slate-200 text-sm font-bold hover:bg-slate-50 dark:hover:bg-slate-800 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              <CheckSquare size={18} />
              {t('historyPage.select')}
            </button>
          )}
        </div>

        {groups.length > 0 ? (
          <div className="space-y-3">
            {groups.map((group) => (
              <HistoryBookSection
                key={group.book_id}
                group={group}
                coverShape={coverShape}
                expanded={expandedBookIds.has(group.book_id)}
                selectionMode={selectionMode}
                selectedIds={selectedIds}
                onToggleExpanded={() => toggleExpanded(group.book_id)}
                onToggleBook={() => toggleBook(group)}
                onToggleProgress={toggleProgress}
              />
            ))}
          </div>
        ) : (
          <div className="bg-white dark:bg-slate-900 rounded-3xl p-10 text-center border border-dashed border-slate-200 dark:border-slate-800">
            <div className="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-slate-100 dark:bg-slate-800 text-slate-400 mb-4">
              <Play size={30} />
            </div>
            <p className="text-slate-500">{t('historyPage.empty')}</p>
            <Link to="/bookshelf" className="mt-4 inline-flex px-5 py-2.5 rounded-xl bg-primary-600 text-white text-sm font-bold">
              {t('historyPage.goBookshelf')}
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

const HistoryBookSection = ({
  group,
  coverShape,
  expanded,
  selectionMode,
  selectedIds,
  onToggleExpanded,
  onToggleBook,
  onToggleProgress,
}: {
  group: HistoryBookGroup;
  coverShape: CoverShape;
  expanded: boolean;
  selectionMode: boolean;
  selectedIds: Set<string>;
  onToggleExpanded: () => void;
  onToggleBook: () => void;
  onToggleProgress: (progress: Progress) => void;
}) => {
  const { t, i18n } = useTranslation();
  const latest = group.latest;
  const latestPercent = progressPercent(latest);
  const chapterIds = group.chapters.map(progressKey);
  const selectedCount = chapterIds.filter((id) => selectedIds.has(id)).length;
  const bookSelected = selectedCount === chapterIds.length;

  return (
    <div className="bg-white dark:bg-slate-900 rounded-3xl border border-slate-100 dark:border-slate-800 shadow-sm overflow-hidden">
      <div className="flex items-center gap-3 md:gap-4 p-3 md:p-4">
        {selectionMode && (
          <button
            onClick={onToggleBook}
            className="shrink-0 text-primary-600 hover:text-primary-700"
            aria-label={t('historyPage.chooseBook', { title: group.book_title })}
          >
            {bookSelected ? <CheckSquare size={22} /> : <Square size={22} />}
          </button>
        )}
        <button
          onClick={onToggleExpanded}
          className="flex-1 min-w-0 flex items-center gap-3 md:gap-4 text-left group"
        >
          <div className={`w-16 md:w-20 ${getCoverAspectClass(coverShape)} rounded-xl overflow-hidden shrink-0 shadow-sm`}>
            <img
              src={getCoverUrl(group.cover_url, group.library_id, group.book_id)}
              alt={group.book_title}
              referrerPolicy="no-referrer"
              className="w-full h-full object-cover group-hover:scale-105 transition-transform duration-300"
              onError={(event) => {
                (event.target as HTMLImageElement).src = 'https://placehold.co/300x400?text=No+Cover';
              }}
            />
          </div>
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2">
              <p className="font-bold text-sm md:text-base dark:text-white group-hover:text-primary-600 transition-colors truncate">
                {group.book_title}
              </p>
              <span className="shrink-0 text-[10px] px-2 py-0.5 rounded-full bg-slate-100 dark:bg-slate-800 text-slate-500 dark:text-slate-400">
                {t('historyPage.chapterUnit', { count: group.chapters.length })}
              </span>
            </div>
            <p className="text-xs text-slate-500 truncate mt-0.5">{latest.chapter_title || t('historyPage.unknownChapter')}</p>
            <p className="text-xs text-slate-400 truncate mt-1 flex items-center gap-1">
              <Clock size={13} className="shrink-0" />
              {t('historyPage.lastListened', { time: formatLastListenedTime(latest.updated_at, t, i18n.language) })}
            </p>
            <div className="flex items-center justify-between mt-3">
              <div className="flex-1 h-1 bg-slate-100 dark:bg-slate-800 rounded-full mr-3 overflow-hidden">
                <div className="h-full bg-primary-500 rounded-full" style={{ width: `${latestPercent}%` }} />
              </div>
              <span className="text-[10px] text-slate-400 shrink-0">{latestPercent}%</span>
            </div>
          </div>
          {expanded ? (
            <ChevronDown size={18} className="text-slate-300 shrink-0" />
          ) : (
            <ChevronRight size={18} className="text-slate-300 shrink-0" />
          )}
        </button>
      </div>

      {expanded && (
        <div className="border-t border-slate-100 dark:border-slate-800 bg-slate-50/50 dark:bg-slate-950/30">
          {group.chapters.map((chapter) => (
            <HistoryChapterRow
              key={progressKey(chapter)}
              progress={chapter}
              selectionMode={selectionMode}
              selected={selectedIds.has(progressKey(chapter))}
              onToggle={() => onToggleProgress(chapter)}
            />
          ))}
        </div>
      )}
    </div>
  );
};

const HistoryChapterRow = ({
  progress,
  selectionMode,
  selected,
  onToggle,
}: {
  progress: Progress;
  selectionMode: boolean;
  selected: boolean;
  onToggle: () => void;
}) => {
  const { t, i18n } = useTranslation();
  const percent = progressPercent(progress);
  const body = (
    <>
      {selectionMode && (
        <button
          onClick={(event) => {
            event.preventDefault();
            onToggle();
          }}
          className="shrink-0 text-primary-600 hover:text-primary-700"
          aria-label={t('historyPage.chooseChapter', {
            title: progress.chapter_title || t('historyPage.chapterFallback'),
          })}
        >
          {selected ? <CheckSquare size={20} /> : <Square size={20} />}
        </button>
      )}
      <div className="flex-1 min-w-0">
        <p className="text-sm font-bold text-slate-800 dark:text-slate-100 truncate">
          {progress.chapter_title || t('historyPage.unknownChapter')}
        </p>
        <p className="text-xs text-slate-400 mt-1 flex items-center gap-1">
          <Clock size={13} className="shrink-0" />
          {formatLastListenedTime(progress.updated_at, t, i18n.language)}
        </p>
        <div className="flex items-center justify-between mt-2">
          <div className="flex-1 h-1 bg-slate-200 dark:bg-slate-800 rounded-full mr-3 overflow-hidden">
            <div
              className={`h-full rounded-full ${percent >= 95 ? 'bg-emerald-500' : 'bg-primary-500'}`}
              style={{ width: `${percent}%` }}
            />
          </div>
          <span className="text-[10px] text-slate-400 shrink-0">{percent >= 95 ? t('historyPage.completed') : `${percent}%`}</span>
        </div>
      </div>
    </>
  );

  if (selectionMode) {
    return (
      <div className="flex items-center gap-3 px-4 md:px-5 py-3 border-t border-slate-100 dark:border-slate-800 first:border-t-0">
        {body}
      </div>
    );
  }

  return (
    <Link
      to={`/book/${progress.book_id}?chapter_id=${encodeURIComponent(progress.chapter_id)}`}
      className="flex items-center gap-3 px-4 md:px-5 py-3 border-t border-slate-100 dark:border-slate-800 first:border-t-0 hover:bg-white dark:hover:bg-slate-900 transition-colors"
    >
      {body}
    </Link>
  );
};

const progressPercent = (progress: Progress) => {
  const duration = progress.chapter_duration || progress.duration || 1;
  return Math.min(100, Math.round((progress.position / duration) * 100));
};

const formatLastListenedTime = (value: string | undefined, t: TFunction, language: string) => {
  if (!value) return t('historyPage.unknownTime');
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return t('historyPage.unknownTime');

  const now = new Date();
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  const startOfDate = new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
  const dayDiff = Math.round((startOfToday - startOfDate) / 86400000);
  const locale = language?.startsWith('en') ? 'en-US' : 'zh-CN';
  const time = date.toLocaleTimeString(locale, { hour: '2-digit', minute: '2-digit' });

  if (dayDiff === 0) return t('historyPage.today', { time });
  if (dayDiff === 1) return t('historyPage.yesterday', { time });
  if (dayDiff > 1 && dayDiff < 7) return t('historyPage.daysAgo', { count: dayDiff, time });

  return date.toLocaleString(locale, {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
};

export default HistoryPage;
