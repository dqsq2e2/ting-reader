import type { Book } from './book';
import type { Series } from './series';

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