export interface Chapter {
  id: string;
  bookId: string;
  title: string;
  path: string;
  duration: number;
  chapterIndex: number;
  isExtra?: number;
  progressPosition?: number;
  progressUpdatedAt?: string;
}

export interface ChapterChange {
  index: number;
  current_title: string | null;
  scraped_title: string | null;
  status: 'match' | 'update' | 'missing' | 'new';
}