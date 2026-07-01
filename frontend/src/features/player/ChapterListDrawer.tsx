import React from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronLeft, ChevronUp, Clock, ListMusic } from 'lucide-react';
import type { Book, Chapter } from '../../core/types';
import { setAlpha, toSolidColor, isLight } from '../../core/utils/color';

interface ChapterGroup {
  start: number;
  end: number;
  chapters: Chapter[];
}

interface Props {
  show: boolean;
  currentBook: Book | null;
  currentChapter: Chapter | null;
  currentChapters: Chapter[];
  groups: ChapterGroup[];
  chaptersPerGroup: number;
  currentGroupIndex: number;
  activeTab: 'main' | 'extra';
  extraChapters: Chapter[];
  isPlaying: boolean;
  effectiveThemeColor?: string;
  scrollRef: React.RefObject<HTMLDivElement | null>;
  onClose: () => void;
  onSetActiveTab: (tab: 'main' | 'extra') => void;
  onSetCurrentGroupIndex: (index: number) => void;
  onScrollGroups: (direction: 'left' | 'right') => void;
  onPlayChapter: (chapter: Chapter) => void;
  formatTime: (s: number) => string;
  getChapterProgressText: (chapter: Chapter) => string | null;
}

const ChapterListDrawer: React.FC<Props> = ({
  show,
  currentBook,
  currentChapter,
  currentChapters,
  groups,
  chaptersPerGroup,
  currentGroupIndex,
  activeTab,
  extraChapters,
  isPlaying,
  effectiveThemeColor,
  scrollRef,
  onClose,
  onSetActiveTab,
  onSetCurrentGroupIndex,
  onScrollGroups,
  onPlayChapter,
  formatTime,
  getChapterProgressText,
}) => {
  const { t } = useTranslation();

  if (!show) return null;

  return (
    <div className="fixed inset-0 z-[250] flex items-end sm:items-center justify-center">
      <div
        className="absolute inset-0 bg-black/40 backdrop-blur-sm animate-in fade-in duration-300"
        onClick={onClose}
      />
      <div className="relative w-full max-w-2xl bg-white dark:bg-slate-900 rounded-t-[32px] sm:rounded-[32px] h-[80vh] sm:h-[70vh] flex flex-col overflow-hidden animate-in slide-in-from-bottom duration-300 shadow-2xl">
        <div className="p-4 sm:p-6 border-b border-slate-100 dark:border-slate-800 flex items-center justify-between">
          <div className="flex items-center gap-3 sm:gap-4">
            <h3 className="text-lg sm:text-xl font-bold dark:text-white flex items-center gap-2">
              <ListMusic size={24} className="text-primary-600" />
              {t('player.chapterList')}
            </h3>
            {extraChapters.length > 0 && (
              <div className="flex bg-slate-100 dark:bg-slate-800 p-1 rounded-xl scale-90 origin-left">
                <button
                  onClick={() => { onSetActiveTab('main'); onSetCurrentGroupIndex(0); }}
                  className={`px-3 py-1 rounded-lg text-xs font-bold transition-all ${
                    activeTab === 'main'
                      ? 'bg-white dark:bg-slate-700 text-primary-600 shadow-sm'
                      : 'text-slate-500 hover:text-slate-700 dark:hover:text-slate-300'
                  }`}
                >
                  {t('player.mainChapters')}
                </button>
                <button
                  onClick={() => { onSetActiveTab('extra'); onSetCurrentGroupIndex(0); }}
                  className={`px-3 py-1 rounded-lg text-xs font-bold transition-all ${
                    activeTab === 'extra'
                      ? 'bg-white dark:bg-slate-700 text-primary-600 shadow-sm'
                      : 'text-slate-500 hover:text-slate-700 dark:hover:text-slate-300'
                  }`}
                >
                  {t('player.extraChapters')}
                </button>
              </div>
            )}
          </div>
          <button
            onClick={onClose}
            className="p-2 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-full transition-colors"
          >
            <ChevronUp className="rotate-180" size={24} />
          </button>
        </div>

        {groups.length > 1 && (
          <div className="relative group/nav border-b border-slate-100 dark:border-slate-800 bg-slate-50 dark:bg-slate-800/50 flex items-center">
            <button
              onClick={() => onScrollGroups('left')}
              className="absolute -left-4 sm:-left-7 top-1/2 -translate-y-1/2 z-10 p-1 bg-white/90 dark:bg-slate-800/90 backdrop-blur shadow-md rounded-full opacity-0 group-hover/nav:opacity-100 transition-opacity hidden sm:block border border-slate-100 dark:border-slate-700"
            >
              <ChevronLeft size={20} className="text-slate-600 dark:text-slate-400" />
            </button>
            <div
              ref={scrollRef}
              className="flex gap-2 p-4 overflow-x-auto no-scrollbar scroll-smooth snap-x mx-1 w-full"
            >
              {groups.map((group, index) => (
                <button
                  key={index}
                  id={`player-group-tab-${index}`}
                  onClick={() => onSetCurrentGroupIndex(index)}
                  className={`px-4 py-2 rounded-xl text-sm font-bold transition-all border shrink-0 snap-start ${
                    currentGroupIndex === index
                      ? `text-white shadow-lg shadow-primary-500/30 ${!effectiveThemeColor ? 'bg-primary-600 border-primary-600' : ''}`
                      : 'bg-white dark:bg-slate-800 text-slate-600 dark:text-slate-400 border border-slate-200 dark:border-slate-700'
                  }`}
                  style={currentGroupIndex === index ? {
                    backgroundColor: effectiveThemeColor ? toSolidColor(effectiveThemeColor) : undefined,
                    borderColor: effectiveThemeColor ? toSolidColor(effectiveThemeColor) : undefined,
                    color: (effectiveThemeColor && isLight(effectiveThemeColor)) ? '#475569' : (effectiveThemeColor ? '#ffffff' : undefined)
                  } : {}}
                >
                  {t('player.chapterRange', { start: group.start, end: group.end })}
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

        <div className="flex-1 overflow-y-auto p-1.5 min-[361px]:p-2 min-[431px]:p-2.5 sm:p-4 space-y-1 min-[361px]:space-y-1.5 min-[431px]:space-y-2 sm:space-y-3">
          {(groups[currentGroupIndex]?.chapters || currentChapters).map((chapter, index) => {
            const actualIndex = currentGroupIndex * chaptersPerGroup + index;
            const isCurrent = currentChapter?.id === chapter.id;
            const progressText = getChapterProgressText(chapter);
            const isCompleted = !!chapter.progress_position
              && !!chapter.duration
              && chapter.progress_position / chapter.duration >= 0.95;

            return (
              <div
                key={chapter.id}
                id={`player-chapter-${chapter.id}`}
                onClick={() => {
                  if (currentBook) {
                    onPlayChapter(chapter);
                    onClose();
                  }
                }}
                className={`group flex items-start sm:items-center justify-between gap-1 min-[361px]:gap-1.5 min-[431px]:gap-2 p-1.5 min-[361px]:p-2 min-[431px]:p-2.5 sm:p-4 rounded-md min-[361px]:rounded-lg min-[431px]:rounded-xl sm:rounded-2xl cursor-pointer transition-all border ${
                  isCurrent
                    ? 'bg-opacity-10 border-opacity-20'
                    : 'bg-white dark:bg-slate-900 border-slate-100 dark:border-slate-800 hover:border-primary-200 dark:hover:border-primary-800'
                }`}
                style={isCurrent ? {
                  backgroundColor: effectiveThemeColor ? setAlpha(effectiveThemeColor, 0.1) : undefined,
                  borderColor: effectiveThemeColor ? setAlpha(effectiveThemeColor, 0.3) : undefined,
                } : {}}
              >
                <div className="flex items-start sm:items-center gap-1.5 min-[361px]:gap-2 min-[431px]:gap-2.5 sm:gap-4 min-w-0 flex-1">
                  <div
                    className={`w-6 h-6 min-[361px]:w-7 min-[361px]:h-7 min-[431px]:w-8 min-[431px]:h-8 sm:w-12 sm:h-12 rounded min-[361px]:rounded-md min-[431px]:rounded-lg sm:rounded-xl flex items-center justify-center font-medium text-[10px] min-[361px]:text-[11px] min-[431px]:text-xs sm:text-base shrink-0 ${
                      isCurrent ? `text-white ${!effectiveThemeColor ? 'bg-primary-600' : ''}` : 'bg-slate-100 dark:bg-slate-800 text-slate-500 dark:text-slate-400'
                    }`}
                    style={isCurrent ? {
                      backgroundColor: effectiveThemeColor ? toSolidColor(effectiveThemeColor) : undefined,
                      color: (effectiveThemeColor && isLight(effectiveThemeColor)) ? '#475569' : (effectiveThemeColor ? '#ffffff' : undefined)
                    } : {}}
                  >
                    {chapter.chapter_index || (actualIndex + 1)}
                  </div>
                  <div className="min-w-0 flex-1">
                    <p
                      className={`text-xs min-[361px]:text-[13px] min-[431px]:text-sm sm:text-base font-medium leading-normal line-clamp-2 break-words ${isCurrent ? '' : 'text-slate-900 dark:text-white'}`}
                      style={isCurrent ? { color: effectiveThemeColor ? toSolidColor(effectiveThemeColor) : undefined } : {}}
                    >
                      {chapter.title}
                    </p>
                    <div className="flex flex-wrap items-center gap-x-2 gap-y-1 mt-1">
                      <div className="flex items-center gap-1 text-[9px] min-[361px]:text-[10px] min-[431px]:text-[11px] sm:text-xs text-slate-400 font-normal">
                        <Clock size={10} className="w-2 h-2 min-[361px]:w-2.5 min-[361px]:h-2.5 sm:w-3 sm:h-3" />
                        {formatTime(chapter.duration)}
                      </div>
                      {progressText && (
                        <div
                          className={`text-[8px] min-[361px]:text-[9px] min-[431px]:text-[10px] font-medium px-0.5 min-[361px]:px-1 min-[431px]:px-1.5 py-0.5 rounded ${
                            isCompleted
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
                {isCurrent && isPlaying && (
                  <div className="flex gap-0.5 sm:gap-1 items-end h-3 min-[361px]:h-3.5 min-[431px]:h-4 sm:h-5 shrink-0 pt-0.5 sm:pt-0">
                    <div className={`w-0.5 sm:w-1 animate-music-bar-1 rounded-full ${!effectiveThemeColor ? 'bg-primary-600' : ''}`} style={{ backgroundColor: effectiveThemeColor ? toSolidColor(effectiveThemeColor) : undefined }}></div>
                    <div className={`w-0.5 sm:w-1 animate-music-bar-2 rounded-full ${!effectiveThemeColor ? 'bg-primary-600' : ''}`} style={{ backgroundColor: effectiveThemeColor ? toSolidColor(effectiveThemeColor) : undefined }}></div>
                    <div className={`w-0.5 sm:w-1 animate-music-bar-3 rounded-full ${!effectiveThemeColor ? 'bg-primary-600' : ''}`} style={{ backgroundColor: effectiveThemeColor ? toSolidColor(effectiveThemeColor) : undefined }}></div>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
};

export default ChapterListDrawer;
