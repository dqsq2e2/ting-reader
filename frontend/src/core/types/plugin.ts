export interface PluginDependency {
  plugin_name: string;
  version_requirement: string;
}

export interface PluginStats {
  total_calls: number;
  successful_calls: number;
  failed_calls: number;
  avg_execution_time_ms: number;
}

export interface PluginCapability {
  id: string;
  kind: string;
  invoke?: string;
  [key: string]: unknown;
}

export interface PluginCapabilityRegistration {
  plugin_id: string;
  plugin_name: string;
  capability: PluginCapability;
}

export interface ToolProviderRegistration extends PluginCapabilityRegistration {
  tool?: unknown;
}

export interface LocalizedText {
  zh?: string;
  en?: string;
  [key: string]: string | undefined;
}

export interface ScraperSearchField {
  key: string;
  label: string;
  label_i18n?: LocalizedText;
  required?: boolean;
  type?: string;
  field_type?: string;
  placeholder?: string;
  placeholder_i18n?: LocalizedText;
  default_from?: string;
}

export interface ScraperSource {
  id: string;
  name: string;
  description?: string;
  version: string;
  enabled: boolean;
  auto_scrape: boolean;
  aggregate_auto_scrape: boolean;
  search_fields: ScraperSearchField[];
  result_fields: string[];
  result_field_labels?: Record<string, LocalizedText>;
}

export interface ScraperSearchItem {
  id: string;
  title?: string;
  author?: string;
  narrator?: string | null;
  cover_url?: string | null;
  intro?: string | null;
  description?: string | null;
  tags?: string[];
  genre?: string | null;
  subtitle?: string | null;
  published_year?: string | null;
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
  plugin_type?: 'scraper' | 'format' | 'utility';
  author: string;
  description: string;
  state: 'active' | 'inactive' | 'loading' | 'failed';
  runtime?: string;
  license?: string;
  repo?: string;
  min_core_version?: string;
  min_flutter_version?: string;
  description_i18n?: LocalizedText;
  is_enabled?: boolean;
  entry_point?: string;
  dependencies?: PluginDependency[];
  permissions?: string[];
  config_schema?: Record<string, unknown>;
  supported_extensions?: string[];
  total_calls?: number;
  successful_calls?: number;
  failed_calls?: number;
  success_rate?: number;
  stats?: PluginStats;
  error?: string;
  capabilities?: PluginCapability[];
}

export interface StorePlugin {
  id: string;
  name: string;
  description: string;
  long_description?: string;
  icon?: string;
  repo?: string;
  version: string;
  download_url: string | Record<string, string>;
  size?: string | Record<string, string>;
  date?: string;
  dependencies?: string[];
  runtime?: string;
  license?: string;
  author?: string;
  description_i18n?: LocalizedText;
  permissions?: string[];
  capabilities?: PluginCapability[];
  config_schema?: Record<string, unknown>;
  min_core_version?: string;
  min_flutter_version?: string;
  downloads?: { name: string; url: string }[];
}
