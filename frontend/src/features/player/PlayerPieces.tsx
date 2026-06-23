import React from 'react';
import {
  Play,
  Pause,
  SkipBack,
  SkipForward,
  Volume2,
  VolumeX,
  ChevronLeft,
  ChevronUp,
  Maximize2,
  RotateCcw,
  RotateCw,
  Settings,
  Clock,
  Zap,
  ListMusic,
} from 'lucide-react';
import type { Book } from '../../core/types';
import { setAlpha, toSolidColor, isLight } from '../../core/utils/color';
import { getCoverUrl } from '../../core/utils/image';
import ProgressBar from './ProgressBar';

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
      <img
        src={getCoverUrl(book?.coverUrl, book?.libraryId, book?.id)}
        alt={book?.title}
        crossOrigin="anonymous"
        className="w-full h-full object-cover"
        onError={(e) => {
          (e.target as HTMLImageElement).src = 'https://placehold.co/300x400?text=No+Cover';
        }}
      />
    </div>
  </div>
);

// ─── ExpandedPlayerHeader ──────────────────────────────────────────────────
// 展开视图顶部条：左侧收起按钮、中间章节/书名标题、右侧设置按钮。

interface ExpandedPlayerHeaderProps {
  chapterTitle: string;
  bookTitle?: string;
  onExit: () => void;
  onOpenSettings: () => void;
}

export const ExpandedPlayerHeader: React.FC<ExpandedPlayerHeaderProps> = ({
  chapterTitle,
  bookTitle,
  onExit,
  onOpenSettings,
}) => (
  <div className="flex items-center justify-between w-full max-w-[520px] mx-auto pb-3">
    <button
      onClick={onExit}
      className="p-2 -ml-2 rounded-full text-slate-700 dark:text-white hover:bg-white/50 dark:hover:bg-slate-800/60 transition-colors"
      title="收起播放器"
    >
      <ChevronUp size={24} className="rotate-180" />
    </button>
    <div className="flex-1 text-center px-3 min-w-0">
      <h2 className="text-sm sm:text-base font-bold dark:text-white text-[#4A3728] truncate">{chapterTitle}</h2>
      <p className="text-[10px] sm:text-xs text-slate-500 truncate">{bookTitle}</p>
    </div>
    <button
      onClick={onOpenSettings}
      className="p-2 -mr-2 rounded-full text-slate-700 dark:text-white hover:bg-white/50 dark:hover:bg-slate-800/60 transition-colors"
      title="播放设置"
    >
      <Settings size={22} />
    </button>
  </div>
);

// ─── ExpandedMainControls ──────────────────────────────────────────────────
// 展开视图的主播控按钮组：上一章 / 播放暂停（大）/ 下一章。
// 播放按钮颜色跟随 themeColor，其他俩按钮固定 glassmorphism 风格。

interface ExpandedMainControlsProps {
  isPlaying: boolean;
  themeColor?: string;
  onPrev: () => void;
  onTogglePlay: () => void;
  onNext: () => void;
}

export const ExpandedMainControls: React.FC<ExpandedMainControlsProps> = ({
  isPlaying,
  themeColor,
  onPrev,
  onTogglePlay,
  onNext,
}) => (
  <div className="flex items-center justify-center gap-6 sm:gap-8 order-3">
    <button
      onClick={onPrev}
      className="w-14 h-14 sm:w-16 sm:h-16 rounded-full bg-white/60 dark:bg-slate-800/60 text-slate-900 dark:text-white flex items-center justify-center backdrop-blur-sm shadow-lg shadow-black/5 hover:bg-white/80 dark:hover:bg-slate-800 hover:scale-105 active:scale-95 transition-all"
    >
      <SkipBack size={26} fill="currentColor" />
    </button>

    <button
      onClick={onTogglePlay}
      className={`w-20 h-20 sm:w-24 sm:h-24 rounded-full text-white flex items-center justify-center shadow-2xl transform hover:scale-105 active:scale-95 transition-all ${!themeColor ? 'bg-primary-600' : ''}`}
      style={themeColor ? {
        backgroundColor: toSolidColor(themeColor),
        color: isLight(themeColor) ? '#475569' : '#ffffff',
      } : {}}
    >
      {isPlaying
        ? <Pause size={32} className="sm:w-12 sm:h-12" fill="currentColor" />
        : <Play size={32} className="sm:w-12 sm:h-12 ml-1 sm:ml-2" fill="currentColor" />}
    </button>

    <button
      onClick={onNext}
      className="w-14 h-14 sm:w-16 sm:h-16 rounded-full bg-white/60 dark:bg-slate-800/60 text-slate-900 dark:text-white flex items-center justify-center backdrop-blur-sm shadow-lg shadow-black/5 hover:bg-white/80 dark:hover:bg-slate-800 hover:scale-105 active:scale-95 transition-all"
    >
      <SkipForward size={26} fill="currentColor" />
    </button>
  </div>
);

