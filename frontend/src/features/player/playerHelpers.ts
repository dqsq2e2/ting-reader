import { useEffect, useMemo, useRef, useState } from 'react';
import { FastAverageColor } from 'fast-average-color';
import apiClient from '../../core/api/client';
import { usePlayerStore } from '../../core/stores/playerStore';
import { getCoverUrl } from '../../core/utils/image';
import type { Book } from '../../core/types';
import type { CoverShape } from '../../core/hooks/useBookshelfCoverShape';

// ─── useIsDarkMode ──────────────────────────────────────────────────────────
// 通过 MutationObserver 监听 <html> 的 class 变化，实时反映当前是否处于 dark 模式。
// 与 useTheme 区别：useTheme 持有用户偏好（light/dark/system），
// 这个 hook 关心的是「当前实际渲染的是不是 dark」——例如 system 模式下需要跟随系统切换。
export const useIsDarkMode = () => {
  const [isDark, setIsDark] = useState(() => document.documentElement.classList.contains('dark'));

  useEffect(() => {
    const observer = new MutationObserver(() => {
      setIsDark(document.documentElement.classList.contains('dark'));
    });
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] });
    return () => observer.disconnect();
  }, []);

  return isDark;
};

// ─── useThemeColorSync ──────────────────────────────────────────────────────
// 同步当前书籍的主题色到 player store：
// 1. 优先使用 book.theme_color（后端/上次缓存）。
// 2. 否则用 FastAverageColor 从封面提取主色调；为避免下次重复提取，
//    会顺手把提取出的颜色回写到 store.currentBook.theme_color。
export const useThemeColorSync = (currentBook: Book | null | undefined) => {
  const setThemeColor = usePlayerStore((state) => state.setThemeColor);

  useEffect(() => {
    if (!currentBook) return;
    const color = currentBook.theme_color;
    if (color) {
      setThemeColor(color);
      return;
    }
    if (!currentBook.cover_url) return;

    const coverUrl = getCoverUrl(currentBook.cover_url, currentBook.library_id, currentBook.id);
    const fac = new FastAverageColor();
    fac
      .getColorAsync(coverUrl, { algorithm: 'dominant' })
      .then((extracted) => {
        setThemeColor(extracted.hex);
        // 把提取出的颜色塞回 store 的 currentBook，避免本会话内重复提取。
        usePlayerStore.setState((state) => ({
          currentBook: state.currentBook
            ? { ...state.currentBook, theme_color: extracted.hex }
            : null,
        }));
      })
      .catch((e) => console.warn('在播放器中从封面提取颜色失败', e));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentBook?.id, currentBook?.theme_color]);
};

// ─── useChapterGroups ───────────────────────────────────────────────────────
// 章节分组：按主线/番外划分，并切成每 100 章一组的页签数据。
// 仅依赖 chapters 和当前 tab，是纯衍生 hook。

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type ChapterLike = any;

export interface ChapterGroup {
  start: number;
  end: number;
  chapters: ChapterLike[];
}

export const CHAPTERS_PER_GROUP = 100;

export const useChapterGroups = (
  chapters: ChapterLike[],
  activeTab: 'main' | 'extra'
) => {
  const { mainChapters, extraChapters } = useMemo(() => ({
    mainChapters: chapters.filter((c) => !c.is_extra),
    extraChapters: chapters.filter((c) => c.is_extra),
  }), [chapters]);

  const currentChapters = activeTab === 'main' ? mainChapters : extraChapters;

  const groups = useMemo<ChapterGroup[]>(() => {
    const g: ChapterGroup[] = [];
    for (let i = 0; i < currentChapters.length; i += CHAPTERS_PER_GROUP) {
      const slice = currentChapters.slice(i, i + CHAPTERS_PER_GROUP);
      g.push({
        start: slice[0]?.chapter_index || (i + 1),
        end: slice[slice.length - 1]?.chapter_index || (i + slice.length),
        chapters: slice,
      });
    }
    return g;
  }, [currentChapters]);

  return { mainChapters, extraChapters, currentChapters, groups };
};

