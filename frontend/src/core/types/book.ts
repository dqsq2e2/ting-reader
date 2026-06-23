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

export interface BookMetadata {
  title: string;
  author: string;
  narrator: string;
  description: string;
  cover_url: string;
  tags?: string[];
  genre?: string;
}