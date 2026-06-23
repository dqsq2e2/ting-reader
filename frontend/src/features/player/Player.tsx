import React, { useRef, useEffect, useState } from 'react';
import { useLocation } from 'react-router-dom';
import { usePlayerStore } from '../../core/stores/playerStore';
import { useAuthStore } from '../../core/stores/authStore';
import { useWebSocket } from '../../core/hooks/useWebSocket';
import apiClient from '../../core/api/client';
import type { Chapter } from '../../core/types';
import { sortChaptersForPlayback } from '../../core/utils/chapter';
import { setAlpha, toSolidColor, isTooLight } from '../../core/utils/color';
import { useBookshelfCoverShape } from '../../core/hooks/useBookshelfCoverShape';
import ProgressBar from './ProgressBar';
import { isAppleMobileBrowser, isStrmPath } from './platform';
import PlayerSettingsModal from './PlayerSettingsModal';
import ChapterListDrawer from './ChapterListDrawer';
import {
  useIsDarkMode,
  useThemeColorSync,
  useChapterGroups,
  useSleepTimer,
  useOutsideClickClose,
  useWidgetFullscreen,
  useStuckDecodeDetector,
  useNextChapterPreloader,
  useProgressSync,
  getPlayerCoverSizes,
  getBufferedEndAt,
  formatPlayerTime,
  getChapterProgressText,
  CHAPTERS_PER_GROUP,
} from './playerHelpers';
import {
  CollapsedPlayerView,
  ExpandedPlayerHeader,
  ExpandedMainControls,
  ExpandedBottomControls,
  ExpandedProgressSection,
  ExpandedCoverAndMeta,
  MiniPlayerBookInfo,
  MiniPlayerDesktopControls,
  MiniPlayerDesktopExtras,
  MiniPlayerMobileControls,
} from './PlayerPieces';

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
    const explicitSeekOffset = currentChapter?.id === chapterId ? seekOffset : null;
    const requestSeekOffset = explicitSeekOffset !== null ? explicitSeekOffset : initialOffset;
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

  const isDark = useIsDarkMode();

  const effectiveThemeColor = themeColor && !isTooLight(themeColor) ? themeColor : undefined;
  const {
    collapsed: collapsedCoverSizeClass,
    mini: miniCoverSizeClass,
    expanded: expandedCoverSizeClass,
  } = getPlayerCoverSizes(coverShape);
  // Always use the theme color for the mini player progress bar, even in dark mode
  const miniPlayerThemeColor = effectiveThemeColor;
  // Determine if we should use dark mode text colors (white/gray) for controls
  // In dark mode, we always want bright white/gray for contrast
  const useDarkControls = isDark;

  // Sync theme color from current book (stored value preferred, fallback to FAC extraction).
  useThemeColorSync(currentBook);

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

  const { extraChapters, currentChapters, groups } = useChapterGroups(chapters, activeTab);
  const chaptersPerGroup = CHAPTERS_PER_GROUP;

  const { sleepTimer, startSleepTimer, cancelSleepTimer } = useSleepTimer({
    isPlaying,
    onExpire: togglePlay,
  });
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
  const hlsRequestIdRef = useRef(0);
  const shouldUseHlsForCurrentChapter = isAppleMobileBrowser() && isStrmPath(currentChapter?.path) && !shouldTranscode;
  const isUsingHlsForCurrentChapter = shouldUseHlsForCurrentChapter && hlsChapterId === currentChapter?.id && !!hlsStreamUrl;
  const getTranscodeStartOffset = () => {
    if (!shouldTranscode) return 0;
    if (seekOffset !== null) return Math.max(0, seekOffset);
    return streamStartOffsetRef.current.chapterId === currentChapter?.id
      ? Math.max(0, streamStartOffsetRef.current.offset)
      : 0;
  };
  const getMediaOffset = () => (
    isUsingHlsForCurrentChapter ? hlsSeekOffset : getTranscodeStartOffset()
  );

  const tryTranscodeFallback = () => {
    if (shouldTranscode || retryCount >= 3) return;
    const chapterStartOffset = streamStartOffsetRef.current.chapterId === currentChapter?.id
      ? streamStartOffsetRef.current.offset
      : 0;
    const fallbackOffset = Math.max(
      0,
      isInitialLoadRef.current ? chapterStartOffset : currentTime,
    );
    setSeekOffset(fallbackOffset);
    setCurrentTime(fallbackOffset);
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

  // Close timer / volume menus when clicking outside.
  useOutsideClickClose([timerMenuRef, volumeControlRef], (ref) => {
    if (ref === timerMenuRef) setShowSleepTimer(false);
    if (ref === volumeControlRef) setShowVolumeControl(false);
  });

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
  useStuckDecodeDetector({
    isPlaying,
    audioRef,
    chapterId: currentChapter?.id,
    shouldTranscode,
    retryCount,
    onStuck: tryTranscodeFallback,
  });

  // Preload and Server-side Cache next chapter logic
  useNextChapterPreloader({
    autoPreload,
    autoCache,
    bookId: currentBook?.id,
    chapterId: currentChapter?.id,
    getNextStreamUrl: getStreamUrl,
  });

  // Handle Skip Intro and Outro
  const handleTimeUpdate = () => {
    if (!audioRef.current) return;

    const rawTime = audioRef.current.currentTime;
    // Server-side seeked streams start from 0 but represent audio at an offset.
    const mediaOffset = getMediaOffset();
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
      setBufferedTime(getBufferedEndAt(audioRef.current.buffered, rawTime) + mediaOffset);
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
      const mediaOffset = getMediaOffset();
      setBufferedTime(getBufferedEndAt(audioRef.current.buffered, rawTime) + mediaOffset);
    }
  };

  useEffect(() => {
    if (!audioRef.current) return;
    audioRef.current.playbackRate = playbackSpeed;
  }, [playbackSpeed]);

  useEffect(() => {
    if (!audioRef.current) return;
    audioRef.current.volume = isMuted ? 0 : volume;
  }, [volume, isMuted]);

  // Sync progress to backend via WebSocket (primary) and HTTP (fallback);
  // also flushes once immediately on pause.
  useProgressSync({
    isPlaying,
    bookId: currentBook?.id,
    chapterId: currentChapter?.id,
    currentTime,
    wsSendProgress,
  });

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
        const mediaOffset = getMediaOffset();
        if (resumePosition > 0) {
          // If progress is very close to the end (e.g., within 2 seconds or > 99%), start from the beginning
          if (browserDuration > 0 && (browserDuration - resumePosition < 2 || resumePosition / browserDuration > 0.99)) {
            console.log(`Chapter ${currentChapter?.title} 已完成，从头开始`);
            if (mediaOffset > 0) {
              setSeekOffset(0);
            } else {
              audioRef.current.currentTime = 0;
            }
            setCurrentTime(0);
          } else if (mediaOffset > 0) {
            // The backend already sought before transcoding. This element's
            // timeline starts at zero, so applying the absolute position again
            // can seek past the shortened stream.
            setCurrentTime(mediaOffset);
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

  const handleSeek = (e: React.SyntheticEvent<HTMLInputElement>) => {
    const time = parseFloat((e.target as HTMLInputElement).value);
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

  const handleSeekEnd = (e: React.SyntheticEvent<HTMLInputElement>) => {
    const time = parseFloat((e.target as HTMLInputElement).value);
    setIsSeeking(false);
    seekToTime(time);
  };

  const formatTime = formatPlayerTime;

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
  const { toggleFullscreen, exitExpanded: handleExitExpanded } = useWidgetFullscreen({
    isWidgetMode,
    setIsExpanded,
  });

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
            <CollapsedPlayerView
              book={currentBook}
              coverSizeClass={collapsedCoverSizeClass}
              themeColor={miniPlayerThemeColor}
              onExpandCollapsed={() => setIsCollapsed(false)}
            />
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
            <MiniPlayerBookInfo
              book={currentBook}
              chapterTitle={currentChapter.title}
              coverSizeClass={miniCoverSizeClass}
              isWidgetMode={isWidgetMode}
              onCoverClick={toggleFullscreen}
            />

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
            <MiniPlayerDesktopControls
              isPlaying={isPlaying}
              currentTime={currentTime}
              duration={duration}
              themeColor={miniPlayerThemeColor}
              effectiveThemeColor={effectiveThemeColor}
              useDarkControls={useDarkControls}
              formatTime={formatTime}
              onPrev={prevChapter}
              onNext={nextChapter}
              onTogglePlay={togglePlay}
              onSeekTo={seekToTime}
              isSeeking={isSeeking}
              seekTime={seekTime}
              bufferedTime={bufferedTime}
              onSeek={handleSeek}
              onSeekStart={handleSeekStart}
              onSeekEnd={handleSeekEnd}
            />

            {/* Mobile Controls - Only visible on small screens */}
            <MiniPlayerMobileControls
              isPlaying={isPlaying}
              isWidgetMode={isWidgetMode}
              themeColor={miniPlayerThemeColor}
              effectiveThemeColor={effectiveThemeColor}
              useDarkControls={useDarkControls}
              currentTime={currentTime}
              duration={duration}
              bufferedTime={bufferedTime}
              isSeeking={isSeeking}
              seekTime={seekTime}
              onSeek={handleSeek}
              onSeekStart={handleSeekStart}
              onSeekEnd={handleSeekEnd}
              onTogglePlay={togglePlay}
              onPrev={prevChapter}
              onNext={nextChapter}
              onSeekTo={seekToTime}
              onCollapse={() => setIsCollapsed(true)}
            />

            {/* Desktop Extra Controls - Visible on Tablet and Desktop */}
            <MiniPlayerDesktopExtras
              volume={volume}
              isMuted={isMuted}
              showVolumeControl={showVolumeControl}
              volumeControlRef={!isExpanded ? volumeControlRef : { current: null }}
              themeColor={miniPlayerThemeColor}
              useDarkControls={useDarkControls}
              playbackSpeed={playbackSpeed}
              onToggleVolumeControl={() => setShowVolumeControl(!showVolumeControl)}
              onChangeVolume={setVolume}
              onToggleMuted={() => setIsMuted(!isMuted)}
              onCyclePlaybackSpeed={() => setPlaybackSpeed(playbackSpeed === 2 ? 1 : playbackSpeed + 0.25)}
              onCollapse={() => setIsCollapsed(true)}
              onExpand={() => setIsExpanded(true)}
            />
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
          <ExpandedPlayerHeader
            chapterTitle={currentChapter.title}
            bookTitle={currentBook?.title}
            onExit={handleExitExpanded}
            onOpenSettings={() => setShowSettings(true)}
          />

          <div className="flex-1 flex flex-col items-center justify-center max-w-[520px] mx-auto w-full gap-5 sm:gap-7">
            <ExpandedCoverAndMeta
              book={currentBook}
              chapterTitle={currentChapter.title}
              expandedCoverSizeClass={expandedCoverSizeClass}
              error={error}
            />

            <div className="w-full flex flex-col gap-7 sm:gap-8">
              {/* Progress Bar Section */}
              <ExpandedProgressSection
                currentTime={currentTime}
                duration={duration}
                bufferedTime={bufferedTime}
                isSeeking={isSeeking}
                seekTime={seekTime}
                themeColor={effectiveThemeColor || '#60a5fa'}
                formatTime={formatTime}
                onSeek={handleSeek}
                onSeekStart={handleSeekStart}
                onSeekEnd={handleSeekEnd}
                onSeekTo={seekToTime}
              />

              <ExpandedMainControls
                isPlaying={isPlaying}
                themeColor={effectiveThemeColor}
                onPrev={prevChapter}
                onTogglePlay={togglePlay}
                onNext={nextChapter}
              />

              <ExpandedBottomControls
                playbackSpeed={playbackSpeed}
                onCyclePlaybackSpeed={() => setPlaybackSpeed(playbackSpeed >= 2 ? 0.5 : playbackSpeed + 0.25)}
                volume={volume}
                isMuted={isMuted}
                showVolumeControl={showVolumeControl}
                volumeControlRef={volumeControlRef}
                onToggleShowVolumeControl={() => setShowVolumeControl(!showVolumeControl)}
                onChangeVolume={setVolume}
                onToggleMuted={() => setIsMuted(!isMuted)}
                sleepTimer={sleepTimer}
                showSleepTimer={showSleepTimer}
                customMinutes={customMinutes}
                timerMenuRef={timerMenuRef}
                onToggleShowSleepTimer={() => setShowSleepTimer(!showSleepTimer)}
                onSetCustomMinutes={setCustomMinutes}
                onStartSleepTimer={startSleepTimer}
                onCancelSleepTimer={cancelSleepTimer}
                onCloseSleepTimer={() => setShowSleepTimer(false)}
                onOpenChapterList={openChapterList}
              />
            </div>
          </div>

          {/* Settings Modal */}
          {showSettings && (
            <PlayerSettingsModal
              editSkipIntro={editSkipIntro}
              editSkipOutro={editSkipOutro}
              onChangeSkipIntro={setEditSkipIntro}
              onChangeSkipOutro={setEditSkipOutro}
              onClose={() => setShowSettings(false)}
              onSave={handleSaveSettings}
            />
          )}

          {/* Chapter List Drawer */}
          <ChapterListDrawer
            show={showChapters}
            currentBook={currentBook}
            currentChapter={currentChapter}
            currentChapters={currentChapters}
            groups={groups}
            chaptersPerGroup={chaptersPerGroup}
            currentGroupIndex={currentGroupIndex}
            activeTab={activeTab}
            extraChapters={extraChapters}
            isPlaying={isPlaying}
            effectiveThemeColor={effectiveThemeColor}
            scrollRef={scrollRef}
            onClose={() => setShowChapters(false)}
            onSetActiveTab={setActiveTab}
            onSetCurrentGroupIndex={setCurrentGroupIndex}
            onScrollGroups={scrollGroups}
            onPlayChapter={(chapter) => playChapter(currentBook!, currentChapters, chapter)}
            formatTime={formatTime}
            getChapterProgressText={getChapterProgressText}
          />
        </div>
      )}
    </div>
  );
};

export default Player;
