export interface AdminStatisticsOverview {
  total_books: number;
  total_chapters: number;
  total_duration: number;
  total_libraries: number;
  local_libraries: number;
  webdav_libraries: number;
  total_users: number;
  admin_users: number;
  active_users: number;
  total_progress_records: number;
  total_listen_seconds: number;
}

export interface LibraryStatistics {
  id: string;
  name: string;
  library_type: string;
  total_books: number;
  total_chapters: number;
  total_duration: number;
  last_scanned_at?: string;
}

export interface UserActivityStatistics {
  id: string;
  username: string;
  role: 'admin' | 'user' | string;
  listened_books: number;
  progress_records: number;
  listen_seconds: number;
  last_active_at?: string;
}

export interface RecentActivityPoint {
  date: string;
  active_users: number;
  progress_updates: number;
  listen_seconds: number;
}

export interface BookActivityStatistics {
  id: string;
  title?: string;
  author?: string;
  library_id: string;
  library_name?: string;
  listeners: number;
  progress_updates: number;
  listen_seconds: number;
}

export interface AdminStatistics {
  overview: AdminStatisticsOverview;
  library_breakdown: LibraryStatistics[];
  user_activity: UserActivityStatistics[];
  recent_activity: RecentActivityPoint[];
  top_books: BookActivityStatistics[];
  generated_at: string;
}
