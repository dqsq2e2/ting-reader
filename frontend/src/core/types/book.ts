export interface Book {
  id: string;
  library_id: string;
  title: string;
  author?: string;
  narrator?: string;
  description?: string;
  cover_url?: string;
  duration?: number;
  size?: number;
  theme_color?: string;
  path: string;
  hash: string;
  created_at: string;
  updated_at?: string;
  is_favorite?: boolean;
  library_type?: 'webdav' | 'local' | 'rss';
  skip_intro?: number;
  skip_outro?: number;
  tags?: string;
  genre?: string;
  year?: number;
  chapter_regex?: string;
}

export interface BookMetadata {
  title: string;
  author: string;
  narrator: string;
  description: string;
  cover_url: string;
  tags?: string[];
  genre?: string;
}
