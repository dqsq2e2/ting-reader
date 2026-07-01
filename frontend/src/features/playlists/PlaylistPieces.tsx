/* eslint-disable react-refresh/only-export-components */

import React from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { ArrowDown, ArrowUp, Layers, ListMusic, Plus, X } from 'lucide-react';
import type { Book, PlaylistItem, Series } from '../../core/types';
import { getCoverUrl } from '../../core/utils/image';
import { getCoverAspectClass, type CoverShape } from '../../core/hooks/useBookshelfCoverShape';

export type EditablePlaylistItem = { item_type: 'book' | 'series'; item_id: string };

export const samePlaylistItem = (
  item: EditablePlaylistItem,
  itemType: 'book' | 'series',
  itemId: string,
) => item.item_type === itemType && item.item_id === itemId;

// ─── BookCover ──────────────────────────────────────────────────────────────
// Single book cover card in the grid; display-only.

interface BookCoverProps {
  book: Book;
  coverShape: CoverShape;
}

export const BookCover: React.FC<BookCoverProps> = ({ book, coverShape }) => {
  const { t } = useTranslation();

  return (
  <div className="group flex flex-col relative">
    <div className={`relative ${getCoverAspectClass(coverShape)} overflow-hidden rounded-md shadow-md bg-white dark:bg-slate-800`}>
      <img
        src={getCoverUrl(book.cover_url, book.library_id, book.id)}
        alt={book.title}
        loading="lazy"
        referrerPolicy="no-referrer"
        className="w-full h-full object-cover"
        onError={(event) => {
          (event.target as HTMLImageElement).src = 'https://placehold.co/300x400?text=No+Cover';
        }}
      />
    </div>
    <div className="mt-2 min-w-0">
      <p className="font-bold text-sm text-slate-900 dark:text-white truncate">{book.title}</p>
      <p className="text-xs text-slate-500 dark:text-slate-400 truncate mt-1">{book.author || t('playlists.unknownAuthor')}</p>
    </div>
  </div>
  );
};

// ─── PlaylistSeriesCard ─────────────────────────────────────────────────────
// Series card in playlist grid; opens the series detail page.

interface PlaylistSeriesCardProps {
  series: Series;
  coverShape: CoverShape;
}

export const PlaylistSeriesCard: React.FC<PlaylistSeriesCardProps> = ({ series, coverShape }) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const coverBook = series.books?.[0];
  const coverUrl = series.cover_url || coverBook?.cover_url;
  const libraryId = series.library_id || coverBook?.library_id;
  const bookId = coverBook?.id;

  return (
    <button
      onClick={() => navigate(`/series/${series.id}`)}
      className="group flex flex-col relative text-left"
    >
      <div className={`relative ${getCoverAspectClass(coverShape)} overflow-hidden rounded-md shadow-md bg-white dark:bg-slate-800`}>
        {coverUrl ? (
          <img
            src={getCoverUrl(coverUrl, libraryId, bookId)}
            alt={series.title}
            loading="lazy"
            referrerPolicy="no-referrer"
            className="w-full h-full object-cover transition-transform duration-300 group-hover:scale-105"
            onError={(event) => {
              (event.target as HTMLImageElement).src = 'https://placehold.co/300x400?text=Series';
            }}
          />
        ) : (
          <div className="w-full h-full flex items-center justify-center text-slate-400">
            <Layers size={34} />
          </div>
        )}
        <div className="absolute top-2 left-2 px-2 py-1 rounded-full bg-primary-600 text-white text-[10px] font-black shadow-sm">
          {t('playlists.seriesBadge')}
        </div>
      </div>
      <div className="mt-2 min-w-0">
        <p className="font-bold text-sm text-slate-900 dark:text-white truncate group-hover:text-primary-600 transition-colors">{series.title}</p>
        <p className="text-xs text-slate-500 dark:text-slate-400 truncate mt-1">{series.author || t('playlists.unknownAuthor')} · {t('playlists.seriesBookCount', { count: series.books?.length || 0 })}</p>
      </div>
    </button>
  );
};

// ─── SeriesSelectCard ───────────────────────────────────────────────────────
// Selectable series card in the playlist editor.

