export interface ScraperConfig {
  default_sources?: string[];
  cover_sources?: string[];
  intro_sources?: string[];
  author_sources?: string[];
  narrator_sources?: string[];
  tags_sources?: string[];
  nfo_writing_enabled?: boolean;
  metadata_writing_enabled?: boolean;
  use_filename_as_title?: boolean;
  metadata_priority?: string[];
  extract_audio_cover?: boolean;
}

export interface Library {
  id: string;
  name: string;
  library_type: 'webdav' | 'local' | 'rss';
  url: string;
  username?: string;
  password?: string;
  root_path: string;
  last_scanned_at?: string;
  scraper_config?: ScraperConfig;
  created_at: string;
}
