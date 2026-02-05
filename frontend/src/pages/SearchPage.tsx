import React, { useState, useEffect, useRef } from 'react';
import apiClient from '../api/client';
import type { Book, Library } from '../types';
import BookCard from '../components/BookCard';
import { Search as SearchIcon, Loader2, BookX, ChevronLeft, ChevronRight, SlidersHorizontal, ChevronUp, ChevronDown } from 'lucide-react';
import { usePlayerStore } from '../store/playerStore';

const SearchPage: React.FC = () => {
  const [query, setQuery] = useState('');
  
  // Filter states
  const [selectedLibraryId, setSelectedLibraryId] = useState<string>('');
  const [selectedTag, setSelectedTag] = useState<string>('');
  const [selectedAuthor, setSelectedAuthor] = useState<string>('');
  const [selectedNarrator, setSelectedNarrator] = useState<string>('');
  const [showFilters, setShowFilters] = useState(false);
  
  // Data options
  const [libraries, setLibraries] = useState<Library[]>([]);
  const [allTags, setAllTags] = useState<string[]>([]);
  const [allAuthors, setAllAuthors] = useState<string[]>([]);
  const [allNarrators, setAllNarrators] = useState<string[]>([]);
  
  const [results, setResults] = useState<Book[]>([]);
  const [loading, setLoading] = useState(false);
  const [debouncedQuery, setDebouncedQuery] = useState('');
  const currentChapter = usePlayerStore((state) => state.currentChapter);

  // Scroll refs for filter rows
  const filterRowRefs = {
    libraries: useRef<HTMLDivElement>(null),
    tags: useRef<HTMLDivElement>(null),
    authors: useRef<HTMLDivElement>(null),
    narrators: useRef<HTMLDivElement>(null),
  };

  const scrollRow = (ref: React.RefObject<HTMLDivElement>, direction: 'left' | 'right') => {
    if (ref.current) {
      const scrollAmount = 300;
      ref.current.scrollBy({
        left: direction === 'left' ? -scrollAmount : scrollAmount,
        behavior: 'smooth'
      });
    }
  };

  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedQuery(query);
    }, 500);
    return () => clearTimeout(timer);
  }, [query]);

  // Fetch all metadata options on mount
  useEffect(() => {
    const fetchMetadata = async () => {
      try {
        const [tagsRes, booksRes, libsRes] = await Promise.all([
          apiClient.get('/api/tags'),
          apiClient.get('/api/books'),
          apiClient.get('/api/libraries')
        ]);
        
        setAllTags(tagsRes.data);
        setLibraries(libsRes.data);
        
        // Extract unique authors and narrators
        const books = booksRes.data as Book[];
        const authors = new Set<string>();
        const narrators = new Set<string>();
        
        books.forEach(book => {
          if (book.author) authors.add(book.author);
          if (book.narrator) narrators.add(book.narrator);
        });
        
        setAllAuthors(Array.from(authors).sort());
        setAllNarrators(Array.from(narrators).sort());
        
      } catch (err) {
        console.error('Failed to fetch metadata', err);
      }
    };
    fetchMetadata();
  }, []);

  useEffect(() => {
    const searchBooks = async () => {
      // If no filters are active, clear results
      if (!debouncedQuery.trim() && !selectedTag && !selectedAuthor && !selectedNarrator && !selectedLibraryId) {
        setResults([]);
        return;
      }

      setLoading(true);
      try {
        const params: any = {};
        if (debouncedQuery.trim()) params.search = debouncedQuery;
        if (selectedTag) params.tag = selectedTag;
        if (selectedLibraryId) params.library_id = selectedLibraryId;
        
        const response = await apiClient.get('/api/books', { params });
        let filtered = response.data as Book[];

        // Apply additional client-side filters
        if (selectedAuthor) {
          filtered = filtered.filter(b => b.author === selectedAuthor);
        }
        
        if (selectedNarrator) {
          filtered = filtered.filter(b => b.narrator === selectedNarrator);
        }
        
        setResults(filtered);
      } catch (err) {
        console.error('Search failed', err);
      } finally {
        setLoading(false);
      }
    };

    searchBooks();
  }, [debouncedQuery, selectedTag, selectedAuthor, selectedNarrator, selectedLibraryId]);

  // Filter Row Component
  const FilterRow = ({ 
    label, 
    items, 
    selected, 
    onSelect, 
    scrollRef 
  }: { 
    label: string, 
    items: string[] | {id: string, name: string}[], 
    selected: string, 
    onSelect: (val: string) => void,
    scrollRef: React.RefObject<HTMLDivElement>
  }) => {
    const [canScrollLeft, setCanScrollLeft] = useState(false);
    const [canScrollRight, setCanScrollRight] = useState(false);

    const checkScroll = () => {
      if (scrollRef.current) {
        const { scrollLeft, scrollWidth, clientWidth } = scrollRef.current;
        setCanScrollLeft(scrollLeft > 0);
        setCanScrollRight(scrollLeft < scrollWidth - clientWidth - 1); // -1 buffer
      }
    };

    useEffect(() => {
      checkScroll();
      window.addEventListener('resize', checkScroll);
      return () => window.removeEventListener('resize', checkScroll);
    }, [items]);

    if (items.length === 0) return null;

    return (
      <div className="flex flex-row items-center gap-3 sm:gap-6 py-1">
        <div className="text-sm font-bold text-slate-400 shrink-0 min-w-[60px] sm:min-w-[70px] text-left">
          {label}
        </div>
        <div className="relative flex-1 group/row min-w-0">
          {/* Left Arrow */}
          {canScrollLeft && (
            <button 
              onClick={() => scrollRow(scrollRef, 'left')}
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
              全部
            </button>
            {items.map((item) => {
              const value = typeof item === 'string' ? item : item.id;
              const label = typeof item === 'string' ? item : item.name;
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
                  {label}
                </button>
              );
            })}
          </div>

          {/* Right Arrow */}
          {canScrollRight && (
            <button 
              onClick={() => scrollRow(scrollRef, 'right')}
              className="absolute -right-3 top-1/2 -translate-y-1/2 z-10 p-1 bg-white/90 dark:bg-slate-800/90 rounded-full shadow-md border border-slate-100 dark:border-slate-700 text-slate-400 hover:text-primary-500 hidden sm:flex items-center justify-center backdrop-blur-sm transition-all animate-in fade-in zoom-in duration-200"
            >
              <ChevronRight size={16} />
            </button>
          )}
        </div>
      </div>
    );
  };

  const hasActiveFilters = selectedLibraryId || selectedTag || selectedAuthor || selectedNarrator;

  return (
    <div className="w-full max-w-screen-2xl mx-auto p-4 sm:p-6 md:p-8 lg:p-10 space-y-6">
      <div className="text-center space-y-4">
        <h1 className="text-3xl md:text-4xl font-bold dark:text-white">发现精彩内容</h1>
        <p className="text-sm md:text-base text-slate-500">搜索书名、作者、演播者或简介</p>
        
        {/* Main Search Input */}
        <div className="w-full max-w-md sm:max-w-xl md:max-w-3xl lg:max-w-5xl xl:max-w-6xl 2xl:max-w-7xl mx-auto relative mt-8">
          <SearchIcon className="absolute left-4 top-1/2 -translate-y-1/2 text-slate-400" size={20} />
          <input 
            type="text"
            placeholder="输入关键词搜索..."
            value={query}
            onChange={(e) => setQuery(e.target.value)}
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
                onClick={() => setQuery('')}
                className="absolute right-4 top-1/2 -translate-y-1/2 text-slate-400 hover:text-slate-600 dark:hover:text-slate-200"
              >
                <BookX size={20} />
              </button>
            )
          )}
        </div>

        {/* Filters Toggle Button */}
        <div className="flex justify-center mt-4">
          <button
            onClick={() => setShowFilters(!showFilters)}
            className={`flex items-center gap-2 px-4 py-2 rounded-full text-sm font-medium transition-colors ${
              showFilters || hasActiveFilters
                ? 'bg-primary-50 dark:bg-primary-900/20 text-primary-600'
                : 'text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800'
            }`}
          >
            <SlidersHorizontal size={16} />
            {showFilters ? '收起筛选' : '展开筛选'}
            {hasActiveFilters && !showFilters && <div className="w-2 h-2 rounded-full bg-primary-500 ml-1" />}
            {showFilters ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
          </button>
        </div>

        {/* Advanced Filters Panel */}
        {showFilters && (
          <div className="w-full max-w-7xl mx-auto mt-6 p-4 sm:p-6 bg-white dark:bg-slate-900 rounded-2xl border border-slate-100 dark:border-slate-800 shadow-sm animate-in slide-in-from-top-4 duration-300">
            <div className="space-y-4 divide-y divide-slate-50 dark:divide-slate-800/50">
              <FilterRow 
                label="媒体库" 
                items={libraries} 
                selected={selectedLibraryId} 
                onSelect={setSelectedLibraryId} 
                scrollRef={filterRowRefs.libraries}
              />
              <FilterRow 
                label="标签" 
                items={allTags} 
                selected={selectedTag} 
                onSelect={setSelectedTag} 
                scrollRef={filterRowRefs.tags}
              />
              <FilterRow 
                label="作者" 
                items={allAuthors} 
                selected={selectedAuthor} 
                onSelect={setSelectedAuthor} 
                scrollRef={filterRowRefs.authors}
              />
              <FilterRow 
                label="演播者" 
                items={allNarrators} 
                selected={selectedNarrator} 
                onSelect={setSelectedNarrator} 
                scrollRef={filterRowRefs.narrators}
              />
            </div>
          </div>
        )}
      </div>

      {results.length > 0 ? (
        <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-6 pt-4">
          {results.map((book) => (
            <BookCard key={book.id} book={book} />
          ))}
        </div>
      ) : (debouncedQuery || hasActiveFilters) && !loading ? (
        <div className="py-20 text-center">
          <div className="inline-flex items-center justify-center w-20 h-20 rounded-full bg-slate-100 dark:bg-slate-900 text-slate-400 mb-4">
            <BookX size={40} />
          </div>
          <h3 className="text-xl font-medium dark:text-white">未找到相关结果</h3>
          <p className="text-slate-500 mt-2">尝试调整筛选条件或搜索关键词</p>
        </div>
      ) : !debouncedQuery && !hasActiveFilters && (
        <div className="py-20 text-center text-slate-400">
          <p>输入关键词或使用上方筛选器开始探索</p>
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
