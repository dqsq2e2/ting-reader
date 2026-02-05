import React, { useEffect, useState } from 'react';
import { useNavigate, Link } from 'react-router-dom';
import apiClient from '../api/client';
import type { Book, Library } from '../types';
import BookCard from '../components/BookCard';
import { Search, Filter, Database, Plus, Library as LibraryIcon } from 'lucide-react';
import { usePlayerStore } from '../store/playerStore';

const BookshelfPage: React.FC = () => {
  const navigate = useNavigate();
  const currentChapter = usePlayerStore((state) => state.currentChapter);
  const [books, setBooks] = useState<Book[]>([]);
  const [libraries, setLibraries] = useState<Library[]>([]);
  const [selectedLibraryId, setSelectedLibraryId] = useState<string>('');
  const [loading, setLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState('');
  const [sortBy, setSortBy] = useState<'created_at' | 'title' | 'author'>('created_at');
  const [showFilterMenu, setShowFilterMenu] = useState(false);

  useEffect(() => {
    const fetchData = async () => {
      setLoading(true);
      try {
        const [booksRes, libsRes] = await Promise.all([
          apiClient.get('/api/books', { params: { library_id: selectedLibraryId || undefined } }),
          apiClient.get('/api/libraries')
        ]);
        setBooks(booksRes.data);
        setLibraries(libsRes.data);
      } catch (err) {
        console.error('Failed to fetch data', err);
      } finally {
        setLoading(false);
      }
    };
    fetchData();
  }, [selectedLibraryId]);

  const sortedBooks = [...books].sort((a, b) => {
    if (sortBy === 'title') return a.title.localeCompare(b.title, 'zh-CN');
    if (sortBy === 'author') return (a.author || '').localeCompare(b.author || '', 'zh-CN');
    return new Date(b.created_at).getTime() - new Date(a.created_at).getTime();
  });

  const filteredBooks = sortedBooks.filter(book => 
    book.title.toLowerCase().includes(searchQuery.toLowerCase()) ||
    book.author?.toLowerCase().includes(searchQuery.toLowerCase()) ||
    book.narrator?.toLowerCase().includes(searchQuery.toLowerCase())
  );

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-primary-600"></div>
      </div>
    );
  }

  return (
    <div className="flex-1 min-h-full flex flex-col p-4 sm:p-6 md:p-8">
      <div className="flex-1 space-y-6">
        <div className="flex flex-col md:flex-row md:items-center justify-between gap-4 mb-2">
          <div>
            <h1 className="text-2xl md:text-3xl font-bold text-slate-900 dark:text-white">我的书架</h1>
            <p className="text-sm md:text-base text-slate-500 dark:text-slate-400 mt-1">发现您收藏的所有有声读物。</p>
          </div>
          
          <div className="flex items-center gap-3">
            {/* Library Selector */}
            {libraries.length > 0 && (
              <div className="relative">
                <div className="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none text-slate-400">
                  <LibraryIcon size={16} />
                </div>
                <select
                  value={selectedLibraryId}
                  onChange={(e) => setSelectedLibraryId(e.target.value)}
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

            <div className="relative flex-1 md:w-64">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" size={18} />
              <input 
                type="text"
                placeholder="搜索书名、作者..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="w-full pl-10 pr-4 py-2.5 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 transition-all dark:text-white"
              />
            </div>
            <div className="relative">
              <button 
                onClick={() => setShowFilterMenu(!showFilterMenu)}
                className={`p-2.5 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl text-slate-600 dark:text-slate-400 hover:bg-slate-50 dark:hover:bg-slate-800 transition-colors ${showFilterMenu ? 'ring-2 ring-primary-500' : ''}`}
              >
                <Filter size={20} />
              </button>

              {showFilterMenu && (
                <div className="absolute right-0 mt-2 w-48 bg-white dark:bg-slate-900 border border-slate-100 dark:border-slate-800 rounded-2xl shadow-xl z-50 py-2 animate-in zoom-in-95 duration-200">
                  <div className="px-4 py-2 text-xs font-bold text-slate-400 uppercase tracking-widest border-b border-slate-50 dark:border-slate-800 mb-1">
                    排序方式
                  </div>
                  <button 
                    onClick={() => { setSortBy('created_at'); setShowFilterMenu(false); }}
                    className={`w-full px-4 py-2.5 text-left text-sm flex items-center justify-between ${sortBy === 'created_at' ? 'text-primary-600 font-bold bg-primary-50/50 dark:bg-primary-900/20' : 'text-slate-600 dark:text-slate-300 hover:bg-slate-50 dark:hover:bg-slate-800'}`}
                  >
                    最近添加
                    {sortBy === 'created_at' && <div className="w-1.5 h-1.5 rounded-full bg-primary-600" />}
                  </button>
                  <button 
                    onClick={() => { setSortBy('title'); setShowFilterMenu(false); }}
                    className={`w-full px-4 py-2.5 text-left text-sm flex items-center justify-between ${sortBy === 'title' ? 'text-primary-600 font-bold bg-primary-50/50 dark:bg-primary-900/20' : 'text-slate-600 dark:text-slate-300 hover:bg-slate-50 dark:hover:bg-slate-800'}`}
                  >
                    书名排序
                    {sortBy === 'title' && <div className="w-1.5 h-1.5 rounded-full bg-primary-600" />}
                  </button>
                  <button 
                    onClick={() => { setSortBy('author'); setShowFilterMenu(false); }}
                    className={`w-full px-4 py-2.5 text-left text-sm flex items-center justify-between ${sortBy === 'author' ? 'text-primary-600 font-bold bg-primary-50/50 dark:bg-primary-900/20' : 'text-slate-600 dark:text-slate-300 hover:bg-slate-50 dark:hover:bg-slate-800'}`}
                  >
                    作者排序
                    {sortBy === 'author' && <div className="w-1.5 h-1.5 rounded-full bg-primary-600" />}
                  </button>
                </div>
              )}
            </div>
          </div>
        </div>

      {books.length > 0 ? (
        <>
          <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-6">
            {filteredBooks.map((book) => (
              <BookCard key={book.id} book={book} />
            ))}
          </div>

          {filteredBooks.length === 0 && (
            <div className="py-20 text-center">
              <div className="inline-flex items-center justify-center w-20 h-20 rounded-full bg-slate-100 dark:bg-slate-900 text-slate-400 mb-4">
                <Search size={40} />
              </div>
              <h3 className="text-lg font-medium dark:text-white">未找到相关书籍</h3>
              <p className="text-slate-500 mt-2">换个关键词试试吧</p>
            </div>
          )}
        </>
      ) : (
        <div className="py-24 text-center bg-white dark:bg-slate-900 rounded-3xl border border-slate-100 dark:border-slate-800 shadow-sm">
          <div className="inline-flex items-center justify-center w-24 h-24 rounded-full bg-primary-50 dark:bg-primary-900/20 text-primary-600 mb-6">
            <Database size={48} />
          </div>
          <h3 className="text-2xl font-bold dark:text-white mb-3">书架空空如也</h3>
          <p className="text-slate-500 max-w-md mx-auto mb-10">您还没有添加任何存储库，或者存储库中还没有扫描到音频文件。</p>
          <Link 
            to="/admin/libraries"
            className="inline-flex items-center gap-2 px-8 py-4 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-2xl shadow-xl shadow-primary-500/30 transition-all active:scale-95"
          >
            <Plus size={20} />
            配置存储库
          </Link>
        </div>
      )}
      </div>

      {/* Dynamic Safe Bottom Spacer */}
      <div 
        className="shrink-0 transition-all duration-300" 
        style={{ height: currentChapter ? 'var(--safe-bottom-with-player)' : 'var(--safe-bottom-base)' }} 
      />
    </div>
  );
};

export default BookshelfPage;
