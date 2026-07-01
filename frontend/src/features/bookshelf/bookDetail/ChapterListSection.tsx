import React from 'react';
import {
  Play,
  ChevronLeft,
  ChevronDown,
  ChevronUp,
  Clock,
  ListMusic,
  Settings,
  Loader2,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { Chapter } from '../../../core/types';
import { setAlpha, toSolidColor, isLight } from '../../../core/utils/color';

interface ChapterGroup {
  start: number;
  end: number;
  offset: number;
  index: number;
}

interface Props {
  isAdmin: boolean;
  chapters: Chapter[];
  visibleChapters: Chapter[];
  chapterTotals: { total: number; main: number; extra: number };
  groups: ChapterGroup[];
  currentGroupIndex: number;
  activeTab: 'main' | 'extra';
  chapterAscending: boolean;
  chapterGroupsDescending: boolean;
  chapterPageLoading: boolean;
  scrollRef: React.RefObject<HTMLDivElement | null>;
  currentChapterId?: string | null;
  highlightedChapterId: string | null;
  isPlaying: boolean;
  effectiveThemeColor?: string;
  onScrollGroups: (direction: 'left' | 'right') => void;
  onSetActiveTab: (tab: 'main' | 'extra') => void;
  onSetCurrentGroupIndex: (index: number) => void;
  onToggleAscending: () => void;
  onPlayChapter: (chapter: Chapter) => void;
  onOpenChapterManager: () => void;
  formatDuration: (seconds: number) => string;
  getChapterProgressText: (chapter: Chapter) => string | null;
}

const ChapterListSection: React.FC<Props> = ({
  isAdmin,
  chapters,
  visibleChapters,
  chapterTotals,
  groups,
  currentGroupIndex,
  activeTab,
  chapterAscending,
  chapterGroupsDescending,
  chapterPageLoading,
  scrollRef,
  currentChapterId,
  highlightedChapterId,
  isPlaying,
  effectiveThemeColor,
  onScrollGroups,
  onSetActiveTab,
  onSetCurrentGroupIndex,
  onToggleAscending,
  onPlayChapter,
  onOpenChapterManager,
  formatDuration,
  getChapterProgressText,
}) => {
  const { t } = useTranslation();
  const displayGroups = chapterGroupsDescending ? [...groups].reverse() : groups;
  const sortButton = (className = '') => (
    <button
      onClick={onToggleAscending}
      className={`${className} items-center gap-2 px-3 py-2 rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 text-sm font-bold text-slate-600 dark:text-slate-300 hover:border-primary-500 hover:text-primary-600 transition-all shadow-sm`}
    >
      {chapterAscending ? <ChevronDown size={16} /> : <ChevronUp size={16} />}
      {t(chapterAscending ? 'bookshelf.ascending' : 'bookshelf.descending')}
    </button>
  );

  return (
  <div className="bg-white dark:bg-slate-900 rounded-3xl p-4 md:p-6 shadow-sm border border-slate-100 dark:border-slate-800">
    <div className="flex flex-col sm:flex-row sm:items-center justify-between mb-6 gap-4">
      <div className="flex items-center gap-2 min-w-0">
        <h2 className="text-xl md:text-2xl font-bold dark:text-white flex items-center gap-2 min-w-0">
          <ListMusic size={24} className="text-primary-600 shrink-0" />
          <span className="whitespace-nowrap">{t('bookshelf.chapterList')}</span>
          {isAdmin && (
            <button
              onClick={onOpenChapterManager}
              className="ml-2 p-1.5 text-slate-400 hover:text-primary-600 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-lg transition-colors shrink-0"
              title={t('bookshelf.manageChapters')}
            >
              <Settings size={20} />
            </button>
          )}
        </h2>
        {sortButton('inline-flex sm:hidden shrink-0')}
      </div>

      <div className="flex flex-wrap items-center gap-2 self-start sm:justify-end">
        {chapterTotals.extra > 0 && (
          <div className="flex bg-slate-100 dark:bg-slate-800 p-1 rounded-xl">
            <button
              onClick={() => onSetActiveTab('main')}
              className={`px-4 py-1.5 rounded-lg text-sm font-bold transition-all ${
                activeTab === 'main'
                  ? 'bg-white dark:bg-slate-700 text-primary-600 shadow-sm'
                  : 'text-slate-500 hover:text-slate-700 dark:hover:text-slate-300'
              }`}
            >
              {t('bookshelf.mainChaptersWithCount', { count: chapterTotals.main })}
            </button>
            <button
              onClick={() => onSetActiveTab('extra')}
              className={`px-4 py-1.5 rounded-lg text-sm font-bold transition-all ${
                activeTab === 'extra'
                  ? 'bg-white dark:bg-slate-700 text-primary-600 shadow-sm'
                  : 'text-slate-500 hover:text-slate-700 dark:hover:text-slate-300'
              }`}
            >
              {t('bookshelf.extraChaptersWithCount', { count: chapterTotals.extra })}
            </button>
          </div>
        )}
        {sortButton('hidden sm:inline-flex')}
      </div>
    </div>

    {groups.length > 1 && (
      <div className="relative group/nav mb-6 flex items-center">
        <button
          onClick={() => onScrollGroups('left')}
          className="absolute -left-4 sm:-left-7 top-1/2 -translate-y-1/2 z-10 p-1 bg-white/90 dark:bg-slate-800/90 backdrop-blur shadow-md rounded-full opacity-0 group-hover/nav:opacity-100 transition-opacity hidden sm:block border border-slate-100 dark:border-slate-700"
        >
          <ChevronLeft size={20} className="text-slate-600 dark:text-slate-400" />
        </button>
        <div
          ref={scrollRef}
          className="flex gap-2 overflow-x-auto no-scrollbar scroll-smooth snap-x pb-2 px-1 mx-1 w-full"
        >
          {displayGroups.map((group) => (
            <button
              key={group.index}
              id={`group-tab-${group.index}`}
              onClick={() => onSetCurrentGroupIndex(group.index)}
              className={`px-4 py-2 rounded-xl text-sm font-bold transition-all border shrink-0 snap-start ${
                currentGroupIndex === group.index
                  ? `text-white shadow-lg shadow-black/10 ${!effectiveThemeColor ? 'bg-primary-600 border-primary-600' : ''}`
                  : 'bg-white dark:bg-slate-800 text-slate-600 dark:text-slate-400 border-slate-200 dark:border-slate-700 hover:bg-slate-50'
              }`}
              style={currentGroupIndex === group.index && effectiveThemeColor ? {
                backgroundColor: toSolidColor(effectiveThemeColor),
                borderColor: toSolidColor(effectiveThemeColor),
                color: isLight(effectiveThemeColor) ? '#475569' : '#ffffff'
              } : {}}
            >
              {t('bookshelf.chapterRange', { start: group.start, end: group.end })}
            </button>
          ))}
        </div>
        <button
          onClick={() => onScrollGroups('right')}
          className="absolute -right-4 sm:-right-7 top-1/2 -translate-y-1/2 z-10 p-1 bg-white/90 dark:bg-slate-800/90 backdrop-blur shadow-md rounded-full opacity-0 group-hover/nav:opacity-100 transition-opacity hidden sm:block border border-slate-100 dark:border-slate-700"
        >
          <ChevronLeft size={20} className="rotate-180 text-slate-600 dark:text-slate-400" />
        </button>
      </div>
    )}

    <div className="space-y-3">
      {chapterPageLoading && chapters.length === 0 ? (
        <div className="flex items-center justify-center py-10 text-slate-400">
          <Loader2 className="w-5 h-5 animate-spin mr-2" />
          {t('bookshelf.loadingChapters')}
        </div>
      ) : chapters.length === 0 ? (
        <div className="py-10 text-center text-slate-400">{t('bookshelf.noChapters')}</div>
      ) : visibleChapters.map((chapter, index) => {
        const groupOffset = groups[currentGroupIndex]?.offset || 0;
        const actualIndex = groupOffset + (chapterAscending ? index : visibleChapters.length - index - 1);
        const isCurrent = currentChapterId === chapter.id;
        const isActive = isCurrent || highlightedChapterId === chapter.id;
        const progressText = getChapterProgressText(chapter);

        return (
          <div
            key={chapter.id}
            id={`chapter-${chapter.id}`}
            onClick={() => onPlayChapter(chapter)}
            className={`group flex items-start sm:items-center justify-between gap-1 min-[361px]:gap-1.5 min-[431px]:gap-2 p-1.5 min-[361px]:p-2 min-[431px]:p-2.5 sm:p-4 rounded-md min-[361px]:rounded-lg min-[431px]:rounded-xl sm:rounded-2xl cursor-pointer transition-all border ${
              isActive
                ? 'bg-opacity-10 border-opacity-20'
                : 'bg-white dark:bg-slate-900 border-slate-100 dark:border-slate-800 hover:border-primary-200 dark:hover:border-primary-800'
            }`}
            style={isActive && effectiveThemeColor ? {
              backgroundColor: setAlpha(effectiveThemeColor, 0.1),
              borderColor: setAlpha(effectiveThemeColor, 0.3),
            } : {}}
          >
            <div className="flex items-start sm:items-center gap-1.5 min-[361px]:gap-2 min-[431px]:gap-2.5 sm:gap-4 min-w-0 flex-1 cursor-pointer">
              <div
                className={`w-6 h-6 min-[361px]:w-7 min-[361px]:h-7 min-[431px]:w-8 min-[431px]:h-8 sm:w-12 sm:h-12 rounded min-[361px]:rounded-md min-[431px]:rounded-lg sm:rounded-xl flex items-center justify-center font-medium text-[10px] min-[361px]:text-[11px] min-[431px]:text-xs sm:text-base shrink-0 ${
                  isActive ? `text-white ${!effectiveThemeColor ? 'bg-primary-600' : ''}` : 'bg-slate-100 dark:bg-slate-800 text-slate-500 dark:text-slate-400'
                }`}
                style={isActive && effectiveThemeColor ? {
                  backgroundColor: toSolidColor(effectiveThemeColor),
                  color: isLight(effectiveThemeColor) ? '#475569' : '#ffffff'
                } : {}}
              >
                {chapter.chapter_index || (actualIndex + 1)}
              </div>
              <div className="min-w-0 flex-1">
                <p
                  className={`text-xs min-[361px]:text-[13px] min-[431px]:text-sm sm:text-base font-medium leading-normal line-clamp-2 break-words ${isActive ? '' : 'text-slate-900 dark:text-white'}`}
                  style={isActive && effectiveThemeColor ? { color: toSolidColor(effectiveThemeColor) } : {}}
                >
                  {chapter.title}
                </p>
                <div className="flex flex-wrap items-center gap-x-2 gap-y-1 mt-1">
                  <div className="flex items-center gap-1 text-[9px] min-[361px]:text-[10px] min-[431px]:text-[11px] sm:text-xs text-slate-400 font-normal">
                    <Clock size={10} className="w-2 h-2 min-[361px]:w-2.5 min-[361px]:h-2.5 sm:w-3 sm:h-3" />
                    {formatDuration(chapter.duration)}
                  </div>
                  {progressText && (
                    <div
                      className={`text-[8px] min-[361px]:text-[9px] min-[431px]:text-[10px] font-medium px-0.5 min-[361px]:px-1 min-[431px]:px-1.5 py-0.5 rounded whitespace-nowrap ${
                        progressText === t('bookshelf.progressComplete')
                          ? 'bg-green-50 text-green-500 dark:bg-green-900/20'
                          : 'bg-primary-50 text-primary-600 dark:bg-primary-900/20'
                      }`}
                    >
                      {progressText}
                    </div>
                  )}
                </div>
              </div>
            </div>

            <div className="flex items-center gap-1 min-[361px]:gap-1.5 min-[431px]:gap-2 sm:gap-4 shrink-0 pt-0.5 sm:pt-0">
              {isCurrent && isPlaying ? (
                <div className="flex gap-0.5 sm:gap-1 items-end h-3 min-[361px]:h-3.5 min-[431px]:h-4 sm:h-5">
                  <div className={`w-0.5 sm:w-1 animate-music-bar-1 rounded-full ${!effectiveThemeColor ? 'bg-primary-600' : ''}`} style={effectiveThemeColor ? { backgroundColor: toSolidColor(effectiveThemeColor) } : {}}></div>
                  <div className={`w-0.5 sm:w-1 animate-music-bar-2 rounded-full ${!effectiveThemeColor ? 'bg-primary-600' : ''}`} style={effectiveThemeColor ? { backgroundColor: toSolidColor(effectiveThemeColor) } : {}}></div>
                  <div className={`w-0.5 sm:w-1 animate-music-bar-3 rounded-full ${!effectiveThemeColor ? 'bg-primary-600' : ''}`} style={effectiveThemeColor ? { backgroundColor: toSolidColor(effectiveThemeColor) } : {}}></div>
                </div>
              ) : (
                <div
                  className="w-6 h-6 min-[361px]:w-7 min-[361px]:h-7 min-[431px]:w-8 min-[431px]:h-8 sm:w-10 sm:h-10 rounded-full bg-slate-50 dark:bg-slate-800 flex items-center justify-center opacity-0 group-hover:opacity-100 transition-all cursor-pointer hover:scale-105"
                  onClick={(e) => {
                    e.stopPropagation();
                    onPlayChapter(chapter);
                  }}
                >
                  <Play size={12} className="text-primary-600 ml-0.5 w-3 h-3 min-[431px]:w-3.5 min-[431px]:h-3.5 sm:ml-1 sm:w-4 sm:h-4" fill="currentColor" style={effectiveThemeColor ? { color: toSolidColor(effectiveThemeColor) } : {}} />
                </div>
              )}
            </div>
          </div>
        );
      })}
    </div>
  </div>
  );
};

export default ChapterListSection;
