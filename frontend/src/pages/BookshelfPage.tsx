import React, { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import apiClient from '../api/client';
import type { Book, Library, Series } from '../types';
import BookCard from '../components/BookCard';
import SeriesCard from '../components/SeriesCard';
import SeriesModal from '../components/SeriesModal';
import { Search, Filter, Database, Plus, Library as LibraryIcon, Layers, Check, X } from 'lucide-react';
import { usePlayerStore } from '../store/playerStore';

const BookshelfPage: React.FC = () => {
  const currentChapter = usePlayerStore((state) => state.currentChapter);
  const [books, setBooks] = useState<Book[]>([]);
  const [series, setSeries] = useState<Series[]>([]);
  const [libraries, setLibraries] = useState<Library[]>([]);
  const [selectedLibraryId, setSelectedLibraryId] = useState<string>('');
  const [loading, setLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState('');
  const [sortBy, setSortBy] = useState<'createdAt' | 'title' | 'author'>('createdAt');
  const [iconSize, setIconSize] = useState<'small' | 'medium' | 'large'>('medium');
  const [showFilterMenu, setShowFilterMenu] = useState(false);
  const [settingsLoaded, setSettingsLoaded] = useState(false);
  
  // Selection mode for creating series
  const [isSelectionMode, setIsSelectionMode] = useState(false);
  const [selectedBookIds, setSelectedBookIds] = useState<string[]>([]);
  const [isSeriesModalOpen, setIsSeriesModalOpen] = useState(false);

  useEffect(() => {
    const loadSettings = async () => {
      try {
        const res = await apiClient.get('/api/settings');
        const settings = res.data.settingsJson || {};
        
        if (settings.bookshelfLibraryId) {
          setSelectedLibraryId(settings.bookshelfLibraryId);
        }
        if (settings.bookshelfSortBy) {
          setSortBy(settings.bookshelfSortBy);
        }
        if (settings.bookshelfIconSize) {
          setIconSize(settings.bookshelfIconSize);
        }
      } catch (err) {
        console.error('Failed to load settings', err);
      } finally {
        setSettingsLoaded(true);
      }
    };
    loadSettings();
  }, []);

  const handleLibraryChange = (newId: string) => {
    setSelectedLibraryId(newId);
    apiClient.post('/api/settings', { bookshelfLibraryId: newId });
  };

  const handleSortChange = (newSort: 'createdAt' | 'title' | 'author') => {
    setSortBy(newSort);
    setShowFilterMenu(false);
    apiClient.post('/api/settings', { bookshelfSortBy: newSort });
  };

  const handleIconSizeChange = (newSize: 'small' | 'medium' | 'large') => {
    setIconSize(newSize);
    setShowFilterMenu(false);
    apiClient.post('/api/settings', { bookshelfIconSize: newSize });
  };

  const fetchData = async () => {
    setLoading(true);
    try {
      const [booksRes, libsRes, seriesRes] = await Promise.all([
        apiClient.get('/api/books', { params: { libraryId: selectedLibraryId || undefined } }),
        apiClient.get('/api/libraries'),
        apiClient.get('/api/v1/series', { params: { library_id: selectedLibraryId || undefined } })
      ]);
      setBooks(booksRes.data);
      setLibraries(libsRes.data);
      setSeries(seriesRes.data);
    } catch (err) {
      console.error('Failed to fetch data', err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (settingsLoaded) {
      fetchData();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedLibraryId, settingsLoaded]);

  const sortedBooks = [...books].sort((a, b) => {
    if (sortBy === 'title') return a.title.localeCompare(b.title, 'zh-CN');
    if (sortBy === 'author') return (a.author || '').localeCompare(b.author || '', 'zh-CN');
    return new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime();
  });

  // Collect all book IDs that are in a series
  const booksInSeries = new Set(series.flatMap(s => s.books?.map(b => b.id) || []));

  const filteredBooks = sortedBooks.filter(book => 
    !booksInSeries.has(book.id) && 
    (book.title.toLowerCase().includes(searchQuery.toLowerCase()) ||
    book.author?.toLowerCase().includes(searchQuery.toLowerCase()) ||
    book.narrator?.toLowerCase().includes(searchQuery.toLowerCase()))
  );

  const filteredSeries = series.filter(s => 
    s.title.toLowerCase().includes(searchQuery.toLowerCase()) ||
    s.author?.toLowerCase().includes(searchQuery.toLowerCase())
  );

  const toggleBookSelection = (id: string) => {
    setSelectedBookIds(prev => 
      prev.includes(id) ? prev.filter(i => i !== id) : [...prev, id]
    );
  };

  const getGridCols = () => {
    switch (iconSize) {
      case 'small':
        return 'grid-cols-3 sm:grid-cols-4 md:grid-cols-5 lg:grid-cols-6 xl:grid-cols-8 gap-x-4 gap-y-8';
      case 'large':
        return 'grid-cols-2 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 gap-x-8 gap-y-12';
      default: // medium
        return 'grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-x-6 gap-y-10';
    }
  };

  if (loading) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center min-h-full">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-primary-600"></div>
        <p className="mt-4 text-sm text-slate-500 dark:text-slate-400">加载中...</p>
      </div>
    );
  }

  return (
    <div className="flex-1 min-h-full flex flex-col p-4 sm:p-6 md:p-8">
      <div className="flex-1 space-y-6">
        <div className="flex flex-col md:flex-row md:items-center justify-between gap-4 mb-2">
          <div>
            <h1 className="text-2xl md:text-3xl font-bold text-slate-900 dark:text-white flex items-center gap-3">
              <LibraryIcon className="text-primary-600" />
              我的书架
            </h1>
            <p className="text-sm md:text-base text-slate-500 dark:text-slate-400 mt-1">发现您收藏的所有有声读物。</p>
          </div>
          
          <div className="flex flex-wrap min-[550px]:flex-nowrap items-center gap-2 sm:gap-3 w-full md:w-auto justify-end">
            {isSelectionMode ? (
              <div className="flex items-center gap-2 order-1">
                <span className="text-sm font-medium text-slate-600 dark:text-slate-400 whitespace-nowrap hidden sm:inline">
                  已选 {selectedBookIds.length}
                </span>
                <button
                  onClick={() => setIsSeriesModalOpen(true)}
                  disabled={selectedBookIds.length === 0}
                  className="flex items-center gap-2 px-3 sm:px-4 py-2 bg-primary-600 text-white rounded-xl text-sm font-bold shadow-lg shadow-primary-500/30 disabled:opacity-50 whitespace-nowrap shrink-0"
                >
                  <Layers size={18} />
                  <span>创建系列</span>
                </button>
                <button
                  onClick={() => { setIsSelectionMode(false); setSelectedBookIds([]); }}
                  className="p-2.5 bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-400 rounded-xl shrink-0"
                >
                  <X size={20} />
                </button>
              </div>
            ) : (
              <button
                onClick={() => setIsSelectionMode(true)}
                className="flex items-center gap-2 px-3 sm:px-4 py-2.5 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl text-slate-600 dark:text-slate-400 hover:bg-slate-50 dark:hover:bg-slate-800 transition-colors text-sm font-medium shrink-0 order-1"
              >
                <Layers size={18} />
                <span>选择模式</span>
              </button>
            )}

            {/* Library Selector */}
            {libraries.length > 0 && (
              <div className={`relative order-2 ${isSelectionMode ? 'hidden sm:block' : ''}`}>
                <div className="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none text-slate-400">
                  <LibraryIcon size={16} />
                </div>
                <select
                  value={selectedLibraryId}
                  onChange={(e) => handleLibraryChange(e.target.value)}
                  className="pl-9 pr-8 py-2.5 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 text-sm font-medium text-slate-700 dark:text-slate-200 appearance-none cursor-pointer max-w-[140px] sm:max-w-none truncate"
                >
                  <option value="">所有媒体库</option>
                  {libraries.map(lib => (
                    <option key={lib.id} value={lib.id}>{lib.name}</option>
                  ))}
                </select>
                <div className="absolute inset-y-0 right-0 pr-2 flex items-center pointer-events-none text-slate-400">
                  <svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                  </svg>
                </div>
              </div>
            )}

            <div className="relative w-full min-[550px]:flex-1 md:w-64 order-first min-[550px]:order-none">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" size={18} />
              <input 
                type="text"
                placeholder="搜索书名、作者..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="w-full pl-10 pr-4 py-2.5 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 transition-all dark:text-white"
              />
            </div>
            <div className="relative min-w-0 order-3">
              <button 
                onClick={() => setShowFilterMenu(!showFilterMenu)}
                className={`p-2.5 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl text-slate-600 dark:text-slate-400 hover:bg-slate-50 dark:hover:bg-slate-800 transition-colors ${showFilterMenu ? 'ring-2 ring-primary-500' : ''}`}
              >
                <Filter size={20} />
              </button>

              {showFilterMenu && (
                <div className="absolute right-0 mt-2 w-56 bg-white dark:bg-slate-900 border border-slate-100 dark:border-slate-800 rounded-2xl shadow-xl z-50 py-2 animate-in zoom-in-95 duration-200">
                  <div className="px-4 py-2 text-xs font-bold text-slate-400 uppercase tracking-widest border-b border-slate-50 dark:border-slate-800 mb-1">
                    排序方式
                  </div>
                  <button 
                    onClick={() => handleSortChange('createdAt')}
                    className={`w-full px-4 py-2.5 text-left text-sm flex items-center justify-between ${sortBy === 'createdAt' ? 'text-primary-600 font-bold bg-primary-50/50 dark:bg-primary-900/20' : 'text-slate-600 dark:text-slate-300 hover:bg-slate-50 dark:hover:bg-slate-800'}`}
                  >
                    最近添加
                    {sortBy === 'createdAt' && <div className="w-1.5 h-1.5 rounded-full bg-primary-600" />}
                  </button>
                  <button 
                    onClick={() => handleSortChange('title')}
                    className={`w-full px-4 py-2.5 text-left text-sm flex items-center justify-between ${sortBy === 'title' ? 'text-primary-600 font-bold bg-primary-50/50 dark:bg-primary-900/20' : 'text-slate-600 dark:text-slate-300 hover:bg-slate-50 dark:hover:bg-slate-800'}`}
                  >
                    书名排序
                    {sortBy === 'title' && <div className="w-1.5 h-1.5 rounded-full bg-primary-600" />}
                  </button>
                  <button 
                    onClick={() => handleSortChange('author')}
                    className={`w-full px-4 py-2.5 text-left text-sm flex items-center justify-between ${sortBy === 'author' ? 'text-primary-600 font-bold bg-primary-50/50 dark:bg-primary-900/20' : 'text-slate-600 dark:text-slate-300 hover:bg-slate-50 dark:hover:bg-slate-800'}`}
                  >
                    作者排序
                    {sortBy === 'author' && <div className="w-1.5 h-1.5 rounded-full bg-primary-600" />}
                  </button>

                  <div className="px-4 py-2 text-xs font-bold text-slate-400 uppercase tracking-widest border-t border-b border-slate-50 dark:border-slate-800 mt-2 mb-1">
                    图标大小
                  </div>
                  <button 
                    onClick={() => handleIconSizeChange('large')}
                    className={`w-full px-4 py-2.5 text-left text-sm flex items-center justify-between ${iconSize === 'large' ? 'text-primary-600 font-bold bg-primary-50/50 dark:bg-primary-900/20' : 'text-slate-600 dark:text-slate-300 hover:bg-slate-50 dark:hover:bg-slate-800'}`}
                  >
                    大图标
                    {iconSize === 'large' && <div className="w-1.5 h-1.5 rounded-full bg-primary-600" />}
                  </button>
                  <button 
                    onClick={() => handleIconSizeChange('medium')}
                    className={`w-full px-4 py-2.5 text-left text-sm flex items-center justify-between ${iconSize === 'medium' ? 'text-primary-600 font-bold bg-primary-50/50 dark:bg-primary-900/20' : 'text-slate-600 dark:text-slate-300 hover:bg-slate-50 dark:hover:bg-slate-800'}`}
                  >
                    中图标 (默认)
                    {iconSize === 'medium' && <div className="w-1.5 h-1.5 rounded-full bg-primary-600" />}
                  </button>
                  <button 
                    onClick={() => handleIconSizeChange('small')}
                    className={`w-full px-4 py-2.5 text-left text-sm flex items-center justify-between ${iconSize === 'small' ? 'text-primary-600 font-bold bg-primary-50/50 dark:bg-primary-900/20' : 'text-slate-600 dark:text-slate-300 hover:bg-slate-50 dark:hover:bg-slate-800'}`}
                  >
                    小图标
                    {iconSize === 'small' && <div className="w-1.5 h-1.5 rounded-full bg-primary-600" />}
                  </button>
                </div>
              )}
            </div>
          </div>
        </div>

      {books.length > 0 || series.length > 0 ? (
        <>
          <div className={`grid ${getGridCols()}`}>
            {/* Show Series first */}
            {!isSelectionMode && filteredSeries.map((s) => (
              <SeriesCard key={s.id} series={s} />
            ))}
            
            {/* Show Books */}
            {filteredBooks.map((book) => (
              <div key={book.id} className="relative">
                {isSelectionMode ? (
                  <>
                    <div className={`absolute top-2 right-2 z-30 w-6 h-6 rounded-full border-2 flex items-center justify-center transition-all pointer-events-none ${selectedBookIds.includes(book.id) ? 'bg-primary-600 border-primary-600 text-white' : 'bg-white/80 dark:bg-slate-900/80 border-slate-300 dark:border-slate-600'}`}>
                      {selectedBookIds.includes(book.id) && <Check size={14} />}
                    </div>
                    <div className={`transition-opacity duration-200 ${selectedBookIds.includes(book.id) ? 'opacity-100' : 'opacity-60 grayscale-[0.5]'}`}>
                      <BookCard 
                        book={book} 
                        disableLink 
                        onClick={() => toggleBookSelection(book.id)} 
                      />
                    </div>
                  </>
                ) : (
                  <BookCard book={book} />
                )}
              </div>
            ))}
          </div>

          {filteredBooks.length === 0 && filteredSeries.length === 0 && (
            <div className="py-20 text-center">
              <div className="inline-flex items-center justify-center w-20 h-20 rounded-full bg-slate-100 dark:bg-slate-900 text-slate-400 mb-4">
                <Search size={40} />
              </div>
              <h3 className="text-lg font-medium dark:text-white">未找到相关内容</h3>
              <p className="text-slate-500 mt-2">换个关键词试试吧</p>
            </div>
          )}
        </>
      ) : (
        <div className="py-20 text-center bg-white dark:bg-slate-900 rounded-3xl border border-slate-100 dark:border-slate-800 shadow-sm">
          <div className="inline-flex items-center justify-center w-20 h-20 rounded-full bg-primary-50 dark:bg-primary-900/20 text-primary-600 mb-6">
            <Database size={40} />
          </div>
          <h3 className="text-xl font-bold dark:text-white mb-2">书架空空如也</h3>
          <p className="text-sm text-slate-500 max-w-md mx-auto mb-8">您还没有添加任何存储库，或者存储库中还没有扫描到音频文件。</p>
          <Link 
            to="/admin/libraries"
            className="inline-flex items-center gap-2 px-6 py-3 bg-primary-600 hover:bg-primary-700 text-white text-sm font-bold rounded-xl shadow-xl shadow-primary-500/30 transition-all active:scale-95"
          >
            <Plus size={18} />
            配置存储库
          </Link>
        </div>
      )}
      </div>

      {/* Series Creation Modal */}
      <SeriesModal
        isOpen={isSeriesModalOpen}
        onClose={() => setIsSeriesModalOpen(false)}
        selectedBooks={books.filter(b => selectedBookIds.includes(b.id))}
        onSuccess={() => {
          setIsSelectionMode(false);
          setSelectedBookIds([]);
          fetchData();
        }}
      />

      {/* Dynamic Safe Bottom Spacer */}
      <div 
        className="shrink-0 transition-all duration-300" 
        style={{ height: currentChapter ? 'var(--safe-bottom-with-player)' : 'var(--safe-bottom-base)' }} 
      />
    </div>
  );
};

export default BookshelfPage;
