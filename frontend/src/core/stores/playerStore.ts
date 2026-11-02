import { create } from 'zustand';
import type { Book, Chapter } from '../types';
import { sortChaptersForPlayback } from '../utils/chapter';
import { isTooLight } from '../utils/color';
import { loadPlayerVolume, savePlayerVolume } from '../utils/playbackPreferences';

interface PlayerState {
  currentBook: Book | null;
  currentChapter: Chapter | null;
  chapters: Chapter[];
  isPlaying: boolean;
  duration: number;
  currentTime: number;
  playbackSpeed: number;
  volume: number;
  themeColor: string;
  isExpanded: boolean;
  isCollapsed: boolean;
  isSeriesEditing: boolean;
  playbackRevision: number;

  // Actions
  playBook: (book: Book, chapters: Chapter[], startChapterId?: string) => void;
  togglePlay: () => void;
  setCurrentTime: (time: number) => void;
  setDuration: (duration: number) => void;
  setPlaybackSpeed: (speed: number) => void;
  setVolume: (volume: number) => void;
  setThemeColor: (color: string) => void;
  nextChapter: () => void;
  prevChapter: () => void;
  playChapter: (book: Book, chapters: Chapter[], chapter: Chapter, resumePosition?: number) => void;
  setIsPlaying: (isPlaying: boolean) => void;
  updateBook: (book: Book) => void;
  setIsExpanded: (isExpanded: boolean) => void;
  setIsCollapsed: (isCollapsed: boolean) => void;
  setIsSeriesEditing: (isSeriesEditing: boolean) => void;
}

/** Check if a chapter's progress indicates it has been fully played */
function isChapterFinished(chapter: Chapter): boolean {
  if (!chapter.progress_position || !chapter.duration || chapter.duration <= 0) return false;
  return chapter.progress_position / chapter.duration >= 0.95;
}

export const usePlayerStore = create<PlayerState>((set, get) => ({
      currentBook: null,
      currentChapter: null,
      chapters: [],
      isPlaying: false,
      duration: 0,
      currentTime: 0,
      playbackSpeed: 1.0,
      volume: loadPlayerVolume(),
      themeColor: '#F2EDE4', // Default background color
      isExpanded: false,
      isCollapsed: false,
      isSeriesEditing: false,
      playbackRevision: 0,

      setIsPlaying: (isPlaying) => set({ isPlaying }),
      updateBook: (book) => set(state => state.currentBook?.id === book.id
        ? { currentBook: { ...state.currentBook, ...book } } : {}),
      setIsExpanded: (isExpanded) => set({ isExpanded }),
      setIsCollapsed: (isCollapsed) => set({ isCollapsed }),
      setIsSeriesEditing: (isSeriesEditing) => set({ isSeriesEditing }),

      playBook: (book, chapters, startChapterId) => {
        const orderedChapters = sortChaptersForPlayback(chapters);
        // If no startChapterId is provided, find the most recently played chapter
        let chapter;
        if (startChapterId) {
          chapter = orderedChapters.find(c => c.id === startChapterId) || orderedChapters[0];
        } else {
          // Sort by progressUpdatedAt descending and take the first one that has progress
          const playedChapters = orderedChapters.filter(c => c.progress_updated_at);
          if (playedChapters.length > 0) {
            playedChapters.sort((a, b) => {
              return new Date(b.progress_updated_at!).getTime() - new Date(a.progress_updated_at!).getTime();
            });
            chapter = playedChapters[0];
          } else {
            chapter = orderedChapters[0];
          }
        }

        if (!chapter) return;

        // If chapter is finished, restart from beginning
        const startPos = isChapterFinished(chapter) ? 0 : (chapter.progress_position || 0);

        const newState: Partial<PlayerState> = {
          currentBook: book,
          chapters: orderedChapters,
          currentChapter: chapter,
          playbackRevision: get().playbackRevision + 1,
          isPlaying: true,
          currentTime: startPos
        };

        if (book.theme_color && !isTooLight(book.theme_color)) {
          newState.themeColor = book.theme_color;
        } else {
          newState.themeColor = '#F2EDE4'; // Reset to default
        }

        set(newState);
      },

      togglePlay: () => {
        const state = get();
        if (!state.isPlaying && state.currentBook && state.currentChapter
          && state.duration > 0 && state.currentTime >= state.duration - 0.2) {
          state.playChapter(state.currentBook, state.chapters, state.currentChapter, 0);
        } else {
          set({ isPlaying: !state.isPlaying });
        }
      },

      setCurrentTime: (time) => set({ currentTime: time }),

      setDuration: (duration) => set({ duration }),

      setPlaybackSpeed: (speed) => {
        if (Number.isFinite(speed)) {
          set({ playbackSpeed: Math.round(Math.max(0.5, Math.min(3, speed)) * 10) / 10 });
        }
      },

      setVolume: (volume) => {
        if (!Number.isFinite(volume)) return;
        const normalized = Math.max(0, Math.min(1, volume));
        savePlayerVolume(normalized);
        set({ volume: normalized });
      },

      setThemeColor: (color) => set({ themeColor: color }),

      nextChapter: () => {
        const { currentChapter, chapters, currentBook } = get();
        if (!currentChapter || !currentBook) return;

        // 确保 chapters 数组不为空且包含当前章节
        if (chapters.length === 0 || !chapters.some(c => c.id === currentChapter.id)) {
          // 如果 chapters 数组为空或不包含当前章节，直接返回
          // 这种情况通常发生在 PWA 恢复时，需要等待章节数据加载
          console.warn('Chapters array is empty or does not contain current chapter, cannot proceed to next chapter');
          return;
        }

        const index = chapters.findIndex(c => c.id === currentChapter.id);
        if (index !== -1 && index < chapters.length - 1) {
          const nextChapter = chapters[index + 1];
          get().playChapter(currentBook, chapters, nextChapter);
        }
      },

      prevChapter: () => {
        const { currentChapter, chapters } = get();
        if (!currentChapter) return;
        const index = chapters.findIndex(c => c.id === currentChapter.id);
        if (index > 0) {
          const prevChapter = chapters[index - 1];
          get().playChapter(get().currentBook!, chapters, prevChapter);
        }
      },

      playChapter: (book, chapters, chapter, resumePosition) => {
        const orderedChapters = sortChaptersForPlayback(chapters);
        const orderedChapter = orderedChapters.find(c => c.id === chapter.id) || chapter;
        let startPos: number;
        if (resumePosition !== undefined) {
          startPos = resumePosition;
        } else if (isChapterFinished(orderedChapter)) {
          // Clicking a finished chapter clears its progress and restarts from beginning
          startPos = 0;
        } else {
          startPos = orderedChapter.progress_position || 0;
        }

        const newState: Partial<PlayerState> = {
          currentBook: book,
          chapters: orderedChapters,
          currentChapter: orderedChapter,
          playbackRevision: get().playbackRevision + 1,
          isPlaying: true,
          currentTime: startPos
        };

        if (book.theme_color && !isTooLight(book.theme_color)) {
          newState.themeColor = book.theme_color;
        } else {
          newState.themeColor = '#F2EDE4'; // Reset to default
        }

        set(newState);
      }
}));
