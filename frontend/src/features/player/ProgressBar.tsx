import React from 'react';
import { setAlpha, toSolidColor, isTooLight } from '../../core/utils/color';

export interface ProgressBarProps {
  isMini?: boolean;
  isSeeking: boolean;
  seekTime: number;
  currentTime: number;
  duration: number;
  bufferedTime: number;
  themeColor?: string | null;
  onSeek: (e: React.SyntheticEvent<HTMLInputElement>) => void;
  onSeekStart: () => void;
  onSeekEnd: (e: React.SyntheticEvent<HTMLInputElement>) => void;
}

const ProgressBar: React.FC<ProgressBarProps> = ({
  isMini = false,
  isSeeking,
  seekTime,
  currentTime,
  duration,
  bufferedTime,
  themeColor,
  onSeek,
  onSeekStart,
  onSeekEnd
}) => {
  const displayTime = isSeeking ? seekTime : currentTime;
  const playedPercent = (Number.isFinite(duration) && duration > 0) ? (displayTime / duration) * 100 : 0;
  const bufferedPercent = (Number.isFinite(duration) && duration > 0) ? (bufferedTime / duration) * 100 : 0;

  // Filter out light colors
  const effectiveThemeColor = themeColor && !isTooLight(themeColor) ? themeColor : undefined;

  const barColor = effectiveThemeColor ? toSolidColor(effectiveThemeColor) : undefined;
  const shadowColor = effectiveThemeColor ? setAlpha(effectiveThemeColor, 0.4) : undefined;

  return (
    <div className={`relative group/progress ${isMini ? 'flex-1 w-full h-3 sm:h-2' : 'w-full h-4'} flex items-center select-none touch-none`}>
      {/* Track Background */}
      <div
        className={`absolute left-0 right-0 top-1/2 -translate-y-1/2 ${isMini ? 'h-1' : 'h-1.5'} bg-slate-300 dark:bg-slate-900 rounded-full overflow-hidden`}
      >
        {/* Buffered Bar */}
        <div
          className="absolute inset-y-0 left-0 bg-slate-400/30 dark:bg-slate-700/40 transition-all duration-300"
          style={{ width: `${bufferedPercent}%` }}
        />
        {/* Played Bar */}
        <div
          className={`absolute inset-y-0 left-0 z-10 ${!barColor ? 'bg-primary-600' : ''}`}
          style={{
            width: `${playedPercent}%`,
            backgroundColor: barColor,
            boxShadow: shadowColor ? `0 0 10px ${shadowColor}` : undefined
          }}
        />
      </div>

      {/* Thumb / Handle */}
      <div
        className={`absolute top-1/2 -translate-y-1/2 z-20 w-3 h-3 bg-white rounded-full shadow-md transition-transform duration-100 ease-out pointer-events-none ${isSeeking ? 'scale-150' : 'scale-100'}`}
        style={{
          left: `${playedPercent}%`,
          marginLeft: '-6px',
          backgroundColor: isSeeking ? '#ffffff' : (barColor || '#ffffff'),
          border: `1px solid ${barColor || 'transparent'}`
        }}
      />

      {/* Range Input for Seeking - Positioned and sized correctly to cover the entire bar */}
      <input
        type="range"
        min="0"
        max={Number.isFinite(duration) ? duration : 0}
        step="any"
        value={displayTime}
        onInput={onSeek}
        onMouseDown={onSeekStart}
        onTouchStart={onSeekStart}
        onMouseUp={onSeekEnd}
        onTouchEnd={onSeekEnd}
        className="absolute inset-0 w-full h-full opacity-0 cursor-pointer z-30"
        style={{
          margin: 0,
          padding: 0,
          WebkitAppearance: 'none'
        }}
      />
    </div>
  );
};

export default ProgressBar;
