export interface User {
  id: string;
  username: string;
  role: 'admin' | 'user';
  created_at: string;
  libraries_accessible?: string[];
  books_accessible?: string[];
}