// ─── VolumeSliderPanel ─────────────────────────────────────────────────────
// 通用音量滑块弹出面板：纵向 range + 数值 + 静音按钮。
// 在 mini-player desktop 控件区和 expanded view 底排控件中各使用一次，
// 外观差异（阴影、margin）通过 className 透传。

interface VolumeSliderPanelProps {
  volume: number;
  isMuted: boolean;
  className?: string;
  onChangeVolume: (volume: number) => void;
  onToggleMuted: () => void;
}

export const VolumeSliderPanel: React.FC<VolumeSliderPanelProps> = ({
  volume,
  isMuted,
  className = '',
  onChangeVolume,
  onToggleMuted,
}) => (
  <div
    className={`bg-white dark:bg-slate-800 rounded-full py-4 border border-slate-100 dark:border-slate-700 w-12 flex flex-col items-center gap-3 cursor-default ${className}`}
    onClick={(e) => e.stopPropagation()}
  >
    <span className="text-[10px] font-bold text-slate-500 min-w-[24px] text-center select-none">
      {Math.round(volume * 100)}
    </span>

    <div className="h-24 w-full flex items-center justify-center relative">
      <input
        type="range"
        min="0"
        max="1"
        step="0.01"
        value={volume}
        onChange={(e) => {
          const next = parseFloat(e.target.value);
          onChangeVolume(next);
          if (isMuted && next > 0) onToggleMuted();
        }}
        className="absolute w-24 h-1.5 bg-slate-200 dark:bg-slate-700 rounded-lg appearance-none cursor-pointer accent-primary-600 -rotate-90 hover:accent-primary-500"
      />
    </div>

    <button
      onClick={onToggleMuted}
      className={`p-2 rounded-full transition-colors ${
        isMuted
          ? 'bg-primary-100 text-primary-600 dark:bg-primary-900/30'
          : 'text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-700'
      }`}
      title={isMuted ? '取消静音' : '静音'}
    >
      {isMuted ? <VolumeX size={16} /> : <Volume2 size={16} />}
    </button>
  </div>
);

// ─── SleepTimerPopover ─────────────────────────────────────────────────────
// 睡眠定时弹出层：按钮 + 弹出面板（预设 4 档 + 自定义输入 + 取消按钮）。
// 倒计时逻辑在 useSleepTimer 里。

interface SleepTimerPopoverProps {
  sleepTimer: number | null;
  show: boolean;
  customMinutes: string;
  onToggleShow: () => void;
  onSetCustomMinutes: (value: string) => void;
  onStart: (durationSeconds: number) => void;
  onCancel: () => void;
  onClose: () => void;
  // 把 menuRef 直接挂在最外层 div 上，让外部 outside-click 逻辑能识别。
  menuRef: React.RefObject<HTMLDivElement | null>;
}

const SLEEP_TIMER_PRESET_MINUTES = [15, 30, 45, 60];

const formatSleepTimerRemaining = (seconds: number) =>
  `${Math.floor(seconds / 60)}:${(seconds % 60).toString().padStart(2, '0')}`;

