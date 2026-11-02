import React from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronUp, Settings } from 'lucide-react';
import type { Book } from '../../core/types';
import { setAlpha } from '../../core/utils/color';
import { getCoverUrl } from '../../core/utils/image';

const PlayerCover: React.FC<{ book: Book | null | undefined }> = ({ book }) => {
  const [failedUrl, setFailedUrl] = React.useState<string>();
  const url = getCoverUrl(book?.cover_url, book?.library_id, book?.id);
  return failedUrl === url ? (
    <div role="img" aria-label={book?.title} className="w-full h-full bg-slate-200 dark:bg-slate-800" />
  ) : (
    <img src={url} alt={book?.title} referrerPolicy="no-referrer"
      className="w-full h-full object-cover" onError={() => setFailedUrl(url)} />
  );
};

// ─── CollapsedPlayerView ───────────────────────────────────────────────────
// 折叠状态下的播放器：左下角只显示一个封面缩略图，点击展开回 mini player。

interface CollapsedPlayerViewProps {
  book: Book | null | undefined;
  coverSizeClass: string;
  themeColor?: string;
  onExpandCollapsed: () => void;
}

export const CollapsedPlayerView: React.FC<CollapsedPlayerViewProps> = ({
  book,
  coverSizeClass,
  themeColor,
  onExpandCollapsed,
}) => (
  <div
    className="h-full flex items-end justify-start pointer-events-auto pb-2 pl-2"
    onClick={onExpandCollapsed}
  >
    <div
      className={`${coverSizeClass} rounded-xl overflow-hidden shadow-2xl cursor-pointer hover:scale-105 transition-transform border-2 border-white/50 dark:border-slate-700/50`}
      style={{ borderColor: themeColor ? setAlpha(themeColor, 0.3) : undefined }}
    >
      <PlayerCover book={book} />
    </div>
  </div>
);

// ─── ExpandedPlayerHeader ──────────────────────────────────────────────────
// 展开视图顶部条：左侧收起按钮、中间书名 + 作者/演播、右侧设置按钮。

interface ExpandedPlayerHeaderProps {
  bookTitle?: string;
  authorText?: string;
  onExit: () => void;
  onOpenSettings: () => void;
}

export const ExpandedPlayerHeader: React.FC<ExpandedPlayerHeaderProps> = ({
  bookTitle,
  authorText,
  onExit,
  onOpenSettings,
}) => {
  const { t } = useTranslation();

  return (
    <div className="flex items-center justify-between w-full max-w-[520px] min-[700px]:landscape:max-w-[1280px] mx-auto pb-3">
      <button
        onClick={onExit}
        className="p-2 -ml-2 rounded-full text-slate-700 dark:text-white hover:bg-white/50 dark:hover:bg-slate-800/60 transition-colors"
        title={t('player.collapse')}
      >
        <ChevronUp size={24} className="rotate-180" />
      </button>
      <div className="flex-1 text-center px-3 min-w-0">
        <h2 className="text-sm sm:text-base font-bold dark:text-white text-[#4A3728] truncate">{bookTitle}</h2>
        <p className="text-[10px] sm:text-xs text-slate-500 truncate">{authorText}</p>
      </div>
      <button
        onClick={onOpenSettings}
        className="p-2 -mr-2 rounded-full text-slate-700 dark:text-white hover:bg-white/50 dark:hover:bg-slate-800/60 transition-colors"
        title={t('player.settings')}
      >
        <Settings size={22} />
      </button>
    </div>
  );
};

// ─── MiniPlayerBookInfo ────────────────────────────────────────────────────
// mini player 最左段：封面（点击切全屏）+ 在 ≥500 时显示书名 / 章节标题。

interface MiniPlayerBookInfoProps {
  book: Book | null | undefined;
  chapterTitle: string;
  coverSizeClass: string;
  isWidgetMode: boolean;
  onCoverClick: () => void;
}

export const MiniPlayerBookInfo: React.FC<MiniPlayerBookInfoProps> = ({
  book,
  chapterTitle,
  coverSizeClass,
  isWidgetMode,
  onCoverClick,
}) => (
  <div className={`flex items-center gap-2 sm:gap-3 min-w-0 ${isWidgetMode ? 'max-[380px]:w-full max-[380px]:max-w-none' : ''} max-[500px]:max-w-[48px] max-[380px]:max-w-[40px] sm:max-w-[200px] md:max-w-[240px] lg:max-w-[320px] md:flex-none flex-1`}>
    <div
      className={`${coverSizeClass} rounded-lg sm:rounded-xl overflow-hidden shadow-md cursor-pointer shrink-0`}
      onClick={onCoverClick}
    >
      <PlayerCover book={book} />
    </div>
    <div className="min-w-0 flex-1 hidden min-[500px]:block md:block max-[380px]:hidden">
      <h4 className="font-bold dark:text-white truncate text-sm max-[380px]:text-xs">{book?.title}</h4>
      <p className="text-slate-500 truncate text-xs max-[380px]:text-[10px]">{chapterTitle}</p>
    </div>
  </div>
);

// ─── ExpandedCover ─────────────────────────────────────────────────────────
// 竖屏与横屏左侧的大封面。

interface ExpandedCoverProps {
  book: Book | null | undefined;
  expandedCoverSizeClass: string;
}

export const ExpandedCover: React.FC<ExpandedCoverProps> = ({
  book,
  expandedCoverSizeClass,
}) => (
  <div className={`${expandedCoverSizeClass} rounded-[28px] sm:rounded-[36px] overflow-hidden shadow-2xl border-4 sm:border-8 border-white dark:border-slate-800 transition-all duration-500`}>
    <PlayerCover book={book} />
  </div>
);

// ─── ExpandedChapterTitle ──────────────────────────────────────────────────
// 章节名：竖屏在封面下方，横屏在右侧功能按钮上方；error 横幅跟随章节名。

interface ExpandedChapterTitleProps {
  chapterTitle: string;
  error: string | null;
}

export const ExpandedChapterTitle: React.FC<ExpandedChapterTitleProps> = ({
  chapterTitle,
  error,
}) => (
  <div className="w-full min-w-0 text-center">
    <h1 className="text-xl sm:text-2xl font-black leading-snug dark:text-white text-[#4A3728] line-clamp-2">
      {chapterTitle}
    </h1>
    {error && (
      <div className="mt-4 w-full rounded-2xl bg-red-500/15 border border-red-200/30 px-4 py-2 text-center text-xs font-bold text-red-600 dark:text-red-200">
        {error}
      </div>
    )}
  </div>
);
