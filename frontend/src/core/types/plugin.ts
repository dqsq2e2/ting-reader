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