export const SleepTimerPopover: React.FC<SleepTimerPopoverProps> = ({
  sleepTimer,
  show,
  customMinutes,
  onToggleShow,
  onSetCustomMinutes,
  onStart,
  onCancel,
  onClose,
  menuRef,
}) => (
  <div className="relative" ref={menuRef}>
    <button
      onClick={onToggleShow}
      className="w-full flex flex-col items-center gap-1.5 transition-all active:scale-95 group"
      title="睡眠定时"
    >
      <div className="w-10 h-10 rounded-2xl bg-white/50 dark:bg-slate-800/60 flex items-center justify-center group-hover:bg-white/70 dark:group-hover:bg-slate-800 transition-colors">
        <Clock size={18} className={sleepTimer ? 'text-primary-600' : ''} />
      </div>
      <span className="text-[10px] sm:text-xs font-bold leading-none whitespace-nowrap">
        {sleepTimer ? formatSleepTimerRemaining(sleepTimer) : '定时'}
      </span>
    </button>

    {show && (
      <div className="absolute bottom-full mb-4 right-0 bg-white dark:bg-slate-800 shadow-2xl rounded-2xl p-3 sm:p-4 border border-slate-100 dark:border-slate-700 min-w-[180px] sm:min-w-[200px] flex flex-col gap-2 z-[220] animate-in zoom-in-95 duration-200">
        <div className="px-2 py-1 text-[10px] font-bold text-slate-400 uppercase tracking-widest border-b border-slate-100 dark:border-slate-700 mb-1 text-center">
          睡眠定时
        </div>
        <div className="grid grid-cols-2 gap-2">
          {SLEEP_TIMER_PRESET_MINUTES.map((mins) => (
            <button
              key={mins}
              onClick={() => {
                onStart(mins * 60);
                onClose();
              }}
              className="px-3 py-2 text-xs sm:text-sm rounded-xl hover:bg-slate-100 dark:hover:bg-slate-700 text-slate-600 dark:text-slate-400 transition-colors border border-transparent hover:border-slate-200 dark:hover:border-slate-600"
            >
              {mins} 分钟
            </button>
          ))}
        </div>

        <div className="mt-1 flex items-center gap-1 p-1 bg-slate-50 dark:bg-slate-900/50 rounded-xl border border-slate-100 dark:border-slate-700 focus-within:border-primary-500/50 transition-colors">
          <input
            type="number"
            min="1"
            value={customMinutes}
            onChange={(e) => {
              const val = e.target.value;
              if (val === '' || parseInt(val) >= 0) {
                onSetCustomMinutes(val);
              }
            }}
            placeholder="自定义分钟"
            className="flex-1 bg-transparent border-none outline-none px-2 py-1.5 text-xs dark:text-white placeholder:text-slate-400 w-0"
          />
          <button
            onClick={() => {
              const mins = parseInt(customMinutes);
              if (mins > 0) {
                onStart(mins * 60);
                onClose();
                onSetCustomMinutes('');
              }
            }}
            className="px-3 py-1.5 text-xs font-bold rounded-lg bg-primary-600 text-white hover:bg-primary-700 transition-colors shrink-0"
          >
            开启
          </button>
        </div>

        <button
          onClick={() => {
            onCancel();
            onClose();
          }}
          className="mt-2 px-4 py-2 text-xs sm:text-sm font-bold rounded-xl bg-red-50 dark:bg-red-900/20 text-red-500 transition-colors"
        >
          取消定时
        </button>
      </div>
    )}
  </div>
);

// ─── ExpandedBottomControls ────────────────────────────────────────────────
// 展开视图底部四宫格：倍速 / 音量 / 睡眠定时 / 选集。

interface ExpandedBottomControlsProps {
  // 倍速
  playbackSpeed: number;
  onCyclePlaybackSpeed: () => void;
  // 音量
  volume: number;
  isMuted: boolean;
  showVolumeControl: boolean;
  volumeControlRef: React.RefObject<HTMLDivElement | null>;
  onToggleShowVolumeControl: () => void;
  onChangeVolume: (v: number) => void;
  onToggleMuted: () => void;
  // 睡眠定时
  sleepTimer: number | null;
  showSleepTimer: boolean;
  customMinutes: string;
  timerMenuRef: React.RefObject<HTMLDivElement | null>;
  onToggleShowSleepTimer: () => void;
  onSetCustomMinutes: (value: string) => void;
  onStartSleepTimer: (durationSeconds: number) => void;
  onCancelSleepTimer: () => void;
  onCloseSleepTimer: () => void;
  // 选集
  onOpenChapterList: () => void;
}

