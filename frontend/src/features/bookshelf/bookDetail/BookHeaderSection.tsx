import React from 'react';
import {
  Play,
  Heart,
  ChevronDown,
  ChevronUp,
  User,
  Mic2,
  ListMusic,
  Info,
  Edit,
  RefreshCw,
} from 'lucide-react';
import type { Book } from '../../../core/types';
import { setAlpha, toSolidColor, isLight } from '../../../core/utils/color';
import { getCoverUrl } from '../../../core/utils/image';
import ExpandableTitle from '../../../shared/widgets/ExpandableTitle';

interface Props {
  book: Book;
  coverShape: 'rect' | 'square';
  displayCoverUrl?: string;
  displayLibraryId?: string;
  effectiveThemeColor?: string;
  chapterTotalCount: number;
  resumeChapterTitle?: string;
  resumeChapterBookMatches: boolean;
  hasResumeChapter: boolean;
  isPlayButtonTextOverflowing: boolean;
  playButtonContainerRef: React.RefObject<HTMLButtonElement | null>;
  isFavorite: boolean;
  isAdmin: boolean;
  tagsRef: React.RefObject<HTMLDivElement | null>;
  isTagsExpanded: boolean;
  isTagsOverflowing: boolean;
  descriptionRef: React.RefObject<HTMLParagraphElement | null>;
  isDescriptionExpanded: boolean;
  isDescriptionOverflowing: boolean;
  onPlayClick: () => void;
  onToggleFavorite: () => void;
  onOpenScrapeDiff: () => void;
  onOpenEditModal: () => void;
  onSetIsTagsExpanded: (expanded: boolean) => void;
  onSetIsDescriptionExpanded: (expanded: boolean) => void;
}

