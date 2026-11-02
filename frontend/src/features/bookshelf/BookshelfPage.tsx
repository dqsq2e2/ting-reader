import { apiErrorMessage } from '../../core/api/errors';
import { useBookOperations } from '../../core/hooks/useBooks';
import { libraryCacheIdentity, libraryViewCache } from '../../core/api/libraryCache';
import React, { useEffect, useState } from 'react';
import { Link } from 'react-router';
import { useTranslation } from 'react-i18next';
import apiClient from '../../core/api/client';
import type { Book, Library, Series } from '../../core/types';
import BookCard from '../../shared/cards/BookCard';
import SeriesCard from '../../shared/cards/SeriesCard';
import SeriesModal from '../../shared/modals/SeriesModal';
import DeleteLibraryItemModal from './bookDetail/DeleteLibraryItemModal';
import DisplaySettingsMenu from '../../shared/widgets/DisplaySettingsMenu';
import { Search, Database, Plus, Library as LibraryIcon, Layers, Check, X, CheckSquare, ChevronDown, Trash2, BookCheck, BookX } from 'lucide-react';
import { usePlayerStore } from '../../core/stores/playerStore';
import { useAuthStore } from '../../core/stores/authStore';
import { getPinyinInitial } from '../../core/utils/pinyin';
import { localeCompare } from '../../core/utils/locale';
import { publishBookshelfCoverShape } from '../../core/hooks/useBookshelfCoverShape';
import BookshelfListItem from './BookshelfListItem';

type ShelfLibrary = Pick<Library, 'id' | 'name' | 'library_type'>;
interface ShelfSnapshot {
  books: Book[];
  series: Series[];
  libraries: ShelfLibrary[];
  libraryId: string;
  sortBy: 'createdAt' | 'title' | 'author' | 'year';
  iconSize: 'small' | 'medium' | 'large';
  coverShape: 'rect' | 'square';
  viewMode: 'grid' | 'list';
}

const BookshelfPage: React.FC = () => {
  const identity = useAuthStore(state => libraryCacheIdentity(state.serverUrl || state.activeUrl, state.token ? state.user?.id : undefined));
  return <BookshelfContents key={identity} cacheIdentity={identity} />;
};

