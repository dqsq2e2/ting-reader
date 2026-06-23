import React, { useState, useEffect } from 'react';
import apiClient from '../../core/api/client';
import type { Book } from '../../core/types';
import { Search, X, Book as BookIcon, Loader2 } from 'lucide-react';
import { getCoverUrl } from '../../core/utils/image';
import { getCoverAspectClass, useBookshelfCoverShape } from '../../core/hooks/useBookshelfCoverShape';

interface Props {
  onSelect: (book: Book) => void;
  onClose: () => void;
  excludeIds?: string[];
}

const BookSelector: React.FC<Props> = ({ onSelect, onClose, excludeIds }) => {
  const coverShape = useBookshelfCoverShape();
  const [search, setSearch] = useState('');
  const [books, setBooks] = useState<Book[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    const fetchBooks = async () => {
      setLoading(true);
      try {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const params: Record<string, any> = { search };
        const res = await apiClient.get('/api/books', { params });
        let list: Book[] = res.data;
        if (excludeIds && excludeIds.length > 0) {
            list = list.filter(b => !excludeIds.includes(b.id));
        }
        setBooks(list);
      } catch (err) {
        console.error('获取书籍失败', err);
      } finally {
        setLoading(false);
      }
    };

    const timer = setTimeout(fetchBooks, 300);
    return () => clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [search, JSON.stringify(excludeIds)]);

  return (
    <div className="fixed inset-0 z-[250] flex items-end sm:items-center justify-center p-0 sm:p-4">
      <div className="absolute inset-0 bg-slate-900/60 backdrop-blur-sm" onClick={onClose}></div>
      <div className="relative w-full sm:max-w-lg max-h-[88vh] sm:max-h-[78vh] bg-white dark:bg-slate-900 rounded-t-3xl sm:rounded-2xl shadow-2xl flex flex-col animate-in zoom-in-95 duration-200 border border-slate-200 dark:border-slate-800">
        
        <div className="p-4 border-b border-slate-100 dark:border-slate-800 flex items-center gap-3 shrink-0">
          <Search size={20} className="text-slate-400" />
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="搜索目标书籍..."
            className="flex-1 bg-transparent border-none p-0 text-slate-900 dark:text-white focus:ring-0 placeholder-slate-400"
            autoFocus
          />
          <button onClick={onClose}>
            <X size={20} className="text-slate-400 hover:text-slate-600 dark:hover:text-slate-300" />
          </button>
        </div>

        <div className="overflow-y-auto p-2">
          {loading ? (
            <div className="py-8 flex justify-center">
              <Loader2 className="animate-spin text-primary-600" />
            </div>
          ) : books.length === 0 ? (
            <div className="py-8 text-center text-slate-500 text-sm">
              未找到相关书籍
            </div>
          ) : (
            <div className="space-y-1">
              {books.map((book) => (
                <button
                  key={book.id}
                  onClick={() => onSelect(book)}
                  className="w-full flex items-center gap-3 p-3 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-xl transition-colors text-left group"
                >
                  <div className={`w-12 sm:w-10 ${getCoverAspectClass(coverShape)} bg-slate-200 dark:bg-slate-700 rounded-md overflow-hidden shrink-0 relative shadow-sm`}>
                    {book.coverUrl ? (
                      <img 
                        src={getCoverUrl(book.coverUrl, book.libraryId, book.id)} 
                        referrerPolicy="no-referrer"
                        className="w-full h-full object-cover" 
                        alt="" 
                      />
                    ) : (
                      <div className="flex items-center justify-center h-full text-slate-400">
                        <BookIcon size={16} />
                      </div>
                    )}
                  </div>
                  <div className="flex-1 min-w-0">
                    <h4 className="font-bold text-slate-900 dark:text-white truncate group-hover:text-primary-600 transition-colors">
                        {book.title}
                    </h4>
                    <p className="text-xs text-slate-500 truncate">{book.author || '未知作者'}</p>
                  </div>
                </button>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

export default BookSelector;
