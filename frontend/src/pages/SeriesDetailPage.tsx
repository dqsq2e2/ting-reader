import React, { useEffect, useState } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import apiClient from '../api/client';
import type { Book, Series } from '../types';
import BookCard from '../components/BookCard';
import BookSelector from '../components/BookSelector';
import { ArrowLeft, Trash2, Save, Settings, X, Plus, Filter } from 'lucide-react';
import { getCoverUrl } from '../utils/image';
import { usePlayerStore } from '../store/playerStore';
import { useAuthStore } from '../store/authStore';

const SeriesDetailPage: React.FC = () => {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const user = useAuthStore((state) => state.user);
  const isAdmin = user?.role === 'admin';
  const [series, setSeries] = useState<Series | null>(null);
  const [books, setBooks] = useState<Book[]>([]);
  const [loading, setLoading] = useState(true);
  const [isEditing, setIsEditing] = useState(false);
  const [showBookSelector, setShowBookSelector] = useState(false);
  const { setIsSeriesEditing } = usePlayerStore();
  
  // Filter & Sort state
  const [sortBy, setSortBy] = useState<'default' | 'title' | 'author' | 'createdAt'>('default');
  const [iconSize, setIconSize] = useState<'small' | 'medium' | 'large'>('medium');
  const [coverShape, setCoverShape] = useState<'rect' | 'square'>('rect');
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
      setCoverUrl(res.data.coverUrl || '');
    } catch (err) {
      console.error('获取系列失败', err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    const loadSettings = async () => {
      try {
        const res = await apiClient.get('/api/settings');
        const settings = res.data.settingsJson || {};
        
        if (settings.seriesSortBy) {
          setSortBy(settings.seriesSortBy);
        }
        if (settings.seriesIconSize) {
          setIconSize(settings.seriesIconSize);
        }
        if (settings.bookshelfCoverShape) {
          setCoverShape(settings.bookshelfCoverShape);
        }
      } catch (err) {
        console.error('加载设置失败', err);
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

  const handleSortChange = (newSort: 'default' | 'title' | 'author' | 'createdAt') => {
    setSortBy(newSort);
    setShowFilterMenu(false);
    apiClient.post('/api/settings', { seriesSortBy: newSort });
  };

  const handleIconSizeChange = (newSize: 'small' | 'medium' | 'large') => {
    setIconSize(newSize);
    setShowFilterMenu(false);
    apiClient.post('/api/settings', { seriesIconSize: newSize });
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
      if (sortBy === 'title') return a.title.localeCompare(b.title, 'zh-CN');
      if (sortBy === 'author') return (a.author || '').localeCompare(b.author || '', 'zh-CN');
      if (sortBy === 'createdAt') return new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime();
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
      console.error('更新系列失败', err);
      alert('更新系列失败');
    }
  };

  const handleDelete = async () => {
    if (!window.confirm('确定要删除此系列吗？系列中的书籍不会被删除。')) return;
    try {
      await apiClient.delete(`/api/v1/series/${id}`);
      navigate('/bookshelf');
    } catch (err) {
      console.error('删除系列失败', err);
    }
  };

  const moveBook = (fromIndex: number, toIndex: number) => {
    const updatedBooks = [...books];
    const [movedBook] = updatedBooks.splice(fromIndex, 1);
    updatedBooks.splice(toIndex, 0, movedBook);
    setBooks(updatedBooks);
  };

  if (loading) return <div className="p-8 text-center">加载中...</div>;
  if (!series) return <div className="p-8 text-center">系列未找到</div>;

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
            {isEditing ? '管理系列' : series.title}
          </h1>
        </div>
        {!isEditing && (
            <div className="flex items-center gap-2 relative">
                <button 
                  onClick={() => setShowFilterMenu(!showFilterMenu)}
                  className={`p-2.5 bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-400 rounded-xl hover:bg-slate-200 dark:hover:bg-slate-700 transition-colors ${showFilterMenu ? 'ring-2 ring-primary-500' : ''}`}
                >
                  <Filter size={20} />
                </button>

                {isAdmin && (
                  <button 
                    onClick={() => setIsEditing(true)}
                    className="p-2.5 bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-400 rounded-xl hover:bg-slate-200 dark:hover:bg-slate-700 transition-colors"
                  >
                    <Settings size={20} />
                  </button>
                )}

                {showFilterMenu && (
                    <div className="absolute right-0 top-full mt-2 w-56 bg-white dark:bg-slate-900 border border-slate-100 dark:border-slate-800 rounded-2xl shadow-xl z-50 py-2 animate-in zoom-in-95 duration-200">
                    <div className="px-4 py-2 text-xs font-bold text-slate-400 uppercase tracking-widest border-b border-slate-50 dark:border-slate-800 mb-1">
                        排序方式
                    </div>
                    <button 
                        onClick={() => handleSortChange('default')}
                        className={`w-full px-4 py-2.5 text-left text-sm flex items-center justify-between ${sortBy === 'default' ? 'text-primary-600 font-bold bg-primary-50/50 dark:bg-primary-900/20' : 'text-slate-600 dark:text-slate-300 hover:bg-slate-50 dark:hover:bg-slate-800'}`}
                    >
                        默认排序
                        {sortBy === 'default' && <div className="w-1.5 h-1.5 rounded-full bg-primary-600" />}
                    </button>
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
        )}
      </div>

      {isEditing ? (
        // EDIT MODE (Original Management View)
        <div className="grid md:grid-cols-[300px_1fr] gap-8 animate-in fade-in slide-in-from-bottom-4 duration-300">
          {/* Sidebar Info - Editing */}
          <div className="space-y-6">
            <div className="aspect-[3/4] rounded-2xl overflow-hidden shadow-lg">
              <img 
                src={getCoverUrl(coverUrl, series.libraryId)} 
                className="w-full h-full object-cover"
                alt={series.title}
              />
            </div>
            
            <div className="space-y-3">
              <input 
                value={title} 
                onChange={e => setTitle(e.target.value)}
                className="w-full p-2 bg-white dark:bg-slate-800 border rounded"
                placeholder="标题"
              />
              <input 
                value={author} 
                onChange={e => setAuthor(e.target.value)}
                className="w-full p-2 bg-white dark:bg-slate-800 border rounded"
                placeholder="作者"
              />
              <input 
                value={narrator} 
                onChange={e => setNarrator(e.target.value)}
                className="w-full p-2 bg-white dark:bg-slate-800 border rounded"
                placeholder="演播"
              />
              <input 
                value={coverUrl} 
                onChange={e => setCoverUrl(e.target.value)}
                className="w-full p-2 bg-white dark:bg-slate-800 border rounded"
                placeholder="封面URL"
              />
              <textarea 
                value={description} 
                onChange={e => setDescription(e.target.value)}
                className="w-full p-2 bg-white dark:bg-slate-800 border rounded"
                placeholder="简介"
              />
              <div className="flex gap-2">
                <button onClick={handleUpdate} className="flex-1 bg-primary-600 text-white py-2 rounded flex items-center justify-center gap-2">
                  <Save size={18} /> 保存
                </button>
                <button onClick={() => setIsEditing(false)} className="flex-1 bg-slate-200 dark:bg-slate-700 py-2 rounded">
                  取消
                </button>
              </div>
              <button onClick={handleDelete} className="w-full p-2 bg-red-50 text-red-600 rounded hover:bg-red-100 flex items-center justify-center gap-2">
                  <Trash2 size={18} /> 删除系列
              </button>
            </div>
          </div>

          {/* Book List / Reordering */}
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <h3 className="text-xl font-bold dark:text-white">包含书籍 ({books.length})</h3>
              <div className="flex items-center gap-2">
                {books.length > 1 && (
                    <p className="text-xs text-slate-400 mr-2">使用箭头调整顺序</p>
                )}
                <button 
                    onClick={() => setShowBookSelector(true)}
                    className="p-1.5 bg-primary-50 dark:bg-primary-900/20 text-primary-600 rounded-lg hover:bg-primary-100 transition-colors flex items-center gap-1 text-sm font-bold px-3"
                >
                    <Plus size={16} /> 添加书籍
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
                  
                  <div className="w-12 h-16 rounded overflow-hidden flex-shrink-0">
                    <img src={getCoverUrl(book.coverUrl, book.libraryId, book.id)} className="w-full h-full object-cover" alt="" />
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
                    <h3 className="text-lg font-bold dark:text-white">包含书籍 ({books.length})</h3>
                </div>
                
                {books.length > 0 ? (
                    <div className={`grid ${getGridCols()}`}>
                        {getSortedBooks().map((book) => (
                            <BookCard key={book.id} book={book} coverShape={coverShape} />
                        ))}
                    </div>
                ) : (
                  <div className="py-20 text-center bg-slate-50 dark:bg-slate-900 rounded-2xl border border-dashed border-slate-200 dark:border-slate-800">
                      <p className="text-slate-500">系列中暂无书籍</p>
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
    </div>
  );
};

export default SeriesDetailPage;