import type { ApiErrorBody } from '../api/errors';

export interface PluginDependency {
  plugin_id: string;
  version_requirement: string;
}

export interface PluginStats {
  total_calls: number;
  successful_calls: number;
  failed_calls: number;
  avg_execution_time_ms: number;
}

export type PluginPermission =
  | { type: "network_access"; domain: string }
  | { type: "file_read" | "file_write"; path: string }
  | { type: "event_subscribe"; event: string }
  | { type: "capability_invoke"; plugin_id: string; capability_id: string }
  | {
      type:
        | "books_read"
        | "books_write"
        | "libraries_read"
        | "chapters_read"
        | "chapters_write"
        | "libraries_write"
        | "progress_read"
        | "media_read_url"
        | "plugin_route_sign"
        | "metadata_write"
        | "task_create"
        | "cache_read"
        | "cache_write"
        | "playlists_read"
        | "playlists_write"
        | "favorites_read"
        | "favorites_write"
        | "bookmarks_read"
        | "bookmarks_write"
        | "user_settings_read"
        | "user_settings_write"
        | "config_read"
        | "storage_read"
        | "storage_write"
        | "task_read"
        | "task_manage"
        | "task_progress"
        | "html_parse"
        | "plugin_route_revoke"
        | "event_publish";
    };

export interface UnverifiedPluginInstallConfirmation {
  requires_confirmation: boolean;
  verification_status: string;
  plugin_id: string;
  plugin_name: string;
  plugin_version: string;
  publisher: string;
  warning: string;
  runtime: string | null;
  permissions: PluginPermission[];
  capabilities: PluginCapability[];
  package_sha256: string;
  package_changed: boolean;
}

export type PluginCapability =
  | { id: string; kind: "metadata_provider"; operations: string[]; book_source?: boolean; auto_scrape: boolean; aggregate_auto_scrape: boolean; search_fields: ScraperSearchField[]; filters_schema?: Record<string, unknown>; result_fields: Array<{ key: string; label: LocalizedText }> }
  | { id: string; kind: "format_handler"; extensions: string[]; operations: string[] }
  | { id: string; kind: "tool_provider"; invoke: "invokeTool"; tools: Array<{ name: string; description: LocalizedText; input_schema: unknown; output_schema: unknown; side_effects: boolean }> }
  | { id: string; kind: "http_route"; route: { method: string; path: string; auth: "user" | "public" | "signed" | "public_or_signed" } }
  | { id: string; kind: "ui_extension"; slots: string[]; contexts: string[]; title: LocalizedText; icon?: string; priority: number; render: { mode: "web_container" | "action"; entry?: string; bridge: { capabilities: string[]; host_methods: string[] } } }
  | { id: string; kind: "plugin_store"; operations: string[] }
  | { id: string; kind: "content_processor"; extensions: string[]; operations: string[] }
  | { id: string; kind: "task_handler"; tasks: Array<{ task_type: string; input_schema: unknown; output_schema: unknown; idempotent: boolean }> }
  | { id: string; kind: "event_handler"; events: Array<{ name: string; schema: unknown }> };

export interface PluginCapabilityRegistration {
  plugin_id: string;
  plugin_name: string;
  admin_only?: boolean;
  client_grant?: string;
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
  book_source?: boolean;
  auto_scrape: boolean;
  aggregate_auto_scrape: boolean;
  search_fields: ScraperSearchField[];
  result_fields: string[];
  result_field_labels?: Record<string, LocalizedText>;
}

export interface ScraperSearchItem {
  id: string | null;
  source_url: string | null;
  added_book_id?: string | null;
  title: string;
  author: string | null;
  narrator?: string | null;
  cover_url?: string | null;
  intro?: string | null;
  tags?: string[];
  genre?: string | null;
  subtitle?: string | null;
  published_year?: number | null;
  published_date?: string | null;
  publisher?: string | null;
  isbn?: string | null;
  asin?: string | null;
  language?: string | null;
  explicit?: boolean | null;
  abridged?: boolean | null;
  duration?: number | null;
  score?: number | null;
  chapter_title_template?: string | null;
  chapter_titles?: string[];
  [key: string]: unknown;
}

export interface Plugin {
  id: string;
  name: string;
  version: string;
  author: string;
  description: string;
  state: 'active' | 'inactive' | 'loading' | 'failed';
  runtime?: string;
  license?: string;
  repo?: string;
  min_core_version?: string;
  min_flutter_version?: string;
  admin_only?: boolean;
  description_i18n?: LocalizedText;
  is_enabled?: boolean;
  entry_point?: string;
  dependencies?: PluginDependency[];
  permissions?: PluginPermission[];
  config_schema?: Record<string, unknown>;
  supported_extensions?: string[];
  total_calls?: number;
  successful_calls?: number;
  failed_calls?: number;
  success_rate?: number;
  stats?: PluginStats;
  error?: ApiErrorBody | null;
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
  permissions?: PluginPermission[];
  capabilities?: PluginCapability[];
  config_schema?: Record<string, unknown>;
  min_core_version?: string;
  min_flutter_version?: string;
  admin_only?: boolean;
  downloads?: { name: string; url: string }[];
}
