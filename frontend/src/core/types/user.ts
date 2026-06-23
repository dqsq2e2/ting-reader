export interface User {
  id: string;
  username: string;
  role: 'admin' | 'user';
  createdAt: string;
  librariesAccessible?: string[];
  booksAccessible?: string[];
}