import React from 'react';
import { useTranslation } from 'react-i18next';
import { Play, Pause, SkipBack, SkipForward, RotateCcw, RotateCw,
  Volume2, VolumeX, ChevronLeft, Maximize2, Zap, ListMusic, Bookmark } from 'lucide-react';
import { setAlpha, toSolidColor, isLight } from '../../core/utils/color';
import ProgressBar, { type ProgressBarProps } from './ProgressBar';
import { SleepTimerPopover, VolumeSliderPanel } from './PlayerSettingsControls';

// Shared tint for the compact player controls.
const miniThemedColor = (themeColor: string | undefined, useDarkControls: boolean) =>
  !themeColor || useDarkControls ? undefined
    : isLight(themeColor) ? '#475569' : setAlpha(themeColor, 0.6);

const MiniPlayerChapterButton: React.FC<{
  onClick: () => void;
  tint?: string;
  useDarkControls: boolean;
  mobile?: boolean;
}> = ({ onClick, tint, useDarkControls, mobile = false }) => {
  const { t } = useTranslation();
  return (
    <button type="button" onClick={onClick} title={t('player.chapterList')}
      aria-label={t('player.chapterList')} aria-haspopup="dialog"
      className={`shrink-0 flex items-center justify-center transition-colors hover:scale-110 ${mobile ? 'w-10 h-10 p-2' : 'w-7 h-7 p-1'} ${useDarkControls ? 'text-slate-200 hover:text-white' : 'text-slate-400 dark:text-slate-300'}`}
      style={{ color: tint }}>
      <ListMusic size={mobile ? 24 : 20} />
    </button>
  );
};

const PlayerPlayButton: React.FC<{
  playing: boolean;
  onClick: () => void;
  themeColor?: string;
  expanded?: boolean;
}> = ({ playing, onClick, themeColor, expanded = false }) => {
  const { t } = useTranslation();
  const Icon = playing ? Pause : Play;
  return (
    <button onClick={onClick} aria-label={t(playing ? 'player.pause' : 'player.play')}
      className={`rounded-full text-white flex items-center justify-center hover:scale-105 active:scale-95 transition-transform ${
        expanded ? 'w-20 h-20 sm:w-24 sm:h-24 shadow-2xl' : 'w-10 h-10 max-[380px]:w-8 max-[380px]:h-8 shadow-md'
      } ${themeColor ? '' : 'bg-primary-600'}`}
      style={themeColor ? {
        backgroundColor: toSolidColor(themeColor),
        color: isLight(themeColor) ? '#475569' : '#ffffff',
      } : undefined}>
      <Icon size={expanded ? 32 : 20} fill="currentColor"
        className={`${expanded ? 'sm:w-12 sm:h-12' : 'max-[380px]:w-4 max-[380px]:h-4'} ${playing ? '' : 'ml-1'}`} />
    </button>
  );
};

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

    <PlayerPlayButton playing={isPlaying} onClick={onTogglePlay} themeColor={themeColor} expanded />

    <button
      onClick={onNext}
      className="w-14 h-14 sm:w-16 sm:h-16 rounded-full bg-white/60 dark:bg-slate-800/60 text-slate-900 dark:text-white flex items-center justify-center backdrop-blur-sm shadow-lg shadow-black/5 hover:bg-white/80 dark:hover:bg-slate-800 hover:scale-105 active:scale-95 transition-all"
    >
      <SkipForward size={26} fill="currentColor" />
    </button>
  </div>
);

// ─── ExpandedBottomControls ────────────────────────────────────────────────
// 展开视图底部四宫格：倍速 / 书签 / 睡眠定时 / 选集。

interface ExpandedBottomControlsProps {
  playbackSpeed: number;
  showSpeedControl: boolean;
  onToggleSpeedControl: () => void;
  speedControlRef: React.RefObject<HTMLDivElement | null>;
  onChangePlaybackSpeed: (speed: number) => void;
  onOpenBookmarks: () => void;
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
  // 按集数睡眠定时
  sleepEpisodes: number | null;
  customEpisodes: string;
  onSetCustomEpisodes: (value: string) => void;
  onStartEpisodeSleepTimer: (episodes: number) => void;
  // 选集
  onOpenChapterList: () => void;
}

