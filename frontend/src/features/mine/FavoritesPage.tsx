import React, { useEffect, useState } from 'react';
import apiClient from '../../core/api/client';
import type { Book } from '../../core/types';
import BookCard from '../../shared/cards/BookCard';
import { Heart } from 'lucide-react';
import { Link } from 'react-router-dom';
import { usePlayerStore } from '../../core/stores/playerStore';
import BackButton from '../../shared/widgets/BackButton';
import LoadingSpinner from '../../shared/ui/LoadingSpinner';
import { useTranslation } from 'react-i18next';

const FavoritesPage: React.FC = () => {
  const { t } = useTranslation();
  const currentChapter = usePlayerStore((state) => state.currentChapter);
  const [books, setBooks] = useState<Book[]>([]);
  const [loading, setLoading] = useState(true);
  const [coverShape, setCoverShape] = useState<'rect' | 'square'>('rect');
  const [iconSize, setIconSize] = useState<'small' | 'medium' | 'large'>('medium');

  useEffect(() => {
    const fetchFavorites = async () => {
      try {
        const response = await apiClient.get('/api/favorites');
        setBooks(response.data);
      } catch (err) {
        console.error('获取收藏失败', err);
      } finally {
        setLoading(false);
      }
    };
    fetchFavorites();
  }, []);

  useEffect(() => {
    const loadSettings = async () => {
      try {
        const settingsRes = await apiClient.get('/api/settings');
        const settings = settingsRes.data.settings_json || {};
        if (settings.bookshelf_cover_shape) {
          setCoverShape(settings.bookshelf_cover_shape);
        }
        if (settings.bookshelf_icon_size) {
          setIconSize(settings.bookshelf_icon_size);
        }
      } catch (err) {
        console.error('加载设置失败', err);
      }
    };
    loadSettings();
  }, []);

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

  if (loading) {
    return (
      <LoadingSpinner />
    );
  }

  return (
    <div className="flex-1 min-h-full flex flex-col p-4 sm:p-6 md:p-8 animate-in fade-in duration-500">
      <div className="flex-1 space-y-6">
        <BackButton fallback="/mine" />

        <div>
          <h1 className="text-2xl md:text-3xl font-bold text-slate-900 dark:text-white flex items-center gap-3">
            <Heart className="text-red-500" fill="currentColor" />
            {t('favoritesPage.title')}
          </h1>
          <p className="text-sm md:text-base text-slate-500 dark:text-slate-400 mt-1">{t('favoritesPage.subtitle', { count: books.length })}</p>
        </div>

      {books.length > 0 ? (
        <div className={`grid ${getGridCols()}`}>
          {books.map((book) => (
            <BookCard key={book.id} book={book} coverShape={coverShape} />
          ))}
        </div>
      ) : (
        <div className="py-20 text-center bg-white dark:bg-slate-900 rounded-3xl border border-dashed border-slate-200 dark:border-slate-800">
          <div className="inline-flex items-center justify-center w-20 h-20 rounded-full bg-red-50 dark:bg-red-900/10 text-red-400 mb-6">
            <Heart size={40} />
          </div>
          <h3 className="text-xl font-bold dark:text-white">{t('favoritesPage.emptyTitle')}</h3>
          <p className="text-sm text-slate-500 mt-2 mb-8">{t('favoritesPage.emptyHint')}</p>
          <Link 
            to="/bookshelf" 
            className="px-6 py-3 bg-primary-600 hover:bg-primary-700 text-white text-sm font-bold rounded-xl shadow-lg shadow-primary-500/30 transition-all"
          >
            {t('favoritesPage.goBookshelf')}
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