export const ExpandedBottomControls: React.FC<ExpandedBottomControlsProps> = ({
  playbackSpeed,
  onCyclePlaybackSpeed,
  volume,
  isMuted,
  showVolumeControl,
  volumeControlRef,
  onToggleShowVolumeControl,
  onChangeVolume,
  onToggleMuted,
  sleepTimer,
  showSleepTimer,
  customMinutes,
  timerMenuRef,
  onToggleShowSleepTimer,
  onSetCustomMinutes,
  onStartSleepTimer,
  onCancelSleepTimer,
  onCloseSleepTimer,
  onOpenChapterList,
}) => (
  <div className="grid grid-cols-4 items-start gap-1 sm:gap-2 w-full text-slate-600 dark:text-slate-400 order-1">
    <button
      onClick={onCyclePlaybackSpeed}
      className="flex flex-col items-center gap-1.5 transition-all active:scale-95 group"
      title="播放速度"
    >
      <div className="w-10 h-10 rounded-2xl bg-white/50 dark:bg-slate-800/60 flex items-center justify-center group-hover:bg-white/70 dark:group-hover:bg-slate-800 transition-colors">
        <Zap size={18} className={playbackSpeed !== 1 ? 'text-primary-600' : ''} />
      </div>
      <span className="text-[10px] sm:text-xs font-bold leading-none">{playbackSpeed}x</span>
    </button>

    <div className="relative" ref={volumeControlRef}>
      <button
        onClick={(e) => {
          e.stopPropagation();
          onToggleShowVolumeControl();
        }}
        className="w-full flex flex-col items-center gap-1.5 transition-all active:scale-95 group"
        title="音量"
      >
        <div className="w-10 h-10 rounded-2xl bg-white/50 dark:bg-slate-800/60 flex items-center justify-center group-hover:bg-white/70 dark:group-hover:bg-slate-800 transition-colors">
          {isMuted || volume === 0 ? <VolumeX size={18} /> : <Volume2 size={18} />}
        </div>
        <span className="text-[10px] sm:text-xs font-bold leading-none">
          {isMuted || volume === 0 ? '静音' : `${Math.round(volume * 100)}%`}
        </span>
      </button>

      {showVolumeControl && (
        <VolumeSliderPanel
          volume={volume}
          isMuted={isMuted}
          className="absolute bottom-full mb-4 left-1/2 -translate-x-1/2 shadow-2xl z-[220] animate-in zoom-in-95 duration-200"
          onChangeVolume={onChangeVolume}
          onToggleMuted={onToggleMuted}
        />
      )}
    </div>

    <SleepTimerPopover
      sleepTimer={sleepTimer}
      show={showSleepTimer}
      customMinutes={customMinutes}
      onToggleShow={onToggleShowSleepTimer}
      onSetCustomMinutes={onSetCustomMinutes}
      onStart={onStartSleepTimer}
      onCancel={onCancelSleepTimer}
      onClose={onCloseSleepTimer}
      menuRef={timerMenuRef}
    />

    <button
      onClick={onOpenChapterList}
      className="flex flex-col items-center gap-1.5 transition-all active:scale-95 group"
      title="章节列表"
    >
      <div className="w-10 h-10 rounded-2xl bg-white/50 dark:bg-slate-800/60 flex items-center justify-center group-hover:bg-white/70 dark:group-hover:bg-slate-800 transition-colors">
        <ListMusic size={18} />
      </div>
      <span className="text-[10px] sm:text-xs font-bold leading-none">选集</span>
    </button>
  </div>
);

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
      <img
        src={getCoverUrl(book?.coverUrl, book?.libraryId, book?.id)}
        alt={book?.title}
        referrerPolicy="no-referrer"
        className="w-full h-full object-cover"
        onError={(e) => {
          (e.target as HTMLImageElement).src = 'https://placehold.co/300x400?text=No+Cover';
        }}
      />
    </div>
    <div className="min-w-0 flex-1 hidden min-[500px]:block md:block max-[380px]:hidden">
      <h4 className="font-bold dark:text-white truncate text-sm max-[380px]:text-xs">{book?.title}</h4>
      <p className="text-slate-500 truncate text-xs max-[380px]:text-[10px]">{chapterTitle}</p>
    </div>
  </div>
);

// ─── ExpandedCoverAndMeta ──────────────────────────────────────────────────
// 展开视图中部：大封面 + 章节标题 + 演播者/作者；error 横幅也在这里渲染。