export const ExpandedBottomControls: React.FC<ExpandedBottomControlsProps> = ({
  playbackSpeed,
  showSpeedControl,
  onToggleSpeedControl,
  speedControlRef,
  onChangePlaybackSpeed,
  onOpenBookmarks,
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
  sleepEpisodes,
  customEpisodes,
  onSetCustomEpisodes,
  onStartEpisodeSleepTimer,
}) => {
  const { t } = useTranslation();

  return (
    <div className="grid grid-cols-4 items-start gap-1 sm:gap-2 w-full text-slate-600 dark:text-slate-400 order-1">
      <div ref={speedControlRef} className="relative flex justify-center">
      <button
        onClick={(e) => {
          e.stopPropagation();
          onToggleSpeedControl();
        }}
        className="flex flex-col items-center gap-1.5 transition-all active:scale-95 group"
        title={t('player.speed')}
      >
        <div className="w-10 h-10 rounded-2xl bg-white/50 dark:bg-slate-800/60 flex items-center justify-center group-hover:bg-white/70 dark:group-hover:bg-slate-800 transition-colors">
          <Zap size={18} className={playbackSpeed !== 1 ? 'text-primary-600' : ''} />
        </div>
        <span className="text-[10px] sm:text-xs font-bold leading-none">{playbackSpeed.toFixed(1)}x</span>
      </button>
      {showSpeedControl && <div className="absolute bottom-full left-0 z-[220] mb-3 w-56 rounded-lg border border-slate-200 bg-white p-3 shadow-xl dark:border-slate-700 dark:bg-slate-900" onClick={(event) => event.stopPropagation()}>
        <div className="mb-2 flex justify-between text-xs text-slate-500"><span>0.5x</span><span className="font-semibold text-slate-800 dark:text-slate-100">{playbackSpeed.toFixed(1)}x</span><span>3.0x</span></div>
        <input aria-label={t('player.speed')} type="range" min="0.5" max="3" step="0.1" value={playbackSpeed} onChange={(event) => onChangePlaybackSpeed(Number(event.target.value))} className="w-full accent-primary-600" />
      </div>}
    </div>

    <button onClick={onOpenBookmarks} className="flex flex-col items-center gap-1.5 transition-all active:scale-95 group" title={t('bookmarks.playerTitle', '书签')}>
      <div className="w-10 h-10 rounded-2xl bg-white/50 dark:bg-slate-800/60 flex items-center justify-center group-hover:bg-white/70 dark:group-hover:bg-slate-800 transition-colors"><Bookmark size={18} /></div>
      <span className="text-[10px] sm:text-xs font-bold leading-none">{t('bookmarks.playerTitle', '书签')}</span>
    </button>

    <SleepTimerPopover
      sleepTimer={sleepTimer}
      show={showSleepTimer}
      customMinutes={customMinutes}
      onToggleShow={onToggleShowSleepTimer}
      onSetCustomMinutes={onSetCustomMinutes}
      onStart={onStartSleepTimer}
      onCancel={onCancelSleepTimer}
      onClose={onCloseSleepTimer}
      sleepEpisodes={sleepEpisodes}
      customEpisodes={customEpisodes}
      onSetCustomEpisodes={onSetCustomEpisodes}
      onStartEpisodes={onStartEpisodeSleepTimer}
      menuRef={timerMenuRef}
    />

    <button
      onClick={onOpenChapterList}
      className="flex flex-col items-center gap-1.5 transition-all active:scale-95 group"
      title={t('player.chapterList')}
    >
      <div className="w-10 h-10 rounded-2xl bg-white/50 dark:bg-slate-800/60 flex items-center justify-center group-hover:bg-white/70 dark:group-hover:bg-slate-800 transition-colors">
        <ListMusic size={18} />
      </div>
      <span className="text-[10px] sm:text-xs font-bold leading-none">{t('player.selectChapter')}</span>
    </button>
  </div>
  );
};

// ─── ProgressBar 通用透传 props ───────────────────────────────────────────
// mini player 在 3 个位置渲染 ProgressBar（widget 紧凑、桌面、移动），重复的入参很长。
type PlayerProgressBarProps = Omit<ProgressBarProps, 'isMini' | 'themeColor'>;

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
        <PlayerPlayButton playing={isPlaying} onClick={onTogglePlay} themeColor={effectiveThemeColor} />
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
  onOpenChapterList: () => void;
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
  onOpenChapterList,
}) => {
  const { t } = useTranslation();
  const tint = miniThemedColor(themeColor, useDarkControls);
  return (
    <div className="hidden md:flex items-center gap-4 lg:gap-6 min-w-[100px] lg:min-w-[140px] justify-end">
      <MiniPlayerChapterButton onClick={onOpenChapterList} tint={tint} useDarkControls={useDarkControls} />
      <div className="relative" ref={volumeControlRef}>
        <button
          onClick={(e) => {
            e.stopPropagation();
            onToggleVolumeControl();
          }}
          className={`transition-colors p-1 hover:scale-110 flex items-center gap-1 ${useDarkControls ? 'text-slate-200 hover:text-white' : 'text-slate-400 dark:text-slate-300'}`}
          style={{ color: tint }}
          title={t('player.volume')}
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
        title={t('player.collapse')}
      >
        <ChevronLeft size={20} />
      </button>
      <button
        onClick={onExpand}
        className={`mini-player-expand-button transition-colors p-1 hover:scale-110 ${useDarkControls ? 'text-slate-200 hover:text-white' : 'text-slate-400 dark:text-slate-300'}`}
        style={{ color: tint }}
        title={t('player.expand')}
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
  onOpenChapterList: () => void;
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
  onOpenChapterList,
}) => {
  const { t } = useTranslation();
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
        <PlayerPlayButton playing={isPlaying} onClick={onTogglePlay} themeColor={effectiveThemeColor} />
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
        <MiniPlayerChapterButton onClick={onOpenChapterList} tint={tint} useDarkControls={useDarkControls} mobile />
        {!isWidgetMode && (
          <button
            onClick={onCollapse}
            className={`p-2 transition-colors ${useDarkControls ? 'text-slate-200 hover:text-white' : 'text-slate-400 dark:text-slate-300'}`}
            style={{ color: tint }}
            title={t('player.collapse')}
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
}) => {
  const { t } = useTranslation();

  return (
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
          title={t('player.rewindSeconds', { seconds: 15 })}
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
          title={t('player.forwardSeconds', { seconds: 15 })}
        >
          <RotateCw size={27} strokeWidth={2.2} className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2" />
          <span className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-[46%] text-[9px] font-black leading-none tabular-nums">15</span>
        </button>
      </div>
    </div>
  );
};
