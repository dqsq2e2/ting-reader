export interface Progress {
  id: string;
  book_id: string;
  chapter_id: string;
  position: number;
  duration?: number;
  updated_at: string;
  book_title?: string;
  chapter_title?: string;
  cover_url?: string;
  library_id?: string;
  chapter_duration?: number;
}

export interface Stats {
  total_books: number;
  total_chapters: number;
  total_duration: number;
  last_scan_time?: string;
}