interface ExpandedCoverAndMetaProps {
  book: Book | null | undefined;
  chapterTitle: string;
  expandedCoverSizeClass: string;
  error: string | null;
}

export const ExpandedCoverAndMeta: React.FC<ExpandedCoverAndMetaProps> = ({
  book,
  chapterTitle,
  expandedCoverSizeClass,
  error,
}) => (
  <>
    <div className={`${expandedCoverSizeClass} rounded-[28px] sm:rounded-[36px] overflow-hidden shadow-2xl border-4 sm:border-8 border-white dark:border-slate-800 transition-all duration-500`}>
      <img
        src={getCoverUrl(book?.coverUrl, book?.libraryId, book?.id)}
        alt={book?.title}
        referrerPolicy="no-referrer"
        className="w-full h-full object-cover"
        onError={(e) => {
          (e.target as HTMLImageElement).src = 'https://placehold.co/300x400?text=No+Cover';
        }}
      />
    </div>

    <div className="text-center w-full min-w-0">
      <h1 className="text-xl sm:text-2xl font-black leading-snug dark:text-white text-[#4A3728] line-clamp-2">
        {chapterTitle}
      </h1>
      <p className="mt-1.5 text-sm text-slate-500 truncate">
        {book?.narrator || book?.author || book?.title || '未知作者'}
      </p>
    </div>

    {error && (
      <div className="w-full rounded-2xl bg-red-500/15 border border-red-200/30 px-4 py-2 text-center text-xs font-bold text-red-600 dark:text-red-200">
        {error}
      </div>
    )}
  </>
);
// mini player 上一堆按钮共用的「灰色 / 主题色（自动按亮度选文字色）」着色规则。
const miniThemedColor = (themeColor: string | undefined, useDarkControls: boolean) => {
  if (!themeColor || useDarkControls) return undefined;
  return isLight(themeColor) ? '#475569' : setAlpha(themeColor, 0.6);
};

// ─── ProgressBar 通用透传 props ───────────────────────────────────────────
// mini player 在 3 个位置渲染 ProgressBar（widget 紧凑、桌面、移动），重复的入参很长。
export interface PlayerProgressBarProps {
  isSeeking: boolean;
  seekTime: number;
  currentTime: number;
  duration: number;
  bufferedTime: number;
  onSeek: (e: React.SyntheticEvent<HTMLInputElement>) => void;
  onSeekStart: () => void;
  onSeekEnd: (e: React.SyntheticEvent<HTMLInputElement>) => void;
}

// ─── MiniPlayerDesktopControls ─────────────────────────────────────────────
// mini player 桌面中段：上一章 / 快退 15s / 播放 / 快进 30s / 下一章，下方一行进度条 + 时间。

interface MiniPlayerDesktopControlsProps extends PlayerProgressBarProps {
  isPlaying: boolean;
  currentTime: number;
  duration: number;
  themeColor?: string;
  effectiveThemeColor?: string;
  useDarkControls: boolean;
  formatTime: (t: number) => string;
  onPrev: () => void;
  onNext: () => void;
  onTogglePlay: () => void;
  onSeekTo: (time: number) => void;
}

