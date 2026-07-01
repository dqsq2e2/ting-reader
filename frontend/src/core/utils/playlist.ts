import type { Playlist, PlaylistItem } from '../types';

export interface PlaylistCoverItem {
  id: string;
  title?: string;
  cover_url?: string;
  library_id?: string;
  book_id?: string;
}

/**
 * Count total books in a playlist (resolving series expansions).
 */
export const getPlaylistBookCount = (playlist?: Playlist | null): number => {
  if (!playlist) return 0;
  return (
    playlist.items?.reduce((total, item) => (
      total + (item.item_type === 'series' ? (item.series?.books?.length || 0) : 1)
    ), 0) ?? playlist.book_ids.length
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
        cover_url: book.cover_url || item.series!.cover_url,
        library_id: book.library_id || item.series!.library_id,
        book_id: book.id,
      });
    });
    return;
  }
  covers.push({
    id: item.series.id,
    title: item.series.title,
    cover_url: item.series.cover_url,
    library_id: item.series.library_id,
  });
};

/**
 * Collect all candidate cover items from a playlist.
 */
export const collectPlaylistCoverCandidates = (playlist: Playlist): PlaylistCoverItem[] => {
  const covers: PlaylistCoverItem[] = [];
  if (playlist.items && playlist.items.length > 0) {
    playlist.items.forEach(item => {
      if (item.item_type === 'series') {
        pushCover(covers, item);
      } else if (item.book) {
        covers.push({
          id: item.book.id,
          title: item.book.title,
          cover_url: item.book.cover_url,
          library_id: item.book.library_id,
          book_id: item.book.id,
        });
      }
    });
  } else {
    playlist.books.forEach(book => {
      covers.push({
        id: book.id,
        title: book.title,
        cover_url: book.cover_url,
        library_id: book.library_id,
        book_id: book.id,
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
