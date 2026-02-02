import React, { useState, useEffect, useRef } from 'react';
import apiClient from '../api/client';
import type { Book } from '../types';
import BookCard from '../components/BookCard';
import { Search as SearchIcon, Loader2, BookX, ChevronLeft, ChevronRight } from 'lucide-react';
import { usePlayerStore } from '../store/playerStore';

const SearchPage: React.FC = () => {
  const [query, setQuery] = useState('');
  const [selectedTag, setSelectedTag] = useState<string | null>(null);
  const [allTags, setAllTags] = useState<string[]>([]);
  const [results, setResults] = useState<Book[]>([]);
  const [loading, setLoading] = useState(false);
  const [debouncedQuery, setDebouncedQuery] = useState('');
  const currentChapter = usePlayerStore((state) => state.currentChapter);
  const tagsScrollRef = useRef<HTMLDivElement>(null);

  const scrollTags = (direction: 'left' | 'right') => {
    if (tagsScrollRef.current) {
      const scrollAmount = 200;
      tagsScrollRef.current.scrollBy({
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

  useEffect(() => {
    const fetchTags = async () => {
      try {
        console.log('Fetching tags...');
        const res = await apiClient.get('/api/tags');
        console.log('Tags received:', res.data);
        setAllTags(res.data);
      } catch (err) {
        console.error('Failed to fetch tags', err);
      }
    };
    fetchTags();
  }, []);

  useEffect(() => {
    const searchBooks = async () => {
      if (!debouncedQuery.trim() && !selectedTag) {
        setResults([]);
        return;
      }

      setLoading(true);
      try {
        const params: any = {};
        if (debouncedQuery.trim()) params.search = debouncedQuery;
        if (selectedTag) params.tag = selectedTag;
        
        const response = await apiClient.get('/api/books', { params });
        setResults(response.data);
      } catch (err) {
        console.error('Search failed', err);
      } finally {
        setLoading(false);
      }
    };

    searchBooks();
  }, [debouncedQuery, selectedTag]);

  return (
    <div className="w-full max-w-screen-2xl mx-auto p-4 sm:p-6 md:p-8 lg:p-10 space-y-8">
      <div className="text-center space-y-4">
        <h1 className="text-3xl md:text-4xl font-bold dark:text-white">发现精彩内容</h1>
        <p className="text-sm md:text-base text-slate-500">搜索书名、作者、演播者或简介</p>
        
        <div className="w-full max-w-md sm:max-w-xl md:max-w-3xl lg:max-w-5xl xl:max-w-6xl 2xl:max-w-7xl mx-auto relative mt-8">
          <SearchIcon className="absolute left-4 top-1/2 -translate-y-1/2 text-slate-400" size={20} />
          <input 
            type="text"
            placeholder="开始搜索..."
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            className="w-full pl-12 pr-4 py-3 md:py-4 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-2xl shadow-lg focus:ring-2 focus:ring-primary-500 outline-none text-base md:text-lg transition-all dark:text-white"
            autoFocus
          />
          {loading && (
            <div className="absolute right-4 top-1/2 -translate-y-1/2">
              <Loader2 className="animate-spin text-primary-600" size={24} />
            </div>
          )}
        </div>

        {allTags.length > 0 && (
          <div className="w-full max-w-7xl mx-auto mt-6 relative group">
            {/* Desktop Arrows */}
            <button 
              onClick={() => scrollTags('left')}
              className="absolute -left-4 top-1/2 -translate-y-1/2 z-10 p-1.5 bg-white dark:bg-slate-800 rounded-full shadow-md border border-slate-100 dark:border-slate-700 text-slate-400 hover:text-primary-500 opacity-0 group-hover:opacity-100 transition-opacity hidden md:block"
            >
              <ChevronLeft size={18} />
            </button>
            <button 
              onClick={() => scrollTags('right')}
              className="absolute -right-4 top-1/2 -translate-y-1/2 z-10 p-1.5 bg-white dark:bg-slate-800 rounded-full shadow-md border border-slate-100 dark:border-slate-700 text-slate-400 hover:text-primary-500 opacity-0 group-hover:opacity-100 transition-opacity hidden md:block"
            >
              <ChevronRight size={18} />
            </button>

            <div 
              ref={tagsScrollRef}
              className="flex items-center gap-2 overflow-x-auto no-scrollbar scroll-smooth px-4"
            >
              <button
                onClick={() => setSelectedTag(null)}
                className={`shrink-0 px-4 py-1.5 rounded-full text-xs font-bold border transition-all ${
                  selectedTag === null
                    ? 'bg-primary-600 border-primary-600 text-white shadow-lg shadow-primary-500/20'
                    : 'bg-white dark:bg-slate-900 border-slate-200 dark:border-slate-800 text-slate-500 hover:border-primary-500 hover:text-primary-500'
                }`}
              >
                全部
              </button>
              {allTags.map((tag) => (
                <button
                  key={tag}
                  onClick={() => setSelectedTag(selectedTag === tag ? null : tag)}
                  className={`shrink-0 px-4 py-1.5 rounded-full text-xs font-bold border transition-all ${
                    selectedTag === tag
                      ? 'bg-primary-600 border-primary-600 text-white shadow-lg shadow-primary-500/20'
                      : 'bg-white dark:bg-slate-900 border-slate-200 dark:border-slate-800 text-slate-500 hover:border-primary-500 hover:text-primary-500'
                  }`}
                >
                  {tag}
                </button>
              ))}
            </div>
          </div>
        )}
      </div>

      {results.length > 0 ? (
        <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-6">
          {results.map((book) => (
            <BookCard key={book.id} book={book} />
          ))}
        </div>
      ) : (debouncedQuery || selectedTag) && !loading ? (
        <div className="py-20 text-center">
          <div className="inline-flex items-center justify-center w-20 h-20 rounded-full bg-slate-100 dark:bg-slate-900 text-slate-400 mb-4">
            <BookX size={40} />
          </div>
          <h3 className="text-xl font-medium dark:text-white">未找到相关结果</h3>
          <p className="text-slate-500 mt-2">换个词或标签试试，或者检查拼写是否正确</p>
        </div>
      ) : !debouncedQuery && !selectedTag && (
        <div className="py-20 text-center text-slate-400">
          <p>输入关键词或选择标签开始探索</p>
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
