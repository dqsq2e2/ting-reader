import React, { useEffect, useMemo, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import apiClient from '../../core/api/client';
import type { Book, Playlist, PlaylistItem } from '../../core/types';
import { getCoverUrl } from '../../core/utils/image';
import { usePlayerStore } from '../../core/stores/playerStore';
import DisplaySettingsMenu from '../../shared/widgets/DisplaySettingsMenu';
import { ListMusic, Plus, Search, X } from 'lucide-react';
import { getCoverAspectClass, useBookshelfCoverShape, type CoverShape } from '../../core/hooks/useBookshelfCoverShape';
import { getPlaylistBookCount, playlistCoverIndex } from '../../core/utils/playlist';

type PlaylistSortBy = 'updatedAt' | 'title' | 'count';
type PlaylistIconSize = 'small' | 'medium' | 'large';

type PlaylistCover = {
  id: string;
  title?: string;
  cover_url?: string;
  library_id?: string;
  book_id?: string;
};

const toBookCover = (book: Book, suffix = ''): PlaylistCover => ({
  id: `${book.id}${suffix}`,
  title: book.title,
  cover_url: book.cover_url,
  library_id: book.library_id,
  book_id: book.id,
});

const collectPlaylistCoverCandidates = (playlist: Playlist): PlaylistCover[] => {
  const covers: PlaylistCover[] = [];
  const pushCover = (cover: PlaylistCover) => {
    covers.push(cover);
  };

  const pushSeriesCovers = (item: PlaylistItem) => {
    if (!item.series) return;

    const seriesBooks = item.series.books || [];
    if (seriesBooks.length > 0) {
      seriesBooks.forEach((book, index) => {
        pushCover({
          id: `${item.series!.id}-${book.id || index}`,
          title: book.title || item.series!.title,
          cover_url: book.cover_url || item.series!.cover_url,
          library_id: book.library_id || item.series!.library_id,
          book_id: book.id,
        });
      });
      return;
    }

    pushCover({
      id: item.series.id,
      title: item.series.title,
      cover_url: item.series.cover_url,
      library_id: item.series.library_id,
    });
  };

  if (playlist.items && playlist.items.length > 0) {
    playlist.items.forEach(item => {
      if (item.item_type === 'series') {
        pushSeriesCovers(item);
      } else if (item.book) {
        pushCover(toBookCover(item.book));
      }
    });
  } else {
    playlist.books.forEach(book => pushCover(toBookCover(book)));
  }

  if (covers.length === 0 && playlist.books.length > 0) {
    playlist.books.forEach(book => pushCover(toBookCover(book, '-fallback')));
  }

  return covers;
};

const collectPlaylistCovers = (playlist: Playlist, seed: number): PlaylistCover[] => {
  const covers = collectPlaylistCoverCandidates(playlist);
  if (covers.length === 0) return [];
  return [covers[playlistCoverIndex(playlist.id, seed, covers.length)]];
};

const getGridGap = (iconSize: PlaylistIconSize) => {
  switch (iconSize) {
    case 'small':
      return 'gap-3';
    case 'large':
      return 'gap-6';
    default:
      return 'gap-4';
  }
};

const getGridStyle = (iconSize: PlaylistIconSize): React.CSSProperties => {
  const cardWidth = iconSize === 'small' ? 170 : iconSize === 'large' ? 440 : 300;
  return {
    gridTemplateColumns: `repeat(auto-fill, minmax(min(100%, ${cardWidth}px), ${cardWidth}px))`,
  };
};

const getMobileGridStyle = (iconSize: PlaylistIconSize): React.CSSProperties => {
  const cardWidth = iconSize === 'small' ? 132 : iconSize === 'large' ? 180 : 156;
  return {
    gridTemplateColumns: `repeat(auto-fit, minmax(${cardWidth}px, 1fr))`,
  };
};

const MyPlaylistsPage: React.FC = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const currentChapter = usePlayerStore((state) => state.currentChapter);
  const coverShape = useBookshelfCoverShape();
  const [playlists, setPlaylists] = useState<Playlist[]>([]);
  const [loading, setLoading] = useState(true);
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [query, setQuery] = useState('');
  const [title, setTitle] = useState('');
  const [description, setDescription] = useState('');
  const [playlistCoverSeed, setPlaylistCoverSeed] = useState(() => Date.now());
  const [saving, setSaving] = useState(false);
  const [sortBy, setSortBy] = useState<PlaylistSortBy>('updatedAt');
  const [iconSize, setIconSize] = useState<PlaylistIconSize>('medium');
  const [showFilterMenu, setShowFilterMenu] = useState(false);

  const fetchPlaylists = async () => {
    setLoading(true);
    try {
      const res = await apiClient.get('/api/playlists');
      setPlaylists(res.data || []);
      setPlaylistCoverSeed(Date.now());
    } catch (err) {
      console.error('Failed to load playlists', err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchPlaylists();
  }, []);

  useEffect(() => {
    const loadSettings = async () => {
      try {
        const res = await apiClient.get('/api/settings');
        const settings = res.data.settings_json || {};

        if (settings.playlist_sort_by === 'updatedAt' || settings.playlist_sort_by === 'title' || settings.playlist_sort_by === 'count') {
          setSortBy(settings.playlist_sort_by);
        }
        if (settings.playlist_icon_size === 'small' || settings.playlist_icon_size === 'medium' || settings.playlist_icon_size === 'large') {
          setIconSize(settings.playlist_icon_size);
        }
      } catch (err) {
        console.error('Failed to load playlist display settings', err);
      }
    };

    loadSettings();
  }, []);

  const filteredPlaylists = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase();
    const nextPlaylists = normalizedQuery
      ? playlists.filter(playlist =>
        playlist.title.toLowerCase().includes(normalizedQuery) ||
        playlist.description?.toLowerCase().includes(normalizedQuery)
      )
      : [...playlists];

    return nextPlaylists.sort((a, b) => {
      if (sortBy === 'title') return a.title.localeCompare(b.title, 'zh-CN');
      if (sortBy === 'count') return getPlaylistBookCount(b) - getPlaylistBookCount(a);
      return new Date(b.updated_at || b.created_at).getTime() - new Date(a.updated_at || a.created_at).getTime();
    });
  }, [playlists, query, sortBy]);

  const handleSortChange = (newSort: PlaylistSortBy) => {
    setSortBy(newSort);
    apiClient.post('/api/settings', { playlist_sort_by: newSort });
  };

  const handleIconSizeChange = (newSize: PlaylistIconSize) => {
    setIconSize(newSize);
    apiClient.post('/api/settings', { playlist_icon_size: newSize });
  };

  const handleCreate = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!title.trim()) return;

    setSaving(true);
    try {
      const res = await apiClient.post('/api/playlists', {
        title,
        description,
        book_ids: [],
      });
      setShowCreateModal(false);
      setTitle('');
      setDescription('');
      navigate(`/playlists/${res.data.id}`);
    } catch (err) {
      console.error('Failed to create playlist', err);
      alert(t('playlists.createFailed'));
    } finally {
      setSaving(false);
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-primary-600"></div>
      </div>
    );
  }

  return (
    <div className="flex-1 min-h-full flex flex-col p-4 sm:p-6 md:p-8 animate-in fade-in duration-500">
      <div className="flex-1 space-y-6">
        <div className="flex flex-col min-[760px]:flex-row min-[760px]:items-center justify-between gap-4">
          <div>
            <h1 className="text-2xl md:text-3xl font-bold text-slate-900 dark:text-white flex items-center gap-3">
              <ListMusic className="text-primary-600" />
              {t('playlists.title')}
            </h1>
            <p className="text-sm md:text-base text-slate-500 dark:text-slate-400 mt-1 max-w-xl">
              {t('playlists.subtitle')}
            </p>
          </div>

          <button
            onClick={() => setShowCreateModal(true)}
            className="inline-flex items-center justify-center gap-2 px-4 py-2.5 bg-primary-600 hover:bg-primary-700 text-white text-sm font-bold rounded-xl shadow-lg shadow-primary-500/25 transition-colors"
          >
            <Plus size={18} />
            {t('playlists.newPlaylist')}
          </button>
        </div>

        {playlists.length > 0 && (
          <div className="flex flex-col sm:flex-row sm:items-center gap-3 max-w-2xl">
            <div className="relative flex-1 min-w-0">
              <Search size={18} className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" />
              <input
                value={query}
                onChange={event => setQuery(event.target.value)}
                placeholder={t('playlists.searchPlaylistPlaceholder')}
                className="w-full pl-10 pr-4 py-3 bg-white dark:bg-slate-900 border border-slate-100 dark:border-slate-800 rounded-2xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white shadow-sm"
                />
            </div>
            <div className="flex items-center justify-end sm:justify-start">
              <DisplaySettingsMenu
                open={showFilterMenu}
                onOpenChange={setShowFilterMenu}
                sheetLabel={t('playlists.closeDisplaySettings')}
                sections={[
                  {
                    title: t('playlists.sortBy'),
                    value: sortBy,
                    options: [
                      { value: 'updatedAt', label: t('playlists.recentlyUpdated') },
                      { value: 'title', label: t('playlists.playlistName') },
                      { value: 'count', label: t('playlists.workCount') },
                    ],
                    onChange: (value) => handleSortChange(value as PlaylistSortBy),
                  },
                  {
                    title: t('playlists.iconSize'),
                    value: iconSize,
                    options: [
                      { value: 'large', label: t('playlists.largeIcon') },
                      { value: 'medium', label: t('playlists.mediumIconDefault') },
                      { value: 'small', label: t('playlists.smallIcon') },
                    ],
                    onChange: (value) => handleIconSizeChange(value as PlaylistIconSize),
                  },
                ]}
              />
            </div>
          </div>
        )}

        {playlists.length === 0 ? (
          <div className="py-20 text-center bg-white dark:bg-slate-900 rounded-3xl border border-dashed border-slate-200 dark:border-slate-800 shadow-sm">
            <div className="inline-flex items-center justify-center w-20 h-20 rounded-2xl bg-primary-50 dark:bg-primary-900/20 text-primary-600 mb-6">
              <ListMusic size={40} />
            </div>
            <h3 className="text-xl font-bold dark:text-white">{t('playlists.emptyTitle')}</h3>
            <p className="text-sm text-slate-500 mt-2 mb-8">{t('playlists.emptyDescription')}</p>
            <button
              onClick={() => setShowCreateModal(true)}
              className="inline-flex items-center gap-2 px-6 py-3 bg-primary-600 hover:bg-primary-700 text-white text-sm font-bold rounded-xl shadow-lg shadow-primary-500/25 transition-colors"
            >
              <Plus size={18} />
              {t('playlists.newPlaylist')}
            </button>
          </div>
        ) : filteredPlaylists.length === 0 ? (
          <div className="py-20 text-center bg-white dark:bg-slate-900 rounded-3xl border border-dashed border-slate-200 dark:border-slate-800 shadow-sm">
            <div className="inline-flex items-center justify-center w-20 h-20 rounded-2xl bg-slate-100 dark:bg-slate-800 text-slate-400 mb-6">
              <Search size={40} />
            </div>
            <h3 className="text-xl font-bold dark:text-white">{t('playlists.noMatchedPlaylist')}</h3>
            <p className="text-sm text-slate-500 mt-2">{t('playlists.tryAnotherKeyword')}</p>
          </div>
        ) : (
          <>
            <div className={`grid sm:hidden ${getGridGap(iconSize)}`} style={getMobileGridStyle(iconSize)}>
              {filteredPlaylists.map(playlist => (
                <PlaylistCard key={playlist.id} playlist={playlist} coverShape={coverShape} iconSize={iconSize} coverSeed={playlistCoverSeed} compactOnMobile />
              ))}
            </div>

            <div className={`hidden sm:grid justify-start ${getGridGap(iconSize)}`} style={getGridStyle(iconSize)}>
              {filteredPlaylists.map(playlist => (
                <PlaylistCard key={playlist.id} playlist={playlist} coverShape={coverShape} iconSize={iconSize} coverSeed={playlistCoverSeed} />
              ))}
            </div>
          </>
        )}
      </div>

      {showCreateModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/50 backdrop-blur-sm animate-in fade-in duration-200">
          <div className="bg-white dark:bg-slate-900 rounded-3xl p-6 w-full max-w-md shadow-2xl border border-slate-100 dark:border-slate-800">
            <div className="flex items-center justify-between mb-5">
              <h2 className="text-xl font-bold dark:text-white">{t('playlists.newPlaylist')}</h2>
              <button
                onClick={() => setShowCreateModal(false)}
                className="p-2 rounded-full hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-500"
              >
                <X size={20} />
              </button>
            </div>

            <form onSubmit={handleCreate} className="space-y-4">
              <div className="space-y-2">
                <label className="text-sm font-bold text-slate-600 dark:text-slate-400">{t('playlists.name')}</label>
                <input
                  value={title}
                  onChange={event => setTitle(event.target.value)}
                  placeholder={t('playlists.namePlaceholder')}
                  className="w-full px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                  autoFocus
                />
              </div>

              <div className="space-y-2">
                <label className="text-sm font-bold text-slate-600 dark:text-slate-400">{t('playlists.description')}</label>
                <textarea
                  value={description}
                  onChange={event => setDescription(event.target.value)}
                  placeholder={t('playlists.descriptionPlaceholder')}
                  rows={3}
                  className="w-full px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white resize-none"
                />
              </div>

              <div className="flex justify-end gap-3 pt-2">
                <button
                  type="button"
                  onClick={() => setShowCreateModal(false)}
                  className="px-5 py-2.5 rounded-xl bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-300 text-sm font-bold"
                >
                  {t('common.cancel')}
                </button>
                <button
                  type="submit"
                  disabled={saving || !title.trim()}
                  className="px-5 py-2.5 rounded-xl bg-primary-600 text-white text-sm font-bold disabled:opacity-50"
                >
                  {saving ? t('playlists.creating') : t('playlists.create')}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      <div
        className="shrink-0 transition-all duration-300"
        style={{ height: currentChapter ? 'var(--safe-bottom-with-player)' : 'var(--safe-bottom-base)' }}
      />
    </div>
  );
};