const BookHeaderSection: React.FC<Props> = ({
  book,
  coverShape,
  displayCoverUrl,
  displayLibraryId,
  effectiveThemeColor,
  chapterTotalCount,
  resumeChapterTitle,
  resumeChapterBookMatches,
  hasResumeChapter,
  isPlayButtonTextOverflowing,
  playButtonContainerRef,
  isFavorite,
  isAdmin,
  tagsRef,
  isTagsExpanded,
  isTagsOverflowing,
  descriptionRef,
  isDescriptionExpanded,
  isDescriptionOverflowing,
  onPlayClick,
  onToggleFavorite,
  onOpenScrapeDiff,
  onOpenEditModal,
  onSetIsTagsExpanded,
  onSetIsDescriptionExpanded,
}) => {
  const playLabel = hasResumeChapter && resumeChapterTitle && resumeChapterBookMatches
    ? `正在播放：${resumeChapterTitle}`
    : hasResumeChapter && resumeChapterTitle
      ? `继续播放：${resumeChapterTitle}`
      : '立即播放';

  const marqueeLabel = hasResumeChapter && resumeChapterTitle && resumeChapterBookMatches
    ? `正在播放：${resumeChapterTitle}    `
    : hasResumeChapter && resumeChapterTitle
      ? `继续播放：${resumeChapterTitle}    `
      : '立即播放';

  return (
    <div className={`flex flex-col md:flex-row gap-6 md:gap-8 ${coverShape === 'square' ? 'md:items-center' : ''}`}>
      <div className="w-48 md:w-72 mx-auto md:mx-0 shrink-0">
        <div className={`${coverShape === 'square' ? 'aspect-square' : 'aspect-[3/4]'} rounded-3xl overflow-hidden shadow-2xl border border-slate-200 dark:border-slate-800`}>
          <img
            src={getCoverUrl(displayCoverUrl, displayLibraryId, book.id)}
            alt={book.title}
            className="w-full h-full object-cover rounded-lg shadow-xl"
            referrerPolicy="no-referrer"
            onError={(e) => {
              const target = e.target as HTMLImageElement;
              target.src = 'https://placehold.co/300x400?text=No+Cover';
              target.onerror = null;
            }}
          />
        </div>
      </div>

      <div className="flex-1 space-y-6 text-center md:text-left flex flex-col">
        <div className="space-y-3 min-w-0">
          <ExpandableTitle
            title={book.title}
            className="font-bold text-slate-900 dark:text-white leading-tight transition-all duration-300 text-xl sm:text-2xl md:text-3xl"
            maxLines={2}
          />
          <div className="flex flex-wrap justify-center md:justify-start gap-x-4 gap-y-2 mt-4 text-sm">
            <div className="flex items-center gap-1.5 text-slate-600 dark:text-slate-400">
              <User size={16} className="text-primary-500" />
              <span className="font-bold">{book.author || '未知作者'}</span>
            </div>
            <div className="flex items-center gap-1.5 text-slate-600 dark:text-slate-400">
              <Mic2 size={16} className="text-primary-500" />
              <span className="font-bold">{book.narrator || '未知演播'}</span>
            </div>
            <div className="flex items-center gap-1.5 text-slate-600 dark:text-slate-400">
              <ListMusic size={16} className="text-primary-500" />
              <span className="font-bold">{chapterTotalCount} 章节</span>
            </div>
          </div>

          {book.tags && (
            <div className="mt-3 flex items-start justify-center md:justify-start w-full gap-2">
              <div
                ref={tagsRef}
                className={`flex flex-wrap gap-2 transition-all duration-300 overflow-hidden justify-center md:justify-start ${
                  isTagsExpanded ? 'max-h-[500px]' : 'max-h-[32px]'
                }`}
              >
                {book.tags.split(/[,，]/).filter(tag => tag.trim()).map((tag, index) => (
                  <span
                    key={index}
                    className="px-2.5 py-1 bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-400 text-xs font-bold rounded-lg border border-slate-200/50 dark:border-slate-700/50 whitespace-nowrap"
                  >
                    {tag.trim()}
                  </span>
                ))}
                {isTagsExpanded && (
                  <button
                    onClick={() => onSetIsTagsExpanded(false)}
                    className="px-2 py-0.5 text-[10px] font-bold text-primary-500 hover:text-primary-600 flex items-center gap-0.5 bg-primary-50 dark:bg-primary-900/20 rounded-md border border-primary-100 dark:border-primary-900/30 shadow-sm self-center"
                  >
                    <ChevronUp size={10} /> 收起
                  </button>
                )}
              </div>
              {isTagsOverflowing && !isTagsExpanded && (
                <button
                  onClick={() => onSetIsTagsExpanded(true)}
                  className="shrink-0 px-2 py-0.5 text-[10px] font-bold text-primary-500 hover:text-primary-600 flex items-center gap-0.5 bg-primary-50 dark:bg-primary-900/20 rounded-md border border-primary-100 dark:border-primary-900/30 shadow-sm mt-1"
                >
                  <ChevronDown size={10} /> 更多
                </button>
              )}
            </div>
          )}
        </div>

        <div className="w-full flex flex-col gap-3 md:max-w-md mx-auto md:mx-0">
          <button
            ref={playButtonContainerRef}
            onClick={onPlayClick}
            className="w-full flex items-center justify-center gap-2 px-5 sm:px-8 py-3.5 sm:py-4 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-2xl shadow-xl shadow-primary-500/30 transition-all active:scale-95 group"
            style={effectiveThemeColor ? {
              backgroundColor: toSolidColor(effectiveThemeColor),
              boxShadow: `0 10px 20px -5px ${setAlpha(effectiveThemeColor, 0.3)}`,
              color: isLight(effectiveThemeColor) ? '#475569' : '#ffffff'
            } : {}}
          >
            <Play size={18} fill="currentColor" className="shrink-0" />
            {isPlayButtonTextOverflowing ? (
              <div className="flex-1 min-w-0 overflow-hidden">
                <div className="whitespace-nowrap inline-block animate-scroll-text">
                  {marqueeLabel}
                  {marqueeLabel !== '立即播放' ? marqueeLabel : ''}
                </div>
              </div>
            ) : (
              <span className="truncate">{playLabel}</span>
            )}
          </button>

          <div className="w-full flex gap-2 sm:gap-3">
            <button
              onClick={onToggleFavorite}
              className={`flex-1 min-w-0 px-3 sm:px-4 py-3 rounded-2xl border transition-all active:scale-95 flex items-center justify-center gap-2 font-bold text-sm ${
                isFavorite
                  ? 'bg-red-50 border-red-100 text-red-500 dark:bg-red-900/20 dark:border-red-900/30'
                  : 'bg-white dark:bg-slate-900 border-slate-200 dark:border-slate-800 text-slate-500 dark:text-slate-300 hover:text-red-500'
              }`}
            >
              <Heart size={20} fill={isFavorite ? "currentColor" : "none"} />
              收藏
            </button>

            {isAdmin && (
              <>
                <button
                  onClick={onOpenScrapeDiff}
                  className="flex-1 min-w-0 px-3 sm:px-4 py-3 rounded-2xl border bg-white dark:bg-slate-900 border-slate-200 dark:border-slate-800 text-slate-500 dark:text-slate-300 hover:text-primary-600 transition-all active:scale-95 flex items-center justify-center gap-2 font-bold text-sm"
                  title="刮削元数据"
                >
                  <RefreshCw size={20} />
                  刮削
                </button>
                <button
                  onClick={onOpenEditModal}
                  className="flex-1 min-w-0 px-3 sm:px-4 py-3 rounded-2xl border bg-white dark:bg-slate-900 border-slate-200 dark:border-slate-800 text-slate-500 dark:text-slate-300 hover:text-primary-600 transition-all active:scale-95 flex items-center justify-center gap-2 font-bold text-sm"
                >
                  <Edit size={20} />
                  编辑
                </button>
              </>
            )}
          </div>
        </div>

        <div
          className="mt-auto space-y-3 p-4 rounded-2xl border border-slate-100 dark:border-slate-800/50 relative group/desc"
          style={effectiveThemeColor ? {
            backgroundColor: setAlpha(effectiveThemeColor, 0.08)
          } : {}}
        >
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2 text-slate-900 dark:text-white font-bold text-sm uppercase tracking-wider opacity-60">
              <Info size={16} />
              简介内容
            </div>
          </div>
          <div className="relative">
            <p
              ref={descriptionRef}
              className={`text-sm md:text-base text-slate-600 dark:text-slate-400 leading-relaxed transition-all duration-300 ${
                !isDescriptionExpanded ? 'line-clamp-2' : ''
              }`}
            >
              {book.description || '暂无简介'}
            </p>
            {(isDescriptionOverflowing || isDescriptionExpanded) && (
              <button
                onClick={() => onSetIsDescriptionExpanded(!isDescriptionExpanded)}
                className="mt-2 text-primary-600 hover:text-primary-700 text-sm font-bold flex items-center gap-1 transition-colors"
              >
                {isDescriptionExpanded ? (
                  <><ChevronUp size={16} />收起详情</>
                ) : (
                  <><ChevronDown size={16} />展开全部</>
                )}
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};

export default BookHeaderSection;