interface SeriesSelectCardProps {
  series: Series;
  selectedItems: EditablePlaylistItem[];
  coverShape: CoverShape;
  onToggle: () => void;
}

export const SeriesSelectCard: React.FC<SeriesSelectCardProps> = ({ series, selectedItems, coverShape, onToggle }) => {
  const { t } = useTranslation();
  const seriesBooks = series.books || [];
  const selected = selectedItems.some(item => samePlaylistItem(item, 'series', series.id));
  const coverBook = seriesBooks[0];
  const coverUrl = series.cover_url || coverBook?.cover_url;
  const libraryId = series.library_id || coverBook?.library_id;
  const coverBookId = coverBook?.id;

  return (
    <div className="bg-white dark:bg-slate-900 border border-slate-100 dark:border-slate-800 rounded-3xl p-4 shadow-sm flex gap-4">
      <div className={`w-20 ${getCoverAspectClass(coverShape)} rounded-2xl overflow-hidden bg-slate-100 dark:bg-slate-800 shrink-0 shadow-sm`}>
        {coverUrl ? (
          <img
            src={getCoverUrl(coverUrl, libraryId, coverBookId)}
            alt={series.title}
            referrerPolicy="no-referrer"
            className="w-full h-full object-cover"
            onError={(event) => {
              (event.target as HTMLImageElement).src = 'https://placehold.co/300x400?text=Series';
            }}
          />
        ) : (
          <div className="w-full h-full flex items-center justify-center text-slate-400">
            <Layers size={26} />
          </div>
        )}
      </div>

      <div className="min-w-0 flex-1 flex flex-col justify-between gap-4">
        <div className="min-w-0">
          <h3 className="font-bold text-slate-900 dark:text-white truncate">{series.title}</h3>
          <p className="text-xs text-slate-500 mt-1 line-clamp-2">{series.author || t('playlists.unknownAuthor')}</p>
          <p className="text-xs text-slate-400 font-bold mt-2">
            {t('playlists.seriesBookCount', { count: seriesBooks.length })} · {selected ? t('playlists.inPlaylist') : t('playlists.canAddAsSeries')}
          </p>
        </div>
        <button
          onClick={onToggle}
          disabled={seriesBooks.length === 0}
          className={`inline-flex items-center justify-center gap-2 px-4 py-2 rounded-xl text-sm font-bold transition-colors disabled:opacity-50 ${
            selected
              ? 'bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700'
              : 'bg-primary-600 text-white hover:bg-primary-700 shadow-lg shadow-primary-500/20'
          }`}
        >
          {selected ? <X size={16} /> : <Plus size={16} />}
          {selected ? t('playlists.removeSeries') : t('playlists.addSeries')}
        </button>
      </div>
    </div>
  );
};

// ─── SelectedOrderPanel ─────────────────────────────────────────────────────
// Selected order panel in the playlist editor.

interface SelectedOrderPanelProps {
  items: PlaylistItem[];
  coverShape: CoverShape;
  onMove: (index: number, direction: -1 | 1) => void;
  onRemove: (itemType: 'book' | 'series', itemId: string) => void;
}

