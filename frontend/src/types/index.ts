export interface User {
  id: string;
  username: string;
  role: 'admin' | 'user';
  createdAt: string;
  librariesAccessible?: string[];
  booksAccessible?: string[];
}

export interface ScraperConfig {
  defaultSources?: string[];
  coverSources?: string[];
  introSources?: string[];
  authorSources?: string[];
  narratorSources?: string[];
  tagsSources?: string[];
  nfo_writing_enabled?: boolean;
  metadata_writing_enabled?: boolean;
  prefer_audio_title?: boolean;
}

export interface Library {
  id: string;
  name: string;
  libraryType: 'webdav' | 'local';
  url: string;
  username?: string;
  password?: string;
  rootPath: string;
  lastScannedAt?: string;
  scraperConfig?: ScraperConfig;
  createdAt: string;
}

export interface Book {
  id: string;
  libraryId: string;
  title: string;
  author?: string;
  narrator?: string;
  description?: string;
  coverUrl?: string;
  themeColor?: string;
  path: string;
  hash: string;
  createdAt: string;
  updatedAt?: string;
  isFavorite?: boolean;
  libraryType?: 'webdav' | 'local';
  skipIntro?: number;
  skipOutro?: number;
  tags?: string;
  genre?: string;
  chapterRegex?: string;
}

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

export interface Progress {
  bookId: string;
  chapterId: string;
  position: number;
  updatedAt: string;
  bookTitle?: string;
  chapterTitle?: string;
  coverUrl?: string;
  libraryId?: string;
  chapterDuration?: number;
}

export interface Stats {
  totalBooks: number;
  totalChapters: number;
  totalDuration: number;
  lastScanTime?: string;
}

export interface Plugin {
  id: string;
  name: string;
  version: string;
  pluginType: 'scraper' | 'format' | 'utility';
  author: string;
  description: string;
  state: 'active' | 'inactive' | 'loading' | 'failed';
  totalCalls?: number;
  successfulCalls?: number;
  failedCalls?: number;
  successRate?: number;
}

export interface StorePlugin {
  id: string;
  name: string;
  description: string;
  longDescription?: string;
  icon?: string;
  repo?: string;
  pluginType: 'scraper' | 'format' | 'utility';
  version: string;
  downloadUrl: string | Record<string, string>;
  size?: string | Record<string, string>;
  date?: string;
}

export interface MergeSuggestion {
  id: string;
  source_book_id: string;
  source_book_title: string;
  target_book_id: string;
  target_book_title: string;
  score: number;
  reason: string;
  status: 'pending' | 'merged' | 'ignored';
  created_at: string;
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

export interface ChapterChange {
  index: number;
  current_title: string | null;
  scraped_title: string | null;
  status: 'match' | 'update' | 'missing' | 'new';
}

export interface Series {
  id: string;
  libraryId: string;
  title: string;
  author?: string;
  narrator?: string;
  description?: string;
  coverUrl?: string;
  createdAt: string;
  updatedAt?: string;
  books?: Book[];
}

export interface ScrapeDiff {
  current: BookMetadata;
  scraped: BookMetadata;
  chapter_changes: ChapterChange[];
}