export const MiniPlayerDesktopControls: React.FC<MiniPlayerDesktopControlsProps> = ({
  isPlaying,
  currentTime,
  duration,
  themeColor,
  effectiveThemeColor,
  useDarkControls,
  formatTime,
  onPrev,
  onNext,
  onTogglePlay,
  onSeekTo,
  isSeeking,
  seekTime,
  bufferedTime,
  onSeek,
  onSeekStart,
  onSeekEnd,
}) => {
  const tint = miniThemedColor(themeColor, useDarkControls);
  return (
    <div className="hidden md:flex flex-col items-center gap-1.5 flex-1 max-xl:max-w-xl px-4 lg:px-8">
      <div className="flex items-center gap-6">
        <button
          onClick={onPrev}
          className="text-slate-400 dark:text-slate-300 hover:scale-110 transition-all"
          style={{ color: tint }}
        >
          <SkipBack size={20} fill="currentColor" />
        </button>
        <button
          onClick={() => onSeekTo(currentTime - 15)}
          className="text-slate-400 dark:text-slate-300 hover:scale-110 transition-all"
          style={{ color: tint }}
        >
          <RotateCcw size={18} />
        </button>
        <button
          onClick={onTogglePlay}
          className={`w-10 h-10 rounded-full text-white flex items-center justify-center shadow-lg hover:scale-105 transition-all ${!effectiveThemeColor ? 'bg-primary-600 dark:bg-primary-600' : ''}`}
          style={{
            backgroundColor: effectiveThemeColor ? toSolidColor(effectiveThemeColor) : undefined,
            boxShadow: effectiveThemeColor ? `0 10px 15px -3px ${setAlpha(effectiveThemeColor, 0.3)}` : undefined,
            color: effectiveThemeColor ? (isLight(effectiveThemeColor) ? '#475569' : '#ffffff') : undefined,
          }}
        >
          {isPlaying ? <Pause size={20} fill="currentColor" /> : <Play size={20} fill="currentColor" className="ml-1" />}
        </button>
        <button
          onClick={() => onSeekTo(currentTime + 30)}
          className="text-slate-400 dark:text-slate-300 hover:scale-110 transition-all"
          style={{ color: tint }}
        >
          <RotateCw size={18} />
        </button>
        <button
          onClick={onNext}
          className="text-slate-400 dark:text-slate-300 hover:scale-110 transition-all"
          style={{ color: tint }}
        >
          <SkipForward size={20} fill="currentColor" />
        </button>
      </div>

      <div className="w-full flex items-center gap-3">
        <span className="text-[10px] text-slate-400 w-8 text-right">{formatTime(currentTime)}</span>
        <ProgressBar
          isMini
          isSeeking={isSeeking}
          seekTime={seekTime}
          currentTime={currentTime}
          duration={duration}
          bufferedTime={bufferedTime}
          themeColor={themeColor}
          onSeek={onSeek}
          onSeekStart={onSeekStart}
          onSeekEnd={onSeekEnd}
        />
        <span className="text-[10px] text-slate-400 w-8">{formatTime(duration)}</span>
      </div>
    </div>
  );
};

// ─── MiniPlayerDesktopExtras ───────────────────────────────────────────────
// mini player 桌面右段：音量按钮（点开 VolumeSliderPanel）/ 倍速文字按钮 / 收起 / 展开。

interface MiniPlayerDesktopExtrasProps {
  volume: number;
  isMuted: boolean;
  showVolumeControl: boolean;
  volumeControlRef: React.RefObject<HTMLDivElement | null>;
  themeColor?: string;
  useDarkControls: boolean;
  playbackSpeed: number;
  onToggleVolumeControl: () => void;
  onChangeVolume: (v: number) => void;
  onToggleMuted: () => void;
  onCyclePlaybackSpeed: () => void;
  onCollapse: () => void;
  onExpand: () => void;
}

export const MiniPlayerDesktopExtras: React.FC<MiniPlayerDesktopExtrasProps> = ({
  volume,
  isMuted,
  showVolumeControl,
  volumeControlRef,
  themeColor,
  useDarkControls,
  playbackSpeed,
  onToggleVolumeControl,
  onChangeVolume,
  onToggleMuted,
  onCyclePlaybackSpeed,
  onCollapse,
  onExpand,
}) => {
  const tint = miniThemedColor(themeColor, useDarkControls);
  return (
    <div className="hidden md:flex items-center gap-4 lg:gap-6 min-w-[100px] lg:min-w-[140px] justify-end">
      <div className="relative" ref={volumeControlRef}>
        <button
          onClick={(e) => {
            e.stopPropagation();
            onToggleVolumeControl();
          }}
          className={`transition-colors p-1 hover:scale-110 flex items-center gap-1 ${useDarkControls ? 'text-slate-200 hover:text-white' : 'text-slate-400 dark:text-slate-300'}`}
          style={{ color: tint }}
          title="音量"
        >
          {isMuted || volume === 0 ? <VolumeX size={20} /> : <Volume2 size={20} />}
        </button>

        {showVolumeControl && (
          <VolumeSliderPanel
            volume={volume}
            isMuted={isMuted}
            className="absolute bottom-full mb-3 left-1/2 -translate-x-1/2 shadow-xl z-[220] animate-in zoom-in-95 duration-200"
            onChangeVolume={onChangeVolume}
            onToggleMuted={onToggleMuted}
          />
        )}
      </div>

      <button
        onClick={onCyclePlaybackSpeed}
        className={`text-[10px] font-bold px-2 py-1 rounded transition-colors ${useDarkControls ? 'text-slate-200 hover:text-white' : 'dark:text-slate-300'}`}
        style={{
          backgroundColor: themeColor && !useDarkControls ? setAlpha(themeColor, 0.1) : undefined,
          color: themeColor && !useDarkControls
            ? (isLight(themeColor) ? '#475569' : setAlpha(themeColor, 0.8))
            : undefined,
        }}
      >
        {playbackSpeed}x
      </button>
      <button
        onClick={onCollapse}
        className={`transition-colors p-1 hover:scale-110 ${useDarkControls ? 'text-slate-200 hover:text-white' : 'text-slate-400 dark:text-slate-300'}`}
        style={{ color: tint }}
        title="收起播放器"
      >
        <ChevronLeft size={20} />
      </button>
      <button
        onClick={onExpand}
        className={`transition-colors p-1 hover:scale-110 ${useDarkControls ? 'text-slate-200 hover:text-white' : 'text-slate-400 dark:text-slate-300'}`}
        style={{ color: tint }}
        title="展开播放器"
      >
        <Maximize2 size={20} />
      </button>
    </div>
  );
};