// ─── useSleepTimer ──────────────────────────────────────────────────────────
// 睡眠定时器：用 endTime 绝对时间戳驱动倒计时，避免标签页休眠导致计数失真。
// 唯一与音频耦合的点是 onExpire（到期时通常用来 togglePlay）。
export const useSleepTimer = (options: {
  isPlaying: boolean;
  onExpire: () => void;
}) => {
  const { isPlaying, onExpire } = options;
  const [sleepTimer, setSleepTimer] = useState<number | null>(null);
  const endTimeRef = useRef<number | null>(null);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // 倒计时：用 endTime 推导剩余秒数。
  useEffect(() => {
    if (sleepTimer === null || sleepTimer <= 0 || !isPlaying || !endTimeRef.current) return;

    if (intervalRef.current) clearInterval(intervalRef.current);

    const interval = setInterval(() => {
      if (endTimeRef.current) {
        const remaining = Math.max(0, Math.floor((endTimeRef.current - Date.now()) / 1000));
        setSleepTimer(remaining);
      }
    }, 1000);

    intervalRef.current = interval;

    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
    };
    // 故意只依赖 sleepTimer 是否为 null 而不是它的具体值，否则每秒都会重建 interval。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sleepTimer === null, isPlaying]);

  // 到期处理。
  useEffect(() => {
    if (sleepTimer !== 0) return;
    if (isPlaying) onExpire();
    endTimeRef.current = null;
    if (intervalRef.current) {
      clearInterval(intervalRef.current);
      intervalRef.current = null;
    }
    setTimeout(() => setSleepTimer(null), 0);
    // 故意不依赖 onExpire，避免父组件每次渲染重跑此 effect。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sleepTimer, isPlaying]);

  const startSleepTimer = (durationSeconds: number) => {
    endTimeRef.current = Date.now() + durationSeconds * 1000;
    setSleepTimer(durationSeconds);
  };

  const cancelSleepTimer = () => {
    setSleepTimer(null);
    endTimeRef.current = null;
    if (intervalRef.current) {
      clearInterval(intervalRef.current);
      intervalRef.current = null;
    }
  };

  return { sleepTimer, startSleepTimer, cancelSleepTimer };
};

// ─── useOutsideClickClose ───────────────────────────────────────────────────
// 监听 document mousedown，当点击发生在所有给定 ref 容器之外时调用 onOutsideClick。
// 用同一个监听器处理多个弹出层，避免每个弹出层各自绑定/解绑一遍 document 事件。
export const useOutsideClickClose = (
  refs: React.RefObject<HTMLElement | null>[],
  onOutsideClick: (ref: React.RefObject<HTMLElement | null>, index: number) => void,
  enabled = true,
) => {
  useEffect(() => {
    if (!enabled) return;
    const handleClickOutside = (event: MouseEvent) => {
      refs.forEach((ref, index) => {
        if (ref.current && !ref.current.contains(event.target as Node)) {
          onOutsideClick(ref, index);
        }
      });
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
    // refs/callback 故意不进依赖：业务方稳定持有 ref，回调是闭包里读最新 state。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled]);
};

// ─── useWidgetFullscreen ────────────────────────────────────────────────────
// Widget 模式下的全屏控制：
// - toggleFullscreen：进入/退出浏览器全屏，并联动 isExpanded 展开状态；
// - exitExpanded：退出展开视图，必要时先 exitFullscreen；
// - 监听 fullscreenchange，当用户按 Esc 时同步关闭 isExpanded。
// 非 widget 模式下，toggleFullscreen 退化为简单的 setIsExpanded(true)。
export const useWidgetFullscreen = (options: {
  isWidgetMode: boolean;
  setIsExpanded: (expanded: boolean) => void;
}) => {
  const { isWidgetMode, setIsExpanded } = options;

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

  const toggleFullscreen = async () => {
    if (!isWidgetMode) {
      setIsExpanded(true);
      return;
    }

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
      // 进入失败时故意不再 setIsExpanded(true)，避免 iframe 里 UI 撑破。
    }
  };

  const exitExpanded = async () => {
    if (isWidgetMode && document.fullscreenElement) {
      try {
        await document.exitFullscreen();
      } catch (err) {
        console.error('退出全屏时出错:', err);
      }
    }
    setIsExpanded(false);
  };

  return { toggleFullscreen, exitExpanded };
};

// ─── getPlayerCoverSizes ────────────────────────────────────────────────────
// 根据封面形状（方形/长方形）返回播放器三种状态（折叠 / mini / 展开）下封面的尺寸 className。
export interface PlayerCoverSizes {
  collapsed: string;
  mini: string;
  expanded: string;
}

export const getPlayerCoverSizes = (coverShape: CoverShape): PlayerCoverSizes => {
  if (coverShape === 'square') {
    return {
      collapsed: 'w-14 h-14 sm:w-16 sm:h-16',
      mini: 'w-12 h-12 max-[380px]:w-10 max-[380px]:h-10 sm:w-16 sm:h-16',
      expanded: 'w-full max-w-[240px] sm:max-w-[320px] lg:max-w-[400px] aspect-square',
    };
  }
  return {
    collapsed: 'w-12 sm:w-14 aspect-[3/4]',
    mini: 'w-10 max-[380px]:w-8 sm:w-12 aspect-[3/4]',
    expanded: 'w-full max-w-[220px] sm:max-w-[280px] lg:max-w-[320px] aspect-[3/4]',
  };
};

// ─── 缓冲区计算 ─────────────────────────────────────────────────────────────
// 给定 currentTime 和 TimeRanges，返回当前正在播放区间的 end；
// 找不到包含 currentTime 的区间就退回到「current 之前最近的一段的 end」。
// handleTimeUpdate 和 handleProgress 都用这套逻辑算 bufferedTime。
export const getBufferedEndAt = (buffered: TimeRanges, rawTime: number): number => {
  let end = 0;
  for (let i = 0; i < buffered.length; i++) {
    if (buffered.start(i) <= rawTime && buffered.end(i) >= rawTime) {
      return buffered.end(i);
    }
  }
  for (let i = buffered.length - 1; i >= 0; i--) {
    if (buffered.start(i) <= rawTime) {
      end = buffered.end(i);
      break;
    }
  }
  return end;
};

// ─── 文案格式化 ─────────────────────────────────────────────────────────────

export const formatPlayerTime = (time: number): string => {
  if (!Number.isFinite(time) || isNaN(time) || time < 0) return '0:00';
  const h = Math.floor(time / 3600);
  const m = Math.floor((time % 3600) / 60);
  const s = Math.floor(time % 60);

  if (h > 0) {
    return `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
  }
  return `${m}:${s.toString().padStart(2, '0')}`;
};

interface ChapterProgressLike {
  progress_position?: number | null;
  duration?: number | null;
}

export const getChapterProgressText = (
  chapter: ChapterProgressLike,
  labels: {
    complete: string;
    percent: (percent: number) => string;
  } = {
    complete: 'Completed',
    percent: (percent) => `Played ${percent}%`,
  }
): string | null => {
  if (!chapter.progress_position || !chapter.duration) return null;

  const percent = Math.floor((chapter.progress_position / chapter.duration) * 100);
  if (percent === 0) return null;
  if (percent >= 95) return labels.complete;
  return labels.percent(percent);
};

// ─── useStuckDecodeDetector ─────────────────────────────────────────────────
// 部分浏览器/编解码组合下，audio 元素显示 "playing"，但 currentTime 一直停在 0。
// 启动 4.5s 计时，若期间没有任何位移且 buffered/readyState 都足够，则认为是解码卡死。
// 触发的 fallback 由调用方决定（一般是切转码流）。
export const useStuckDecodeDetector = (options: {
  isPlaying: boolean;
  audioRef: React.RefObject<HTMLAudioElement | null>;
  chapterId: string | undefined;
  shouldTranscode: boolean;
  retryCount: number;
  onStuck: () => void;
}) => {
  const { isPlaying, audioRef, chapterId, shouldTranscode, retryCount, onStuck } = options;

  useEffect(() => {
    if (!isPlaying || !chapterId || !audioRef.current) return;

    const initialTime = audioRef.current.currentTime || 0;
    const timer = setTimeout(() => {
      const audio = audioRef.current;
      if (!audio || audio.paused || audio.seeking || audio.ended) return;
      if (audio.error) return;

      const time = audio.currentTime || 0;
      const progressed = time - initialTime;
      const bufferedEnd = audio.buffered.length > 0
        ? audio.buffered.end(audio.buffered.length - 1)
        : 0;

      const readyForDecode = audio.readyState >= HTMLMediaElement.HAVE_FUTURE_DATA;
      const decodeStuck = time < 0.05 && progressed < 0.03 && bufferedEnd > 1.5 && readyForDecode;
      if (decodeStuck) onStuck();
    }, 4500);

    return () => clearTimeout(timer);
    // onStuck 故意不进依赖：调用方稳定的 useRef 拿不到最新值，闭包重读即可。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isPlaying, chapterId, shouldTranscode, retryCount]);
};

// ─── useNextChapterPreloader ────────────────────────────────────────────────
// 当 autoPreload 或 autoCache 任一开启时：
//  - autoPreload：本地建一个隐藏 <audio> 提前请求下一章 URL（命中浏览器 HTTP 缓存）
//  - autoCache：调用后端 /api/cache/:id 触发 WebDAV 服务器端缓存
// 调用方负责传 getNextStreamUrl，因为 URL 构建依赖 shouldTranscode / token 等本地状态。
export const useNextChapterPreloader = (options: {
  autoPreload: boolean;
  autoCache: boolean;
  bookId: string | undefined;
  chapterId: string | undefined;
  getNextStreamUrl: (chapterId: string) => string;
}) => {
  const { autoPreload, autoCache, bookId, chapterId, getNextStreamUrl } = options;
  const preloadAudioRef = useRef<HTMLAudioElement | null>(null);

  useEffect(() => {
    if ((!autoPreload && !autoCache) || !chapterId || !bookId) return;

    apiClient.get(`/api/books/${bookId}/chapters`).then(res => {
      const list = res.data as Array<{ id: string; title?: string }>;
      const currentIndex = list.findIndex((c) => c.id === chapterId);
      if (currentIndex === -1 || currentIndex >= list.length - 1) return;
      const nextChapterId = list[currentIndex + 1].id;

      if (autoPreload) {
        if (!preloadAudioRef.current) {
          preloadAudioRef.current = new Audio();
          preloadAudioRef.current.preload = 'auto';
        }
        const nextSrc = getNextStreamUrl(nextChapterId);
        if (preloadAudioRef.current.src !== nextSrc) {
          console.log('正在预加载下一章:', list[currentIndex + 1].title);
          preloadAudioRef.current.src = nextSrc;
          preloadAudioRef.current.load();
        }
      }

      if (autoCache) {
        console.log('触发服务器端缓存:', list[currentIndex + 1].title);
        apiClient.post(`/api/cache/${nextChapterId}`).catch(err => {
          console.error('触发服务器端缓存失败', err);
        });
      }
    }).catch(err => console.error('预加载失败', err));
    // getNextStreamUrl 是组件内闭包，每次渲染都新建，故意不进依赖避免抖动。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [chapterId, autoPreload, autoCache, bookId]);
};

// ─── useProgressSync ────────────────────────────────────────────────────────
// 进度同步：播放中走 WS（2s 一次）+ HTTP（15s 一次），第一拍打 playbackStart 标记；
// 同时暂停瞬间立即 flush 一次，避免丢最后几秒进度。
// currentTime 用 ref 间接读，效避免每次时间变化都重建定时器。
export const useProgressSync = (options: {
  isPlaying: boolean;
  bookId: string | undefined;
  chapterId: string | undefined;
  currentTime: number;
  wsSendProgress: (
    bookId: string,
    chapterId: string,
    position: number,
    playbackStart?: number,
  ) => void;
}) => {
  const { isPlaying, bookId, chapterId, currentTime, wsSendProgress } = options;

  const currentTimeRef = useRef(0);
  useEffect(() => {
    currentTimeRef.current = currentTime;
  }, [currentTime]);

  const timersRef = useRef<{ ws: ReturnType<typeof setInterval>; http: ReturnType<typeof setInterval> } | null>(null);

  useEffect(() => {
    if (isPlaying && bookId && chapterId) {
      const saveWs = (playbackStart?: number) => {
        wsSendProgress(bookId, chapterId, Math.floor(currentTimeRef.current), playbackStart);
      };
      const saveHttp = (playbackStart?: number) => {
        apiClient.post('/api/progress', {
          book_id: bookId,
          chapter_id: chapterId,
          position: Math.floor(currentTimeRef.current),
          ...(playbackStart !== undefined ? { playback_start: playbackStart } : {}),
        }).catch(err => console.error('HTTP进度同步失败', err));
      };

      const start = Math.floor(currentTimeRef.current);
      saveWs(start);
      saveHttp(start);

      timersRef.current = {
        ws: setInterval(saveWs, 2000),
        http: setInterval(saveHttp, 15000),
      };
    } else if (timersRef.current) {
      clearInterval(timersRef.current.ws);
      clearInterval(timersRef.current.http);
      timersRef.current = null;
    }

    return () => {
      if (timersRef.current) {
        clearInterval(timersRef.current.ws);
        clearInterval(timersRef.current.http);
        timersRef.current = null;
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isPlaying, bookId, chapterId]);

  // 暂停瞬间 flush
  const prevIsPlayingRef = useRef(isPlaying);
  useEffect(() => {
    if (prevIsPlayingRef.current && !isPlaying && bookId && chapterId) {
      const pos = Math.floor(currentTimeRef.current);
      wsSendProgress(bookId, chapterId, pos);
      apiClient.post('/api/progress', { book_id: bookId, chapter_id: chapterId, position: pos })
        .catch(err => console.error('暂停时保存进度失败', err));
    }
    prevIsPlayingRef.current = isPlaying;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isPlaying]);
};