export const SelectedOrderPanel: React.FC<SelectedOrderPanelProps> = ({ items, coverShape, onMove, onRemove }) => {
  const { t } = useTranslation();

  return (
  <section className="bg-white dark:bg-slate-900 border border-slate-100 dark:border-slate-800 rounded-3xl shadow-sm overflow-hidden">
    <div className="flex items-center justify-between gap-3 px-4 md:px-5 py-4 border-b border-slate-100 dark:border-slate-800">
      <div>
        <h2 className="text-lg font-bold dark:text-white">{t('playlists.selectedOrder')}</h2>
        <p className="text-xs text-slate-500 mt-0.5">{t('playlists.selectedOrderHint')}</p>
      </div>
      <span className="text-sm font-bold text-slate-500 whitespace-nowrap">{t('playlists.itemCount', { count: items.length })}</span>
    </div>

    {items.length > 0 ? (
      <div className="max-h-80 overflow-y-auto divide-y divide-slate-100 dark:divide-slate-800">
        {items.map((item, index) => {
          const isSeries = item.item_type === 'series';
          const title = isSeries ? item.series?.title : item.book?.title;
          const subtitle = isSeries
            ? `${item.series?.author || t('playlists.unknownAuthor')} · ${t('playlists.seriesBookCount', { count: item.series?.books?.length || 0 })}`
            : item.book?.author || t('playlists.unknownAuthor');
          const coverBook = item.series?.books?.[0];
          const coverUrl = isSeries
            ? item.series?.cover_url || coverBook?.cover_url
            : item.book?.cover_url;
          const libraryId = isSeries
            ? item.series?.library_id || coverBook?.library_id
            : item.book?.library_id;
          const bookId = isSeries ? coverBook?.id : item.book?.id;

          return (
          <div key={`${item.item_type}-${item.item_id}`} className="flex items-center gap-3 p-3 md:p-4">
            <span className="w-7 text-center text-xs font-black text-slate-400 shrink-0">{index + 1}</span>
            <div className={`w-10 ${getCoverAspectClass(coverShape)} rounded-lg overflow-hidden bg-slate-100 dark:bg-slate-800 shrink-0 shadow-sm`}>
              {coverUrl ? (
                <img
                  src={getCoverUrl(coverUrl, libraryId, bookId)}
                  alt={title}
                  referrerPolicy="no-referrer"
                  className="w-full h-full object-cover"
                  onError={(event) => {
                    (event.target as HTMLImageElement).src = 'https://placehold.co/300x400?text=No+Cover';
                  }}
                />
              ) : (
                <div className="w-full h-full flex items-center justify-center text-slate-400">
                  {isSeries ? <Layers size={18} /> : <ListMusic size={18} />}
                </div>
              )}
            </div>
            <div className="min-w-0 flex-1">
              <p className="font-bold text-sm text-slate-900 dark:text-white truncate">{title}</p>
              <p className="text-xs text-slate-500 truncate">{isSeries ? t('playlists.seriesBadge') : t('playlists.bookType')} · {subtitle}</p>
            </div>
            <div className="flex items-center gap-1 shrink-0">
              <button
                onClick={() => onMove(index, -1)}
                disabled={index === 0}
                className="p-2 rounded-lg text-slate-400 hover:text-primary-600 hover:bg-primary-50 dark:hover:bg-primary-900/20 disabled:opacity-30 disabled:hover:bg-transparent disabled:hover:text-slate-400"
                title={t('playlists.moveUp')}
              >
                <ArrowUp size={16} />
              </button>
              <button
                onClick={() => onMove(index, 1)}
                disabled={index === items.length - 1}
                className="p-2 rounded-lg text-slate-400 hover:text-primary-600 hover:bg-primary-50 dark:hover:bg-primary-900/20 disabled:opacity-30 disabled:hover:bg-transparent disabled:hover:text-slate-400"
                title={t('playlists.moveDown')}
              >
                <ArrowDown size={16} />
              </button>
              <button
                onClick={() => onRemove(item.item_type, item.item_id)}
                className="p-2 rounded-lg text-slate-400 hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20"
                title={t('playlists.remove')}
              >
                <X size={16} />
              </button>
            </div>
          </div>
          );
        })}
      </div>
    ) : (
      <div className="py-8 text-center text-sm text-slate-500">{t('playlists.noSelectedContent')}</div>
    )}
  </section>
  );
};

// ─── EmptyPlaylistState ─────────────────────────────────────────────────────
// Shared empty state card: icon, title, and description.

// ─── EmptyPlaylistState ─────────────────────────────────────────────────────
// Shared empty state card: icon, title, and description.

interface EmptyPlaylistStateProps {
  icon: React.ReactNode;
  title: string;
  description: string;
}

export const EmptyPlaylistState: React.FC<EmptyPlaylistStateProps> = ({ icon, title, description }) => (
  <div className="py-20 text-center bg-white dark:bg-slate-900 rounded-3xl border border-dashed border-slate-200 dark:border-slate-800 shadow-sm">
    <div className="inline-flex items-center justify-center w-20 h-20 rounded-2xl bg-primary-50 dark:bg-primary-900/20 text-primary-600 mb-6">
      {icon}
    </div>
    <h3 className="text-xl font-bold dark:text-white">{title}</h3>
    <p className="text-sm text-slate-500 mt-2 mb-8">{description}</p>
  </div>
);
