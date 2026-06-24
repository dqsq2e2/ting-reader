export interface Progress {
  id: string;
  bookId: string;
  chapterId: string;
  position: number;
  duration?: number;
  updatedAt: string;
  bookTitle?: string;
  chapterTitle?: string;
  coverUrl?: string;
  libraryId?: string;
  chapterDuration?: number;
}

export interface Stats {
  totalBooks: number;
  totalChapters: number;
  totalDuration: number;
  lastScanTime?: string;
}
