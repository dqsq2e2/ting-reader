import type { Book } from './book';

export interface Series {
  id: string;
  libraryId: string;
  title: string;
  author?: string;
  narrator?: string;
  description?: string;
  coverUrl?: string;
  createdAt: string;
  updatedAt?: string;
  books?: Book[];
}