// ─── MiniPlayerMobileControls ──────────────────────────────────────────────
// mini player 在 md 以下的右段：横向进度条 + 主播放按钮；widget 模式下还有快进 / 上下章。

interface MiniPlayerMobileControlsProps extends PlayerProgressBarProps {
  isPlaying: boolean;
  isWidgetMode: boolean;
  themeColor?: string;
  effectiveThemeColor?: string;
  useDarkControls: boolean;
  currentTime: number;
  onTogglePlay: () => void;
  onPrev: () => void;
  onNext: () => void;
  onSeekTo: (time: number) => void;
  onCollapse: () => void;
}

export const MiniPlayerMobileControls: React.FC<MiniPlayerMobileControlsProps> = ({
  isPlaying,
  isWidgetMode,
  themeColor,
  effectiveThemeColor,
  useDarkControls,
  currentTime,
  duration,
  bufferedTime,
  isSeeking,
  seekTime,
  onSeek,
  onSeekStart,
  onSeekEnd,
  onTogglePlay,
  onPrev,
  onNext,
  onSeekTo,
  onCollapse,
}) => {
  const tint = miniThemedColor(themeColor, useDarkControls);
  return (
    <div className={`flex md:hidden items-center gap-2 sm:gap-3 flex-1 min-w-0 justify-end ${isWidgetMode ? 'max-[380px]:w-full max-[380px]:justify-center max-[380px]:gap-6 max-[380px]:flex-none' : ''}`}>
      <div className={`flex-1 min-w-0 h-1.5 py-4 flex items-center w-full ${isWidgetMode ? 'max-[380px]:hidden' : ''}`}>
        <ProgressBar
          isMini
          isSeeking={isSeeking}
          seekTime={seekTime}
          currentTime={currentTime}
          duration={duration}
          bufferedTime={bufferedTime}
          themeColor={themeColor}
          onSeek={onSeek}
          onSeekStart={onSeekStart}
          onSeekEnd={onSeekEnd}
        />
      </div>
      <div className="flex items-center gap-1 shrink-0">
        {isWidgetMode && (
          <div className="flex items-center gap-1">
            <button
              onClick={() => onSeekTo(currentTime - 15)}
              className={`p-1.5 transition-colors hover:text-primary-500 ${useDarkControls ? 'text-slate-200' : 'text-slate-400 dark:text-slate-300'}`}
              style={{ color: tint }}
            >
              <RotateCcw size={16} />
            </button>
            <button
              onClick={onPrev}
              className={`p-1.5 transition-colors hover:text-primary-500 ${useDarkControls ? 'text-slate-200' : 'text-slate-400 dark:text-slate-300'}`}
              style={{ color: tint }}
            >
              <SkipBack size={16} fill="currentColor" />
            </button>
          </div>
        )}
        <button
          onClick={onTogglePlay}
          className={`w-10 h-10 max-[380px]:w-8 max-[380px]:h-8 rounded-full text-white flex items-center justify-center shadow-md hover:scale-105 transition-transform ${!effectiveThemeColor ? 'bg-primary-600 dark:bg-primary-600' : ''}`}
          style={{
            backgroundColor: effectiveThemeColor ? toSolidColor(effectiveThemeColor) : undefined,
            color: effectiveThemeColor ? (isLight(effectiveThemeColor) ? '#475569' : '#ffffff') : undefined,
          }}
        >
          {isPlaying
            ? <Pause size={20} className="max-[380px]:w-4 max-[380px]:h-4" fill="currentColor" />
            : <Play size={20} className="ml-1 max-[380px]:w-4 max-[380px]:h-4" fill="currentColor" />}
        </button>
        {isWidgetMode && (
          <div className="flex items-center gap-1">
            <button
              onClick={onNext}
              className={`p-1.5 transition-colors hover:text-primary-500 ${useDarkControls ? 'text-slate-200' : 'text-slate-400 dark:text-slate-300'}`}
              style={{ color: tint }}
            >
              <SkipForward size={16} fill="currentColor" />
            </button>
            <button
              onClick={() => onSeekTo(currentTime + 30)}
              className={`p-1.5 transition-colors hover:text-primary-500 ${useDarkControls ? 'text-slate-200' : 'text-slate-400 dark:text-slate-300'}`}
              style={{ color: tint }}
            >
              <RotateCw size={16} />
            </button>
          </div>
        )}
        {!isWidgetMode && (
          <button
            onClick={onCollapse}
            className={`p-2 transition-colors ${useDarkControls ? 'text-slate-200 hover:text-white' : 'text-slate-400 dark:text-slate-300'}`}
            style={{ color: tint }}
            title="收起播放器"
          >
            <ChevronLeft size={24} />
          </button>
        )}
      </div>
    </div>
  );
};

