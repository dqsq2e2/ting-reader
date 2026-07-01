import type { Chapter } from '../../../core/types';

export type ChapterTab = 'main' | 'extra';

export interface ChapterGroup {
  start: number;
  end: number;
  index: number;
}

export interface ChapterEditDraft {
  title: string;
  chapterIndex: number;
  isExtra: boolean;
}

export type EditableChapter = Chapter;
