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