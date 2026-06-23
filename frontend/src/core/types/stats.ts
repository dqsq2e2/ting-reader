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