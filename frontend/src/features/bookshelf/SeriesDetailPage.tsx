import { apiErrorMessage } from '../../core/api/errors';
import { useBookOperations } from '../../core/hooks/useBooks';
import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useParams, useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import apiClient from '../../core/api/client';
import type { Book, Series } from '../../core/types';
import BookCard from '../../shared/cards/BookCard';
import BookshelfListItem from './BookshelfListItem';
import BookSelector from '../../shared/modals/BookSelector';
import DisplaySettingsMenu from '../../shared/widgets/DisplaySettingsMenu';
import { ArrowLeft, Trash2, Save, Settings, X, Plus, Check, CheckSquare, Layers, ChevronDown, BookCheck, BookX } from 'lucide-react';
import { getCoverUrl } from '../../core/utils/image';
import { localeCompare } from '../../core/utils/locale';
import { usePlayerStore } from '../../core/stores/playerStore';
import { useAuthStore } from '../../core/stores/authStore';
import { getCoverAspectClass, useBookshelfCoverShape } from '../../core/hooks/useBookshelfCoverShape';
import DeleteLibraryItemModal from './bookDetail/DeleteLibraryItemModal';

type SeriesSortBy = 'default' | 'title' | 'author';

const SeriesDetailPage: React.FC = () => {
  const bookOperations = useBookOperations();
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
  const [saving, setSaving] = useState(false);
  const [isSelectionMode, setIsSelectionMode] = useState(false);
  const [selectedBookIds, setSelectedBookIds] = useState<string[]>([]);
  const [isOperationsOpen, setIsOperationsOpen] = useState(false);
  const [batchBusy, setBatchBusy] = useState(false);
  const [confirmation, setConfirmation] = useState<'unread' | 'delete' | null>(null);
  const [deleteSourceFiles, setDeleteSourceFiles] = useState(false);
  const operationsRef = useRef<HTMLDivElement>(null);
  const requestVersion = useRef(0);
  const pageVersion = useRef(0);
  const setIsSeriesEditing = usePlayerStore((state) => state.setIsSeriesEditing);
  
  // Filter & Sort state
  const [sortBy, setSortBy] = useState<SeriesSortBy>('default');
  const [iconSize, setIconSize] = useState<'small' | 'medium' | 'large'>('medium');
  const [viewMode, setViewMode] = useState<'grid' | 'list'>('grid');
  const [showFilterMenu, setShowFilterMenu] = useState(false);

  // Edit form state
  const [title, setTitle] = useState('');
  const [author, setAuthor] = useState('');
  const [narrator, setNarrator] = useState('');
  const [description, setDescription] = useState('');
  const [coverUrl, setCoverUrl] = useState('');

  const fetchSeries = useCallback(async () => {
    const version = ++requestVersion.current;
    try {
      const res = await apiClient.get(`/api/v1/series/${id}`);
      if (version !== requestVersion.current) return;
      setSeries(res.data);
      setBooks(res.data.books || []);
      const availableIds = new Set<string>((res.data.books || []).map((book: Book) => book.id));
      setSelectedBookIds((current) => current.filter((bookId) => availableIds.has(bookId)));
      setTitle(res.data.title);
      setAuthor(res.data.author || '');
      setNarrator(res.data.narrator || '');
      setDescription(res.data.description || '');
      setCoverUrl(res.data.cover_url || '');
    } catch (err) {
      console.error('Failed to fetch series', err);
    } finally {
      if (version === requestVersion.current) setLoading(false);
    }
  }, [id]);

  const invalidatePage = useCallback(() => {
    requestVersion.current++;
    pageVersion.current++;
  }, []);

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
        setViewMode(settings.series_view_mode === 'list' ? 'list' : 'grid');
      } catch (err) {
        console.error('Failed to load series settings', err);
      }
    };
    loadSettings();
    const timer = window.setTimeout(() => {
      setLoading(true);
      setSeries(null);
      setIsEditing(false);
      setIsSelectionMode(false);
      setSelectedBookIds([]);
      setIsOperationsOpen(false);
      setConfirmation(null);
      setDeleteSourceFiles(false);
      setBatchBusy(false);
      void fetchSeries();
    }, 0);
    return () => {
      window.clearTimeout(timer);
      invalidatePage();
    };
  }, [fetchSeries, invalidatePage]);

  useEffect(() => {
    if (!isOperationsOpen && !confirmation) return;
    const handleClick = (event: MouseEvent) => {
      if (!operationsRef.current?.contains(event.target as Node)) setIsOperationsOpen(false);
    };
    const handleKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setIsOperationsOpen(false);
        if (!batchBusy) setConfirmation(null);
      }
    };
    document.addEventListener('click', handleClick);
    document.addEventListener('keydown', handleKey);
    return () => {
      document.removeEventListener('click', handleClick);
      document.removeEventListener('keydown', handleKey);
    };
  }, [isOperationsOpen, confirmation, batchBusy]);

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
  const handleViewModeChange = (newMode: 'grid' | 'list') => {
    setViewMode(newMode);
    setShowFilterMenu(false);
    apiClient.post('/api/settings', { series_view_mode: newMode });
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
    if (saving) return;
    setSaving(true);
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
      alert(apiErrorMessage(err, t('bookshelf.updateSeriesFailed')));
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async () => {
    try {
      setDeleting(true);
      if (!id) return;
      await bookOperations.current?.deleteSeries([id]);
      navigate('/bookshelf');
    } catch (err) {
      console.error('Failed to delete series', err);
      alert(apiErrorMessage(err, t('bookshelf.deleteSeriesFailed')));
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

  const exitSelectionMode = () => {
    if (batchBusy) return;
    setIsSelectionMode(false);
    setSelectedBookIds([]);
    setIsOperationsOpen(false);
  };

  const toggleBookSelection = (bookId: string) => {
    if (batchBusy || confirmation) return;
    setSelectedBookIds((current) => current.includes(bookId)
      ? current.filter((item) => item !== bookId) : [...current, bookId]);
  };

  const handleBatchAction = async (action: 'read' | 'unread' | 'delete') => {
    if (batchBusy || !selectedBookIds.length || (action === 'delete' && !isAdmin)) return;
    const version = pageVersion.current;
    const ids = [...selectedBookIds];
    setBatchBusy(true);
    setIsOperationsOpen(false);
    try {
      if (action === 'delete') {
        await bookOperations.current?.deleteBooks(ids, deleteSourceFiles);
      } else {
        await bookOperations.current?.markRead(ids, action === 'read');
      }
      if (version !== pageVersion.current) return;
      setConfirmation(null);
      setSelectedBookIds([]);
      if (action === 'delete') setIsSelectionMode(false);
      await fetchSeries();
    } catch (error) {
      console.error('Failed to update selected series books', error);
      if (version === pageVersion.current) {
        alert(apiErrorMessage(error, t(action === 'delete' ? 'bookshelf.deleteBookFailed' : 'common.saveFailed')));
        setConfirmation(null);
        // Refresh any batches that completed before a later batch failed.
        await fetchSeries();
      }
    } finally {
      if (version === pageVersion.current) setBatchBusy(false);
    }
  };

  const cancelEditing = () => {
    setIsEditing(false);
    void fetchSeries();
  };

  const selectedBooks = books.filter((book) => selectedBookIds.includes(book.id));
  const bookForDeletion: Book | null = selectedBooks.length > 1 ? {
    ...selectedBooks[0],
    id: 'bulk',
    title: selectedBooks.length.toString(),
    library_type: selectedBooks.some((book) => book.library_type === 'local')
      ? 'local' : selectedBooks[0].library_type,
  } : selectedBooks[0] || null;

  if (loading) return <div className="p-8 text-center">{t('common.loading')}</div>;
  if (!series) return <div className="p-8 text-center">{t('bookshelf.seriesNotFound')}</div>;

  return (
    <div className="flex-1 p-4 sm:p-6 md:p-8 space-y-8">
      {/* Header */}
      <div className="flex flex-col min-[880px]:flex-row min-[880px]:items-center justify-between gap-4">
        <div className="flex items-center gap-4 min-w-0">
          <button 
            onClick={() => isEditing ? cancelEditing() : navigate(-1)}
            className="p-2 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-full shrink-0"
            title={t('common.back')}
          >
            <ArrowLeft size={24} />
          </button>
          <h1 className="text-2xl font-bold dark:text-white truncate">
            {isEditing ? t('bookshelf.manageSeries') : series.title}
          </h1>
        </div>
        {!isEditing && (
          <div className="flex flex-wrap items-center justify-end gap-2">
            {isSelectionMode ? (
              <>
                <span className="text-sm font-medium text-slate-600 dark:text-slate-400 whitespace-nowrap">
                  {t('bookshelf.selectedCount', { count: selectedBookIds.length })}
                </span>
                <button
                  onClick={() => setSelectedBookIds(selectedBookIds.length === books.length ? [] : books.map((book) => book.id))}
                  disabled={batchBusy}
                  title={t('bookshelf.selectAllCurrent')}
                  className="flex items-center gap-2 px-3 py-2 bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-400 rounded-xl text-sm font-bold hover:bg-slate-200 dark:hover:bg-slate-700 disabled:opacity-50"
                >
                  <CheckSquare size={18} /> {t('bookshelf.selectAll')}
                </button>
                <div ref={operationsRef} className="relative">
                  <button
                    onClick={() => setIsOperationsOpen(!isOperationsOpen)}
                    aria-haspopup="menu"
                    aria-expanded={isOperationsOpen}
                    disabled={batchBusy}
                    className="flex items-center gap-2 px-3 sm:px-4 py-2 bg-primary-600 hover:bg-primary-700 text-white rounded-xl text-sm font-bold shadow-lg shadow-primary-500/30 disabled:opacity-50"
                  >
                    {t('bookshelf.batchOperations')} <ChevronDown size={14} />
                  </button>
                  {isOperationsOpen && (
                    <div role="menu" className="absolute right-0 top-full mt-2 w-60 max-w-[calc(100vw-2rem)] bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-2xl shadow-2xl z-50 p-1.5">
                      {!selectedBookIds.length ? (
                        <p className="px-4 py-3 text-xs text-center text-slate-400">{t('bookshelf.selectBooksFirst')}</p>
                      ) : (
                        <>
                          <button role="menuitem" onClick={() => void handleBatchAction('read')} className="w-full flex items-center gap-2 px-4 py-2.5 text-left text-sm font-semibold rounded-xl text-slate-700 dark:text-slate-200 hover:bg-slate-50 dark:hover:bg-slate-800">
                            <BookCheck size={16} /> {t('bookshelf.markRead')}
                          </button>
                          <button role="menuitem" onClick={() => { setIsOperationsOpen(false); setConfirmation('unread'); }} className="w-full flex items-center gap-2 px-4 py-2.5 text-left text-sm font-semibold rounded-xl text-slate-700 dark:text-slate-200 hover:bg-slate-50 dark:hover:bg-slate-800">
                            <BookX size={16} /> {t('bookshelf.markUnread')}
                          </button>
                          {isAdmin && (
                            <>
                              <div className="mx-3 my-1 border-t border-slate-100 dark:border-slate-800" />
                              <button role="menuitem" onClick={() => { setIsOperationsOpen(false); setDeleteSourceFiles(false); setConfirmation('delete'); }} className="w-full flex items-center gap-2 px-4 py-2.5 text-left text-sm font-semibold rounded-xl text-red-600 hover:bg-red-50 dark:hover:bg-red-950/20">
                                <Trash2 size={16} /> {t('common.delete')}
                              </button>
                            </>
                          )}
                        </>
                      )}
                    </div>
                  )}
                </div>
                <button onClick={exitSelectionMode} disabled={batchBusy} title={t('common.cancel')} className="p-2.5 bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-400 rounded-xl disabled:opacity-50">
                  <X size={20} />
                </button>
              </>
            ) : (
              <button
                onClick={() => { setShowFilterMenu(false); setIsSelectionMode(true); }}
                className="flex items-center gap-2 px-3 sm:px-4 py-2.5 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl text-slate-600 dark:text-slate-400 hover:bg-slate-50 dark:hover:bg-slate-800 text-sm font-medium"
              >
                <Layers size={18} />
                <span className="sm:hidden">{t('bookshelf.select')}</span>
                <span className="hidden sm:inline">{t('bookshelf.selectionMode')}</span>
              </button>
            )}
            {!isSelectionMode && <>
            <DisplaySettingsMenu
              open={showFilterMenu}
              onOpenChange={setShowFilterMenu}
              sheetLabel={t('bookshelf.closeSeriesDisplaySettings')}
              sections={[
                {
                  title: t('bookshelf.viewMode'),
                  value: viewMode,
                  options: [
                    { value: 'grid', label: t('bookshelf.gridView') },
                    { value: 'list', label: t('bookshelf.listView') },
                  ],
                  onChange: (value) => handleViewModeChange(value as 'grid' | 'list'),
                },
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
                title={t('bookshelf.manageSeries')}
                className="p-2.5 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl text-slate-600 dark:text-slate-400 hover:bg-slate-50 dark:hover:bg-slate-800 transition-colors"
              >
                <Settings size={20} />
              </button>
            )}
            </>}
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
                <button disabled={saving} onClick={handleUpdate} className="flex-1 bg-primary-600 text-white py-2 rounded flex items-center justify-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed">
                  <Save size={18} /> {t('common.save')}
                </button>
                <button disabled={saving} onClick={cancelEditing} className="flex-1 bg-slate-200 dark:bg-slate-700 py-2 rounded disabled:opacity-50">
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
                    <div className={viewMode === 'list' ? 'overflow-hidden rounded-2xl border border-slate-100 bg-white divide-y divide-slate-100 dark:border-slate-800 dark:bg-slate-900 dark:divide-slate-800' : `grid ${getGridCols()}`}>
                        {getSortedBooks().map((book) => (
                            <div key={book.id} className="relative">
                              {viewMode === 'list' ? (
                                <BookshelfListItem
                                  item={book}
                                  kind="book"
                                  coverShape={coverShape}
                                  iconSize={iconSize}
                                  selected={selectedBookIds.includes(book.id)}
                                  onSelect={isSelectionMode ? () => toggleBookSelection(book.id) : undefined}
                                />
                              ) : (
                                <>
                                  {isSelectionMode && (
                                    <div className={`absolute top-2 right-2 z-30 w-6 h-6 rounded-full border-2 flex items-center justify-center pointer-events-none ${selectedBookIds.includes(book.id) ? 'bg-primary-600 border-primary-600 text-white' : 'bg-white/80 border-slate-300 dark:bg-slate-900/80 dark:border-slate-600'}`}>
                                      {selectedBookIds.includes(book.id) && <Check size={14} strokeWidth={3} />}
                                    </div>
                                  )}
                                  <div className={isSelectionMode && !selectedBookIds.includes(book.id) ? 'opacity-60 grayscale-[0.5]' : ''}>
                                    <BookCard
                                      book={book}
                                      coverShape={coverShape}
                                      disableLink={isSelectionMode}
                                      onClick={isSelectionMode ? () => toggleBookSelection(book.id) : undefined}
                                    />
                                  </div>
                                </>
                              )}
                            </div>
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
      
      {confirmation === 'unread' && (
        <div className="fixed inset-0 z-[400] flex items-center justify-center bg-black/50 p-4 backdrop-blur-sm" onMouseDown={(event) => { if (event.target === event.currentTarget && !batchBusy) setConfirmation(null); }}>
          <section role="dialog" aria-modal="true" aria-labelledby="series-batch-title" aria-describedby="series-batch-description" className="w-full max-w-md rounded-3xl border border-slate-100 bg-white p-6 shadow-xl dark:border-slate-800 dark:bg-slate-900">
            <div className="mb-4 flex h-11 w-11 items-center justify-center rounded-2xl bg-primary-50 text-primary-600 dark:bg-primary-950/50 dark:text-primary-400">
              <BookX size={22} />
            </div>
            <h2 id="series-batch-title" className="text-lg font-bold text-slate-900 dark:text-white">{t('bookshelf.confirmUnreadTitle')}</h2>
            <p id="series-batch-description" className="mt-2 text-sm leading-relaxed text-slate-500 dark:text-slate-400">{t('bookshelf.confirmUnreadMessage', { count: selectedBookIds.length })}</p>
            <div className="mt-6 flex justify-end gap-3">
              <button autoFocus onClick={() => setConfirmation(null)} disabled={batchBusy} className="rounded-xl border border-slate-200 px-4 py-2.5 text-sm font-medium text-slate-600 disabled:opacity-50 dark:border-slate-700 dark:text-slate-300">{t('common.cancel')}</button>
              <button onClick={() => void handleBatchAction('unread')} disabled={batchBusy} className="rounded-xl bg-primary-600 px-4 py-2.5 text-sm font-semibold text-white hover:bg-primary-700 disabled:opacity-50">{batchBusy ? t('common.loading') : t('bookshelf.markUnread')}</button>
            </div>
          </section>
        </div>
      )}

      {confirmation === 'delete' && bookForDeletion && (
        <DeleteLibraryItemModal
          book={bookForDeletion}
          deleting={batchBusy}
          deleteSourceFiles={deleteSourceFiles}
          onToggleDeleteSourceFiles={() => { if (!batchBusy) setDeleteSourceFiles(!deleteSourceFiles); }}
          onClose={() => { if (!batchBusy) setConfirmation(null); }}
          onConfirm={() => void handleBatchAction('delete')}
        />
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
        <DeleteLibraryItemModal
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
