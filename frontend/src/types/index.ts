export interface User {
  id: string;
  username: string;
  role: 'admin' | 'user';
  created_at: string;
  librariesAccessible?: string[];
  booksAccessible?: string[];
}

export interface Library {
  id: string;
  name: string;
  type: 'webdav' | 'local';
  url: string;
  username?: string;
  password?: string;
  root_path: string;
  last_scanned_at?: string;
}

export interface Book {
  id: string;
  library_id: string;
  title: string;
  author?: string;
  narrator?: string;
  description?: string;
  cover_url?: string;
  theme_color?: string;
  path: string;
  book_hash: string;
  created_at: string;
  updated_at: string;
  is_favorite?: boolean;
  library_type?: 'webdav' | 'local';
  skip_intro?: number;
  skip_outro?: number;
  tags?: string;
}

export interface Chapter {
  id: string;
  book_id: string;
  title: string;
  path: string;
  duration: number;
  chapter_index: number;
  is_extra?: number;
  progress_position?: number;
}

export interface Progress {
  book_id: string;
  chapter_id: string;
  position: number;
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
