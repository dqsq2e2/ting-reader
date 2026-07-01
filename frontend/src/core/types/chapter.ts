export interface Chapter {
  id: string;
  book_id: string;
  title: string;
  path: string;
  duration: number;
  chapter_index: number;
  is_extra?: number;
  progress_position?: number;
  progress_updated_at?: string;
}

export interface ChapterChange {
  index: number;
  current_title: string | null;
  scraped_title: string | null;
  status: 'match' | 'update' | 'missing' | 'new';
}
