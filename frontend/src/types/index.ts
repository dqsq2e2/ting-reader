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
  metadataPriority?: string[];
  extractAudioCover?: boolean;
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
  duration?: number;
  size?: number;
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
  year?: number;
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

export interface AdminStatisticsOverview {
  totalBooks: number;
  totalChapters: number;
  totalDuration: number;
  totalLibraries: number;
  localLibraries: number;
  webdavLibraries: number;
  totalUsers: number;
  adminUsers: number;
  activeUsers: number;
  totalProgressRecords: number;
  totalListenSeconds: number;
}

export interface LibraryStatistics {
  id: string;
  name: string;
  libraryType: string;
  totalBooks: number;
  totalChapters: number;
  totalDuration: number;
  lastScannedAt?: string;
}

export interface UserActivityStatistics {
  id: string;
  username: string;
  role: 'admin' | 'user' | string;
  listenedBooks: number;
  progressRecords: number;
  listenSeconds: number;
  lastActiveAt?: string;
}

export interface RecentActivityPoint {
  date: string;
  activeUsers: number;
  progressUpdates: number;
  listenSeconds: number;
}

export interface BookActivityStatistics {
  id: string;
  title?: string;
  author?: string;
  libraryId: string;
  libraryName?: string;
  listeners: number;
  progressUpdates: number;
  listenSeconds: number;
}

export interface AdminStatistics {
  overview: AdminStatisticsOverview;
  libraryBreakdown: LibraryStatistics[];
  userActivity: UserActivityStatistics[];
  recentActivity: RecentActivityPoint[];
  topBooks: BookActivityStatistics[];
  generatedAt: string;
}

export interface NotificationEventOption {
  id: string;
  label: string;
  description: string;
}

export interface NotificationWebhook {
  id: string;
  name: string;
  url: string;
  enabled: boolean;
  events: string[];
  secret?: string;
  createdAt: string;
  updatedAt: string;
}

export interface PluginDependency {
  pluginName: string;
  versionRequirement: string;
}

export interface PluginStats {
  totalCalls: number;
  successfulCalls: number;
  failedCalls: number;
  avgExecutionTimeMs: number;
}

export interface Plugin {
  id: string;
  name: string;
  version: string;
  pluginType: 'scraper' | 'format' | 'utility';
  author: string;
  description: string;
  state: 'active' | 'inactive' | 'loading' | 'failed';
  runtime?: string;
  license?: string;
  repo?: string;
  descriptionEn?: string;
  isEnabled?: boolean;
  entryPoint?: string;
  dependencies?: PluginDependency[];
  permissions?: string[];
  configSchema?: Record<string, unknown>;
  supportedExtensions?: string[];
  totalCalls?: number;
  successfulCalls?: number;
  failedCalls?: number;
  successRate?: number;
  stats?: PluginStats;
  error?: string;
  scraper?: {
    autoScrape?: boolean;
    searchFields?: ScraperSearchField[];
    resultFields?: string[];
  };
}

export interface ScraperSearchField {
  key: string;
  label: string;
  required?: boolean;
  type?: string;
  fieldType?: string;
  placeholder?: string;
  defaultFrom?: string;
}

export interface ScraperSource {
  id: string;
  name: string;
  description?: string;
  version: string;
  enabled: boolean;
  autoScrape: boolean;
  searchFields: ScraperSearchField[];
  resultFields: string[];
}

export interface ScraperSearchItem {
  id: string;
  title?: string;
  author?: string;
  narrator?: string | null;
  coverUrl?: string | null;
  cover_url?: string | null;
  intro?: string | null;
  description?: string | null;
  tags?: string[];
  genre?: string | null;
  subtitle?: string | null;
  publishedYear?: string | null;
  published_year?: string | null;
  publishedDate?: string | null;
  published_date?: string | null;
  publisher?: string | null;
  isbn?: string | null;
  asin?: string | null;
  language?: string | null;
  explicit?: boolean | null;
  abridged?: boolean | null;
  duration?: number | null;
  [key: string]: unknown;
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
  dependencies?: string[];
  runtime?: string;
  license?: string;
  author?: string;
  descriptionEn?: string;
  permissions?: string[];
  configSchema?: Record<string, unknown>;
  supportedExtensions?: string[];
  minCoreVersion?: string;
  downloads?: { name: string; url: string }[];
  scraper?: {
    autoScrape?: boolean;
    searchFields?: ScraperSearchField[];
    resultFields?: string[];
  };
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

export interface Playlist {
  id: string;
  userId: string;
  title: string;
  description?: string;
  createdAt: string;
  updatedAt: string;
  bookIds: string[];
  books: Book[];
  items?: PlaylistItem[];
}

export interface PlaylistItem {
  itemType: 'book' | 'series';
  itemId: string;
  order: number;
  book?: Book;
  series?: Series;
}

export interface ScrapeDiff {
  current: BookMetadata;
  scraped: BookMetadata;
  chapter_changes: ChapterChange[];
}
