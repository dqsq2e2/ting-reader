import React, { useEffect, useState } from 'react';
import apiClient from '../api/client';
import type { Book } from '../types';
import BookCard from '../components/BookCard';
import { Heart, BookX } from 'lucide-react';
import { Link } from 'react-router-dom';
import { usePlayerStore } from '../store/playerStore';

const FavoritesPage: React.FC = () => {
  const currentChapter = usePlayerStore((state) => state.currentChapter);
  const [books, setBooks] = useState<Book[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const fetchFavorites = async () => {
      try {
        const response = await apiClient.get('/api/favorites');
        setBooks(response.data);
      } catch (err) {
        console.error('Failed to fetch favorites', err);
      } finally {
        setLoading(false);
      }
    };
    fetchFavorites();
  }, []);

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-primary-600"></div>
      </div>
    );
  }

  return (
    <div className="flex-1 min-h-full flex flex-col p-4 sm:p-6 md:p-8 animate-in fade-in duration-500">
      <div className="flex-1 space-y-6">
        <div>
          <h1 className="text-2xl md:text-3xl font-bold text-slate-900 dark:text-white flex items-center gap-3">
            <Heart className="text-red-500" fill="currentColor" />
            我的收藏
          </h1>
          <p className="text-sm md:text-base text-slate-500 dark:text-slate-400 mt-1">您最喜爱的 {books.length} 部作品</p>
        </div>

      {books.length > 0 ? (
        <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-6">
          {books.map((book) => (
            <BookCard key={book.id} book={book} />
          ))}
        </div>
      ) : (
        <div className="py-20 text-center bg-white dark:bg-slate-900 rounded-3xl border border-dashed border-slate-200 dark:border-slate-800">
          <div className="inline-flex items-center justify-center w-20 h-20 rounded-full bg-red-50 dark:bg-red-900/10 text-red-400 mb-6">
            <Heart size={40} />
          </div>
          <h3 className="text-xl font-bold dark:text-white">您的收藏夹还是空的</h3>
          <p className="text-sm text-slate-500 mt-2 mb-8">点击书籍详情页的爱心图标，即可收藏您喜欢的作品</p>
          <Link 
            to="/bookshelf" 
            className="px-6 py-3 bg-primary-600 hover:bg-primary-700 text-white text-sm font-bold rounded-xl shadow-lg shadow-primary-500/30 transition-all"
          >
            去书架看看
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

export default FavoritesPage;
