/* eslint-disable react-refresh/only-export-components */

import React from 'react';
import { useNavigate } from 'react-router-dom';
import { ArrowDown, ArrowUp, Layers, ListMusic, Plus, X } from 'lucide-react';
import type { Book, PlaylistItem, Series } from '../../core/types';
import { getCoverUrl } from '../../core/utils/image';
import { getCoverAspectClass, type CoverShape } from '../../core/hooks/useBookshelfCoverShape';

export type EditablePlaylistItem = { itemType: 'book' | 'series'; itemId: string };

export const samePlaylistItem = (
  item: EditablePlaylistItem,
  itemType: 'book' | 'series',
  itemId: string,
) => item.itemType === itemType && item.itemId === itemId;

// ─── BookCover ──────────────────────────────────────────────────────────────
// 网格里的单本书封面卡片，纯展示，不带任何交互。

interface BookCoverProps {
  book: Book;
  coverShape: CoverShape;
}

export const BookCover: React.FC<BookCoverProps> = ({ book, coverShape }) => (
  <div className="group flex flex-col relative">
    <div className={`relative ${getCoverAspectClass(coverShape)} overflow-hidden rounded-md shadow-md bg-white dark:bg-slate-800`}>
      <img
        src={getCoverUrl(book.coverUrl, book.libraryId, book.id)}
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
      <p className="text-xs text-slate-500 dark:text-slate-400 truncate mt-1">{book.author || '未知作者'}</p>
    </div>
  </div>
);

// ─── PlaylistSeriesCard ─────────────────────────────────────────────────────
// 书单网格里的系列卡片：点击跳转到系列详情。

interface PlaylistSeriesCardProps {
  series: Series;
  coverShape: CoverShape;
}

export const PlaylistSeriesCard: React.FC<PlaylistSeriesCardProps> = ({ series, coverShape }) => {
  const navigate = useNavigate();
  const coverBook = series.books?.[0];
  const coverUrl = series.coverUrl || coverBook?.coverUrl;
  const libraryId = series.libraryId || coverBook?.libraryId;
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
          系列
        </div>
      </div>
      <div className="mt-2 min-w-0">
        <p className="font-bold text-sm text-slate-900 dark:text-white truncate group-hover:text-primary-600 transition-colors">{series.title}</p>
        <p className="text-xs text-slate-500 dark:text-slate-400 truncate mt-1">{series.author || '未知作者'} · {series.books?.length || 0} 本</p>
      </div>
    </button>
  );
};

// ─── SeriesSelectCard ───────────────────────────────────────────────────────
// 编辑器里"待选系列"列表里的可勾选卡片。

interface SeriesSelectCardProps {
  series: Series;
  selectedItems: EditablePlaylistItem[];
  coverShape: CoverShape;
  onToggle: () => void;
}

export const SeriesSelectCard: React.FC<SeriesSelectCardProps> = ({ series, selectedItems, coverShape, onToggle }) => {
  const seriesBooks = series.books || [];
  const selected = selectedItems.some(item => samePlaylistItem(item, 'series', series.id));
  const coverBook = seriesBooks[0];
  const coverUrl = series.coverUrl || coverBook?.coverUrl;
  const libraryId = series.libraryId || coverBook?.libraryId;
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
          <p className="text-xs text-slate-500 mt-1 line-clamp-2">{series.author || '未知作者'}</p>
          <p className="text-xs text-slate-400 font-bold mt-2">
            {seriesBooks.length} 本 · {selected ? '已在书单' : '可作为系列加入'}
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
          {selected ? '移出系列' : '加入系列'}
        </button>
      </div>
    </div>
  );
};

// ─── SelectedOrderPanel ─────────────────────────────────────────────────────
// 编辑器里"已选顺序"面板：列出当前选中的书/系列，支持上下移和移除。

interface SelectedOrderPanelProps {
  items: PlaylistItem[];
  coverShape: CoverShape;
  onMove: (index: number, direction: -1 | 1) => void;
  onRemove: (itemType: 'book' | 'series', itemId: string) => void;
}

export const SelectedOrderPanel: React.FC<SelectedOrderPanelProps> = ({ items, coverShape, onMove, onRemove }) => (
  <section className="bg-white dark:bg-slate-900 border border-slate-100 dark:border-slate-800 rounded-3xl shadow-sm overflow-hidden">
    <div className="flex items-center justify-between gap-3 px-4 md:px-5 py-4 border-b border-slate-100 dark:border-slate-800">
      <div>
        <h2 className="text-lg font-bold dark:text-white">已选顺序</h2>
        <p className="text-xs text-slate-500 mt-0.5">这里的顺序就是保存后的书单播放顺序。</p>
      </div>
      <span className="text-sm font-bold text-slate-500 whitespace-nowrap">{items.length} 项</span>
    </div>

    {items.length > 0 ? (
      <div className="max-h-80 overflow-y-auto divide-y divide-slate-100 dark:divide-slate-800">
        {items.map((item, index) => {
          const isSeries = item.itemType === 'series';
          const title = isSeries ? item.series?.title : item.book?.title;
          const subtitle = isSeries
            ? `${item.series?.author || '未知作者'} · ${item.series?.books?.length || 0} 本`
            : item.book?.author || '未知作者';
          const coverBook = item.series?.books?.[0];
          const coverUrl = isSeries
            ? item.series?.coverUrl || coverBook?.coverUrl
            : item.book?.coverUrl;
          const libraryId = isSeries
            ? item.series?.libraryId || coverBook?.libraryId
            : item.book?.libraryId;
          const bookId = isSeries ? coverBook?.id : item.book?.id;

          return (
          <div key={`${item.itemType}-${item.itemId}`} className="flex items-center gap-3 p-3 md:p-4">
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
              <p className="text-xs text-slate-500 truncate">{isSeries ? '系列' : '书籍'} · {subtitle}</p>
            </div>
            <div className="flex items-center gap-1 shrink-0">
              <button
                onClick={() => onMove(index, -1)}
                disabled={index === 0}
                className="p-2 rounded-lg text-slate-400 hover:text-primary-600 hover:bg-primary-50 dark:hover:bg-primary-900/20 disabled:opacity-30 disabled:hover:bg-transparent disabled:hover:text-slate-400"
                title="上移"
              >
                <ArrowUp size={16} />
              </button>
              <button
                onClick={() => onMove(index, 1)}
                disabled={index === items.length - 1}
                className="p-2 rounded-lg text-slate-400 hover:text-primary-600 hover:bg-primary-50 dark:hover:bg-primary-900/20 disabled:opacity-30 disabled:hover:bg-transparent disabled:hover:text-slate-400"
                title="下移"
              >
                <ArrowDown size={16} />
              </button>
              <button
                onClick={() => onRemove(item.itemType, item.itemId)}
                className="p-2 rounded-lg text-slate-400 hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20"
                title="移除"
              >
                <X size={16} />
              </button>
            </div>
          </div>
          );
        })}
      </div>
    ) : (
      <div className="py-8 text-center text-sm text-slate-500">还没有选择内容。</div>
    )}
  </section>
);

// ─── EmptyPlaylistState ─────────────────────────────────────────────────────
// 通用"空状态"卡片：图标 + 标题 + 描述。

// ─── EmptyPlaylistState ─────────────────────────────────────────────────────
// 通用"空状态"卡片：图标 + 标题 + 描述。

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