const BookshelfContents: React.FC<{ cacheIdentity: string }> = ({ cacheIdentity }) => {
  const [snapshot] = useState(() => libraryViewCache.get<ShelfSnapshot>(cacheIdentity, 'shelf'));
  const [savedView] = useState(() => libraryViewCache.get<{ count: number; scroll: number }>(cacheIdentity, 'shelf-view'));
  const dataRequest = React.useRef<AbortController | null>(null);
  const loadedLibrary = React.useRef(snapshot?.libraryId);
  const bookOperations = useBookOperations();
  const { t } = useTranslation();
  const currentChapter = usePlayerStore((state) => state.currentChapter);
  const user = useAuthStore((state) => state.user);
  const isAdmin = user?.role === 'admin';
  const [books, setBooks] = useState<Book[]>(snapshot?.books ?? []);
  const [series, setSeries] = useState<Series[]>(snapshot?.series ?? []);
  const [libraries, setLibraries] = useState<ShelfLibrary[]>(snapshot?.libraries ?? []);
  const [selectedLibraryId, setSelectedLibraryId] = useState<string>(snapshot?.libraryId ?? '');
  const [loading, setLoading] = useState(!snapshot);
  const [sortBy, setSortBy] = useState<ShelfSnapshot['sortBy']>(snapshot?.sortBy ?? 'createdAt');
  const [iconSize, setIconSize] = useState<ShelfSnapshot['iconSize']>(snapshot?.iconSize ?? 'medium');
  const [coverShape, setCoverShape] = useState<ShelfSnapshot['coverShape']>(snapshot?.coverShape ?? 'square');
  const [viewMode, setViewMode] = useState<ShelfSnapshot['viewMode']>(snapshot?.viewMode ?? 'grid');
  const [showFilterMenu, setShowFilterMenu] = useState(false);
  const [settingsLoaded, setSettingsLoaded] = useState(!!snapshot);
  
  // Alphabet Index State
  const [activeLetter, setActiveLetter] = useState<string | null>(null);
  const [isTouchingIndex, setIsTouchingIndex] = useState(false);
  
  // Selection mode for creating series and bulk actions
  const [isSelectionMode, setIsSelectionMode] = useState(false);
  const [selectedBookIds, setSelectedBookIds] = useState<string[]>([]);
  const [selectedSeriesIds, setSelectedSeriesIds] = useState<string[]>([]);
  const [isSeriesModalOpen, setIsSeriesModalOpen] = useState(false);
  const [isOperationsOpen, setIsOperationsOpen] = useState(false);
  const [isDeleteBookOpen, setIsDeleteBookOpen] = useState(false);
  const [isDeleteSeriesOpen, setIsDeleteSeriesOpen] = useState(false);
  const [deletingBooks, setDeletingBooks] = useState(false);
  const [deletingSeries, setDeletingSeries] = useState(false);
  const [deleteSourceFiles, setDeleteSourceFiles] = useState(false);
  const [savingReadStatus, setSavingReadStatus] = useState(false);
  const [confirmUnread, setConfirmUnread] = useState(false);

  useEffect(() => {
    if (!confirmUnread) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !savingReadStatus) setConfirmUnread(false);
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [confirmUnread, savingReadStatus]);

  // Lazy loading state
  const [visibleCount, setVisibleCount] = useState(savedView?.count ?? 50);
  const loadMoreRef = React.useRef<HTMLDivElement>(null);
  const visibleCountRef = React.useRef(visibleCount);
  useEffect(() => { visibleCountRef.current = visibleCount; }, [visibleCount]);
  React.useLayoutEffect(() => {
    const container = document.getElementById('main-content');
    if (!container) return;
    const scroll = savedView?.scroll ?? 0;
    container.scrollTo({ top: scroll, behavior: 'instant' });
    let latestScroll = scroll;
    const record = () => { latestScroll = container.scrollTop; };
    container.addEventListener('scroll', record, { passive: true });
    return () => {
      container.removeEventListener('scroll', record);
      libraryViewCache.set(cacheIdentity, 'shelf-view', { count: visibleCountRef.current, scroll: latestScroll });
    };
  }, [cacheIdentity, savedView]);

  useEffect(() => {
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0].isIntersecting) {
          setVisibleCount((prev) => prev + 50);
        }
      },
      { threshold: 0.1 }
    );

    const currentRef = loadMoreRef.current;

    if (currentRef) {
      observer.observe(currentRef);
    }

    return () => {
      if (currentRef) {
        observer.unobserve(currentRef);
      }
    };
  }, [books.length, series.length, sortBy, selectedLibraryId]);

  // Reset visible count when filters change
  const previousFilter = React.useRef(`${sortBy}:${selectedLibraryId}`);
  useEffect(() => {
    const filter = `${sortBy}:${selectedLibraryId}`;
    if (previousFilter.current === filter) return;
    previousFilter.current = filter;
    const timer = window.setTimeout(() => setVisibleCount(50), 0);
    return () => window.clearTimeout(timer);
  }, [sortBy, selectedLibraryId]);

  useEffect(() => {
    const request = new AbortController();
    const loadSettings = async () => {
      try {
        const res = await apiClient.get('/api/settings', { signal: request.signal });
        if (request.signal.aborted) return;
        const settings = res.data.settings_json || {};
        
        setSelectedLibraryId(settings.bookshelf_library_id || '');
        if (settings.bookshelf_sort_by) {
          setSortBy(settings.bookshelf_sort_by);
        }
        if (settings.bookshelf_icon_size) {
          setIconSize(settings.bookshelf_icon_size);
        }
        if (settings.bookshelf_cover_shape) {
          setCoverShape(settings.bookshelf_cover_shape);
        }
        setViewMode(settings.bookshelf_view_mode === 'list' ? 'list' : 'grid');
      } catch (err) {
        if (!request.signal.aborted) console.error('Failed to load bookshelf settings', err);
      } finally {
        if (!request.signal.aborted) setSettingsLoaded(true);
      }
    };
    loadSettings();
    return () => request.abort();
  }, []);

  // Close operations menu when clicking outside
  useEffect(() => {
    if (!isOperationsOpen) return;
    
    const handleOutsideClick = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      if (!target.closest('.operations-dropdown-container')) {
        setIsOperationsOpen(false);
      }
    };
    
    const timer = setTimeout(() => {
      document.addEventListener('click', handleOutsideClick);
    }, 50);
    
    return () => {
      clearTimeout(timer);
      document.removeEventListener('click', handleOutsideClick);
    };
  }, [isOperationsOpen]);

  const handleLibraryChange = (newId: string) => {
    setSelectedLibraryId(newId);
    apiClient.post('/api/settings', { bookshelf_library_id: newId });
  };

  const handleSortChange = (newSort: 'createdAt' | 'title' | 'author' | 'year') => {
    setSortBy(newSort);
    setShowFilterMenu(false);
    apiClient.post('/api/settings', { bookshelf_sort_by: newSort });
  };

  const handleIconSizeChange = (newSize: 'small' | 'medium' | 'large') => {
    setIconSize(newSize);
    setShowFilterMenu(false);
    apiClient.post('/api/settings', { bookshelf_icon_size: newSize });
  };

  const handleCoverShapeChange = (newShape: 'rect' | 'square') => {
    setCoverShape(newShape);
    setShowFilterMenu(false);
    publishBookshelfCoverShape(newShape);
    apiClient.post('/api/settings', { bookshelf_cover_shape: newShape });
  };

  const handleViewModeChange = (newMode: 'grid' | 'list') => {
    setViewMode(newMode);
    setShowFilterMenu(false);
    apiClient.post('/api/settings', { bookshelf_view_mode: newMode });
  };

  const fetchData = async () => {
    dataRequest.current?.abort();
    const request = new AbortController();
    dataRequest.current = request;
    setLoading(loadedLibrary.current !== selectedLibraryId);
    try {
      // 1. Fetch Libraries first
      const libsRes = await apiClient.get('/api/libraries', { signal: request.signal });
      if (request.signal.aborted) return;
      const libs = libsRes.data;
      setLibraries(libs);

      // 2. Determine effective library ID
      let effectiveLibraryId = selectedLibraryId;
      
      // If we have a selected ID but it's not in the fetched libraries, reset it
      if (selectedLibraryId) {
        const exists = libs.find((l: Library) => l.id === selectedLibraryId);
        if (!exists) {
          console.warn(`Selected library ${selectedLibraryId} was not found. Resetting to default.`);
          effectiveLibraryId = '';
          setSelectedLibraryId('');
          // Update settings to clear the invalid ID
          apiClient.post('/api/settings', { bookshelf_library_id: '' });
        }
      }

      // 3. Fetch Books & Series with effective ID
      const [booksRes, seriesRes] = await Promise.all([
        apiClient.get('/api/books', { signal: request.signal, params: { library_id: effectiveLibraryId || undefined } }),
        apiClient.get('/api/v1/series', { signal: request.signal, params: { library_id: effectiveLibraryId || undefined } })
      ]);
      if (request.signal.aborted) return;
      loadedLibrary.current = effectiveLibraryId;
      setBooks(booksRes.data);
      setSeries(seriesRes.data);
    } catch (err) {
      if (!request.signal.aborted) console.error('Failed to fetch bookshelf data', err);
    } finally {
      if (!request.signal.aborted) setLoading(false);
    }
  };

  useEffect(() => {
    if (settingsLoaded) {
      const timer = window.setTimeout(() => void fetchData(), 0);
      return () => { window.clearTimeout(timer); dataRequest.current?.abort(); };
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedLibraryId, settingsLoaded]);

  useEffect(() => {
    if (loading || loadedLibrary.current !== selectedLibraryId) return;
    libraryViewCache.set(cacheIdentity, 'shelf', {
      books, series,
      libraries: libraries.map(({ id, name, library_type }) => ({ id, name, library_type })),
      libraryId: selectedLibraryId, sortBy, iconSize, coverShape, viewMode,
    } satisfies ShelfSnapshot);
  }, [cacheIdentity, books, series, libraries, selectedLibraryId, sortBy, iconSize, coverShape, viewMode, loading]);

  const sortedBooks = [...books].sort((a, b) => {
    if (sortBy === 'title') return localeCompare(a.title, b.title);
    if (sortBy === 'author') return localeCompare(a.author || '', b.author || '');
    if (sortBy === 'year') {
      const yearA = a.year || 0;
      const yearB = b.year || 0;
      return yearB - yearA; // Descending order (newest first)
    }
    return new Date(b.created_at).getTime() - new Date(a.created_at).getTime();
  });

  // Collect all book IDs that are in a series
  const booksInSeries = new Set(series.flatMap(s => s.books?.map(b => b.id) || []));

  const filteredBooks = sortedBooks.filter(book => !booksInSeries.has(book.id));

  const filteredSeries = series;

  const toggleBookSelection = (id: string) => {
    setSelectedBookIds(prev => 
      prev.includes(id) ? prev.filter(i => i !== id) : [...prev, id]
    );
  };

  const toggleSeriesSelection = (id: string) => {
    setSelectedSeriesIds(prev => 
      prev.includes(id) ? prev.filter(i => i !== id) : [...prev, id]
    );
  };

  const exitSelectionMode = () => {
    setIsSelectionMode(false);
    setSelectedBookIds([]);
    setSelectedSeriesIds([]);
    setIsOperationsOpen(false);
    fetchData();
  };

  const handleSelectAll = () => {
    if (filteredBooks.length === 0 && filteredSeries.length === 0) return;
    
    const allBooksSelected = filteredBooks.every(b => selectedBookIds.includes(b.id));
    const allSeriesSelected = filteredSeries.every(s => selectedSeriesIds.includes(s.id));
    
    if (allBooksSelected && allSeriesSelected) {
      setSelectedBookIds([]);
      setSelectedSeriesIds([]);
    } else {
      setSelectedBookIds(filteredBooks.map(b => b.id));
      setSelectedSeriesIds(filteredSeries.map(s => s.id));
    }
  };

  const handleMarkRead = async (read: boolean) => {
    if (selectedBookIds.length === 0 || savingReadStatus) return;
    setSavingReadStatus(true);
    try {
      await bookOperations.current?.markRead(selectedBookIds, read);
      setConfirmUnread(false);
      await fetchData();
      setSelectedBookIds([]);
    } catch (err) {
      console.error('Failed to update book read status', err);
      alert(apiErrorMessage(err, t('common.saveFailed')));
    } finally {
      setSavingReadStatus(false);
    }
  };

  const handleDeleteClick = () => {
    setDeleteSourceFiles(false);
    if (selectedSeriesIds.length > 0) {
      setIsDeleteSeriesOpen(true);
    } else if (selectedBookIds.length > 0) {
      setIsDeleteBookOpen(true);
    }
  };

  const handleDeleteBooks = async () => {
    try {
      setDeletingBooks(true);
      await bookOperations.current?.deleteBooks(selectedBookIds, deleteSourceFiles);
      exitSelectionMode();
      fetchData();
    } catch (err) {
      console.error('Failed to delete books', err);
      alert(apiErrorMessage(err, t('bookshelf.deleteBookFailed')));
    } finally {
      setDeletingBooks(false);
      setIsDeleteBookOpen(false);
    }
  };

  const handleDeleteSeries = async () => {
    try {
      setDeletingSeries(true);
      await bookOperations.current?.deleteSeries(selectedSeriesIds);
      setIsDeleteSeriesOpen(false);
      if (selectedBookIds.length > 0) {
        setDeleteSourceFiles(false);
        setIsDeleteBookOpen(true);
      } else {
        exitSelectionMode();
        fetchData();
      }
    } catch (err) {
      console.error('Failed to delete series', err);
      alert(apiErrorMessage(err, t('bookshelf.deleteSeriesFailed')));
      setIsDeleteSeriesOpen(false);
    } finally {
      setDeletingSeries(false);
    }
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

  // Group items by first letter if sorting by title or author, or by year if sorting by year
  const groupedItems = React.useMemo(() => {
    if (sortBy === 'createdAt') return null;

    const groups: Record<string, (Book | Series)[]> = {};
    const otherKey = '#';

    // Process books
    filteredBooks.forEach(book => {
      let key = '';
      if (sortBy === 'title') {
        key = getPinyinInitial(book.title);
      } else if (sortBy === 'author') {
        key = getPinyinInitial(book.author || '');
      } else if (sortBy === 'year') {
        const year = book.year;
        key = year ? String(year).slice(-2) : otherKey; // Use last 2 digits (e.g., "24" for 2024)
      }
      if (!groups[key]) groups[key] = [];
      groups[key].push(book);
    });

    // Process series
    filteredSeries.forEach(series => {
      let key = '';
      if (sortBy === 'title') {
        key = getPinyinInitial(series.title);
      } else if (sortBy === 'author') {
        key = getPinyinInitial(series.author || '');
      } else if (sortBy === 'year') {
        // For series, we could use the year of the first book if available
        // For now, skip series in year sorting or use a default
        key = otherKey;
      }
      if (!groups[key]) groups[key] = [];
      groups[key].push(series);
    });

    // Sort items within each group
    Object.keys(groups).forEach(key => {
      groups[key].sort((a, b) => {
        if (sortBy === 'title') return localeCompare(a.title, b.title);
        if (sortBy === 'author') return localeCompare(a.author || '', b.author || '');
        if (sortBy === 'year') {
          const yearA = 'year' in a ? (a.year || 0) : 0;
          const yearB = 'year' in b ? (b.year || 0) : 0;
          return yearB - yearA; // Descending order within group
        }
        return 0;
      });
    });

    const sortedKeys = Object.keys(groups).sort((a, b) => {
        if (a === otherKey) return 1;
        if (b === otherKey) return -1;
        if (sortBy === 'year') {
          // For year sorting, sort keys numerically in descending order
          return Number(b) - Number(a);
        }
        return localeCompare(a, b);
    });

    return { groups, sortedKeys };
  }, [filteredBooks, filteredSeries, sortBy]);

  const visibleGroupedItems = React.useMemo(() => {
    if (!groupedItems) return null;
    let count = 0;
    const newGroups: Record<string, (Book | Series)[]> = {};
    const newSortedKeys: string[] = [];

    for (const key of groupedItems.sortedKeys) {
      if (count >= visibleCount) break;
      const itemsInGroup = groupedItems.groups[key];
      const itemsToTake = Math.min(itemsInGroup.length, visibleCount - count);
      
      if (itemsToTake > 0) {
        newGroups[key] = itemsInGroup.slice(0, itemsToTake);
        newSortedKeys.push(key);
        count += itemsToTake;
      }
    }
    return { groups: newGroups, sortedKeys: newSortedKeys };
  }, [groupedItems, visibleCount]);

  const visibleSeries = filteredSeries.slice(0, visibleCount);
  const remainingCount = Math.max(0, visibleCount - visibleSeries.length);
  const visibleBooks = filteredBooks.slice(0, remainingCount);

  const scrollToGroup = (key: string) => {
    setActiveLetter(key);
    const element = document.getElementById(`group-${key}`);
    const container = document.getElementById('main-content');
    
    if (element && container) {
      const containerRect = container.getBoundingClientRect();
      const elementRect = element.getBoundingClientRect();
      const offset = elementRect.top - containerRect.top + container.scrollTop;
      
      // Mobile header height (64px) + padding or Desktop padding
      const headerOffset = window.innerWidth < 1280 ? 80 : 20;
      
      container.scrollTo({ top: offset - headerOffset, behavior: 'auto' });
    }
  };

  const handleTouchMove = (e: React.TouchEvent) => {
    e.preventDefault();
    const touch = e.touches[0];
    const element = document.elementFromPoint(touch.clientX, touch.clientY);
    const key = element?.getAttribute('data-key');
    if (key && key !== activeLetter) {
      scrollToGroup(key);
    }
  };

  const mockBookForDeletion = React.useMemo(() => {
    if (selectedBookIds.length === 0) return null;
    const selectedBooks = selectedBookIds
      .map(id => books.find(b => b.id === id))
      .filter((book): book is Book => Boolean(book));
    if (selectedBookIds.length === 1) {
      return selectedBooks[0] || null;
    }
    const firstBook = selectedBooks[0];
    const hasLocalBook = selectedBooks.some(book => book.library_type === 'local');
    return {
      id: 'bulk',
      title: selectedBookIds.length.toString(),
      library_type: hasLocalBook ? 'local' : firstBook?.library_type,
    } as Book;
  }, [selectedBookIds, books]);

  const mockSeriesForDeletion = React.useMemo(() => {
    if (selectedSeriesIds.length === 0) return null;
    if (selectedSeriesIds.length === 1) {
      return series.find(s => s.id === selectedSeriesIds[0]) || null;
    }
    return {
      id: 'bulk',
      title: selectedSeriesIds.length.toString(),
    } as Series;
  }, [selectedSeriesIds, series]);

  const shelfLayoutClass = viewMode === 'list'
    ? 'divide-y divide-slate-100 overflow-hidden rounded-2xl border border-slate-100 bg-white dark:divide-slate-800 dark:border-slate-800 dark:bg-slate-900'
    : `grid ${getGridCols()}`;

  const renderShelfItem = (item: Book | Series, kind: 'book' | 'series') => {
    const selected = (kind === 'series' ? selectedSeriesIds : selectedBookIds).includes(item.id);
    const onSelect = () => kind === 'series' ? toggleSeriesSelection(item.id) : toggleBookSelection(item.id);
    if (viewMode === 'list') {
      return (
        <BookshelfListItem key={`${kind}-${item.id}`} item={item} kind={kind}
          coverShape={coverShape} iconSize={iconSize} selected={selected}
          onSelect={isSelectionMode ? onSelect : undefined} />
      );
    }
    const card = kind === 'series'
      ? <SeriesCard series={item as Series} coverShape={coverShape} onClick={isSelectionMode ? onSelect : undefined} />
      : <BookCard book={item as Book} coverShape={coverShape} disableLink={isSelectionMode} onClick={isSelectionMode ? onSelect : undefined} />;
    return (
      <div key={`${kind}-${item.id}`} className="relative">
        {isSelectionMode ? (
          <>
            <div className={`absolute top-2 right-2 z-30 w-6 h-6 rounded-full border-2 flex items-center justify-center transition-all pointer-events-none ${selected ? 'bg-primary-600 border-primary-600 text-white' : 'bg-white/80 dark:bg-slate-900/80 border-slate-300 dark:border-slate-600'}`}>
              {selected && <Check size={14} />}
            </div>
            <div className={`transition-opacity duration-200 ${selected ? 'opacity-100' : 'opacity-60 grayscale-[0.5]'}`}>{card}</div>
          </>
        ) : card}
      </div>
    );
  };

  if (loading) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center min-h-full">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-primary-600"></div>
        <p className="mt-4 text-sm text-slate-500 dark:text-slate-400">{t('common.loading')}</p>
      </div>
    );
  }

  return (
    <div className="flex-1 min-h-full flex flex-col p-4 sm:p-6 md:p-8">
      <div className="flex-1 space-y-6">
        <div className="flex flex-col min-[880px]:flex-row min-[880px]:items-center justify-between gap-4 mb-2">
          <div>
            <h1 className="text-2xl md:text-3xl font-bold text-slate-900 dark:text-white flex items-center gap-3">
              <LibraryIcon className="text-primary-600" />
              {t('bookshelf.myBookshelf')}
            </h1>
            <p className="text-sm md:text-base text-slate-500 dark:text-slate-400 mt-1">{t('bookshelf.subtitle')}</p>
          </div>
          
          <div className="flex flex-wrap min-[550px]:flex-nowrap items-center gap-2 sm:gap-3 w-full min-[880px]:w-auto justify-end">
            {isSelectionMode ? (
              <div className="flex items-center gap-2 order-1 relative">
                <button
                  onClick={handleSelectAll}
                  className="flex items-center gap-2 px-3 py-2 bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-400 rounded-xl text-sm font-bold hover:bg-slate-200 dark:hover:bg-slate-700 transition-colors shrink-0"
                  title={t('bookshelf.selectAllCurrent')}
                >
                  <CheckSquare size={18} />
                  <span>{t('bookshelf.selectAll')}</span>
                </button>
                  <div className="relative operations-dropdown-container">
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        setIsOperationsOpen(!isOperationsOpen);
                      }}
                      aria-haspopup="menu"
                      aria-expanded={isOperationsOpen}
                      disabled={savingReadStatus}
                      className="flex items-center gap-2 px-3 sm:px-4 py-2 bg-primary-600 hover:bg-primary-700 text-white rounded-xl text-sm font-bold shadow-lg shadow-primary-500/30 whitespace-nowrap shrink-0 transition-all active:scale-95"
                    >
                      <span className="tabular-nums" aria-live="polite" aria-atomic="true">
                        {t('bookshelf.batchOperationsWithCount', { count: selectedBookIds.length + selectedSeriesIds.length })}
                      </span>
                      <ChevronDown size={14} className={`transition-transform duration-200 ${isOperationsOpen ? 'rotate-180' : ''}`} />
                    </button>
                    {isOperationsOpen && (
                      <div role="menu" className="absolute right-0 top-full mt-2 w-60 max-w-[calc(100vw-2rem)] bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-2xl shadow-2xl z-50 overflow-hidden animate-in fade-in zoom-in-95 duration-200">
                        <div className="p-1.5 space-y-1">
                          {selectedBookIds.length === 0 && selectedSeriesIds.length === 0 ? (
                            <div className="px-4 py-3 text-xs text-center text-slate-400 dark:text-slate-500 font-medium">
                              {t('bookshelf.selectItemsFirst', '请先在下方勾选书籍或系列')}
                            </div>
                          ) : (
                            <>
                              <button
                                role="menuitem"
                                onClick={() => {
                                  setIsOperationsOpen(false);
                                  void handleMarkRead(true);
                                }}
                                disabled={selectedBookIds.length === 0 || savingReadStatus}
                                className="w-full text-left px-4 py-2.5 text-sm font-semibold rounded-xl transition-colors text-slate-700 hover:bg-slate-50 dark:text-slate-200 dark:hover:bg-slate-800 flex items-center gap-2 disabled:opacity-50"
                              >
                                <BookCheck size={16} />
                                <span>{t('bookshelf.markRead')}</span>
                              </button>
                              <button
                                role="menuitem"
                                onClick={() => {
                                  setIsOperationsOpen(false);
                                  setConfirmUnread(true);
                                }}
                                disabled={selectedBookIds.length === 0 || savingReadStatus}
                                className="w-full text-left px-4 py-2.5 text-sm font-semibold rounded-xl transition-colors text-slate-700 hover:bg-slate-50 dark:text-slate-200 dark:hover:bg-slate-800 flex items-center gap-2 disabled:opacity-50"
                              >
                                <BookX size={16} />
                                <span>{t('bookshelf.markUnread')}</span>
                              </button>
                              {isAdmin && <>
                              <div className="mx-3 my-1 border-t border-slate-100 dark:border-slate-800" />
                              <button
                                role="menuitem"
                                onClick={() => {
                                  setIsOperationsOpen(false);
                                  setIsSeriesModalOpen(true);
                                }}
                                disabled={selectedBookIds.length === 0 || selectedSeriesIds.length > 0}
                                className="w-full text-left px-4 py-2.5 text-sm font-semibold rounded-xl transition-colors text-slate-700 hover:bg-slate-50 dark:text-slate-200 dark:hover:bg-slate-800 flex items-center gap-2 disabled:opacity-50 disabled:hover:bg-transparent"
                              >
                                <Layers size={16} />
                                <span>{t('bookshelf.createSeries')}</span>
                              </button>
                              <button
                                role="menuitem"
                                onClick={() => {
                                  setIsOperationsOpen(false);
                                  handleDeleteClick();
                                }}
                                className="w-full text-left px-4 py-2.5 text-sm font-semibold rounded-xl transition-colors text-red-600 hover:bg-red-50 dark:hover:bg-red-950/20 flex items-center gap-2"
                              >
                                <Trash2 size={16} />
                                <span>{t('common.delete')}</span>
                              </button>
                              </>}
                            </>
                          )}
                        </div>
                      </div>
                    )}
                  </div>
                <button
                  onClick={exitSelectionMode}
                  className="p-2.5 bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-400 rounded-xl shrink-0"
                >
                  <X size={20} />
                </button>
              </div>
            ) : (
              (
                <button
                  onClick={() => setIsSelectionMode(true)}
                  className="flex items-center gap-2 px-3 sm:px-4 py-2.5 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl text-slate-600 dark:text-slate-400 hover:bg-slate-50 dark:hover:bg-slate-800 transition-colors text-sm font-medium shrink-0 order-1"
                >
                  <Layers size={18} />
                  <span className="sm:hidden">{t('bookshelf.select')}</span>
                  <span className="hidden sm:inline">{t('bookshelf.selectionMode')}</span>
                </button>
              )
            )}

            {/* Library Selector */}
            {libraries.length > 0 && (
              <div className={`relative order-2 w-[9.75rem] sm:w-[12rem] md:w-[14rem] ${isSelectionMode ? 'hidden sm:block' : ''}`}>
                <div className="absolute inset-y-0 left-0 pl-2.5 flex items-center pointer-events-none text-slate-400">
                  <LibraryIcon size={16} />
                </div>
                <select
                  value={selectedLibraryId}
                  onChange={(e) => handleLibraryChange(e.target.value)}
                  className="w-full pl-8 pr-7 py-2.5 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 text-sm font-medium text-slate-700 dark:text-slate-200 appearance-none cursor-pointer truncate"
                >
                  <option value="">{t('bookshelf.allLibraries')}</option>
                  {libraries.map(lib => (
                    <option key={lib.id} value={lib.id}>{lib.name}</option>
                  ))}
                </select>
                <div className="absolute inset-y-0 right-0 pr-2.5 flex items-center pointer-events-none text-slate-400">
                  <svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                  </svg>
                </div>
              </div>
            )}

            <Link
              to="/search"
              className="w-full min-[550px]:w-auto min-[550px]:min-w-48 md:w-64 order-first min-[550px]:order-none inline-flex items-center gap-2 px-4 py-2.5 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl text-slate-500 dark:text-slate-400 hover:text-primary-600 hover:border-primary-200 dark:hover:border-primary-900 transition-colors"
            >
              <Search size={18} className="shrink-0" />
              <span className="text-sm font-medium truncate">{t('bookshelf.searchPlaceholder')}</span>
            </Link>
            <DisplaySettingsMenu
              className="order-3"
              open={showFilterMenu}
              onOpenChange={setShowFilterMenu}
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
                    { value: 'createdAt', label: t('bookshelf.sortRecentlyAdded') },
                    { value: 'title', label: t('bookshelf.sortTitle') },
                    { value: 'author', label: t('bookshelf.sortAuthor') },
                    { value: 'year', label: t('bookshelf.sortYear') },
                  ],
                  onChange: (value) => handleSortChange(value as 'createdAt' | 'title' | 'author' | 'year'),
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
                {
                  title: t('bookshelf.coverShape'),
                  value: coverShape,
                  options: [
                    { value: 'rect', label: t('bookshelf.rectCover') },
                    { value: 'square', label: t('bookshelf.squareCoverDefault') },
                  ],
                  onChange: (value) => handleCoverShapeChange(value as 'rect' | 'square'),
                },
              ]}
            />
          </div>
        </div>

      {books.length > 0 || series.length > 0 ? (
        <>
          {/* Alphabet Scroll Bar */}
          {groupedItems && (
            <>
              {/* Central Big Letter Overlay */}
              {isTouchingIndex && activeLetter && (
                <div className="fixed inset-0 z-50 flex items-center justify-center pointer-events-none">
                  <div className="w-20 h-20 bg-slate-900/50 backdrop-blur-sm rounded-xl flex items-center justify-center text-4xl font-bold text-white shadow-xl">
                    {activeLetter}
                  </div>
                </div>
              )}
              
              <div 
                className="fixed right-2 top-1/2 -translate-y-1/2 z-40 flex flex-col items-center bg-transparent py-2 select-none touch-none"
                onTouchStart={(e) => {
                  e.preventDefault();
                  setIsTouchingIndex(true);
                }}
                onTouchMove={handleTouchMove}
                onTouchEnd={() => {
                  setIsTouchingIndex(false);
                  setTimeout(() => setActiveLetter(null), 1000);
                }}
              >
                {groupedItems.sortedKeys.map(key => (
                  <button
                    key={key}
                    data-key={key}
                    onClick={(e) => {
                      e.preventDefault();
                      scrollToGroup(key);
                      setIsTouchingIndex(true);
                      setTimeout(() => {
                        setIsTouchingIndex(false);
                        setActiveLetter(null);
                      }, 500);
                    }}
                    className={`w-4 h-4 flex items-center justify-center text-[10px] font-medium transition-all cursor-pointer rounded-full my-[1px]
                      ${activeLetter === key && isTouchingIndex
                        ? 'bg-primary-600 text-white scale-125 font-bold shadow-sm' 
                        : 'text-slate-400 hover:text-primary-600 dark:text-slate-500 dark:hover:text-primary-400'
                      }`}
                  >
                    {key}
                  </button>
                ))}
              </div>
            </>
          )}



          {visibleGroupedItems ? (
             // Grouped Layout
             <div className="space-y-6">
               {visibleGroupedItems.sortedKeys.map(key => (
                 <div key={key} id={`group-${key}`}>
                   <div className="text-xs font-bold text-slate-400 dark:text-slate-500 mb-2 pl-1">
                      {key}
                   </div>
                   <div className={shelfLayoutClass}>
                     {visibleGroupedItems.groups[key].map(item => renderShelfItem(item, 'books' in item ? 'series' : 'book'))}
                   </div>
                 </div>
               ))}
             </div>
          ) : (
            // Default Layout (Recent)
            <div className={shelfLayoutClass}>
              {visibleSeries.map(item => renderShelfItem(item, 'series'))}
              {visibleBooks.map(item => renderShelfItem(item, 'book'))}
            </div>
          )}

          {/* Observer target for lazy loading */}
          <div ref={loadMoreRef} className="h-10 w-full" />

          {filteredBooks.length === 0 && (filteredSeries.length === 0 || (isSelectionMode && sortBy === 'createdAt')) && (
            <div className="py-20 text-center">
              <div className="inline-flex items-center justify-center w-20 h-20 rounded-full bg-slate-100 dark:bg-slate-900 text-slate-400 mb-4">
                <Search size={40} />
              </div>
              <h3 className="text-lg font-medium dark:text-white">{t('bookshelf.noDisplayContent')}</h3>
              <p className="text-slate-500 mt-2">{t('bookshelf.noDisplayHint')}</p>
            </div>
          )}
        </>
      ) : (
        <div className="py-20 text-center bg-white dark:bg-slate-900 rounded-3xl border border-slate-100 dark:border-slate-800 shadow-sm">
          <div className="inline-flex items-center justify-center w-20 h-20 rounded-full bg-primary-50 dark:bg-primary-900/20 text-primary-600 mb-6">
            <Database size={40} />
          </div>
          <h3 className="text-xl font-bold dark:text-white mb-2">{t('bookshelf.emptyTitle')}</h3>
          <p className="text-sm text-slate-500 max-w-md mx-auto mb-8">{t('bookshelf.emptyDescription')}</p>
          <Link 
            to="/admin/libraries"
            className="inline-flex items-center gap-2 px-6 py-3 bg-primary-600 hover:bg-primary-700 text-white text-sm font-bold rounded-xl shadow-xl shadow-primary-500/30 transition-all active:scale-95"
          >
            <Plus size={18} />
            {t('bookshelf.configureLibrary')}
          </Link>
        </div>
      )}
      </div>

      {/* Series Creation Modal */}
      {confirmUnread && (
        <div className="fixed inset-0 z-[400] flex items-center justify-center bg-black/50 p-4 backdrop-blur-sm" onMouseDown={(event) => { if (event.target === event.currentTarget && !savingReadStatus) setConfirmUnread(false); }}>
          <section role="dialog" aria-modal="true" aria-labelledby="mark-unread-title" aria-describedby="mark-unread-description" className="w-full max-w-md rounded-3xl border border-slate-100 bg-white p-6 shadow-xl dark:border-slate-800 dark:bg-slate-900">
            <div className="mb-4 flex h-11 w-11 items-center justify-center rounded-2xl bg-primary-50 text-primary-600 dark:bg-primary-950/50 dark:text-primary-400"><BookX size={22} /></div>
            <h2 id="mark-unread-title" className="text-lg font-bold text-slate-900 dark:text-white">{t('bookshelf.confirmUnreadTitle')}</h2>
            <p id="mark-unread-description" className="mt-2 text-sm leading-relaxed text-slate-500 dark:text-slate-400">{t('bookshelf.confirmUnreadMessage', { count: selectedBookIds.length })}</p>
            <div className="mt-6 flex justify-end gap-3">
              <button autoFocus onClick={() => setConfirmUnread(false)} disabled={savingReadStatus} className="rounded-xl border border-slate-200 px-4 py-2.5 text-sm font-medium text-slate-600 disabled:opacity-50 dark:border-slate-700 dark:text-slate-300">{t('common.cancel')}</button>
              <button onClick={() => void handleMarkRead(false)} disabled={savingReadStatus} className="rounded-xl bg-primary-600 px-4 py-2.5 text-sm font-semibold text-white transition-colors hover:bg-primary-700 disabled:opacity-50">{savingReadStatus ? t('common.loading') : t('bookshelf.markUnread')}</button>
            </div>
          </section>
        </div>
      )}

      <SeriesModal
        isOpen={isSeriesModalOpen}
        onClose={() => setIsSeriesModalOpen(false)}
        selectedBooks={books.filter(b => selectedBookIds.includes(b.id))}
        onSuccess={exitSelectionMode}
      />

      {isDeleteBookOpen && mockBookForDeletion && (
        <DeleteLibraryItemModal
          book={mockBookForDeletion}
          deleting={deletingBooks}
          deleteSourceFiles={deleteSourceFiles}
          onToggleDeleteSourceFiles={() => setDeleteSourceFiles(!deleteSourceFiles)}
          onClose={() => setIsDeleteBookOpen(false)}
          onConfirm={handleDeleteBooks}
        />
      )}

      {isDeleteSeriesOpen && mockSeriesForDeletion && (
        <DeleteLibraryItemModal
          series={mockSeriesForDeletion}
          deleting={deletingSeries}
          onClose={() => setIsDeleteSeriesOpen(false)}
          onConfirm={handleDeleteSeries}
        />
      )}

      {/* Dynamic Safe Bottom Spacer */}
      <div 
        className="shrink-0 transition-all duration-300" 
        style={{ height: currentChapter ? 'var(--safe-bottom-with-player)' : 'var(--safe-bottom-base)' }} 
      />
    </div>
  );
};

export default BookshelfPage;