// ─── ExpandedProgressSection ───────────────────────────────────────────────
// 展开视图的进度条 + 左右各一个 15 秒快退/快进按钮，按钮里叠加数字「15」。

interface ExpandedProgressSectionProps extends PlayerProgressBarProps {
  currentTime: number;
  duration: number;
  themeColor: string;
  formatTime: (t: number) => string;
  onSeekTo: (time: number) => void;
}

export const ExpandedProgressSection: React.FC<ExpandedProgressSectionProps> = ({
  currentTime,
  duration,
  bufferedTime,
  isSeeking,
  seekTime,
  themeColor,
  formatTime,
  onSeek,
  onSeekStart,
  onSeekEnd,
  onSeekTo,
}) => (
  <div className="px-1 sm:px-2 order-2">
    <ProgressBar
      isSeeking={isSeeking}
      seekTime={seekTime}
      currentTime={currentTime}
      duration={duration}
      bufferedTime={bufferedTime}
      themeColor={themeColor}
      onSeek={onSeek}
      onSeekStart={onSeekStart}
      onSeekEnd={onSeekEnd}
    />
    <div className="mt-3 grid grid-cols-[auto_auto_1fr_auto_auto] items-center gap-3 text-xs font-bold text-slate-500 dark:text-slate-400">
      <button
        onClick={() => onSeekTo(currentTime - 15)}
        className="relative w-9 h-9 rounded-full hover:bg-white/50 dark:hover:bg-slate-800/60 transition-colors text-slate-600 dark:text-slate-300"
        title="快退 15 秒"
      >
        <RotateCcw size={27} strokeWidth={2.2} className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2" />
        <span className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-[46%] text-[9px] font-black leading-none tabular-nums">15</span>
      </button>
      <span>{formatTime(currentTime)}</span>
      <span />
      <span>{formatTime(duration)}</span>
      <button
        onClick={() => onSeekTo(currentTime + 15)}
        className="relative w-9 h-9 rounded-full hover:bg-white/50 dark:hover:bg-slate-800/60 transition-colors text-slate-600 dark:text-slate-300"
        title="快进 15 秒"
      >
        <RotateCw size={27} strokeWidth={2.2} className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2" />
        <span className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-[46%] text-[9px] font-black leading-none tabular-nums">15</span>
      </button>
    </div>
  </div>
);
