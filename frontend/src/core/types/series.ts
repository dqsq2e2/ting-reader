import type { Book } from './book';

export interface Series {
  id: string;
  library_id: string;
  title: string;
  author?: string;
  narrator?: string;
  description?: string;
  cover_url?: string;
  created_at: string;
  updated_at?: string;
  books?: Book[];
}
