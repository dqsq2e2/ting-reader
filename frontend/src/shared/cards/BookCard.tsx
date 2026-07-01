import React from 'react';
import type { Book } from '../../core/types';
import { Play } from 'lucide-react';
import { Link } from 'react-router-dom';

import { getCoverUrl } from '../../core/utils/image';
import { toSolidColor, isLight, isTooLight } from '../../core/utils/color';
import ExpandableTitle from '../widgets/ExpandableTitle';
import { useTranslation } from 'react-i18next';

interface BookCardProps {
  book: Book;
  onClick?: (e: React.MouseEvent) => void;
  disableLink?: boolean;
  coverShape?: 'rect' | 'square';
}

const BookCard: React.FC<BookCardProps> = ({ book, onClick, disableLink, coverShape = 'rect' }) => {
  const { t } = useTranslation();
  const effectiveThemeColor = book.theme_color && !isTooLight(book.theme_color) ? book.theme_color : undefined;

  const content = (
    <>
      <div className={`relative ${coverShape === 'square' ? 'aspect-square' : 'aspect-[3/4]'} overflow-hidden rounded-md shadow-md bg-white dark:bg-slate-800`}>
        <img
          src={getCoverUrl(book.cover_url, book.library_id, book.id)}
          alt={book.title}
          loading="lazy"
          referrerPolicy="no-referrer"
          className="w-full h-full object-cover transition-transform duration-300 group-hover:scale-105"
          onError={(e) => {
            (e.target as HTMLImageElement).src = 'https://placehold.co/300x400?text=No+Cover';
          }}
        />
        <div className="absolute inset-0 bg-black/40 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center">
          <div 
            className={`w-10 h-10 rounded-full text-white flex items-center justify-center shadow-lg transform translate-y-4 group-hover:translate-y-0 transition-transform ${!effectiveThemeColor ? 'bg-primary-600' : ''}`}
            style={effectiveThemeColor ? { 
              backgroundColor: toSolidColor(effectiveThemeColor),
              color: isLight(effectiveThemeColor) ? '#475569' : '#ffffff'
            } : {}}
          >
            <Play size={20} fill="currentColor" />
          </div>
        </div>
      </div>
      <div className="mt-2 min-w-0">
        <ExpandableTitle 
          title={book.title} 
          className="font-bold text-sm text-slate-900 dark:text-white group-hover:text-primary-600 transition-colors leading-tight" 
          maxLines={1}
        />
        <div className="mt-1 flex flex-col gap-0.5">
          <div className="flex items-center gap-1.5 text-xs text-slate-500 dark:text-slate-400">
            <span className="line-clamp-1">{book.author || t('shared.unknownAuthor')}</span>
          </div>
        </div>
      </div>
    </>
  );

  if (disableLink) {
    return (
      <div 
        className="group flex flex-col relative cursor-pointer"
        onClick={onClick}
      >
        {content}
      </div>
    );
  }

  return (
    <Link 
      to={`/book/${book.id}`}
      className="group flex flex-col relative"
      onClick={onClick}
    >
      {content}
    </Link>
  );
};

export default BookCard;
