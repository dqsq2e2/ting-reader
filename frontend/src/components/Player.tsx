import React, { useRef, useEffect, useLayoutEffect, useState } from 'react';
import { useLocation } from 'react-router-dom';
import { usePlayerStore } from '../store/playerStore';
import { useAuthStore } from '../store/authStore';
import { useWebSocket } from '../hooks/useWebSocket';
import apiClient from '../api/client';
import { FastAverageColor } from 'fast-average-color';
import type { Chapter } from '../types';
import { 
  Play, 
  Pause, 
  SkipBack, 
  SkipForward, 
  Volume2, 
  VolumeX, 
  // FastForward, 
  ChevronUp,
  ChevronLeft,
  Maximize2,
  Clock,
  Settings,
  RotateCcw,
  RotateCw,
  Zap,
  ListMusic,
  X,
  Check
} from 'lucide-react';
import { getCoverUrl } from '../utils/image';
import { sortChaptersForPlayback } from '../utils/chapter';
import { setAlpha, toSolidColor, isLight, isTooLight } from '../utils/color';
import { useBookshelfCoverShape } from '../hooks/useBookshelfCoverShape';

interface ProgressBarProps {
  isMini?: boolean;
  isSeeking: boolean;
  seekTime: number;
  currentTime: number;
  duration: number;
  bufferedTime: number;
  themeColor?: string | null;
  onSeek: (e: React.ChangeEvent<HTMLInputElement>) => void;
  onSeekStart: () => void;
  onSeekEnd: (e: React.ChangeEvent<HTMLInputElement>) => void;
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

const isAppleMobileBrowser = () => {
  if (typeof navigator === 'undefined') return false;
  const ua = navigator.userAgent || '';
  const isiPhoneOrIPad = /iPad|iPhone|iPod/.test(ua);
  const isModernIPad = navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1;
  return isiPhoneOrIPad || isModernIPad;
};

const isStrmPath = (path?: string) => path?.toLowerCase().split('?')[0].endsWith('.strm') ?? false;

const Player: React.FC = () => {
  const coverShape = useBookshelfCoverShape();
  const { token, activeUrl } = useAuthStore();
  const API_BASE_URL = activeUrl || import.meta.env.VITE_API_BASE_URL || (import.meta.env.PROD ? '' : 'http://localhost:3000');
  const toAbsoluteMediaUrl = (url: string) => {
    if (/^https?:\/\//i.test(url)) return url;
    const base = API_BASE_URL || window.location.origin;
    return `${base.replace(/\/$/, '')}${url.startsWith('/') ? url : `/${url}`}`;
  };
  const streamStartOffsetRef = useRef<{ chapterId: string | null; offset: number }>({
    chapterId: null,
    offset: 0,
  });

  const getStreamUrl = (chapterId: string) => {
    let url = '';
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    if ((window as any).electronAPI) {
      // Electron mode: use custom protocol for caching
      const remote = encodeURIComponent(API_BASE_URL);
      url = `ting://stream/${chapterId}?token=${token}&remote=${remote}`;
    } else {
      url = `${API_BASE_URL}/api/stream/${chapterId}?token=${token}`;
    }
    
    if (shouldTranscode) {
      url += '&transcode=mp3';
    }
    
    // Keep this fixed per chapter so the backend can log the start position without reloading as progress changes.
    const initialOffset = streamStartOffsetRef.current.chapterId === chapterId
      ? streamStartOffsetRef.current.offset
      : 0;
    const requestSeekOffset = seekOffset !== null ? seekOffset : initialOffset;
    if (requestSeekOffset > 0) {
      url += `&seek=${Math.floor(requestSeekOffset)}`;
    }
    
    // Add retry count to force URL refresh even if shouldTranscode didn't change (e.g. network retry)
    if (retryCount > 0) {
        url += `&retry=${retryCount}`;
    }
    
    return url;
  };

  const { 
    currentBook, 
    currentChapter, 
    isPlaying, 
    togglePlay, 
    currentTime, 
    duration, 
    setCurrentTime, 
    setDuration,
    nextChapter,
    prevChapter,
    playbackSpeed,
    setPlaybackSpeed,
    volume,
    setVolume,
    themeColor,
    setThemeColor,
    playChapter,
    setIsPlaying,
    isExpanded,
    setIsExpanded,
    isCollapsed,
    setIsCollapsed,
    isSeriesEditing
  } = usePlayerStore();

  if (currentChapter?.id && streamStartOffsetRef.current.chapterId !== currentChapter.id) {
    streamStartOffsetRef.current = {
      chapterId: currentChapter.id,
      offset: Math.max(0, Math.floor(currentTime || 0)),
    };
  }

  const { sendProgress: wsSendProgress } = useWebSocket();

  const audioRef = useRef<HTMLAudioElement>(null);
  const location = useLocation();
  const [isMuted, setIsMuted] = useState(false);
  const [showChapters, setShowChapters] = useState(false);
  const [showSleepTimer, setShowSleepTimer] = useState(false);
  const [showVolumeControl, setShowVolumeControl] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [currentGroupIndex, setCurrentGroupIndex] = useState(0);
  const [activeTab, setActiveTab] = useState<'main' | 'extra'>('main');
  const scrollRef = useRef<HTMLDivElement>(null);
  const volumeControlRef = useRef<HTMLDivElement>(null);

  const scrollGroups = (direction: 'left' | 'right') => {
    if (scrollRef.current) {
      const scrollAmount = 200;
      scrollRef.current.scrollBy({
        left: direction === 'left' ? -scrollAmount : scrollAmount,
        behavior: 'smooth'
      });
    }
  };
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const [chapters, setChapters] = useState<any[]>([]);
  const [customMinutes, setCustomMinutes] = useState('');
  const [editSkipIntro, setEditSkipIntro] = useState(0);
  const [editSkipOutro, setEditSkipOutro] = useState(0);

  const [isDark, setIsDark] = useState(() => document.documentElement.classList.contains('dark'));

  useEffect(() => {
    const observer = new MutationObserver(() => {
      setIsDark(document.documentElement.classList.contains('dark'));
    });
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] });
    return () => observer.disconnect();
  }, []);

  const effectiveThemeColor = themeColor && !isTooLight(themeColor) ? themeColor : undefined;
  const collapsedCoverSizeClass = coverShape === 'square'
    ? 'w-14 h-14 sm:w-16 sm:h-16'
    : 'w-12 sm:w-14 aspect-[3/4]';
  const miniCoverSizeClass = coverShape === 'square'
    ? 'w-12 h-12 max-[380px]:w-10 max-[380px]:h-10 sm:w-16 sm:h-16'
    : 'w-10 max-[380px]:w-8 sm:w-12 aspect-[3/4]';
  const expandedCoverSizeClass = coverShape === 'square'
    ? 'w-full max-w-[240px] sm:max-w-[320px] lg:max-w-[400px] aspect-square'
    : 'w-full max-w-[220px] sm:max-w-[280px] lg:max-w-[320px] aspect-[3/4]';
  // Always use the theme color for the mini player progress bar, even in dark mode
  const miniPlayerThemeColor = effectiveThemeColor;
  // Determine if we should use dark mode text colors (white/gray) for controls
  // In dark mode, we always want bright white/gray for contrast
  const useDarkControls = isDark;

  // Use stored theme color from book to avoid flash
  useEffect(() => {
    // Prefer camelCase if available, otherwise snake_case
    const color = currentBook?.themeColor;
    if (color) {
      setThemeColor(color);
    } else if (currentBook?.coverUrl) {
      // If no theme color but we have a cover, extract it client-side
      const coverUrl = getCoverUrl(currentBook.coverUrl, currentBook.libraryId, currentBook.id);
      const fac = new FastAverageColor();
      fac.getColorAsync(coverUrl, { algorithm: 'dominant' })
        .then(color => {
          setThemeColor(color.hex);
          // Update the store's currentBook locally so it persists in this session and avoids re-extraction
          usePlayerStore.setState(state => ({
            currentBook: state.currentBook ? {
              ...state.currentBook,
              themeColor: color.hex
            } : null
          }));
        })
        .catch(e => console.warn('在播放器中从封面提取颜色失败', e));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentBook?.id, currentBook?.themeColor]);

  useEffect(() => {
    if (currentBook) {
      setTimeout(() => {
        setEditSkipIntro(currentBook.skipIntro || 0);
        setEditSkipOutro(currentBook.skipOutro || 0);
      }, 0);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentBook?.id]);

  const handleSaveSettings = async () => {
    if (!currentBook) return;
    try {
      await apiClient.patch(`/api/books/${currentBook.id}`, {
        skipIntro: editSkipIntro,
        skipOutro: editSkipOutro
      });
      // Update local store state if necessary, but currentBook is in store
      usePlayerStore.setState(state => ({
        currentBook: state.currentBook ? {
          ...state.currentBook,
          skipIntro: editSkipIntro,
          skipOutro: editSkipOutro
        } : null
      }));
      setShowSettings(false);
    } catch (err) {
      console.error('保存设置失败', err);
    }
  };

  const { mainChapters, extraChapters } = React.useMemo(() => {
    return {
      mainChapters: chapters.filter(c => !c.isExtra),
      extraChapters: chapters.filter(c => c.isExtra)
    };
  }, [chapters]);

  const currentChapters = activeTab === 'main' ? mainChapters : extraChapters;

  const chaptersPerGroup = 100;
  const groups = React.useMemo(() => {
    const g = [];
    for (let i = 0; i < currentChapters.length; i += chaptersPerGroup) {
      const slice = currentChapters.slice(i, i + chaptersPerGroup);
      g.push({
        start: slice[0]?.chapterIndex || (i + 1),
        end: slice[slice.length - 1]?.chapterIndex || (i + slice.length),
        chapters: slice
      });
    }
    return g;
  }, [currentChapters]);

  const [sleepTimer, setSleepTimer] = useState<number | null>(null);
  const sleepTimerEndTimeRef = useRef<number | null>(null);
  const progressTimerRef = useRef<{ ws: ReturnType<typeof setInterval>; http: ReturnType<typeof setInterval> } | null>(null);
  const sleepTimerIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const timerMenuRef = useRef<HTMLDivElement>(null);

  const [error, setError] = useState<string | null>(null);
  const [bufferedTime, setBufferedTime] = useState(0);
  const [autoPreload, setAutoPreload] = useState(false);
  const [autoCache, setAutoCache] = useState(false);
  const [retryCount, setRetryCount] = useState(0);
  const [shouldTranscode, setShouldTranscode] = useState(false);
  const [seekOffset, setSeekOffset] = useState<number | null>(null);
  const [hlsStreamUrl, setHlsStreamUrl] = useState<string | null>(null);
  const [hlsSessionId, setHlsSessionId] = useState<string | null>(null);
  const [hlsChapterId, setHlsChapterId] = useState<string | null>(null);
  const [hlsSeekOffset, setHlsSeekOffset] = useState(0);
  const isInitialLoadRef = useRef(true);
  const preloadAudioRef = useRef<HTMLAudioElement | null>(null);
  const hlsRequestIdRef = useRef(0);
  const shouldUseHlsForCurrentChapter = isAppleMobileBrowser() && isStrmPath(currentChapter?.path) && !shouldTranscode;
  const isUsingHlsForCurrentChapter = shouldUseHlsForCurrentChapter && hlsChapterId === currentChapter?.id && !!hlsStreamUrl;

  const tryTranscodeFallback = () => {
    if (shouldTranscode || retryCount >= 3) return;
    // Silently retry with transcoding, no need to show error message
    // setError('检测到浏览器兼容性问题，正在切换兼容音频流...');
    setShouldTranscode(true);
    setRetryCount(prev => prev + 1);
    isInitialLoadRef.current = true;
  };

  // Fetch settings for auto_preload and user preferences
  useEffect(() => {
    apiClient.get('/api/settings').then(res => {
      // API returns camelCase
      setAutoPreload(!!res.data.autoPreload);
      setAutoCache(!!res.data.autoCache);
      
      // Apply user's default playback speed
      if (res.data.playbackSpeed) {
        setPlaybackSpeed(res.data.playbackSpeed);
      }
      
      // Apply volume if present in settings (check both root and settings_json)
      // Note: Volume might be stored in settings_json as it's not a core column
      const vol = res.data.volume ?? res.data.settingsJson?.volume;
      if (vol !== undefined) {
        setVolume(vol);
      }
    }).catch(err => console.error('获取设置失败', err));
  }, [setPlaybackSpeed, setVolume]);

  // Fetch chapters for the current book
  useEffect(() => {
    if (currentBook?.id) {
      apiClient.get(`/api/books/${currentBook.id}/chapters`).then(res => {
        const sortedChapters = sortChaptersForPlayback(res.data);
        setChapters(sortedChapters);
        setCurrentGroupIndex(0); // Reset group index when book changes
        // 更新 store 中的 chapters 数据，确保 nextChapter 函数能正确工作
        usePlayerStore.setState({ chapters: sortedChapters });
      }).catch(err => console.error('获取章节失败', err));
    }
  }, [currentBook?.id]);

  // 当组件加载时，如果 currentBook 存在但 store 中的 chapters 数组为空，主动获取章节数据
  useEffect(() => {
    const storeChapters = usePlayerStore.getState().chapters;
    if (currentBook?.id && storeChapters.length === 0) {
      apiClient.get(`/api/books/${currentBook.id}/chapters`).then(res => {
        const sortedChapters = sortChaptersForPlayback(res.data);
        setChapters(sortedChapters);
        usePlayerStore.setState({ chapters: sortedChapters });
      }).catch(err => console.error('获取章节失败', err));
    }
  }, [currentBook?.id]);

  // Close timer menu when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (timerMenuRef.current && !timerMenuRef.current.contains(event.target as Node)) {
        setShowSleepTimer(false);
      }
      if (volumeControlRef.current && !volumeControlRef.current.contains(event.target as Node)) {
        setShowVolumeControl(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  // Reset all playback state when chapter ID changes
  useEffect(() => {
    isInitialLoadRef.current = true;
    setShouldTranscode(false);
    setSeekOffset(null);
    setHlsStreamUrl(null);
    setHlsSessionId(null);
    setHlsChapterId(null);
    setHlsSeekOffset(0);
    hlsRequestIdRef.current += 1;
    setTimeout(() => {
      setBufferedTime(0);
      setRetryCount(0);
    }, 0);
    
    // 立即从章节数据设置时长，不等待音频加载
    if (currentChapter?.duration && currentChapter.duration > 0) {
      setDuration(currentChapter.duration);
      console.log(`章节切换，立即设置时长: ${currentChapter.duration}s`);
    } else {
      setDuration(0);
      console.log('章节切换，等待音频加载获取时长');
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentChapter?.id]);

  // Update duration display when chapter duration is updated (e.g., after FFprobe)
  // without resetting shouldTranscode or retryCount
  useEffect(() => {
    if (currentChapter?.duration && currentChapter.duration > 0) {
      setDuration(currentChapter.duration);
    }
  }, [currentChapter?.duration, setDuration]);

  useEffect(() => {
    if (!currentChapter || !shouldUseHlsForCurrentChapter) {
      setHlsStreamUrl(null);
      setHlsSessionId(null);
      setHlsChapterId(null);
      setHlsSeekOffset(0);
      return;
    }

    const requestId = ++hlsRequestIdRef.current;
    let startAt = Math.max(0, usePlayerStore.getState().currentTime || 0);
    if (isInitialLoadRef.current && currentBook?.skipIntro && startAt < currentBook.skipIntro) {
      startAt = currentBook.skipIntro;
    }

    setHlsChapterId(currentChapter.id);
    setHlsStreamUrl(null);
    setHlsSessionId(null);
    setHlsSeekOffset(startAt);
    setCurrentTime(startAt);
    isInitialLoadRef.current = false;

    const params: Record<string, string | number> = { transcode: 'hls' };
    if (token) params.token = token;
    if (startAt > 0) params.seek = startAt;

    apiClient.get(`/api/stream/${currentChapter.id}`, { params }).then(res => {
      if (requestId !== hlsRequestIdRef.current) return;
      const playlistUrl = res.data?.playlistUrl || res.data?.playlist_url;
      const sessionId = res.data?.sessionId || res.data?.session_id;
      if (!playlistUrl || !sessionId) {
        throw new Error('HLS response missing playlist URL or session ID');
      }
      setHlsSessionId(sessionId);
      setHlsStreamUrl(toAbsoluteMediaUrl(playlistUrl));
    }).catch(err => {
      if (requestId !== hlsRequestIdRef.current) return;
      console.error('HLS stream initialization failed', err);
      setHlsStreamUrl(null);
      setHlsSessionId(null);
      setHlsChapterId(null);
      tryTranscodeFallback();
    });

    return () => {
      hlsRequestIdRef.current += 1;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentChapter?.id, currentChapter?.path, shouldUseHlsForCurrentChapter, currentBook?.skipIntro, token]);

  // Reset initial load ref when retrying (to allow resume logic to run again)
  useEffect(() => {
    if (retryCount > 0) {
      isInitialLoadRef.current = true;
    }
  }, [retryCount]);

  // Sync state with audio element
  useEffect(() => {
    if (!audioRef.current || !currentChapter) return;
    if (shouldUseHlsForCurrentChapter && !hlsStreamUrl) return;
    setTimeout(() => setError(null), 0); // Clear error on source change
    
    // Reset retry count when chapter changes (this is also handled in another effect, but safe to double check)
    // IMPORTANT: If source changes due to transcoding, we do NOT want to reset retry count immediately here
    // or we might enter a loop. 
    // Actually, retryCount is part of the dependency array, so this runs on retry too.
    
    if (isPlaying) {
      const playPromise = audioRef.current.play();
      if (playPromise !== undefined) {
        playPromise.catch(err => {
          // Ignore AbortError which happens when pausing/switching quickly
          if (err.name === 'AbortError' || err.code === 20) {
            console.log('播放承诺已中止 (正常)');
            return;
          }
          if (err.name === 'NotAllowedError') {
            // Safari/iOS may reject play() when it isn't treated as a direct user gesture.
            setIsPlaying(false);
            setError('浏览器阻止了自动播放，请再次点击播放按钮');
            console.warn('播放被浏览器策略阻止', err);
            return;
          }
          console.error('播放失败', err);
          // Don't set user-visible error yet, let onError handler try to recover first
          // setError('播放失败，可能是文件格式不支持或网络错误');
        });
      }
    } else {
      audioRef.current.pause();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isPlaying, currentChapter?.id, retryCount, shouldTranscode, seekOffset, hlsStreamUrl, hlsSeekOffset]);

  // Some browsers may report "playing" while decode hasn't actually advanced.
  // Detect "stuck at start" by checking that currentTime does not move after a delay,
  // while enough data is already buffered and media is in a playable readyState.
  useEffect(() => {
    if (!isPlaying || !currentChapter || !audioRef.current) return;

    const initialTime = audioRef.current.currentTime || 0;
    const timer = setTimeout(() => {
      const audio = audioRef.current;
      if (!audio || audio.paused || audio.seeking || audio.ended) return;
      if (audio.error) return;

      const time = audio.currentTime || 0;
      const progressed = time - initialTime;
      let bufferedEnd = 0;
      if (audio.buffered.length > 0) {
        bufferedEnd = audio.buffered.end(audio.buffered.length - 1);
      }

      const readyForDecode = audio.readyState >= HTMLMediaElement.HAVE_FUTURE_DATA;
      const decodeStuck = time < 0.05 && progressed < 0.03 && bufferedEnd > 1.5 && readyForDecode;
      if (decodeStuck) {
        tryTranscodeFallback();
      }
    }, 4500);

    return () => clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isPlaying, currentChapter?.id, shouldTranscode, retryCount]);

  // Preload and Server-side Cache next chapter logic
  useEffect(() => {
    if ((!autoPreload && !autoCache) || !currentChapter || !currentBook) return;
    
    // Find next chapter index
    apiClient.get(`/api/books/${currentBook.id}/chapters`).then(res => {
      const chapters = res.data;
      const currentIndex = chapters.findIndex((c: { id: string }) => c.id === currentChapter.id);
      if (currentIndex !== -1 && currentIndex < chapters.length - 1) {
        const nextChapterId = chapters[currentIndex + 1].id;
        const nextSrc = getStreamUrl(nextChapterId);
        
        // 1. Auto Preload (Memory)
        if (autoPreload) {
          if (!preloadAudioRef.current) {
            preloadAudioRef.current = new Audio();
            preloadAudioRef.current.preload = 'auto';
          }
          
          if (preloadAudioRef.current.src !== nextSrc) {
            console.log('正在预加载下一章:', chapters[currentIndex + 1].title);
            preloadAudioRef.current.src = nextSrc;
            preloadAudioRef.current.load();
          }
        }

        // 2. Auto Cache (Server-side WebDAV)
        if (autoCache) {
           console.log('触发服务器端缓存:', chapters[currentIndex + 1].title);
           apiClient.post(`/api/cache/${nextChapterId}`).catch(err => {
              console.error('触发服务器端缓存失败', err);
           });
        }
      }
    }).catch(err => console.error('预加载失败', err));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentChapter?.id, autoPreload, autoCache, currentBook?.id]);

  // Handle Skip Intro and Outro
  const handleTimeUpdate = () => {
    if (!audioRef.current) return;

    const rawTime = audioRef.current.currentTime;
    // Server-side seeked streams start from 0 but represent audio at an offset.
    const mediaOffset = isUsingHlsForCurrentChapter
      ? hlsSeekOffset
      : ((shouldTranscode && seekOffset !== null && seekOffset > 0) ? seekOffset : 0);
    const time = rawTime + mediaOffset;
    
    // Prevent overwriting persisted progress with 0 on initial load
    // If we are at the very beginning (time < 0.5) but store has significant progress (> 2s),
    // ignore this update until we've resumed properly.
    if (isInitialLoadRef.current && rawTime < 0.5 && currentTime > 2) {
      return;
    }

    // Mark initial load as done if we have successfully played past 1s
    if (isInitialLoadRef.current && rawTime > 1) {
       isInitialLoadRef.current = false;
    }

    setCurrentTime(time);

    // Update buffered time more accurately
    if (audioRef.current.buffered.length > 0) {
      // Find the range that contains the current time
      let currentRangeEnd = 0;
      for (let i = 0; i < audioRef.current.buffered.length; i++) {
        if (audioRef.current.buffered.start(i) <= rawTime && audioRef.current.buffered.end(i) >= rawTime) {
          currentRangeEnd = audioRef.current.buffered.end(i);
          break;
        }
      }
      
      // If no range contains current time, just use the end of the last range before current time
      if (currentRangeEnd === 0) {
        for (let i = audioRef.current.buffered.length - 1; i >= 0; i--) {
          if (audioRef.current.buffered.start(i) <= rawTime) {
            currentRangeEnd = audioRef.current.buffered.end(i);
            break;
          }
        }
      }

      setBufferedTime(currentRangeEnd + mediaOffset);
    }

    // Handle Skip Intro
    if (isInitialLoadRef.current && currentBook?.skipIntro) {
      if (time < currentBook.skipIntro) {
        audioRef.current.currentTime = currentBook.skipIntro;
        setCurrentTime(currentBook.skipIntro);
      }
      isInitialLoadRef.current = false;
    }

    // Handle Skip Outro
    if (currentBook?.skipOutro && duration > 0) {
      // Only skip if the chapter is long enough to actually have an outro
      // and we've played at least some of it
      const minChapterDuration = (currentBook.skipIntro || 0) + currentBook.skipOutro + 10;
      if (duration > minChapterDuration && (duration - time) <= currentBook.skipOutro) {
        nextChapter();
      }
    }
  };

  const handleProgress = () => {
    if (audioRef.current && audioRef.current.buffered.length > 0) {
      const rawTime = audioRef.current.currentTime;
      const mediaOffset = isUsingHlsForCurrentChapter
        ? hlsSeekOffset
        : ((shouldTranscode && seekOffset !== null && seekOffset > 0) ? seekOffset : 0);
      let currentRangeEnd = 0;
      for (let i = 0; i < audioRef.current.buffered.length; i++) {
        if (audioRef.current.buffered.start(i) <= rawTime && audioRef.current.buffered.end(i) >= rawTime) {
          currentRangeEnd = audioRef.current.buffered.end(i);
          break;
        }
      }
      if (currentRangeEnd === 0) {
        for (let i = audioRef.current.buffered.length - 1; i >= 0; i--) {
          if (audioRef.current.buffered.start(i) <= rawTime) {
            currentRangeEnd = audioRef.current.buffered.end(i);
            break;
          }
        }
      }
      setBufferedTime(currentRangeEnd + mediaOffset);
    }
  };

  // Handle Sleep Timer Countdown
  useEffect(() => {
    if (sleepTimer === null || sleepTimer <= 0 || !isPlaying || !sleepTimerEndTimeRef.current) return;

    // Clear any existing interval
    if (sleepTimerIntervalRef.current) {
      clearInterval(sleepTimerIntervalRef.current);
    }

    // Set up new interval to update remaining time based on end time
    const interval = setInterval(() => {
      if (sleepTimerEndTimeRef.current) {
        const remaining = Math.max(0, Math.floor((sleepTimerEndTimeRef.current - Date.now()) / 1000));
        setSleepTimer(remaining);
      }
    }, 1000);

    sleepTimerIntervalRef.current = interval;

    return () => {
      if (sleepTimerIntervalRef.current) {
        clearInterval(sleepTimerIntervalRef.current);
        sleepTimerIntervalRef.current = null;
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sleepTimer === null, isPlaying]);

  // Handle Sleep Timer Expiration
  useEffect(() => {
    if (sleepTimer === 0) {
      if (isPlaying) {
        togglePlay();
      }
      
      // Reset sleep timer references
      sleepTimerEndTimeRef.current = null;
      if (sleepTimerIntervalRef.current) {
        clearInterval(sleepTimerIntervalRef.current);
        sleepTimerIntervalRef.current = null;
      }
      
      setTimeout(() => setSleepTimer(null), 0);
    }
  }, [sleepTimer, isPlaying, togglePlay]);

  useEffect(() => {
    if (!audioRef.current) return;
    audioRef.current.playbackRate = playbackSpeed;
  }, [playbackSpeed]);

  useEffect(() => {
    if (!audioRef.current) return;
    audioRef.current.volume = isMuted ? 0 : volume;
  }, [volume, isMuted]);

  const currentTimeRef = useRef(0);
  useLayoutEffect(() => {
    currentTimeRef.current = currentTime;
  }, [currentTime]);

  // Sync progress to backend via WebSocket (primary) and HTTP (fallback)
  useEffect(() => {
    if (isPlaying && currentBook && currentChapter) {
      const saveProgressWs = (playbackStart?: number) => {
        wsSendProgress(
          currentBook.id,
          currentChapter.id,
          Math.floor(currentTimeRef.current),
          playbackStart
        );
      };

      const saveProgressHttp = (playbackStart?: number) => {
        apiClient.post('/api/progress', {
          bookId: currentBook.id,
          chapterId: currentChapter.id,
          position: Math.floor(currentTimeRef.current),
          ...(playbackStart !== undefined ? { playbackStart } : {})
        }).catch(err => console.error('HTTP进度同步失败', err));
      };

      // Mark only the actual start/resume packet. Periodic packets are progress heartbeats.
      const playbackStart = Math.floor(currentTimeRef.current);
      saveProgressWs(playbackStart);
      saveProgressHttp(playbackStart);

      // WS-based sync every 2 seconds for real-time progress tracking
      const wsTimer = setInterval(saveProgressWs, 2000);
      // HTTP fallback sync every 15 seconds
      const httpTimer = setInterval(saveProgressHttp, 15000);

      progressTimerRef.current = { ws: wsTimer, http: httpTimer };
    } else {
      if (progressTimerRef.current) {
        clearInterval(progressTimerRef.current.ws);
        clearInterval(progressTimerRef.current.http);
        progressTimerRef.current = null;
      }
    }
    return () => {
      if (progressTimerRef.current) {
        clearInterval(progressTimerRef.current.ws);
        clearInterval(progressTimerRef.current.http);
        progressTimerRef.current = null;
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isPlaying, currentBook?.id, currentChapter?.id]);

  // Save progress immediately when pausing to prevent progress loss
  const prevIsPlayingRef = useRef(isPlaying);
  useEffect(() => {
    if (prevIsPlayingRef.current && !isPlaying && currentBook && currentChapter) {
      const pos = Math.floor(currentTimeRef.current);
      wsSendProgress(currentBook.id, currentChapter.id, pos);
      apiClient.post('/api/progress', {
        bookId: currentBook.id,
        chapterId: currentChapter.id,
        position: pos
      }).catch(err => console.error('暂停时保存进度失败', err));
    }
    prevIsPlayingRef.current = isPlaying;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isPlaying]);

  const handleLoadedMetadata = () => {
    if (audioRef.current) {
      let browserDuration = audioRef.current.duration;
      
      // 优先使用章节数据中的时长（数据库中已有）
      if (currentChapter?.duration && currentChapter.duration > 0) {
        browserDuration = currentChapter.duration;
        console.log(`使用章节数据中的时长: ${browserDuration}s`);
      } 
      // 只在章节数据中没有时长时，才使用浏览器返回的时长
      else if (Number.isFinite(browserDuration) && !isNaN(browserDuration) && browserDuration > 0) {
        console.log(`使用浏览器返回的时长: ${browserDuration}s`);
      }
      // 两者都无效，尝试从音频元素获取
      else {
        console.warn('无法获取有效时长，使用默认值 0');
        browserDuration = 0;

        // 转码流（chunked transfer）浏览器报告 Infinity/NaN，
        // 但后端在启动 FFmpeg 之前已经通过 FFprobe 获取了真实时长并写入了数据库。
        // 重新从服务器拉取章节数据，拿到 FFprobe 发现的时长。
        if (shouldTranscode && currentBook?.id && currentChapter?.id) {
          const fetchBookId = currentBook.id;
          const fetchChapterId = currentChapter.id;
          apiClient.get(`/api/books/${fetchBookId}/chapters`).then(res => {
            const updatedChapters = sortChaptersForPlayback(res.data);
            const updatedChapter = updatedChapters.find((c: Chapter) => c.id === fetchChapterId);
            if (updatedChapter && updatedChapter.duration && updatedChapter.duration > 0) {
              setChapters(updatedChapters);
              usePlayerStore.setState({
                chapters: updatedChapters,
                currentChapter: updatedChapter,
              });
              setDuration(updatedChapter.duration);
              console.log(`转码音频：从服务器获取到时长: ${updatedChapter.duration}s`);
            }
          }).catch(err => console.error('获取转码音频时长失败', err));
        }
      }

      setDuration(browserDuration);

      // Resume position from store if this is the initial load for this chapter
      if (isInitialLoadRef.current && !isUsingHlsForCurrentChapter) {
        const resumePosition = usePlayerStore.getState().currentTime;
        if (resumePosition > 0) {
          // If progress is very close to the end (e.g., within 2 seconds or > 99%), start from the beginning
          if (browserDuration > 0 && (browserDuration - resumePosition < 2 || resumePosition / browserDuration > 0.99)) {
            console.log(`Chapter ${currentChapter?.title} 已完成，从头开始`);
            audioRef.current.currentTime = 0;
            setCurrentTime(0);
          } else {
            console.log(`继续章节 ${currentChapter?.title} at ${resumePosition}s`);
            audioRef.current.currentTime = resumePosition;
          }
        }
      }

      // Ensure playback rate is applied
      audioRef.current.playbackRate = playbackSpeed;

      // 只在章节数据中没有时长，且浏览器返回了有效时长时，才同步回服务器
      if (currentChapter && (!currentChapter.duration || currentChapter.duration === 0)) {
        if (Number.isFinite(browserDuration) && browserDuration > 0) {
          const audioDuration = audioRef.current.duration;
          if (Number.isFinite(audioDuration) && audioDuration > 0) {
            console.log(`章节数据中无时长，同步浏览器时长到服务器: ${Math.round(audioDuration)}s`);
            apiClient.patch(`/api/chapters/${currentChapter.id}`, { duration: Math.round(audioDuration) })
              .catch(err => console.error('同步持续时间失败', err));
          }
        }
      }
    }
  };

  const [isSeeking, setIsSeeking] = useState(false);
  const [seekTime, setSeekTime] = useState(0);

  const handleSeek = (e: React.ChangeEvent<HTMLInputElement>) => {
    const time = parseFloat(e.target.value);
    setSeekTime(time);
    if (!isSeeking) {
      if (isUsingHlsForCurrentChapter) {
        setCurrentTime(time);
        return;
      }
      if (audioRef.current) {
        audioRef.current.currentTime = time;
      }
      setCurrentTime(time);
    }
  };

  const handleSeekStart = () => {
    setIsSeeking(true);
    setSeekTime(currentTime);
  };

  const seekToTime = (time: number) => {
    const targetTime = Math.max(0, duration > 0 ? Math.min(time, duration) : time);

    if (audioRef.current) {
      if (isUsingHlsForCurrentChapter && hlsSessionId) {
        const requestId = ++hlsRequestIdRef.current;
        setHlsSeekOffset(targetTime);
        setCurrentTime(targetTime);
        isInitialLoadRef.current = false;

        apiClient.post(`/api/stream/hls/${hlsSessionId}/seek`, null, {
          params: { seek: targetTime }
        }).then(res => {
          if (requestId !== hlsRequestIdRef.current) return;
          const playlistUrl = res.data?.playlistUrl || res.data?.playlist_url;
          if (!playlistUrl) {
            throw new Error('HLS seek response missing playlist URL');
          }
          setHlsStreamUrl(toAbsoluteMediaUrl(playlistUrl));
        }).catch(err => {
          if (requestId !== hlsRequestIdRef.current) return;
          console.error('HLS seek failed', err);
          tryTranscodeFallback();
        });
        return;
      }

      // For transcoded streams, native seeking won't work (no Range support)
      // Detect by checking if seekable ranges are empty or if we're in transcode mode
      const isNonSeekable = shouldTranscode || audioRef.current.seekable.length === 0;

      if (isNonSeekable && shouldTranscode) {
        // Reload audio with seek parameter (server-side seek via FFmpeg -ss)
        setSeekOffset(targetTime);
        setCurrentTime(targetTime);
        isInitialLoadRef.current = false;
      } else {
        audioRef.current.currentTime = targetTime;
        setCurrentTime(targetTime);
      }
    } else {
      setCurrentTime(targetTime);
    }
  };

  const handleSeekEnd = (e: React.ChangeEvent<HTMLInputElement>) => {
    const time = parseFloat(e.target.value);
    setIsSeeking(false);
    seekToTime(time);
  };

  const formatTime = (time: number) => {
    if (!Number.isFinite(time) || isNaN(time) || time < 0) return '0:00';
    const h = Math.floor(time / 3600);
    const m = Math.floor((time % 3600) / 60);
    const s = Math.floor(time % 60);
    
    if (h > 0) {
      return `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
    }
    return `${m}:${s.toString().padStart(2, '0')}`;
  };

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const getChapterProgressText = (chapter: any) => {
    if (!chapter.progressPosition || !chapter.duration) return null;
    
    const percent = Math.floor((chapter.progressPosition / chapter.duration) * 100);
    if (percent === 0) return null;
    if (percent >= 95) return '已播完';
    return `已播${percent}%`;
  };

  const hiddenPaths = ['/admin', '/settings', '/downloads', '/cache'];
  const isHiddenPage = hiddenPaths.some(path => location.pathname.startsWith(path));
  const isWidgetMode = window.location.pathname.startsWith('/widget');

  // Auto collapse player when navigating to hidden pages
  useEffect(() => {
    if (isHiddenPage && isExpanded) {
      setTimeout(() => setIsExpanded(false), 0);
    }
  }, [location.pathname, isExpanded, isHiddenPage, setIsExpanded]);

  useEffect(() => {
    setShowVolumeControl(false);
  }, [isExpanded]);

  // Fullscreen Logic for Widget
  const toggleFullscreen = async () => {
    if (!isWidgetMode) {
      setIsExpanded(true);
      return;
    }

    // Check if fullscreen is allowed
    if (!document.fullscreenEnabled) {
      console.warn('在此上下文中未启用全屏');
      return;
    }

    try {
      if (!document.fullscreenElement) {
        await document.documentElement.requestFullscreen();
        setIsExpanded(true);
      } else {
        await document.exitFullscreen();
        setIsExpanded(false);
      }
    } catch (err) {
      console.error('切换全屏时出错:', err);
      // Do NOT fallback to isExpanded=true if fullscreen fails
      // This prevents the UI from breaking inside a small iframe
    }
  };

  // Exit Expanded/Fullscreen View
  const handleExitExpanded = async () => {
    if (isWidgetMode && document.fullscreenElement) {
      try {
        await document.exitFullscreen();
      } catch (err) {
        console.error('退出全屏时出错:', err);
      }
    }
    setIsExpanded(false);
  };

  // Sync state when fullscreen changes (e.g. user presses Esc)
  useEffect(() => {
    if (!isWidgetMode) return;

    const handleFullscreenChange = () => {
      if (!document.fullscreenElement) {
        setIsExpanded(false);
      }
    };

    document.addEventListener('fullscreenchange', handleFullscreenChange);
    return () => document.removeEventListener('fullscreenchange', handleFullscreenChange);
  }, [isWidgetMode, setIsExpanded]);

  if (!currentChapter) return null;

  const miniPlayerStyle = !isExpanded ? {
    bottom: isWidgetMode ? '0' : 'var(--mini-player-offset)',
    height: isWidgetMode ? '100%' : (isCollapsed ? '64px' : 'var(--player-h)'),
    left: isWidgetMode ? '0' : undefined,
    right: isWidgetMode ? '0' : undefined,
  } : {};

  const audioSrc = shouldUseHlsForCurrentChapter
    ? (hlsChapterId === currentChapter.id ? (hlsStreamUrl || '') : '')
    : getStreamUrl(currentChapter.id);

  const handleEnded = () => {
    if (currentBook && currentChapter) {
      const finalPosition = Math.floor(duration);
      // Sync via both WS and HTTP for reliability
      wsSendProgress(currentBook.id, currentChapter.id, finalPosition);
      apiClient.post('/api/progress', {
        bookId: currentBook.id,
        chapterId: currentChapter.id,
        position: finalPosition
      }).catch(err => console.error('同步最终进度失败', err));
    }
    nextChapter();
  };

  const openChapterList = () => {
    if (currentChapter && chapters.length > 0) {
      const isExtra = !!currentChapter.isExtra || /番外|SP|Extra/i.test(currentChapter.title);
      const targetTab = isExtra ? 'extra' : 'main';
      if (activeTab !== targetTab) setActiveTab(targetTab);

      const targetList = chapters.filter(chapter => {
        const chapterIsExtra = !!chapter.isExtra || /番外|SP|Extra/i.test(chapter.title);
        return chapterIsExtra === isExtra;
      });

      const index = targetList.findIndex(chapter => chapter.id === currentChapter.id);
      if (index !== -1) {
        const groupIndex = Math.floor(index / chaptersPerGroup);
        setCurrentGroupIndex(groupIndex);

        setTimeout(() => {
          const chapterEl = document.getElementById(`player-chapter-${currentChapter.id}`);
          if (chapterEl) {
            chapterEl.scrollIntoView({ block: 'center', behavior: 'smooth' });
          }

          const groupTab = document.getElementById(`player-group-tab-${groupIndex}`);
          const container = scrollRef.current;
          if (groupTab && container) {
            const containerWidth = container.offsetWidth;
            const tabWidth = groupTab.offsetWidth;
            const tabLeft = groupTab.offsetLeft;

            container.scrollTo({
              left: tabLeft - containerWidth / 2 + tabWidth / 2,
              behavior: 'smooth'
            });
          }
        }, 100);
      }
    }

    setShowChapters(true);
  };

  return (
    <div 
      className={`
        absolute transition-all duration-500 ease-in-out
        ${(isHiddenPage || isSeriesEditing) && !isExpanded ? 'translate-y-full opacity-0 pointer-events-none' : ''}
        ${isExpanded 
          ? 'inset-0 z-[110] bg-white dark:bg-slate-950' 
          : 'left-0 right-0 z-[30] bg-transparent pointer-events-none'
        }
      `}
      style={miniPlayerStyle}
    >
      <audio
        ref={audioRef}
        src={audioSrc || undefined}
        crossOrigin="anonymous"
        onTimeUpdate={handleTimeUpdate}
        onProgress={handleProgress}
        onLoadedMetadata={handleLoadedMetadata}
        onEnded={handleEnded}
        onPlay={() => {
          setIsPlaying(true);
          if (audioRef.current) {
            audioRef.current.playbackRate = playbackSpeed;
          }
        }}
        onPause={() => setIsPlaying(false)}
        onError={(e) => {
          const audio = audioRef.current;
          console.log('触发音频错误事件', { 
            error: audio?.error, 
            code: audio?.error?.code, 
            message: audio?.error?.message,
            retryCount,
            shouldTranscode
          });

          if (audio && audio.error) {
            // Ignore aborted errors (code 4) ONLY if we are not already trying to recover
            // Actually code 4 is MEDIA_ERR_SRC_NOT_SUPPORTED, which is exactly what we want to catch for WMA
            // Code 1 is MEDIA_ERR_ABORTED
            
            if (audio.error.code === 1) {
              console.log('播放已中止 (用户操作)');
              return;
            }

            // Auto retry on network (2), decode error (3) or source not supported (4)
            // We include network error (2) in retry logic just in case, but transcode mainly fixes 3 & 4
            if (retryCount < 3) {
                 tryTranscodeFallback();
                 return;
            }
            console.error('音频元素错误', audio.error);
          } else {
            // Even if audio.error is null, if we have an error event and haven't retried max times, try transcoding
            // This handles edge cases where browser doesn't populate error object properly
            if (retryCount < 3) {
                tryTranscodeFallback();
                return;
            }
            console.error('音频元素错误 (未知)', e);
          }
          setError('音频加载出错，请尝试重新扫描库或稍后再试');
        }}
      />

      {error && !isExpanded && (
        <div className="absolute top-0 left-4 right-4 bg-red-500 text-white text-[10px] py-1 px-2 text-center rounded-t-lg animate-pulse z-[101]">
          {error}
        </div>
      )}

      {/* Mini Player - Floating Card Style on Mobile */}
      {!isExpanded && (
        <div className={`h-full ${isWidgetMode ? 'px-0' : 'px-2 sm:px-4'} pointer-events-none`}>
          {isCollapsed ? (
            /* Collapsed State - Cover Only in Bottom Left */
            <div 
              className="h-full flex items-end justify-start pointer-events-auto pb-2 pl-2"
              onClick={() => setIsCollapsed(false)}
            >
              <div 
                className={`${collapsedCoverSizeClass} rounded-xl overflow-hidden shadow-2xl cursor-pointer hover:scale-105 transition-transform border-2 border-white/50 dark:border-slate-700/50`}
                style={{ 
                  borderColor: miniPlayerThemeColor ? setAlpha(miniPlayerThemeColor, 0.3) : undefined
                }}
              >
                <img 
                  src={getCoverUrl(currentBook?.coverUrl, currentBook?.libraryId, currentBook?.id)} 
                  alt={currentBook?.title}
                  crossOrigin="anonymous"
                  className="w-full h-full object-cover"
                  onError={(e) => {
                    (e.target as HTMLImageElement).src = 'https://placehold.co/300x400?text=No+Cover';
                  }}
                />
              </div>
            </div>
          ) : (
            /* Normal Mini Player */
          <div 
            className={`
              h-full ${isWidgetMode ? 'max-w-none rounded-none border-none shadow-none' : 'max-w-7xl mx-auto rounded-2xl sm:rounded-3xl shadow-2xl shadow-black/10 border border-slate-200/50 dark:border-slate-800/50'}
              bg-white/95 dark:bg-slate-900/95 backdrop-blur-md 
              flex items-center justify-between gap-3 sm:gap-4 ${isWidgetMode ? 'px-3 max-[380px]:flex-col max-[380px]:justify-center max-[380px]:gap-1.5 max-[380px]:py-2' : 'px-3 sm:px-6'} pointer-events-auto
              transition-all duration-300
            `}
            style={{ 
              backgroundColor: isWidgetMode ? undefined : (miniPlayerThemeColor ? setAlpha(miniPlayerThemeColor, 0.05) : undefined),
              borderColor: isWidgetMode ? undefined : (miniPlayerThemeColor ? setAlpha(miniPlayerThemeColor, 0.2) : undefined)
            }}
          >
            {/* Info */}
            <div className={`flex items-center gap-2 sm:gap-3 min-w-0 ${isWidgetMode ? 'max-[380px]:w-full max-[380px]:max-w-none' : ''} max-[500px]:max-w-[48px] max-[380px]:max-w-[40px] sm:max-w-[200px] md:max-w-[240px] lg:max-w-[320px] md:flex-none flex-1`}>
              <div 
                className={`${miniCoverSizeClass} rounded-lg sm:rounded-xl overflow-hidden shadow-md cursor-pointer shrink-0`}
                onClick={toggleFullscreen}
              >
                <img 
                  src={getCoverUrl(currentBook?.coverUrl, currentBook?.libraryId, currentBook?.id)} 
                  alt={currentBook?.title}
                  referrerPolicy="no-referrer"
                  className="w-full h-full object-cover"
                  onError={(e) => {
                    (e.target as HTMLImageElement).src = 'https://placehold.co/300x400?text=No+Cover';
                  }}
                />
              </div>
              <div className="min-w-0 flex-1 hidden min-[500px]:block md:block max-[380px]:hidden">
                <h4 className="font-bold dark:text-white truncate text-sm max-[380px]:text-xs">{currentBook?.title}</h4>
                <p className="text-slate-500 truncate text-xs max-[380px]:text-[10px]">{currentChapter.title}</p>
              </div>
            </div>

            {/* Widget Vertical Layout: Progress Bar (Visible only on small widget) */}
            {isWidgetMode && (
              <div className="hidden max-[380px]:block w-full px-1 py-1">
                 <ProgressBar 
                   isMini={true} 
                   isSeeking={isSeeking}
                   seekTime={seekTime}
                   currentTime={currentTime}
                   duration={duration}
                   bufferedTime={bufferedTime}
                  themeColor={miniPlayerThemeColor}
                  onSeek={handleSeek}
                   onSeekStart={handleSeekStart}
                   onSeekEnd={handleSeekEnd}
                 />
              </div>
            )}

            {/* Controls (Desktop) */}
            <div className="hidden md:flex flex-col items-center gap-1.5 flex-1 max-xl:max-w-xl px-4 lg:px-8">
              <div className="flex items-center gap-6">
                <button 
                  onClick={prevChapter} 
                  className="text-slate-400 dark:text-slate-300 hover:scale-110 transition-all"
                  style={{ color: (miniPlayerThemeColor && !useDarkControls) ? (isLight(miniPlayerThemeColor) ? '#475569' : setAlpha(miniPlayerThemeColor, 0.6)) : undefined }}
                >
                  <SkipBack size={20} fill="currentColor" />
                </button>
                <button
                  onClick={() => seekToTime(currentTime - 15)}
                  className="text-slate-400 dark:text-slate-300 hover:scale-110 transition-all"
                  style={{ color: (miniPlayerThemeColor && !useDarkControls) ? (isLight(miniPlayerThemeColor) ? '#475569' : setAlpha(miniPlayerThemeColor, 0.6)) : undefined }}
                >
                  <RotateCcw size={18} />
                </button>
                <button
                    onClick={togglePlay}
                      className={`w-10 h-10 rounded-full text-white flex items-center justify-center shadow-lg hover:scale-105 transition-all ${!effectiveThemeColor ? 'bg-primary-600 dark:bg-primary-600' : ''}`}
                      style={{ 
                        backgroundColor: effectiveThemeColor ? toSolidColor(effectiveThemeColor) : undefined,
                        boxShadow: effectiveThemeColor ? `0 10px 15px -3px ${setAlpha(effectiveThemeColor, 0.3)}` : undefined,
                        color: (effectiveThemeColor && isLight(effectiveThemeColor)) ? '#475569' : (effectiveThemeColor ? '#ffffff' : undefined)
                      }}
                  >
                  {isPlaying ? <Pause size={20} fill="currentColor" /> : <Play size={20} fill="currentColor" className="ml-1" />}
                </button>
                <button
                  onClick={() => seekToTime(currentTime + 30)}
                  className="text-slate-400 dark:text-slate-300 hover:scale-110 transition-all"
                  style={{ color: (miniPlayerThemeColor && !useDarkControls) ? (isLight(miniPlayerThemeColor) ? '#475569' : setAlpha(miniPlayerThemeColor, 0.6)) : undefined }}
                >
                  <RotateCw size={18} />
                </button>
                <button 
                  onClick={nextChapter} 
                  className="text-slate-400 dark:text-slate-300 hover:scale-110 transition-all"
                  style={{ color: (miniPlayerThemeColor && !useDarkControls) ? (isLight(miniPlayerThemeColor) ? '#475569' : setAlpha(miniPlayerThemeColor, 0.6)) : undefined }}
                >
                  <SkipForward size={20} fill="currentColor" />
                </button>
              </div>

              <div className="w-full flex items-center gap-3">
                <span className="text-[10px] text-slate-400 w-8 text-right">{formatTime(currentTime)}</span>
                <ProgressBar 
                  isMini={true} 
                  isSeeking={isSeeking}
                  seekTime={seekTime}
                  currentTime={currentTime}
                  duration={duration}
                  bufferedTime={bufferedTime}
                  themeColor={miniPlayerThemeColor}
                  onSeek={handleSeek}
                  onSeekStart={handleSeekStart}
                  onSeekEnd={handleSeekEnd}
                />
                <span className="text-[10px] text-slate-400 w-8">{formatTime(duration)}</span>
              </div>
            </div>

            {/* Mobile Controls - Only visible on small screens */}
            <div className={`flex md:hidden items-center gap-2 sm:gap-3 flex-1 min-w-0 justify-end ${isWidgetMode ? 'max-[380px]:w-full max-[380px]:justify-center max-[380px]:gap-6 max-[380px]:flex-none' : ''}`}>
              <div className={`flex-1 min-w-0 h-1.5 py-4 flex items-center w-full ${isWidgetMode ? 'max-[380px]:hidden' : ''}`}>
                <ProgressBar 
                  isMini={true} 
                  isSeeking={isSeeking}
                  seekTime={seekTime}
                  currentTime={currentTime}
                  duration={duration}
                  bufferedTime={bufferedTime}
                  themeColor={miniPlayerThemeColor}
                  onSeek={handleSeek}
                  onSeekStart={handleSeekStart}
                  onSeekEnd={handleSeekEnd}
                />
              </div>
              <div className="flex items-center gap-1 shrink-0">
                {isWidgetMode && (
                  <div className="flex items-center gap-1">
                    <button
                    onClick={() => seekToTime(currentTime - 15)}
                    className={`p-1.5 transition-colors hover:text-primary-500 ${useDarkControls ? 'text-slate-200' : 'text-slate-400 dark:text-slate-300'}`}
                    style={{ color: (miniPlayerThemeColor && !useDarkControls) ? (isLight(miniPlayerThemeColor) ? '#475569' : setAlpha(miniPlayerThemeColor, 0.6)) : undefined }}
                  >
                    <RotateCcw size={16} />
                  </button>
                  <button 
                    onClick={prevChapter}
                    className={`p-1.5 transition-colors hover:text-primary-500 ${useDarkControls ? 'text-slate-200' : 'text-slate-400 dark:text-slate-300'}`}
                    style={{ color: (miniPlayerThemeColor && !useDarkControls) ? (isLight(miniPlayerThemeColor) ? '#475569' : setAlpha(miniPlayerThemeColor, 0.6)) : undefined }}

                  >
                      <SkipBack size={16} fill="currentColor" />
                    </button>
                  </div>
                )}
                <button 
                  onClick={togglePlay}
                  className={`w-10 h-10 max-[380px]:w-8 max-[380px]:h-8 rounded-full text-white flex items-center justify-center shadow-md hover:scale-105 transition-transform ${!effectiveThemeColor ? 'bg-primary-600 dark:bg-primary-600' : ''}`}
                  style={{ 
                    backgroundColor: effectiveThemeColor ? toSolidColor(effectiveThemeColor) : undefined,
                    color: (effectiveThemeColor && isLight(effectiveThemeColor)) ? '#475569' : (effectiveThemeColor ? '#ffffff' : undefined)
                  }}
                >
                  {isPlaying ? <Pause size={20} className="max-[380px]:w-4 max-[380px]:h-4" fill="currentColor" /> : <Play size={20} className="ml-1 max-[380px]:w-4 max-[380px]:h-4" fill="currentColor" />}
                </button>
                {isWidgetMode && (
                  <div className="flex items-center gap-1">
                    {/* Always show Next button */}
                    <button 
                      onClick={nextChapter}
                      className={`p-1.5 transition-colors hover:text-primary-500 ${useDarkControls ? 'text-slate-200' : 'text-slate-400 dark:text-slate-300'}`}
                      style={{ color: (miniPlayerThemeColor && !useDarkControls) ? (isLight(miniPlayerThemeColor) ? '#475569' : setAlpha(miniPlayerThemeColor, 0.6)) : undefined }}
                    >
                      <SkipForward size={16} fill="currentColor" />
                    </button>
                    <button
                      onClick={() => seekToTime(currentTime + 30)}
                      className={`p-1.5 transition-colors hover:text-primary-500 ${useDarkControls ? 'text-slate-200' : 'text-slate-400 dark:text-slate-300'}`}
                      style={{ color: (miniPlayerThemeColor && !useDarkControls) ? (isLight(miniPlayerThemeColor) ? '#475569' : setAlpha(miniPlayerThemeColor, 0.6)) : undefined }}
                    >
                      <RotateCw size={16} />
                    </button>
                  </div>
                )}
                {!isWidgetMode && (
                  <button 
                    onClick={() => setIsCollapsed(true)}
                    className={`p-2 transition-colors ${useDarkControls ? 'text-slate-200 hover:text-white' : 'text-slate-400 dark:text-slate-300'}`}
                    style={{ color: (miniPlayerThemeColor && !useDarkControls) ? (isLight(miniPlayerThemeColor) ? '#475569' : setAlpha(miniPlayerThemeColor, 0.6)) : undefined }}
                    title="收起播放器"
                  >
                    <ChevronLeft size={24} />
                  </button>
                )}
              </div>
            </div>

            {/* Desktop Extra Controls - Visible on Tablet and Desktop */}
            <div className="hidden md:flex items-center gap-4 lg:gap-6 min-w-[100px] lg:min-w-[140px] justify-end">
              {/* Volume Control */}
              <div className="relative" ref={!isExpanded ? volumeControlRef : null}>
                <button 
                  onClick={(e) => {
                    e.stopPropagation();
                    setShowVolumeControl(!showVolumeControl);
                  }}
                  className={`transition-colors p-1 hover:scale-110 flex items-center gap-1 ${useDarkControls ? 'text-slate-200 hover:text-white' : 'text-slate-400 dark:text-slate-300'}`}
                  style={{ color: (miniPlayerThemeColor && !useDarkControls) ? (isLight(miniPlayerThemeColor) ? '#475569' : setAlpha(miniPlayerThemeColor, 0.6)) : undefined }}
                  title="音量"
                >
                  {isMuted || volume === 0 ? (
                    <VolumeX size={20} />
                  ) : (
                    <Volume2 size={20} />
                  )}
                </button>

                {showVolumeControl && (
                  <div 
                    className="absolute bottom-full mb-3 left-1/2 -translate-x-1/2 bg-white dark:bg-slate-800 shadow-xl rounded-full py-4 border border-slate-100 dark:border-slate-700 w-12 flex flex-col items-center gap-3 z-[220] animate-in zoom-in-95 duration-200 cursor-default"
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
                          setVolume(parseFloat(e.target.value));
                          if (isMuted && parseFloat(e.target.value) > 0) setIsMuted(false);
                        }}
                        className="absolute w-24 h-1.5 bg-slate-200 dark:bg-slate-700 rounded-lg appearance-none cursor-pointer accent-primary-600 -rotate-90 hover:accent-primary-500"
                      />
                    </div>

                    <button
                      onClick={() => setIsMuted(!isMuted)}
                      className={`p-2 rounded-full transition-colors ${
                        isMuted 
                          ? 'bg-primary-100 text-primary-600 dark:bg-primary-900/30' 
                          : 'text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-700'
                      }`}
                      title={isMuted ? "取消静音" : "静音"}
                    >
                      {isMuted ? <VolumeX size={16} /> : <Volume2 size={16} />}
                    </button>
                  </div>
                )}
              </div>

              <button 
                onClick={() => setPlaybackSpeed(playbackSpeed === 2 ? 1 : playbackSpeed + 0.25)} 
                className={`text-[10px] font-bold px-2 py-1 rounded transition-colors ${useDarkControls ? 'text-slate-200 hover:text-white' : 'dark:text-slate-300'}`}
                style={{ 
                  backgroundColor: (miniPlayerThemeColor && !useDarkControls) ? setAlpha(miniPlayerThemeColor, 0.1) : undefined,
                  color: (miniPlayerThemeColor && !useDarkControls) ? (isLight(miniPlayerThemeColor) ? '#475569' : setAlpha(miniPlayerThemeColor, 0.8)) : undefined
                }}
              >
                {playbackSpeed}x
              </button>
              <button 
                onClick={() => setIsCollapsed(true)} 
                className={`transition-colors p-1 hover:scale-110 ${useDarkControls ? 'text-slate-200 hover:text-white' : 'text-slate-400 dark:text-slate-300'}`}
                style={{ color: (miniPlayerThemeColor && !useDarkControls) ? (isLight(miniPlayerThemeColor) ? '#475569' : setAlpha(miniPlayerThemeColor, 0.6)) : undefined }}
                title="收起播放器"
              >
                <ChevronLeft size={20} />
              </button>
              <button 
                onClick={() => setIsExpanded(true)} 
                className={`transition-colors p-1 hover:scale-110 ${useDarkControls ? 'text-slate-200 hover:text-white' : 'text-slate-400 dark:text-slate-300'}`}
                style={{ color: (miniPlayerThemeColor && !useDarkControls) ? (isLight(miniPlayerThemeColor) ? '#475569' : setAlpha(miniPlayerThemeColor, 0.6)) : undefined }}
                title="展开播放器"
              >
                <Maximize2 size={20} />
              </button>
            </div>
          </div>
          )}
        </div>
      )}

      {/* Expanded Player View */}
      {isExpanded && (
        <div 
          className="absolute inset-0 flex flex-col p-4 sm:p-8 md:p-12 overflow-y-auto animate-in slide-in-from-bottom duration-500 pb-40 xl:pb-12 bg-white dark:bg-slate-950"
          style={{ backgroundColor: isWidgetMode ? (effectiveThemeColor ? toSolidColor(effectiveThemeColor) : '#1e293b') : (effectiveThemeColor ? setAlpha(effectiveThemeColor, 0.05) : undefined) }}
        >
          {/* Header */}
          <div className="flex items-center justify-between w-full max-w-[520px] mx-auto pb-3">
            <button 
              onClick={handleExitExpanded}
              className="p-2 -ml-2 rounded-full text-slate-700 dark:text-white hover:bg-white/50 dark:hover:bg-slate-800/60 transition-colors"
              title="收起播放器"
            >
              <ChevronUp size={24} className="rotate-180" />
            </button>
            <div className="flex-1 text-center px-3 min-w-0">
              <h2 className="text-sm sm:text-base font-bold dark:text-white text-[#4A3728] truncate">{currentChapter.title}</h2>
              <p className="text-[10px] sm:text-xs text-slate-500 truncate">{currentBook?.title}</p>
            </div>
            <button 
              onClick={() => setShowSettings(true)}
              className="p-2 -mr-2 rounded-full text-slate-700 dark:text-white hover:bg-white/50 dark:hover:bg-slate-800/60 transition-colors"
              title="播放设置"
            >
              <Settings size={22} />
            </button>
          </div>

          <div className="flex-1 flex flex-col items-center justify-center max-w-[520px] mx-auto w-full gap-5 sm:gap-7">
            <div className={`${expandedCoverSizeClass} rounded-[28px] sm:rounded-[36px] overflow-hidden shadow-2xl border-4 sm:border-8 border-white dark:border-slate-800 transition-all duration-500`}>
              <img 
                src={getCoverUrl(currentBook?.coverUrl, currentBook?.libraryId, currentBook?.id)} 
                alt={currentBook?.title}
                referrerPolicy="no-referrer"
                className="w-full h-full object-cover"
                onError={(e) => {
                  (e.target as HTMLImageElement).src = 'https://placehold.co/300x400?text=No+Cover';
                }}
              />
            </div>

            <div className="text-center w-full min-w-0">
              <h1 className="text-xl sm:text-2xl font-black leading-snug dark:text-white text-[#4A3728] line-clamp-2">
                {currentChapter.title}
              </h1>
              <p className="mt-1.5 text-sm text-slate-500 truncate">
                {currentBook?.narrator || currentBook?.author || currentBook?.title || '未知作者'}
              </p>
            </div>

            {error && (
              <div className="w-full rounded-2xl bg-red-500/15 border border-red-200/30 px-4 py-2 text-center text-xs font-bold text-red-600 dark:text-red-200">
                {error}
              </div>
            )}

            <div className="w-full flex flex-col gap-7 sm:gap-8">
              {/* Progress Bar Section */}
              <div className="px-1 sm:px-2 order-2">
                <ProgressBar 
                  isSeeking={isSeeking}
                  seekTime={seekTime}
                  currentTime={currentTime}
                  duration={duration}
                  bufferedTime={bufferedTime}
                  themeColor={effectiveThemeColor || '#60a5fa'}
                  onSeek={handleSeek}
                  onSeekStart={handleSeekStart}
                  onSeekEnd={handleSeekEnd}
                />
                <div className="mt-3 grid grid-cols-[auto_auto_1fr_auto_auto] items-center gap-3 text-xs font-bold text-slate-500 dark:text-slate-400">
                  <button
                    onClick={() => seekToTime(currentTime - 15)}
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
                    onClick={() => seekToTime(currentTime + 15)}
                    className="relative w-9 h-9 rounded-full hover:bg-white/50 dark:hover:bg-slate-800/60 transition-colors text-slate-600 dark:text-slate-300"
                    title="快进 15 秒"
                  >
                    <RotateCw size={27} strokeWidth={2.2} className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2" />
                    <span className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-[46%] text-[9px] font-black leading-none tabular-nums">15</span>
                  </button>
                </div>
              </div>

              {/* Main Controls */}
              <div className="flex items-center justify-center gap-6 sm:gap-8 order-3">
                <button 
                  onClick={prevChapter}
                  className="w-14 h-14 sm:w-16 sm:h-16 rounded-full bg-white/60 dark:bg-slate-800/60 text-slate-900 dark:text-white flex items-center justify-center backdrop-blur-sm shadow-lg shadow-black/5 hover:bg-white/80 dark:hover:bg-slate-800 hover:scale-105 active:scale-95 transition-all"
                >
                  <SkipBack size={26} fill="currentColor" />
                </button>
                
                <button
                  onClick={togglePlay}
                  className={`w-20 h-20 sm:w-24 sm:h-24 rounded-full text-white flex items-center justify-center shadow-2xl transform hover:scale-105 active:scale-95 transition-all ${!effectiveThemeColor ? 'bg-primary-600' : ''}`}
                  style={effectiveThemeColor ? { 
                    backgroundColor: toSolidColor(effectiveThemeColor),
                    color: isLight(effectiveThemeColor) ? '#475569' : '#ffffff'
                  } : {}}
                >
                  {isPlaying ? <Pause size={32} className="sm:w-12 sm:h-12" fill="currentColor" /> : <Play size={32} className="sm:w-12 sm:h-12 ml-1 sm:ml-2" fill="currentColor" />}
                </button>

                <button 
                  onClick={nextChapter}
                  className="w-14 h-14 sm:w-16 sm:h-16 rounded-full bg-white/60 dark:bg-slate-800/60 text-slate-900 dark:text-white flex items-center justify-center backdrop-blur-sm shadow-lg shadow-black/5 hover:bg-white/80 dark:hover:bg-slate-800 hover:scale-105 active:scale-95 transition-all"
                >
                  <SkipForward size={26} fill="currentColor" />
                </button>
              </div>

              {/* Bottom Row Controls */}
              <div className="grid grid-cols-4 items-start gap-1 sm:gap-2 w-full text-slate-600 dark:text-slate-400 order-1">
                <button 
                  onClick={() => setPlaybackSpeed(playbackSpeed >= 2 ? 0.5 : playbackSpeed + 0.25)}
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
                      setShowVolumeControl(!showVolumeControl);
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
                    <div 
                      className="absolute bottom-full mb-4 left-1/2 -translate-x-1/2 bg-white dark:bg-slate-800 shadow-2xl rounded-full py-4 border border-slate-100 dark:border-slate-700 w-12 flex flex-col items-center gap-3 z-[220] animate-in zoom-in-95 duration-200 cursor-default"
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
                            setVolume(parseFloat(e.target.value));
                            if (isMuted && parseFloat(e.target.value) > 0) setIsMuted(false);
                          }}
                          className="absolute w-24 h-1.5 bg-slate-200 dark:bg-slate-700 rounded-lg appearance-none cursor-pointer accent-primary-600 -rotate-90 hover:accent-primary-500"
                        />
                      </div>

                      <button
                        onClick={() => setIsMuted(!isMuted)}
                        className={`p-2 rounded-full transition-colors ${
                          isMuted 
                            ? 'bg-primary-100 text-primary-600 dark:bg-primary-900/30' 
                            : 'text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-700'
                        }`}
                        title={isMuted ? "取消静音" : "静音"}
                      >
                        {isMuted ? <VolumeX size={16} /> : <Volume2 size={16} />}
                      </button>
                    </div>
                  )}
                </div>

                <div className="relative" ref={timerMenuRef}>
                  <button 
                    onClick={() => setShowSleepTimer(!showSleepTimer)}
                    className="w-full flex flex-col items-center gap-1.5 transition-all active:scale-95 group"
                    title="睡眠定时"
                  >
                    <div className="w-10 h-10 rounded-2xl bg-white/50 dark:bg-slate-800/60 flex items-center justify-center group-hover:bg-white/70 dark:group-hover:bg-slate-800 transition-colors">
                      <Clock size={18} className={sleepTimer ? 'text-primary-600' : ''} />
                    </div>
                    <span className="text-[10px] sm:text-xs font-bold leading-none whitespace-nowrap">
                      {sleepTimer ? `${Math.floor(sleepTimer / 60)}:${(sleepTimer % 60).toString().padStart(2, '0')}` : '定时'}
                    </span>
                  </button>
                  
                  {showSleepTimer && (
                    <div className="absolute bottom-full mb-4 right-0 bg-white dark:bg-slate-800 shadow-2xl rounded-2xl p-3 sm:p-4 border border-slate-100 dark:border-slate-700 min-w-[180px] sm:min-w-[200px] flex flex-col gap-2 z-[220] animate-in zoom-in-95 duration-200">
                      <div className="px-2 py-1 text-[10px] font-bold text-slate-400 uppercase tracking-widest border-b border-slate-100 dark:border-slate-700 mb-1 text-center">
                        睡眠定时
                      </div>
                      <div className="grid grid-cols-2 gap-2">
                        {[15, 30, 45, 60].map(mins => (
                          <button
                            key={mins}
                            onClick={() => {
                              const duration = mins * 60;
                              const endTime = Date.now() + duration * 1000;
                              sleepTimerEndTimeRef.current = endTime;
                              setSleepTimer(duration);
                              setShowSleepTimer(false);
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
                              setCustomMinutes(val);
                            }
                          }}
                          placeholder="自定义分钟"
                          className="flex-1 bg-transparent border-none outline-none px-2 py-1.5 text-xs dark:text-white placeholder:text-slate-400 w-0"
                        />
                        <button
                          onClick={() => {
                            const mins = parseInt(customMinutes);
                            if (mins > 0) {
                              const duration = mins * 60;
                              const endTime = Date.now() + duration * 1000;
                              sleepTimerEndTimeRef.current = endTime;
                              setSleepTimer(duration);
                              setShowSleepTimer(false);
                              setCustomMinutes('');
                            }
                          }}
                          className="px-3 py-1.5 text-xs font-bold rounded-lg bg-primary-600 text-white hover:bg-primary-700 transition-colors shrink-0"
                        >
                          开启
                        </button>
                      </div>

                      <button
                        onClick={() => {
                          setSleepTimer(null);
                          sleepTimerEndTimeRef.current = null;
                          if (sleepTimerIntervalRef.current) {
                            clearInterval(sleepTimerIntervalRef.current);
                            sleepTimerIntervalRef.current = null;
                          }
                          setShowSleepTimer(false);
                        }}
                        className="mt-2 px-4 py-2 text-xs sm:text-sm font-bold rounded-xl bg-red-50 dark:bg-red-900/20 text-red-500 transition-colors"
                      >
                        取消定时
                      </button>
                    </div>
                  )}
                </div>

                <button
                  onClick={openChapterList}
                  className="flex flex-col items-center gap-1.5 transition-all active:scale-95 group"
                  title="章节列表"
                >
                  <div className="w-10 h-10 rounded-2xl bg-white/50 dark:bg-slate-800/60 flex items-center justify-center group-hover:bg-white/70 dark:group-hover:bg-slate-800 transition-colors">
                    <ListMusic size={18} />
                  </div>
                  <span className="text-[10px] sm:text-xs font-bold leading-none">选集</span>
                </button>
              </div>
            </div>
          </div>

          {/* Settings Modal */}
          {showSettings && (
            <div className="fixed inset-0 z-[300] flex items-center justify-center p-4">
              <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" onClick={() => setShowSettings(false)}></div>
              <div className="relative w-full max-w-sm bg-white dark:bg-slate-900 rounded-[32px] shadow-2xl overflow-hidden animate-in zoom-in-95 duration-200">
                <div className="p-6 sm:p-8">
                  <div className="flex items-center justify-between mb-6">
                    <h3 className="text-xl font-bold text-slate-900 dark:text-white">播放设置</h3>
                    <button onClick={() => setShowSettings(false)} className="p-2 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-full">
                      <X size={20} className="text-slate-400" />
                    </button>
                  </div>

                  <div className="space-y-6">
                    <div className="space-y-2">
                      <label className="text-xs font-bold text-slate-500 uppercase tracking-wider flex items-center gap-2">
                        <SkipBack size={14} />
                        跳过片头 (秒)
                      </label>
                      <input 
                        type="number" 
                        value={editSkipIntro}
                        onChange={e => setEditSkipIntro(parseInt(e.target.value) || 0)}
                        className="w-full px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-2xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                        placeholder="例如: 30"
                      />
                    </div>

                    <div className="space-y-2">
                      <label className="text-xs font-bold text-slate-500 uppercase tracking-wider flex items-center gap-2">
                        <SkipForward size={14} />
                        跳过片尾 (秒)
                      </label>
                      <input 
                        type="number" 
                        value={editSkipOutro}
                        onChange={e => setEditSkipOutro(parseInt(e.target.value) || 0)}
                        className="w-full px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-2xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                        placeholder="例如: 15"
                      />
                    </div>
                  </div>

                  <div className="mt-8 flex gap-3">
                    <button 
                      onClick={() => setShowSettings(false)}
                      className="flex-1 py-3.5 font-bold text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-2xl transition-all"
                    >
                      取消
                    </button>
                    <button 
                      onClick={handleSaveSettings}
                      className="flex-1 py-3.5 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-2xl shadow-lg shadow-primary-500/30 flex items-center justify-center gap-2 transition-all"
                    >
                      <Check size={20} />
                      保存
                    </button>
                  </div>
                </div>
              </div>
            </div>
          )}

          {/* Chapter List Drawer */}
          {showChapters && (
            <div className="fixed inset-0 z-[250] flex items-end sm:items-center justify-center">
              <div 
                className="absolute inset-0 bg-black/40 backdrop-blur-sm animate-in fade-in duration-300" 
                onClick={() => setShowChapters(false)}
              />
              <div className="relative w-full max-w-2xl bg-white dark:bg-slate-900 rounded-t-[32px] sm:rounded-[32px] h-[80vh] sm:h-[70vh] flex flex-col overflow-hidden animate-in slide-in-from-bottom duration-300 shadow-2xl">
                <div className="p-4 sm:p-6 border-b border-slate-100 dark:border-slate-800 flex items-center justify-between">
                  <div className="flex items-center gap-3 sm:gap-4">
                    <h3 className="text-lg sm:text-xl font-bold dark:text-white flex items-center gap-2">
                      <ListMusic size={24} className="text-primary-600" />
                      章节列表
                    </h3>
                    {extraChapters.length > 0 && (
                      <div className="flex bg-slate-100 dark:bg-slate-800 p-1 rounded-xl scale-90 origin-left">
                        <button 
                          onClick={() => { setActiveTab('main'); setCurrentGroupIndex(0); }}
                          className={`px-3 py-1 rounded-lg text-xs font-bold transition-all ${
                            activeTab === 'main' 
                              ? 'bg-white dark:bg-slate-700 text-primary-600 shadow-sm' 
                              : 'text-slate-500 hover:text-slate-700 dark:hover:text-slate-300'
                          }`}
                        >
                          正文
                        </button>
                        <button 
                          onClick={() => { setActiveTab('extra'); setCurrentGroupIndex(0); }}
                          className={`px-3 py-1 rounded-lg text-xs font-bold transition-all ${
                            activeTab === 'extra' 
                              ? 'bg-white dark:bg-slate-700 text-primary-600 shadow-sm' 
                              : 'text-slate-500 hover:text-slate-700 dark:hover:text-slate-300'
                          }`}
                        >
                          番外
                        </button>
                      </div>
                    )}
                  </div>
                  <button 
                    onClick={() => setShowChapters(false)}
                    className="p-2 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-full transition-colors"
                  >
                    <ChevronUp className="rotate-180" size={24} />
                  </button>
                </div>

                {groups.length > 1 && (
                  <div className="relative group/nav border-b border-slate-100 dark:border-slate-800 bg-slate-50 dark:bg-slate-800/50 flex items-center">
                    <button 
                      onClick={() => scrollGroups('left')}
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
                          onClick={() => setCurrentGroupIndex(index)}
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
                          第 {group.start}-{group.end} 章
                        </button>
                      ))}
                    </div>
                    <button 
                      onClick={() => scrollGroups('right')}
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
                    
                    return (
                      <div 
                        key={chapter.id}
                        id={`player-chapter-${chapter.id}`}
                        onClick={() => {
                          playChapter(currentBook!, currentChapters, chapter);
                          setShowChapters(false);
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
                            {chapter.chapterIndex || (actualIndex + 1)}
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
                              {getChapterProgressText(chapter) && (
                                <div 
                                  className={`text-[8px] min-[361px]:text-[9px] min-[431px]:text-[10px] font-medium px-0.5 min-[361px]:px-1 min-[431px]:px-1.5 py-0.5 rounded ${
                                    getChapterProgressText(chapter) === '已播完' 
                                      ? 'bg-green-50 text-green-500 dark:bg-green-900/20' 
                                      : 'bg-primary-50 text-primary-600 dark:bg-primary-900/20'
                                  }`}
                                >
                                  {getChapterProgressText(chapter)}
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
          )}
        </div>
      )}
    </div>
  );
};

export default Player;