const PlaylistCard = ({
  playlist,
  coverShape,
  iconSize,
  coverSeed,
  compactOnMobile = false,
}: {
  playlist: Playlist;
  coverShape: CoverShape;
  iconSize: PlaylistIconSize;
  coverSeed: number;
  compactOnMobile?: boolean;
}) => {
  const { t } = useTranslation();
  const covers = collectPlaylistCovers(playlist, coverSeed);
  const bookCount = getPlaylistBookCount(playlist);
  const titleClass = iconSize === 'large' ? 'text-xl' : iconSize === 'small' ? 'text-sm' : 'text-lg';
  const paddingClass = iconSize === 'large' ? 'p-5' : iconSize === 'small' ? 'p-3' : 'p-4';

  return (
    <Link
      to={`/playlists/${playlist.id}`}
      className={`group bg-white dark:bg-slate-900 border border-slate-100 dark:border-slate-800 rounded-3xl shadow-sm hover:shadow-md transition-shadow ${paddingClass} ${compactOnMobile ? 'max-sm:rounded-2xl max-sm:p-3' : ''}`}
    >
      <div className={`w-full ${getCoverAspectClass(coverShape)} rounded-2xl overflow-hidden bg-slate-100 dark:bg-slate-800 relative ${compactOnMobile ? 'max-sm:rounded-xl' : ''}`}>
        {covers.length > 0 ? (
          <div className={covers.length === 1 ? 'h-full' : 'grid grid-cols-2 h-full'}>
            {covers.map(book => (
              <img
                key={book.id}
                src={getCoverUrl(book.cover_url, book.library_id, book.book_id)}
                alt={book.title}
                referrerPolicy="no-referrer"
                className="w-full h-full object-cover group-hover:scale-105 transition-transform duration-300"
                onError={(event) => {
                  (event.target as HTMLImageElement).src = 'https://placehold.co/300x400?text=No+Cover';
                }}
              />
            ))}
          </div>
        ) : (
          <div className="h-full flex items-center justify-center text-white bg-primary-600">
            <ListMusic size={48} />
          </div>
        )}
      </div>

      <div className={`${compactOnMobile ? 'mt-3' : 'mt-4'} min-w-0`}>
        <h3 className={`${titleClass} ${compactOnMobile ? 'max-sm:text-sm' : ''} font-bold text-slate-900 dark:text-white truncate group-hover:text-primary-600 transition-colors`}>
          {playlist.title}
        </h3>
        <p className={`text-sm text-slate-500 mt-1 ${iconSize === 'small' || compactOnMobile ? 'truncate' : 'line-clamp-2 min-h-10'}`}>
          {playlist.description || t('playlists.bookCount', { count: bookCount })}
        </p>
        <p className="text-xs text-slate-400 font-bold mt-3">{t('playlists.bookCount', { count: bookCount })}</p>
      </div>
    </Link>
  );
};

export default MyPlaylistsPage;
