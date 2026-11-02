import React, { useCallback, useState, useEffect, useRef } from 'react';
import apiClient from '../../core/api/client';
import { getBookshelfGridClass, useBookshelfDisplaySettings } from '../../core/hooks/useBookshelfCoverShape';
import type { Book, Library, Series } from '../../core/types';
import type { ScraperSearchItem, ScraperSource } from '../../core/types/plugin';
import { apiErrorMessage } from '../../core/api/errors';
import BookCard from '../../shared/cards/BookCard';
import { bookPrimaryCredit } from '../../core/utils/book';
import { Search as SearchIcon, Loader2, BookX, BookOpen, Check, Plus, ChevronLeft, ChevronRight, SlidersHorizontal, ChevronUp, ChevronDown, Globe2 } from 'lucide-react';
import { usePlayerStore } from '../../core/stores/playerStore';
import BackButton from '../../shared/widgets/BackButton';
import { useTranslation } from 'react-i18next';

type FilterOption = string | { id: string; name: string };
type SearchMode = 'all' | 'local' | 'online';
type SourceSearchItem = ScraperSearchItem & { source_id: string };

interface FilterRowProps {
  label: string;
  allLabel: string;
  items: FilterOption[];
  selected: string;
  onSelect: (value: string) => void;
  scrollRef: React.RefObject<HTMLDivElement | null>;
}

const scrollFilterRow = (
  ref: React.RefObject<HTMLDivElement | null>,
  direction: 'left' | 'right',
) => {
  ref.current?.scrollBy({
    left: direction === 'left' ? -300 : 300,
    behavior: 'smooth',
  });
};

const FilterRow: React.FC<FilterRowProps> = ({
  label,
  allLabel,
  items,
  selected,
  onSelect,
  scrollRef,
}) => {
  const [canScrollLeft, setCanScrollLeft] = useState(false);
  const [canScrollRight, setCanScrollRight] = useState(false);

  const checkScroll = useCallback(() => {
    const element = scrollRef.current;
    if (!element) return;
    const { scrollLeft, scrollWidth, clientWidth } = element;
    setCanScrollLeft(scrollLeft > 0);
    setCanScrollRight(scrollLeft < scrollWidth - clientWidth - 1);
  }, [scrollRef]);

  useEffect(() => {
    const frame = window.requestAnimationFrame(checkScroll);
    window.addEventListener('resize', checkScroll);
    return () => {
      window.cancelAnimationFrame(frame);
      window.removeEventListener('resize', checkScroll);
    };
  }, [checkScroll, items]);

  if (items.length === 0) return null;

  return (
    <div className="flex flex-row items-center gap-3 sm:gap-6 py-1">
      <div className="text-sm font-bold text-slate-400 shrink-0 min-w-[60px] sm:min-w-[70px] text-left">
        {label}
      </div>
      <div className="relative flex-1 group/row min-w-0">
        {canScrollLeft && (
          <button
            onClick={() => scrollFilterRow(scrollRef, 'left')}
            className="absolute -left-3 top-1/2 -translate-y-1/2 z-10 p-1 bg-white/90 dark:bg-slate-800/90 rounded-full shadow-md border border-slate-100 dark:border-slate-700 text-slate-400 hover:text-primary-500 hidden sm:flex items-center justify-center backdrop-blur-sm transition-all animate-in fade-in zoom-in duration-200"
          >
            <ChevronLeft size={16} />
          </button>
        )}

        <div
          ref={scrollRef}
          onScroll={checkScroll}
          className="flex items-center gap-2 overflow-x-auto no-scrollbar scroll-smooth pr-4 sm:pr-0 -mr-4 sm:mr-0 mask-linear-fade"
        >
          <button
            onClick={() => onSelect('')}
            className={`shrink-0 px-3 py-1.5 rounded-lg text-sm transition-all whitespace-nowrap ${
              selected === ''
                ? 'bg-primary-500 text-white font-medium shadow-md shadow-primary-500/20'
                : 'text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800'
            }`}
          >
            {allLabel}
          </button>
          {items.map((item) => {
            const value = typeof item === 'string' ? item : item.id;
            const itemLabel = typeof item === 'string' ? item : item.name;
            return (
              <button
                key={value}
                onClick={() => onSelect(selected === value ? '' : value)}
                className={`shrink-0 px-3 py-1.5 rounded-lg text-sm transition-all whitespace-nowrap ${
                  selected === value
                    ? 'bg-primary-500 text-white font-medium shadow-md shadow-primary-500/20'
                    : 'text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800'
                }`}
              >
                {itemLabel}
              </button>
            );
          })}
        </div>

        {canScrollRight && (
          <button
            onClick={() => scrollFilterRow(scrollRef, 'right')}
            className="absolute -right-3 top-1/2 -translate-y-1/2 z-10 p-1 bg-white/90 dark:bg-slate-800/90 rounded-full shadow-md border border-slate-100 dark:border-slate-700 text-slate-400 hover:text-primary-500 hidden sm:flex items-center justify-center backdrop-blur-sm transition-all animate-in fade-in zoom-in duration-200"
          >
            <ChevronRight size={16} />
          </button>
        )}
      </div>
    </div>
  );
};

