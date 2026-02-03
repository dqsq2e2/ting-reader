import { create } from 'zustand';
import type { Book, Chapter } from '../types';

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
}

export const usePlayerStore = create<PlayerState>((set, get) => ({
  currentBook: null,
  currentChapter: null,
  chapters: [],
  isPlaying: false,
  duration: 0,
  currentTime: 0,
  playbackSpeed: 1.0,
  volume: 1.0,
  themeColor: '#F2EDE4', // Default background color

  setIsPlaying: (isPlaying) => set({ isPlaying }),

  playBook: (book, chapters, startChapterId) => {
    // If no startChapterId is provided, find the most recently played chapter
    let chapter;
    if (startChapterId) {
      chapter = chapters.find(c => c.id === startChapterId) || chapters[0];
    } else {
      // Sort by progress_updated_at descending and take the first one that has progress
      const playedChapters = [...chapters].filter(c => (c as any).progress_updated_at);
      if (playedChapters.length > 0) {
        playedChapters.sort((a, b) => {
          return new Date((b as any).progress_updated_at).getTime() - new Date((a as any).progress_updated_at).getTime();
        });
        chapter = playedChapters[0];
      } else {
        chapter = chapters[0];
      }
    }
    
    const newState: Partial<PlayerState> = { 
      currentBook: book, 
      chapters, 
      currentChapter: chapter,
      isPlaying: true,
      currentTime: (chapter as any).progress_position || 0
    };

    if (book.theme_color) {
      newState.themeColor = book.theme_color;
    }

    set(newState);
  },

  togglePlay: () => set((state) => ({ isPlaying: !state.isPlaying })),
  
  setCurrentTime: (time) => set({ currentTime: time }),
  
  setDuration: (duration) => set({ duration }),
  
  setPlaybackSpeed: (speed) => set({ playbackSpeed: speed }),
  
  setVolume: (volume) => set({ volume }),

  setThemeColor: (color) => set({ themeColor: color }),

  nextChapter: () => {
    const { currentChapter, chapters } = get();
    if (!currentChapter) return;
    const index = chapters.findIndex(c => c.id === currentChapter.id);
    if (index < chapters.length - 1) {
      const nextChapter = chapters[index + 1];
      set({ 
        currentChapter: nextChapter, 
        currentTime: (nextChapter as any).progress_position || 0 
      });
    }
  },

  prevChapter: () => {
    const { currentChapter, chapters } = get();
    if (!currentChapter) return;
    const index = chapters.findIndex(c => c.id === currentChapter.id);
    if (index > 0) {
      const prevChapter = chapters[index - 1];
      set({ 
        currentChapter: prevChapter, 
        currentTime: (prevChapter as any).progress_position || 0 
      });
    }
  },

  playChapter: (book, chapters, chapter, resumePosition) => {
    const newState: Partial<PlayerState> = { 
      currentBook: book, 
      chapters, 
      currentChapter: chapter, 
      isPlaying: true, 
      currentTime: resumePosition !== undefined ? resumePosition : ((chapter as any).progress_position || 0)
    };

    if (book.theme_color) {
      newState.themeColor = book.theme_color;
    }

    set(newState);
  },
}));
