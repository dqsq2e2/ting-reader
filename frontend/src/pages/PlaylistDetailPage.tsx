import React, { useEffect, useMemo, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import apiClient from '../api/client';
import type { Book, Playlist, PlaylistItem, Series } from '../types';
import BookCard from '../components/BookCard';
import BackButton from '../components/BackButton';
import { usePlayerStore } from '../store/playerStore';
import { ArrowDown, ArrowUp, Check, Edit3, Layers, ListMusic, Plus, Save, Search, Trash2, X } from 'lucide-react';
import { getCoverUrl } from '../utils/image';
import { getCoverAspectClass, useBookshelfCoverShape, type CoverShape } from '../hooks/useBookshelfCoverShape';

type EditablePlaylistItem = Pick<PlaylistItem, 'itemType' | 'itemId'>;

const getPlaylistItems = (playlist?: Playlist | null): PlaylistItem[] => {
  if (!playlist) return [];
  if (playlist.items && playlist.items.length > 0) return playlist.items;
  return (playlist.books || []).map((book, index) => ({
    itemType: 'book',
    itemId: book.id,
    order: index + 1,
    book,
  }));
};

const getPlaylistBookCount = (playlist?: Playlist | null) => (
  getPlaylistItems(playlist).reduce((total, item) => (
    total + (item.itemType === 'series' ? (item.series?.books?.length || 0) : 1)
  ), 0)
);

const samePlaylistItem = (item: EditablePlaylistItem, itemType: 'book' | 'series', itemId: string) => (
  item.itemType === itemType && item.itemId === itemId
);

const PlaylistDetailPage: React.FC = () => {
  const { id } = useParams();
  const navigate = useNavigate();
  const currentChapter = usePlayerStore((state) => state.currentChapter);
  const coverShape = useBookshelfCoverShape();
  const [playlist, setPlaylist] = useState<Playlist | null>(null);
  const [books, setBooks] = useState<Book[]>([]);
  const [series, setSeries] = useState<Series[]>([]);
  const [loading, setLoading] = useState(true);
  const [isManaging, setIsManaging] = useState(false);
  const [selectedItems, setSelectedItems] = useState<EditablePlaylistItem[]>([]);
  const [query, setQuery] = useState('');
  const [manageView, setManageView] = useState<'books' | 'series'>('books');
  const [showEditModal, setShowEditModal] = useState(false);
  const [editTitle, setEditTitle] = useState('');
  const [editDescription, setEditDescription] = useState('');
  const [saving, setSaving] = useState(false);

  const fetchData = async () => {
    if (!id) return;
    setLoading(true);
    try {
      const [playlistRes, booksRes, seriesRes] = await Promise.all([
        apiClient.get(`/api/playlists/${id}`),
        apiClient.get('/api/books'),
        apiClient.get('/api/v1/series'),
      ]);
      setPlaylist(playlistRes.data);
      setSelectedItems(getPlaylistItems(playlistRes.data).map(item => ({
        itemType: item.itemType,
        itemId: item.itemId,
      })));
      setEditTitle(playlistRes.data.title || '');
      setEditDescription(playlistRes.data.description || '');
      setBooks(booksRes.data || []);
      setSeries(seriesRes.data || []);
    } catch (err) {
      console.error('加载书单失败', err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchData();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id]);

  const filteredBooks = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase();
    if (!normalizedQuery) return books;
    return books.filter(book =>
      book.title.toLowerCase().includes(normalizedQuery) ||
      book.author?.toLowerCase().includes(normalizedQuery) ||
      book.narrator?.toLowerCase().includes(normalizedQuery)
    );
  }, [books, query]);

  const filteredPlaylistBooks = useMemo(() => {
    const playlistBooks = playlist?.books || [];
    const normalizedQuery = query.trim().toLowerCase();
    if (!normalizedQuery) return playlistBooks;
    return playlistBooks.filter(book =>
      book.title.toLowerCase().includes(normalizedQuery) ||
      book.author?.toLowerCase().includes(normalizedQuery) ||
      book.narrator?.toLowerCase().includes(normalizedQuery)
    );
  }, [playlist?.books, query]);

  const filteredSeries = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase();
    if (!normalizedQuery) return series;
    return series.filter(item =>
      item.title.toLowerCase().includes(normalizedQuery) ||
      item.author?.toLowerCase().includes(normalizedQuery) ||
      item.narrator?.toLowerCase().includes(normalizedQuery) ||
      item.books?.some(book =>
        book.title.toLowerCase().includes(normalizedQuery) ||
        book.author?.toLowerCase().includes(normalizedQuery)
      )
    );
  }, [series, query]);

  const bookMap = useMemo(() => {
    const map = new Map<string, Book>();
    books.forEach(book => map.set(book.id, book));
    playlist?.books.forEach(book => map.set(book.id, book));
    return map;
  }, [books, playlist?.books]);

  const seriesMap = useMemo(() => {
    const map = new Map<string, Series>();
    series.forEach(item => map.set(item.id, item));
    getPlaylistItems(playlist).forEach(item => {
      if (item.series) map.set(item.series.id, item.series);
    });
    return map;
  }, [playlist, series]);

  const selectedDisplayItems = useMemo(() => (
    selectedItems
      .map((item, index) => {
        if (item.itemType === 'series') {
          const selectedSeries = seriesMap.get(item.itemId);
          return selectedSeries
            ? { itemType: 'series' as const, itemId: item.itemId, order: index + 1, series: selectedSeries }
            : null;
        }

        const book = bookMap.get(item.itemId);
        return book
          ? { itemType: 'book' as const, itemId: item.itemId, order: index + 1, book }
          : null;
      })
      .filter(Boolean) as PlaylistItem[]
  ), [bookMap, selectedItems, seriesMap]);

  const toggleBook = (bookId: string) => {
    setSelectedItems(prev => (
      prev.some(item => samePlaylistItem(item, 'book', bookId))
        ? prev.filter(item => !samePlaylistItem(item, 'book', bookId))
        : [...prev, { itemType: 'book', itemId: bookId }]
    ));
  };

  const toggleSeries = (item: Series) => {
    setSelectedItems(prev => (
      prev.some(selected => samePlaylistItem(selected, 'series', item.id))
        ? prev.filter(selected => !samePlaylistItem(selected, 'series', item.id))
        : [...prev, { itemType: 'series', itemId: item.id }]
    ));
  };

  const removeSelectedItem = (itemType: 'book' | 'series', itemId: string) => {
    setSelectedItems(prev => prev.filter(item => !samePlaylistItem(item, itemType, itemId)));
  };

  const moveSelectedItem = (index: number, direction: -1 | 1) => {
    const nextIndex = index + direction;
    if (nextIndex < 0 || nextIndex >= selectedItems.length) return;

    setSelectedItems(prev => {
      const next = [...prev];
      const [moved] = next.splice(index, 1);
      next.splice(nextIndex, 0, moved);
      return next;
    });
  };

  const handleSaveBooks = async () => {
    if (!playlist) return;
    setSaving(true);
    try {
      const res = await apiClient.put(`/api/playlists/${playlist.id}`, {
        items: selectedItems,
      });
      setPlaylist(res.data);
      setSelectedItems(getPlaylistItems(res.data).map(item => ({
        itemType: item.itemType,
        itemId: item.itemId,
      })));
      setIsManaging(false);
      setQuery('');
      setManageView('books');
    } catch (err) {
      console.error('保存书单书籍失败', err);
      alert('保存失败');
    } finally {
      setSaving(false);
    }
  };

  const handleSaveInfo = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!playlist || !editTitle.trim()) return;

    setSaving(true);
    try {
      const res = await apiClient.put(`/api/playlists/${playlist.id}`, {
        title: editTitle,
        description: editDescription,
      });
      setPlaylist(res.data);
      setShowEditModal(false);
    } catch (err) {
      console.error('保存书单信息失败', err);
      alert('保存失败');
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async () => {
    if (!playlist) return;
    if (!window.confirm(`删除书单「${playlist.title}」？`)) return;

    try {
      await apiClient.delete(`/api/playlists/${playlist.id}`);
      navigate('/playlists');
    } catch (err) {
      console.error('删除书单失败', err);
      alert('删除失败');
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-primary-600"></div>
      </div>
    );
  }

  if (!playlist) {
    return (
      <div className="flex-1 min-h-full flex flex-col p-4 sm:p-6 md:p-8">
        <BackButton fallback="/playlists" />
        <div className="py-20 text-center text-slate-500">书单不存在</div>
      </div>
    );
  }

  const playlistItems = getPlaylistItems(playlist);
  const filteredPlaylistItems = playlistItems.filter(item => {
    const normalizedQuery = query.trim().toLowerCase();
    if (!normalizedQuery) return true;

    const title = item.itemType === 'series' ? item.series?.title : item.book?.title;
    const author = item.itemType === 'series' ? item.series?.author : item.book?.author;
    const narrator = item.itemType === 'series' ? item.series?.narrator : item.book?.narrator;
    return (
      title?.toLowerCase().includes(normalizedQuery) ||
      author?.toLowerCase().includes(normalizedQuery) ||
      narrator?.toLowerCase().includes(normalizedQuery)
    );
  });
  const displayBooks = isManaging ? filteredBooks : filteredPlaylistBooks;

  return (
    <div className="flex-1 min-h-full flex flex-col p-4 sm:p-6 md:p-8 animate-in fade-in duration-500">
      <div className="flex-1 space-y-6">
        <BackButton fallback="/playlists" />

        <div className="flex flex-col xl:flex-row xl:items-end justify-between gap-5">
          <div className="flex items-center gap-4 min-w-0">
            <div
              className="w-16 h-16 rounded-2xl bg-primary-600 text-white flex items-center justify-center shrink-0 shadow-lg shadow-primary-500/25"
            >
              <ListMusic size={30} />
            </div>
            <div className="min-w-0">
              <h1 className="text-2xl md:text-3xl font-bold text-slate-900 dark:text-white truncate">
                {playlist.title}
              </h1>
              <p className="text-sm md:text-base text-slate-500 dark:text-slate-400 mt-1 line-clamp-2">
                {playlist.description || `${getPlaylistBookCount(playlist)} 本书`}
              </p>
            </div>
          </div>

          <div className="flex flex-wrap gap-2">
            {isManaging ? (
              <>
                <button
                  onClick={() => {
                    setIsManaging(false);
                    setSelectedItems(getPlaylistItems(playlist).map(item => ({
                      itemType: item.itemType,
                      itemId: item.itemId,
                    })));
                    setQuery('');
                    setManageView('books');
                  }}
                  className="inline-flex items-center gap-2 px-4 py-2.5 rounded-xl bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-300 text-sm font-bold"
                >
                  <X size={18} />
                  取消
                </button>
                <button
                  onClick={handleSaveBooks}
                  disabled={saving}
                  className="inline-flex items-center gap-2 px-4 py-2.5 rounded-xl bg-primary-600 text-white text-sm font-bold disabled:opacity-50"
                >
                  <Save size={18} />
                  {saving ? '保存中...' : '保存书单'}
                </button>
              </>
            ) : (
              <>
                <button
                  onClick={() => setShowEditModal(true)}
                  className="inline-flex items-center gap-2 px-4 py-2.5 rounded-xl bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 text-slate-600 dark:text-slate-300 text-sm font-bold hover:bg-slate-50 dark:hover:bg-slate-800"
                >
                  <Edit3 size={18} />
                  编辑信息
                </button>
                <button
                  onClick={() => {
                    setIsManaging(true);
                    setQuery('');
                    setManageView('books');
                  }}
                  className="inline-flex items-center gap-2 px-4 py-2.5 rounded-xl bg-primary-600 text-white text-sm font-bold shadow-lg shadow-primary-500/25"
                >
                  <Plus size={18} />
                  管理内容
                </button>
                <button
                  onClick={handleDelete}
                  className="inline-flex items-center gap-2 px-4 py-2.5 rounded-xl bg-red-50 dark:bg-red-900/20 text-red-600 text-sm font-bold hover:bg-red-100 dark:hover:bg-red-900/30"
                >
                  <Trash2 size={18} />
                  删除
                </button>
              </>
            )}
          </div>
        </div>

        {(isManaging || playlist.books.length > 0) && (
          <div className="flex flex-col lg:flex-row lg:items-center justify-between gap-3 bg-white dark:bg-slate-900 border border-slate-100 dark:border-slate-800 rounded-2xl p-3 shadow-sm">
            {isManaging && (
              <div className="flex bg-slate-100 dark:bg-slate-800 rounded-xl p-1">
                <button
                  onClick={() => setManageView('books')}
                  className={`flex items-center gap-2 px-3 py-2 text-sm font-bold rounded-lg transition-all ${
                    manageView === 'books'
                      ? 'bg-white dark:bg-slate-700 shadow-sm text-primary-600'
                      : 'text-slate-500'
                  }`}
                >
                  <ListMusic size={16} />
                  书籍
                </button>
                <button
                  onClick={() => setManageView('series')}
                  className={`flex items-center gap-2 px-3 py-2 text-sm font-bold rounded-lg transition-all ${
                    manageView === 'series'
                      ? 'bg-white dark:bg-slate-700 shadow-sm text-primary-600'
                      : 'text-slate-500'
                  }`}
                >
                  <Layers size={16} />
                  系列
                </button>
              </div>
            )}
            <div className="relative flex-1">
              <Search size={18} className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" />
              <input
                value={query}
                onChange={event => setQuery(event.target.value)}
                placeholder={isManaging ? (manageView === 'series' ? '搜索系列、作者或系列内书籍' : '搜索书名、作者、演播者') : '搜索书单内作品'}
                className="w-full pl-10 pr-4 py-2.5 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
              />
            </div>
            <span className="text-sm font-bold text-slate-500 px-2 whitespace-nowrap">
              {isManaging ? `已选 ${selectedItems.length} 项` : `${filteredPlaylistItems.length} / ${playlistItems.length} 项`}
            </span>
          </div>
        )}

        {isManaging && (
          <SelectedOrderPanel
            items={selectedDisplayItems}
            coverShape={coverShape}
            onMove={moveSelectedItem}
            onRemove={removeSelectedItem}
          />
        )}

        {!isManaging ? (
          filteredPlaylistItems.length > 0 ? (
            <div className="grid grid-cols-3 sm:grid-cols-5 md:grid-cols-5 lg:grid-cols-6 xl:grid-cols-6 2xl:grid-cols-7 gap-x-5 gap-y-9">
              {filteredPlaylistItems.map(item => (
                item.itemType === 'series' && item.series ? (
                  <PlaylistSeriesCard key={`series-${item.itemId}`} series={item.series} coverShape={coverShape} />
                ) : item.book ? (
                  <BookCard key={`book-${item.itemId}`} book={item.book} coverShape={coverShape} />
                ) : null
              ))}
            </div>
          ) : (
            <EmptyPlaylistState
              icon={<ListMusic size={40} />}
              title={query ? '没有匹配的作品' : '书单里还没有书'}
              description={query ? '换个关键词试试' : '点击“管理内容”加入作品或系列'}
            />
          )
        ) : manageView === 'series' ? (
          filteredSeries.length > 0 ? (
            <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
              {filteredSeries.map(item => (
                <SeriesSelectCard
                  key={item.id}
                  series={item}
                  selectedItems={selectedItems}
                  coverShape={coverShape}
                  onToggle={() => toggleSeries(item)}
                />
              ))}
            </div>
          ) : (
            <EmptyPlaylistState
              icon={<Layers size={40} />}
              title="没有匹配的系列"
              description="换个关键词试试"
            />
          )
        ) : displayBooks.length > 0 ? (
          <div className="grid grid-cols-3 sm:grid-cols-5 md:grid-cols-5 lg:grid-cols-6 xl:grid-cols-6 2xl:grid-cols-7 gap-x-5 gap-y-9">
            {displayBooks.map(book => (
              <div key={book.id} className="relative">
                <button
                  onClick={() => toggleBook(book.id)}
                  className="block w-full text-left"
                >
                  <div className={`absolute top-2 right-2 z-30 w-7 h-7 rounded-full border-2 flex items-center justify-center transition-all pointer-events-none ${
                    selectedItems.some(item => samePlaylistItem(item, 'book', book.id))
                      ? 'bg-primary-600 border-primary-600 text-white'
                      : 'bg-white/90 dark:bg-slate-900/90 border-slate-300 dark:border-slate-600'
                  }`}>
                    {selectedItems.some(item => samePlaylistItem(item, 'book', book.id)) && <Check size={15} />}
                  </div>
                  <div className={`transition-opacity ${selectedItems.some(item => samePlaylistItem(item, 'book', book.id)) ? 'opacity-100' : 'opacity-60 grayscale-[0.4]'}`}>
                    <BookCover book={book} coverShape={coverShape} />
                  </div>
                </button>
              </div>
            ))}
          </div>
        ) : (
          <EmptyPlaylistState
            icon={<ListMusic size={40} />}
            title={isManaging ? '没有匹配的书籍' : (query ? '没有匹配的作品' : '书单里还没有书')}
            description={isManaging ? '换个关键词试试' : (query ? '换个关键词试试' : '点击“管理内容”加入作品或系列')}
          />
        )}
      </div>

      {showEditModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/50 backdrop-blur-sm animate-in fade-in duration-200">
          <div className="bg-white dark:bg-slate-900 rounded-3xl p-6 w-full max-w-md shadow-2xl border border-slate-100 dark:border-slate-800">
            <div className="flex items-center justify-between mb-5">
              <h2 className="text-xl font-bold dark:text-white">编辑书单</h2>
              <button
                onClick={() => setShowEditModal(false)}
                className="p-2 rounded-full hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-500"
              >
                <X size={20} />
              </button>
            </div>

            <form onSubmit={handleSaveInfo} className="space-y-4">
              <div className="space-y-2">
                <label className="text-sm font-bold text-slate-600 dark:text-slate-400">名称</label>
                <input
                  value={editTitle}
                  onChange={event => setEditTitle(event.target.value)}
                  className="w-full px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                />
              </div>
              <div className="space-y-2">
                <label className="text-sm font-bold text-slate-600 dark:text-slate-400">描述</label>
                <textarea
                  value={editDescription}
                  onChange={event => setEditDescription(event.target.value)}
                  rows={3}
                  className="w-full px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white resize-none"
                />
              </div>
              <div className="flex justify-end gap-3 pt-2">
                <button
                  type="button"
                  onClick={() => setShowEditModal(false)}
                  className="px-5 py-2.5 rounded-xl bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-300 text-sm font-bold"
                >
                  取消
                </button>
                <button
                  type="submit"
                  disabled={saving || !editTitle.trim()}
                  className="px-5 py-2.5 rounded-xl bg-primary-600 text-white text-sm font-bold disabled:opacity-50"
                >
                  {saving ? '保存中...' : '保存'}
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

const SelectedOrderPanel = ({
  items,
  coverShape,
  onMove,
  onRemove,
}: {
  items: PlaylistItem[];
  coverShape: CoverShape;
  onMove: (index: number, direction: -1 | 1) => void;
  onRemove: (itemType: 'book' | 'series', itemId: string) => void;
}) => (
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

const SeriesSelectCard = ({
  series,
  selectedItems,
  coverShape,
  onToggle,
}: {
  series: Series;
  selectedItems: EditablePlaylistItem[];
  coverShape: CoverShape;
  onToggle: () => void;
}) => {
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

const EmptyPlaylistState = ({
  icon,
  title,
  description,
}: {
  icon: React.ReactNode;
  title: string;
  description: string;
}) => (
  <div className="py-20 text-center bg-white dark:bg-slate-900 rounded-3xl border border-dashed border-slate-200 dark:border-slate-800 shadow-sm">
    <div className="inline-flex items-center justify-center w-20 h-20 rounded-2xl bg-primary-50 dark:bg-primary-900/20 text-primary-600 mb-6">
      {icon}
    </div>
    <h3 className="text-xl font-bold dark:text-white">{title}</h3>
    <p className="text-sm text-slate-500 mt-2 mb-8">{description}</p>
  </div>
);

const PlaylistSeriesCard = ({ series, coverShape }: { series: Series; coverShape: CoverShape }) => {
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

const BookCover = ({ book, coverShape }: { book: Book; coverShape: CoverShape }) => (
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

export default PlaylistDetailPage;
