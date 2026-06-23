import type { Playlist, PlaylistItem } from '../types';

export interface PlaylistCoverItem {
  id: string;
  title?: string;
  coverUrl?: string;
  libraryId?: string;
  bookId?: string;
}

/**
 * Count total books in a playlist (resolving series expansions).
 */
export const getPlaylistBookCount = (playlist?: Playlist | null): number => {
  if (!playlist) return 0;
  return (
    playlist.items?.reduce((total, item) => (
      total + (item.itemType === 'series' ? (item.series?.books?.length || 0) : 1)
    ), 0) ?? playlist.bookIds.length
  );
};

/**
 * Deterministic hash to pick a cover from a set of candidates.
 */
export const playlistCoverIndex = (playlistId: string, seed: number, count: number): number => {
  if (count <= 1) return 0;
  let hash = seed & 0x7fffffff;
  for (let i = 0; i < playlistId.length; i++) {
    hash = (hash * 31 + playlistId.charCodeAt(i)) & 0x7fffffff;
  }
  return hash % count;
};

const pushCover = (covers: PlaylistCoverItem[], item: PlaylistItem) => {
  if (!item.series) return;
  const seriesBooks = item.series.books || [];
  if (seriesBooks.length > 0) {
    seriesBooks.forEach((book, index) => {
      covers.push({
        id: `${item.series!.id}-${book.id || index}`,
        title: book.title || item.series!.title,
        coverUrl: book.coverUrl || item.series!.coverUrl,
        libraryId: book.libraryId || item.series!.libraryId,
        bookId: book.id,
      });
    });
    return;
  }
  covers.push({
    id: item.series.id,
    title: item.series.title,
    coverUrl: item.series.coverUrl,
    libraryId: item.series.libraryId,
  });
};

/**
 * Collect all candidate cover items from a playlist.
 */
export const collectPlaylistCoverCandidates = (playlist: Playlist): PlaylistCoverItem[] => {
  const covers: PlaylistCoverItem[] = [];
  if (playlist.items && playlist.items.length > 0) {
    playlist.items.forEach(item => {
      if (item.itemType === 'series') {
        pushCover(covers, item);
      } else if (item.book) {
        covers.push({
          id: item.book.id,
          title: item.book.title,
          coverUrl: item.book.coverUrl,
          libraryId: item.book.libraryId,
          bookId: item.book.id,
        });
      }
    });
  } else {
    playlist.books.forEach(book => {
      covers.push({
        id: book.id,
        title: book.title,
        coverUrl: book.coverUrl,
        libraryId: book.libraryId,
        bookId: book.id,
      });
    });
  }
  return covers;
};

/**
 * Pick a single cover from a playlist's candidates using a seeded hash.
 */
export const collectPlaylistCovers = (playlist: Playlist, seed: number): PlaylistCoverItem[] => {
  const covers = collectPlaylistCoverCandidates(playlist);
  if (covers.length === 0) return [];
  return [covers[playlistCoverIndex(playlist.id, seed, covers.length)]];
};