interface SourceBookCardProps {
  item: SourceSearchItem;
  coverShape: 'rect' | 'square';
  pending: boolean;
  onAdd: () => void;
  addLabel: string;
  addingLabel: string;
  addedLabel: string;
}

const SourceBookCard: React.FC<SourceBookCardProps> = ({
  item,
  coverShape,
  pending,
  onAdd,
  addLabel,
  addingLabel,
  addedLabel,
}) => {
  const credits = bookPrimaryCredit(item.author, item.narrator);
  const added = Boolean(item.added_book_id);

  return (
    <article className="group flex min-w-0 flex-col">
      <div className={`relative ${coverShape === 'square' ? 'aspect-square' : 'aspect-[3/4]'} overflow-hidden rounded-md bg-white shadow-md dark:bg-slate-800`}>
        {item.cover_url ? (
          <img
            src={item.cover_url}
            alt={item.title}
            loading="lazy"
            referrerPolicy="no-referrer"
            className="h-full w-full object-cover transition-transform duration-300 group-hover:scale-105"
          />
        ) : (
          <div className="flex h-full items-center justify-center text-slate-400">
            <BookOpen size={40} />
          </div>
        )}
      </div>
      <div className="mt-2 min-w-0">
        <h2 title={item.title} className="truncate text-sm font-bold leading-tight text-slate-900 dark:text-white">
          {item.title}
        </h2>
        {credits && (
          <p className="mt-1 truncate text-xs text-slate-500 dark:text-slate-400" title={credits}>
            {credits}
          </p>
        )}
      </div>
      <button
        type="button"
        disabled={!item.id || pending || added}
        onClick={onAdd}
        className="mt-2 flex h-9 w-full min-w-0 items-center justify-center gap-1 rounded-lg border border-slate-200 px-1.5 py-1.5 text-xs text-primary-600 transition-colors hover:border-primary-300 disabled:cursor-default disabled:text-slate-400 dark:border-slate-700"
      >
        {pending ? <Loader2 size={14} className="shrink-0 animate-spin" /> : added ? <Check size={14} className="shrink-0" /> : <Plus size={14} className="shrink-0" />}
        <span className="min-w-0 truncate">{pending ? addingLabel : added ? addedLabel : addLabel}</span>
      </button>
    </article>
  );
};

