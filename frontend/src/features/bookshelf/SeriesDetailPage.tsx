import React, { useEffect, useState } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import apiClient from '../../core/api/client';
import type { Book, Series } from '../../core/types';
import BookCard from '../../shared/cards/BookCard';
import BookSelector from '../../shared/modals/BookSelector';
import DisplaySettingsMenu from '../../shared/widgets/DisplaySettingsMenu';
import { ArrowLeft, Trash2, Save, Settings, X, Plus } from 'lucide-react';
import { getCoverUrl } from '../../core/utils/image';
import { localeCompare } from '../../core/utils/locale';
import { usePlayerStore } from '../../core/stores/playerStore';
import { useAuthStore } from '../../core/stores/authStore';
import { getCoverAspectClass, useBookshelfCoverShape } from '../../core/hooks/useBookshelfCoverShape';
import DeleteSeriesModal from './bookDetail/DeleteSeriesModal';

type SeriesSortBy = 'default' | 'title' | 'author';

const SeriesDetailPage: React.FC = () => {
  const { t } = useTranslation();
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const user = useAuthStore((state) => state.user);
  const isAdmin = user?.role === 'admin';
  const coverShape = useBookshelfCoverShape();
  const [series, setSeries] = useState<Series | null>(null);
  const [books, setBooks] = useState<Book[]>([]);
  const [loading, setLoading] = useState(true);
  const [isEditing, setIsEditing] = useState(false);
  const [showBookSelector, setShowBookSelector] = useState(false);
  const [isDeleteModalOpen, setIsDeleteModalOpen] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const { setIsSeriesEditing } = usePlayerStore();
  
  // Filter & Sort state
  const [sortBy, setSortBy] = useState<SeriesSortBy>('default');
  const [iconSize, setIconSize] = useState<'small' | 'medium' | 'large'>('medium');
  const [showFilterMenu, setShowFilterMenu] = useState(false);

  // Edit form state
  const [title, setTitle] = useState('');
  const [author, setAuthor] = useState('');
  const [narrator, setNarrator] = useState('');
  const [description, setDescription] = useState('');
  const [coverUrl, setCoverUrl] = useState('');

  const fetchSeries = async () => {
    try {
      const res = await apiClient.get(`/api/v1/series/${id}`);
      setSeries(res.data);
      setBooks(res.data.books || []);
      setTitle(res.data.title);
      setAuthor(res.data.author || '');
      setNarrator(res.data.narrator || '');
      setDescription(res.data.description || '');
      setCoverUrl(res.data.cover_url || '');
    } catch (err) {
      console.error('Failed to fetch series', err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    const loadSettings = async () => {
      try {
        const res = await apiClient.get('/api/settings');
        const settings = res.data.settings_json || {};
        
        if (settings.series_sort_by === 'default' || settings.series_sort_by === 'title' || settings.series_sort_by === 'author') {
          setSortBy(settings.series_sort_by);
        }
        if (settings.series_icon_size === 'small' || settings.series_icon_size === 'medium' || settings.series_icon_size === 'large') {
          setIconSize(settings.series_icon_size);
        }
      } catch (err) {
        console.error('Failed to load series settings', err);
      }
    };
    loadSettings();
    fetchSeries();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id]);

  // Control player visibility based on editing state
  useEffect(() => {
    setIsSeriesEditing(isEditing);
    
    // Cleanup: reset when component unmounts
    return () => {
      setIsSeriesEditing(false);
    };
  }, [isEditing, setIsSeriesEditing]);

  const handleSortChange = (newSort: SeriesSortBy) => {
    setSortBy(newSort);
    setShowFilterMenu(false);
    apiClient.post('/api/settings', { series_sort_by: newSort });
  };

  const handleIconSizeChange = (newSize: 'small' | 'medium' | 'large') => {
    setIconSize(newSize);
    setShowFilterMenu(false);
    apiClient.post('/api/settings', { series_icon_size: newSize });
  };

  const getGridCols = () => {
    switch (iconSize) {
      case 'small':
        return 'grid-cols-4 sm:grid-cols-6 md:grid-cols-7 lg:grid-cols-8 xl:grid-cols-8 2xl:grid-cols-10 gap-x-3 gap-y-7';
      case 'large':
        return 'grid-cols-2 sm:grid-cols-4 md:grid-cols-4 lg:grid-cols-4 xl:grid-cols-4 2xl:grid-cols-6 gap-x-6 gap-y-10';
      default: // medium
        return 'grid-cols-3 sm:grid-cols-5 md:grid-cols-5 lg:grid-cols-6 xl:grid-cols-6 2xl:grid-cols-7 gap-x-5 gap-y-9';
    }
  };

  const getSortedBooks = () => {
    if (sortBy === 'default') return books;
    
    return [...books].sort((a, b) => {
      if (sortBy === 'title') return localeCompare(a.title, b.title);
      if (sortBy === 'author') return localeCompare(a.author || '', b.author || '');
      return 0;
    });
  };

  const handleUpdate = async () => {
    try {
      await apiClient.put(`/api/v1/series/${id}`, {
        title,
        author,
        narrator,
        description,
        cover_url: coverUrl,
        book_ids: books.map(b => b.id) // Preserving order
      });
      setIsEditing(false);
      fetchSeries();
    } catch (err) {
      console.error('Failed to update series', err);
      alert(t('bookshelf.updateSeriesFailed'));
    }
  };

  const handleDelete = async () => {
    try {
      setDeleting(true);
      await apiClient.delete(`/api/v1/series/${id}`);
      navigate('/bookshelf');
    } catch (err) {
      console.error('Failed to delete series', err);
      alert(t('bookshelf.deleteSeriesFailed'));
    } finally {
      setDeleting(false);
      setIsDeleteModalOpen(false);
    }
  };

  const moveBook = (fromIndex: number, toIndex: number) => {
    const updatedBooks = [...books];
    const [movedBook] = updatedBooks.splice(fromIndex, 1);
    updatedBooks.splice(toIndex, 0, movedBook);
    setBooks(updatedBooks);
  };

  if (loading) return <div className="p-8 text-center">{t('common.loading')}</div>;
  if (!series) return <div className="p-8 text-center">{t('bookshelf.seriesNotFound')}</div>;

  return (
    <div className="flex-1 p-4 sm:p-6 md:p-8 space-y-8">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-4">
          <button 
            onClick={() => isEditing ? setIsEditing(false) : navigate(-1)} 
            className="p-2 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-full"
          >
            <ArrowLeft size={24} />
          </button>
          <h1 className="text-2xl font-bold dark:text-white">
            {isEditing ? t('bookshelf.manageSeries') : series.title}
          </h1>
        </div>
        {!isEditing && (
          <div className="flex items-center gap-2">
            <DisplaySettingsMenu
              open={showFilterMenu}
              onOpenChange={setShowFilterMenu}
              sheetLabel={t('bookshelf.closeSeriesDisplaySettings')}
              sections={[
                {
                  title: t('bookshelf.sortBy'),
                  value: sortBy,
                  options: [
                    { value: 'default', label: t('bookshelf.defaultSort') },
                    { value: 'title', label: t('bookshelf.sortTitle') },
                    { value: 'author', label: t('bookshelf.sortAuthor') },
                  ],
                  onChange: (value) => handleSortChange(value as SeriesSortBy),
                },
                {
                  title: t('bookshelf.iconSize'),
                  value: iconSize,
                  options: [
                    { value: 'large', label: t('bookshelf.largeIcon') },
                    { value: 'medium', label: t('bookshelf.mediumIconDefault') },
                    { value: 'small', label: t('bookshelf.smallIcon') },
                  ],
                  onChange: (value) => handleIconSizeChange(value as 'small' | 'medium' | 'large'),
                },
              ]}
            />

            {isAdmin && (
              <button
                onClick={() => setIsEditing(true)}
                className="p-2.5 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl text-slate-600 dark:text-slate-400 hover:bg-slate-50 dark:hover:bg-slate-800 transition-colors"
              >
                <Settings size={20} />
              </button>
            )}
          </div>
        )}
      </div>

      {isEditing ? (
        // EDIT MODE (Original Management View)
        <div className="grid md:grid-cols-[300px_1fr] gap-8 animate-in fade-in slide-in-from-bottom-4 duration-300">
          {/* Sidebar Info - Editing */}
          <div className="space-y-6">
            <div className={`${getCoverAspectClass(coverShape)} rounded-2xl overflow-hidden shadow-lg`}>
              <img 
                src={getCoverUrl(coverUrl, series.library_id)}
                className="w-full h-full object-cover"
                alt={series.title}
              />
            </div>
            
            <div className="space-y-3">
              <input 
                value={title} 
                onChange={e => setTitle(e.target.value)}
                className="w-full p-2 bg-white dark:bg-slate-800 border rounded"
                placeholder={t('bookshelf.titlePlaceholder')}
              />
              <input 
                value={author} 
                onChange={e => setAuthor(e.target.value)}
                className="w-full p-2 bg-white dark:bg-slate-800 border rounded"
                placeholder={t('bookshelf.authorField')}
              />
              <input 
                value={narrator} 
                onChange={e => setNarrator(e.target.value)}
                className="w-full p-2 bg-white dark:bg-slate-800 border rounded"
                placeholder={t('bookshelf.narratorPlaceholder')}
              />
              <input 
                value={coverUrl} 
                onChange={e => setCoverUrl(e.target.value)}
                className="w-full p-2 bg-white dark:bg-slate-800 border rounded"
                placeholder={t('bookshelf.coverUrlCompact')}
              />
              <textarea 
                value={description} 
                onChange={e => setDescription(e.target.value)}
                className="w-full p-2 bg-white dark:bg-slate-800 border rounded"
                placeholder={t('bookshelf.descriptionField')}
              />
              <div className="flex gap-2">
                <button onClick={handleUpdate} className="flex-1 bg-primary-600 text-white py-2 rounded flex items-center justify-center gap-2">
                  <Save size={18} /> {t('common.save')}
                </button>
                <button onClick={() => setIsEditing(false)} className="flex-1 bg-slate-200 dark:bg-slate-700 py-2 rounded">
                  {t('common.cancel')}
                </button>
              </div>
              <button onClick={() => setIsDeleteModalOpen(true)} className="w-full p-2 bg-red-50 text-red-600 rounded hover:bg-red-100 flex items-center justify-center gap-2">
                  <Trash2 size={18} /> {t('bookshelf.deleteSeries')}
              </button>
            </div>
          </div>

          {/* Book List / Reordering */}
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <h3 className="text-xl font-bold dark:text-white">{t('bookshelf.includedBooks', { count: books.length })}</h3>
              <div className="flex items-center gap-2">
                {books.length > 1 && (
                    <p className="text-xs text-slate-400 mr-2">{t('bookshelf.reorderWithArrows')}</p>
                )}
                <button 
                    onClick={() => setShowBookSelector(true)}
                    className="p-1.5 bg-primary-50 dark:bg-primary-900/20 text-primary-600 rounded-lg hover:bg-primary-100 transition-colors flex items-center gap-1 text-sm font-bold px-3"
                >
                    <Plus size={16} /> {t('bookshelf.addBook')}
                </button>
              </div>
            </div>

            <div className="space-y-3">
              {books.map((book, index) => (
                <div key={book.id} className="flex items-center gap-4 p-3 bg-white dark:bg-slate-900 border border-slate-100 dark:border-slate-800 rounded-xl group">
                  <div className="flex flex-col gap-1">
                    <button 
                      disabled={index === 0}
                      onClick={() => moveBook(index, index - 1)}
                      className="p-1 hover:bg-slate-100 dark:hover:bg-slate-800 rounded disabled:opacity-20"
                    >
                      <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 15l7-7 7 7" /></svg>
                    </button>
                    <button 
                      disabled={index === books.length - 1}
                      onClick={() => moveBook(index, index + 1)}
                      className="p-1 hover:bg-slate-100 dark:hover:bg-slate-800 rounded disabled:opacity-20"
                    >
                      <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" /></svg>
                    </button>
                  </div>
                  
                  <div className={`w-12 ${getCoverAspectClass(coverShape)} rounded overflow-hidden flex-shrink-0`}>
                    <img src={getCoverUrl(book.cover_url, book.library_id, book.id)} className="w-full h-full object-cover" alt="" />
                  </div>
                  
                  <div className="flex-1 min-w-0">
                    <h4 className="font-bold text-slate-900 dark:text-white truncate">{book.title}</h4>
                    <p className="text-xs text-slate-500">{book.author}</p>
                  </div>

                  <button 
                    onClick={() => {
                      const newBooks = books.filter(b => b.id !== book.id);
                      setBooks(newBooks);
                    }}
                    className="opacity-0 group-hover:opacity-100 p-2 text-red-500 hover:bg-red-50 rounded transition-opacity"
                  >
                    <X size={18} />
                  </button>
                </div>
              ))}
            </div>
          </div>
        </div>
      ) : (
        // VIEW MODE (New Bookshelf View)
        <div className="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-300">
           {/* Books Grid */}
             <div className="flex-1 w-full">
                <div className="flex items-center justify-between mb-4">
                    <h3 className="text-lg font-bold dark:text-white">{t('bookshelf.includedBooks', { count: books.length })}</h3>
                </div>
                
                {books.length > 0 ? (
                    <div className={`grid ${getGridCols()}`}>
                        {getSortedBooks().map((book) => (
                            <BookCard key={book.id} book={book} coverShape={coverShape} />
                        ))}
                    </div>
                ) : (
                  <div className="py-20 text-center bg-slate-50 dark:bg-slate-900 rounded-2xl border border-dashed border-slate-200 dark:border-slate-800">
                      <p className="text-slate-500">{t('bookshelf.noSeriesBooks')}</p>
                  </div>
              )}
           </div>
        </div>
      )}
      
      {showBookSelector && (
        <BookSelector
          excludeIds={books.map(b => b.id)}
          onClose={() => setShowBookSelector(false)}
          onSelect={(book) => {
            setBooks([...books, book]);
            setShowBookSelector(false);
          }}
        />
      )}

      {isDeleteModalOpen && series && (
        <DeleteSeriesModal
          series={series}
          deleting={deleting}
          onClose={() => setIsDeleteModalOpen(false)}
          onConfirm={handleDelete}
        />
      )}
    </div>
  );
};

export default SeriesDetailPage;
