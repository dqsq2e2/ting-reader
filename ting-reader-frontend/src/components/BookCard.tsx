import React from 'react';
import type { Book } from '../types';
import { Play, Clock, User } from 'lucide-react';
import { Link } from 'react-router-dom';

import { getCoverUrl } from '../utils/image';
import { toSolidColor } from '../utils/color';
import ExpandableTitle from './ExpandableTitle';

interface BookCardProps {
  book: Book;
}

const BookCard: React.FC<BookCardProps> = ({ book }) => {
  return (
    <Link 
      to={`/book/${book.id}`}
      className="group bg-white dark:bg-slate-900 rounded-2xl md:rounded-3xl shadow-sm hover:shadow-xl transition-all duration-300 overflow-hidden border border-slate-100 dark:border-slate-800 flex flex-col"
    >
      <div className="relative aspect-[3/4] overflow-hidden">
        <img 
          src={getCoverUrl(book.cover_url, book.library_id, book.id)} 
          alt={book.title}
          crossOrigin="anonymous"
          className="w-full h-full object-cover transition-transform duration-300 group-hover:scale-105"
          onError={(e) => {
            (e.target as HTMLImageElement).src = 'https://placehold.co/300x400?text=No+Cover';
          }}
        />
        <div className="absolute inset-0 bg-black/40 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center">
          <div 
            className="w-10 h-10 md:w-12 md:h-12 rounded-full bg-primary-600 text-white flex items-center justify-center shadow-lg transform translate-y-4 group-hover:translate-y-0 transition-transform"
            style={book.theme_color ? { backgroundColor: toSolidColor(book.theme_color) } : {}}
          >
            <Play size={20} fill="currentColor" />
          </div>
        </div>
      </div>
      <div className="p-3 md:p-4 flex-1 min-w-0">
        <ExpandableTitle 
          title={book.title} 
          className="font-bold text-sm md:text-base text-slate-900 dark:text-white group-hover:text-primary-600 transition-colors" 
          maxLines={1}
        />
        <div className="mt-1 md:mt-2 space-y-0.5 md:space-y-1">
          <div className="flex items-center gap-1.5 text-[10px] md:text-xs text-slate-500 dark:text-slate-400">
            <User size={10} className="md:w-3 md:h-3" />
            <span className="line-clamp-1 font-medium">{book.author || '未知作者'}</span>
          </div>
        </div>
      </div>
    </Link>
  );
};

export default BookCard;