const SearchPage: React.FC = () => {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const [searchMode, setSearchMode] = useState<SearchMode>('local');
  const [sources, setSources] = useState<ScraperSource[]>([]);
  const [selectedSourceIds, setSelectedSourceIds] = useState<string[]>([]);
  const [sourceMenuOpen, setSourceMenuOpen] = useState(false);
  const [sourceResults, setSourceResults] = useState<SourceSearchItem[]>([]);
  const [sourcePage, setSourcePage] = useState(1);
  const [sourceHasMore, setSourceHasMore] = useState(false);
  const [sourceError, setSourceError] = useState('');
  const [adding, setAdding] = useState<Set<string>>(new Set());
  const addRequests = useRef(new Set<string>());
  
  // Filter states
  const [selectedLibraryId, setSelectedLibraryId] = useState<string>('');
  const [selectedSeries, setSelectedSeries] = useState<string>('');
  const [selectedTag, setSelectedTag] = useState<string>('');
  const [selectedGenre, setSelectedGenre] = useState<string>('');
  const [selectedYear, setSelectedYear] = useState<string>('');
  const [selectedAuthor, setSelectedAuthor] = useState<string>('');
  const [selectedNarrator, setSelectedNarrator] = useState<string>('');
  const [showFilters, setShowFilters] = useState(false);
  
  // Data options
  const [libraries, setLibraries] = useState<Library[]>([]);
  const [allSeries, setAllSeries] = useState<Series[]>([]);
  const [allTags, setAllTags] = useState<string[]>([]);
  const [allGenres, setAllGenres] = useState<string[]>([]);
  const [allYears, setAllYears] = useState<string[]>([]);
  const [allAuthors, setAllAuthors] = useState<string[]>([]);
  const [allNarrators, setAllNarrators] = useState<string[]>([]);
  
  const [results, setResults] = useState<Book[]>([]);
  const [loading, setLoading] = useState(false);
  const [debouncedQuery, setDebouncedQuery] = useState('');
  const searchIdentity = JSON.stringify([searchMode, debouncedQuery, sourcePage, selectedSourceIds]);
  const activeSearch = useRef(searchIdentity);
  useEffect(() => { activeSearch.current = searchIdentity; }, [searchIdentity]);
  const currentChapter = usePlayerStore((state) => state.currentChapter);
  
  // Cover shape and icon size settings from bookshelf
  const { coverShape, iconSize } = useBookshelfDisplaySettings();

  // Scroll refs for filter rows
  const librariesScrollRef = useRef<HTMLDivElement>(null);
  const seriesScrollRef = useRef<HTMLDivElement>(null);
  const tagsScrollRef = useRef<HTMLDivElement>(null);
  const genresScrollRef = useRef<HTMLDivElement>(null);
  const yearsScrollRef = useRef<HTMLDivElement>(null);
  const authorsScrollRef = useRef<HTMLDivElement>(null);
  const narratorsScrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedQuery(query);
    }, 500);
    return () => clearTimeout(timer);
  }, [query]);

  useEffect(() => {
    const controller = new AbortController();
    apiClient.get<{ sources: ScraperSource[] }>('/api/v1/scraper/sources', { signal: controller.signal })
      .then(({ data }) => {
        const available = data.sources.filter(source => source.enabled && source.book_source);
        setSources(available);
        setSelectedSourceIds(current => current.length > 0
          ? current.filter(id => available.some(source => source.id === id))
          : available.map(source => source.id));
      })
      .catch(error => { if (!controller.signal.aborted) console.error('Book sources unavailable', error); });
    return () => controller.abort();
  }, []);

  // Fetch all metadata options on mount
  useEffect(() => {
    const fetchMetadata = async () => {
      try {
        const [tagsRes, booksRes, libsRes, seriesRes] = await Promise.all([
          apiClient.get('/api/tags'),
          apiClient.get('/api/books'),
          apiClient.get('/api/libraries'),
          apiClient.get('/api/v1/series')
        ]);
        
        setAllTags(tagsRes.data);
        setLibraries(libsRes.data);
        setAllSeries(seriesRes.data);
        
        // Extract unique authors, narrators, genres, and years
        const books = booksRes.data as Book[];
        const authors = new Set<string>();
        const narrators = new Set<string>();
        const genres = new Set<string>();
        const years = new Set<string>();
        
        books.forEach(book => {
          if (book.author) authors.add(book.author);
          if (book.narrator) narrators.add(book.narrator);
          if (book.genre) {
            book.genre.split(',').forEach(g => {
              const trimmed = g.trim();
              if (trimmed) genres.add(trimmed);
            });
          }
          if (book.year) years.add(book.year.toString());
        });
        
        setAllAuthors(Array.from(authors).sort());
        setAllNarrators(Array.from(narrators).sort());
        setAllGenres(Array.from(genres).sort());
        setAllYears(Array.from(years).sort((a, b) => parseInt(b) - parseInt(a))); // 降序排列
        
      } catch (err) {
        console.error('获取元数据失败', err);
      }
    };
    fetchMetadata();
  }, []);

  useEffect(() => {
    const controller = new AbortController();
    const searchBooks = async () => {
      const wantsLocal = searchMode === 'local' || searchMode === 'all';
      const wantsOnline = searchMode === 'online' || searchMode === 'all';
      const hasLocalFilters = searchMode === 'local' && Boolean(
        selectedTag
        || selectedGenre
        || selectedYear
        || selectedAuthor
        || selectedNarrator
        || selectedLibraryId
        || selectedSeries,
      );
      const hasLocalQuery = Boolean(debouncedQuery.trim()) || hasLocalFilters;
      const sourceIds = selectedSourceIds;
      const hasOnlineQuery = Boolean(debouncedQuery.trim()) && sourceIds.length > 0;

      if (!hasLocalQuery && !hasOnlineQuery) {
        setResults([]);
        setSourceResults([]);
        setSourceHasMore(false);
        setSourceError('');
        setLoading(false);
        return;
      }

      setLoading(true);
      setSourceError('');
      try {
        const localPromise = wantsLocal && hasLocalQuery
          ? apiClient.get('/api/books', {
            params: {
              ...(debouncedQuery.trim() ? { search: debouncedQuery } : {}),
              ...(searchMode === 'local' && selectedTag ? { tag: selectedTag } : {}),
              ...(searchMode === 'local' && selectedLibraryId ? { library_id: selectedLibraryId } : {}),
            },
            signal: controller.signal,
          })
          : Promise.resolve(null);

        const onlinePromise = wantsOnline && hasOnlineQuery
          ? Promise.allSettled(sourceIds.map(async sourceId => {
            const { data } = await apiClient.post<{
              items: ScraperSearchItem[];
              has_more: boolean | null;
              total: number | null;
              page_size: number;
            }>('/api/v1/scraper/search', {
              source: sourceId,
              query: debouncedQuery.trim(),
              page: sourcePage,
              page_size: 20,
            }, { signal: controller.signal });
            return {
              items: data.items.map(item => ({ ...item, source_id: sourceId })),
              hasMore: data.has_more ?? (data.total !== null && sourcePage * data.page_size < data.total),
            };
          }))
          : Promise.resolve([]);

        const [localResponse, onlineResponses] = await Promise.all([localPromise, onlinePromise]);
        if (controller.signal.aborted) return;

        if (localResponse) {
          let filtered = localResponse.data as Book[];
          if (searchMode === 'local' && selectedAuthor) filtered = filtered.filter(book => book.author === selectedAuthor);
          if (searchMode === 'local' && selectedNarrator) filtered = filtered.filter(book => book.narrator === selectedNarrator);
          if (searchMode === 'local' && selectedGenre) {
            filtered = filtered.filter(book => book.genre && book.genre.split(',').map(genre => genre.trim()).includes(selectedGenre));
          }
          if (searchMode === 'local' && selectedYear) filtered = filtered.filter(book => book.year && book.year.toString() === selectedYear);
          if (searchMode === 'local' && selectedSeries) {
            const series = allSeries.find(item => item.id === selectedSeries);
            const seriesBookIds = new Set(series?.books?.map(book => book.id) ?? []);
            filtered = filtered.filter(book => seriesBookIds.has(book.id));
          }
          setResults(filtered);
        } else {
          setResults([]);
        }

        if (Array.isArray(onlineResponses)) {
          const successful = onlineResponses.filter((response): response is PromiseFulfilledResult<{
            items: SourceSearchItem[];
            hasMore: boolean;
          }> => response.status === 'fulfilled');
          const failed = onlineResponses.filter(response => response.status === 'rejected');
          setSourceResults(successful.flatMap(response => response.value.items));
          setSourceHasMore(successful.some(response => response.value.hasMore));
          if (failed.length > 0 && successful.length === 0) {
            setSourceError(t('searchPage.sourceSearchFailed'));
          }
        } else {
          setSourceResults([]);
          setSourceHasMore(false);
        }
      } catch (error) {
        if (!controller.signal.aborted) {
          setResults([]);
          setSourceResults([]);
          setSourceHasMore(false);
          setSourceError(apiErrorMessage(error, t('searchPage.sourceSearchFailed')));
        }
      } finally {
        if (!controller.signal.aborted) setLoading(false);
      }
    };

    searchBooks();
    return () => controller.abort();
  }, [searchMode, sourcePage, debouncedQuery, selectedSourceIds, sources, selectedTag, selectedGenre, selectedYear, selectedAuthor, selectedNarrator, selectedLibraryId, selectedSeries, allSeries, t]);

  const addSourceBook = async (item: SourceSearchItem) => {
    if (!item.id || item.added_book_id) return;
    const key = JSON.stringify([item.source_id, item.id]);
    if (addRequests.current.has(key)) return;
    addRequests.current.add(key);
    setAdding(new Set(addRequests.current));
    setSourceError('');
    const startedFor = activeSearch.current;
    try {
      const { data } = await apiClient.post<{ book_id: string }>('/api/v1/scraper/books', {
        source: item.source_id, book: { id: item.id, url: item.source_url },
      });
      // The current result objects also identify the source/query that was
      // displayed when this request started; a later search must not be changed.
      setSourceResults(items => items.map(current => current.source_id === item.source_id && current.id === item.id
        ? { ...current, added_book_id: data.book_id }
        : current));
    } catch (error) {
      if (activeSearch.current === startedFor) {
        setSourceError(apiErrorMessage(error, t('searchPage.sourceAddFailed')));
      }
    } finally {
      addRequests.current.delete(key);
      setAdding(new Set(addRequests.current));
    }
  };

  const hasActiveFilters = selectedLibraryId || selectedTag || selectedGenre || selectedYear || selectedAuthor || selectedNarrator || selectedSeries;
  const showLocalResults = searchMode === 'local' || searchMode === 'all';
  const showLocalFilters = searchMode === 'local';
  const showOnlineResults = searchMode === 'online' || searchMode === 'all';
  const hasSearch = Boolean(debouncedQuery.trim()) || (showLocalFilters && Boolean(hasActiveFilters));
  const selectedSourceLabel = sources.length > 0 && selectedSourceIds.length === sources.length
    ? t('searchPage.allOnlineSources')
    : t('searchPage.selectedOnlineSources', { count: selectedSourceIds.length });

  return (
    <div className="w-full max-w-screen-2xl mx-auto p-4 sm:p-6 md:p-8 lg:p-10 space-y-6">
      <BackButton fallback="/bookshelf" />

      <div className="text-center space-y-4">
        <h1 className="text-3xl md:text-4xl font-bold dark:text-white">{t('searchPage.title')}</h1>
        <p className="text-sm md:text-base text-slate-500">{t('searchPage.subtitle')}</p>
        
        {/* Main Search Input */}
        <div className="w-full max-w-md sm:max-w-xl md:max-w-3xl lg:max-w-5xl xl:max-w-6xl 2xl:max-w-7xl mx-auto relative mt-8">
          <SearchIcon className="absolute left-4 top-1/2 -translate-y-1/2 text-slate-400" size={20} />
          <input 
            type="text"
            placeholder={t('searchPage.placeholder')}
            value={query}
            onChange={(e) => { setQuery(e.target.value); setSourcePage(1); }}
            className="w-full pl-12 pr-12 py-3 md:py-4 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-2xl shadow-lg focus:ring-2 focus:ring-primary-500 outline-none text-base md:text-lg transition-all dark:text-white"
            autoFocus
          />
          {loading ? (
            <div className="absolute right-4 top-1/2 -translate-y-1/2">
              <Loader2 className="animate-spin text-primary-600" size={24} />
            </div>
          ) : (
            query && (
              <button 
                onClick={() => { setQuery(''); setSourcePage(1); }}
                className="absolute right-4 top-1/2 -translate-y-1/2 text-slate-400 hover:text-slate-600 dark:hover:text-slate-200"
              >
                <BookX size={20} />
              </button>
            )
          )}
        </div>

        <div className="mt-4 flex justify-center">
          <div role="tablist" aria-label={t('searchPage.scope')} className="relative inline-flex items-center rounded-xl border border-slate-200 bg-white p-1 shadow-sm dark:border-slate-700 dark:bg-slate-900">
            {(['all', 'local'] as SearchMode[]).map(mode => (
                <button
                  key={mode}
                  type="button"
                  role="tab"
                  aria-selected={searchMode === mode}
                  onClick={() => {
                    setSearchMode(mode);
                    setSourcePage(1);
                    setSourceResults([]);
                    setSourceError('');
                    setSourceMenuOpen(false);
                  }}
                  className={`inline-flex min-w-[72px] items-center justify-center gap-1 rounded-lg px-4 py-2 text-sm font-medium transition-colors ${
                    searchMode === mode
                      ? 'bg-primary-500 text-white shadow-sm'
                      : 'text-slate-600 hover:bg-slate-100 dark:text-slate-300 dark:hover:bg-slate-800'
                  }`}
                >
                  {t(`searchPage.${mode}Scope`)}
                </button>
            ))}
            <div className={`inline-flex items-center rounded-lg text-sm font-medium transition-colors ${
              searchMode === 'online'
                ? 'bg-primary-500 text-white shadow-sm'
                : 'text-slate-600 dark:text-slate-300'
            }`}>
              <button
                type="button"
                role="tab"
                aria-selected={searchMode === 'online'}
                onClick={() => {
                  setSearchMode('online');
                  setSourcePage(1);
                  setSourceResults([]);
                  setSourceError('');
                }}
                className="min-w-[60px] rounded-l-lg px-3 py-2 text-sm font-medium"
              >
                {t('searchPage.onlineScope')}
              </button>
              <button
                type="button"
                aria-label={t('searchPage.onlineScope')}
                aria-expanded={sourceMenuOpen}
                aria-haspopup="menu"
                onClick={() => {
                  setSearchMode('online');
                  setSourcePage(1);
                  setSourceResults([]);
                  setSourceError('');
                  setSourceMenuOpen(open => !open);
                }}
                className="rounded-r-lg px-2 py-2 transition-colors hover:bg-black/5 dark:hover:bg-white/10"
              >
                <ChevronDown size={15} className={`transition-transform ${sourceMenuOpen ? 'rotate-180' : ''}`} />
              </button>
            </div>
            {searchMode === 'online' && sourceMenuOpen && (
              <div role="menu" className="absolute left-1/2 top-full z-30 mt-2 w-[min(90vw,20rem)] -translate-x-1/2 rounded-xl border border-slate-200 bg-white p-2 text-left shadow-xl dark:border-slate-700 dark:bg-slate-900">
                <div className="flex items-center gap-2 px-3 py-2 text-xs font-medium text-slate-400">
                  <Globe2 size={15} className="shrink-0 text-primary-500" />
                  <span className="truncate">{selectedSourceLabel}</span>
                </div>
                {sources.length > 0 ? (
                  <>
                    <button
                      type="button"
                      role="menuitemcheckbox"
                      aria-checked={selectedSourceIds.length === sources.length}
                      onClick={() => setSelectedSourceIds(current => current.length === sources.length ? [] : sources.map(source => source.id))}
                      className="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-sm text-slate-600 hover:bg-slate-100 dark:text-slate-300 dark:hover:bg-slate-800"
                    >
                      <span className={`flex h-4 w-4 items-center justify-center rounded border ${selectedSourceIds.length === sources.length ? 'border-primary-500 bg-primary-500 text-white' : 'border-slate-300 dark:border-slate-600'}`}>
                        {selectedSourceIds.length === sources.length && <Check size={12} />}
                      </span>
                      {t('searchPage.allOnlineSources')}
                    </button>
                    <div className="my-1 border-t border-slate-100 dark:border-slate-800" />
                    {sources.map(source => {
                      const checked = selectedSourceIds.includes(source.id);
                      return (
                        <button
                          key={source.id}
                          type="button"
                          role="menuitemcheckbox"
                          aria-checked={checked}
                          onClick={() => {
                            setSelectedSourceIds(current => checked
                              ? current.filter(id => id !== source.id)
                              : [...current, source.id]);
                            setSourcePage(1);
                          }}
                          className="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-sm text-slate-600 hover:bg-slate-100 dark:text-slate-300 dark:hover:bg-slate-800"
                        >
                          <span className={`flex h-4 w-4 items-center justify-center rounded border ${checked ? 'border-primary-500 bg-primary-500 text-white' : 'border-slate-300 dark:border-slate-600'}`}>
                            {checked && <Check size={12} />}
                          </span>
                          <span className="min-w-0 truncate">{source.name}</span>
                        </button>
                      );
                    })}
                  </>
                ) : (
                  <p className="px-3 py-2 text-sm text-slate-400">{t('searchPage.noOnlineSources')}</p>
                )}
              </div>
            )}
          </div>
        </div>

        {/* Filters Toggle Button */}
        {showLocalFilters && <div className="flex justify-center mt-4">
          <button
            onClick={() => setShowFilters(!showFilters)}
            className={`flex items-center gap-2 px-4 py-2 rounded-full text-sm font-medium transition-colors ${
              showFilters || hasActiveFilters
                ? 'bg-primary-50 dark:bg-primary-900/20 text-primary-600'
                : 'text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800'
            }`}
          >
            <SlidersHorizontal size={16} />
            {t(showFilters ? 'searchPage.hideFilters' : 'searchPage.showFilters')}
            {hasActiveFilters && !showFilters && <div className="w-2 h-2 rounded-full bg-primary-500 ml-1" />}
            {showFilters ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
          </button>
        </div>}

        {/* Advanced Filters Panel */}
        {showLocalFilters && showFilters && (
          <div className="w-full max-w-7xl mx-auto mt-6 p-4 sm:p-6 bg-white dark:bg-slate-900 rounded-2xl border border-slate-100 dark:border-slate-800 shadow-sm animate-in slide-in-from-top-4 duration-300">
            <div className="space-y-4 divide-y divide-slate-50 dark:divide-slate-800/50">
              <FilterRow
                label={t('searchPage.library')}
                allLabel={t('searchPage.all')}
                items={libraries}
                selected={selectedLibraryId}
                onSelect={setSelectedLibraryId}
                scrollRef={librariesScrollRef}
              />
              <FilterRow
                label={t('searchPage.series')}
                allLabel={t('searchPage.all')}
                items={allSeries.map(s => ({ id: s.id, name: s.title }))}
                selected={selectedSeries}
                onSelect={setSelectedSeries}
                scrollRef={seriesScrollRef}
              />
              <FilterRow
                label={t('searchPage.tags')}
                allLabel={t('searchPage.all')}
                items={allTags}
                selected={selectedTag}
                onSelect={setSelectedTag}
                scrollRef={tagsScrollRef}
              />
              <FilterRow
                label={t('searchPage.genre')}
                allLabel={t('searchPage.all')}
                items={allGenres}
                selected={selectedGenre}
                onSelect={setSelectedGenre}
                scrollRef={genresScrollRef}
              />
              <FilterRow
                label={t('searchPage.year')}
                allLabel={t('searchPage.all')}
                items={allYears}
                selected={selectedYear}
                onSelect={setSelectedYear}
                scrollRef={yearsScrollRef}
              />
              <FilterRow
                label={t('searchPage.author')}
                allLabel={t('searchPage.all')}
                items={allAuthors}
                selected={selectedAuthor}
                onSelect={setSelectedAuthor}
                scrollRef={authorsScrollRef}
              />
              <FilterRow
                label={t('searchPage.narrator')}
                allLabel={t('searchPage.all')}
                items={allNarrators}
                selected={selectedNarrator}
                onSelect={setSelectedNarrator} 
                scrollRef={narratorsScrollRef}
              />
            </div>
          </div>
        )}
      </div>

      {showLocalResults && hasSearch && (
        <section aria-labelledby={searchMode === 'all' ? 'local-search-results' : undefined} aria-live="polite" aria-busy={loading}>
          {searchMode === 'all' && (
            <h2 id="local-search-results" className="border-b border-slate-200 pb-3 text-lg font-semibold text-slate-900 dark:border-slate-800 dark:text-white">
              {t('searchPage.localScope')}
            </h2>
          )}
          {results.length > 0 ? (
            <div className={`grid ${getBookshelfGridClass(iconSize)} pt-4`}>
              {results.map((book) => (
                <BookCard key={book.id} book={book} coverShape={coverShape} />
              ))}
            </div>
          ) : loading ? (
            <p className="py-10 text-center text-sm text-slate-400">{t('common.loading')}</p>
          ) : (
            <div className={searchMode === 'all' ? 'py-10 text-center' : 'py-20 text-center'}>
              {searchMode !== 'all' && (
                <div className="inline-flex items-center justify-center w-20 h-20 rounded-full bg-slate-100 dark:bg-slate-900 text-slate-400 mb-4">
                  <BookX size={40} />
                </div>
              )}
              <h3 className={searchMode === 'all' ? 'text-sm text-slate-400' : 'text-xl font-medium dark:text-white'}>{t('searchPage.noResults')}</h3>
              {searchMode !== 'all' && <p className="text-slate-500 mt-2">{t('searchPage.noResultsHint')}</p>}
            </div>
          )}
        </section>
      )}

      {showOnlineResults && (searchMode === 'online' || hasSearch) && (
        <section aria-labelledby={searchMode === 'all' ? 'online-search-results' : undefined} aria-live="polite" aria-busy={loading}>
          {searchMode === 'all' && (
            <h2 id="online-search-results" className="mb-4 border-b border-slate-200 pb-3 text-lg font-semibold text-slate-900 dark:border-slate-800 dark:text-white">
              {t('searchPage.onlineScope')}
            </h2>
          )}
          {sourceError && <p role="alert" className="mb-4 text-sm text-red-600 dark:text-red-400">{sourceError}</p>}
          {sourceResults.length > 0 ? (
            <div className={`grid ${getBookshelfGridClass(iconSize)} pt-4`}>
              {sourceResults.map(item => {
                const pending = adding.has(JSON.stringify([item.source_id, item.id]));
                return (
                  <SourceBookCard
                    key={`${item.source_id}:${item.id || item.source_url}`}
                    item={item}
                    coverShape={coverShape}
                    pending={pending}
                    onAdd={() => void addSourceBook(item)}
                    addLabel={t('searchPage.sourceAddAction')}
                    addingLabel={t('searchPage.sourceAdding')}
                    addedLabel={t('searchPage.sourceAdded')}
                  />
                );
              })}
            </div>
          ) : !sourceError && (
            <p className="py-10 text-center text-sm text-slate-400">
              {t(loading ? 'common.loading' : debouncedQuery.trim() ? 'searchPage.noResults' : 'searchPage.sourceEmptyHint')}
            </p>
          )}
          {(sourcePage > 1 || sourceHasMore) && (
            <nav aria-label={t('searchPage.sourcePages')} className="mt-6 flex items-center justify-center gap-4">
              <button title={t('searchPage.previousPage')} disabled={loading || sourcePage <= 1} onClick={() => setSourcePage(page => page - 1)} className="p-2 disabled:opacity-30"><ChevronLeft size={20} /></button>
              <span className="text-sm text-slate-500">{sourcePage}</span>
              <button title={t('searchPage.nextPage')} disabled={loading || !sourceHasMore} onClick={() => setSourcePage(page => page + 1)} className="p-2 disabled:opacity-30"><ChevronRight size={20} /></button>
            </nav>
          )}
        </section>
      )}

      {!hasSearch && searchMode !== 'online' && (
        <div className="py-20 text-center text-slate-400">
          <p>{t('searchPage.emptyHint')}</p>
        </div>
      )}

      {/* Dynamic Safe Bottom Spacer */}
      <div 
        className="shrink-0 transition-all duration-300" 
        style={{ height: currentChapter ? 'var(--safe-bottom-with-player)' : 'var(--safe-bottom-base)' }} 
      />
    </div>
  );
};

export default SearchPage;
