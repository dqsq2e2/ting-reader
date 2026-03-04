import React, { useState, useEffect } from 'react';
import apiClient from '../api/client';
import type { Book } from '../types';
import { Search, X, Book as BookIcon, Loader2 } from 'lucide-react';

interface Props {
  onSelect: (book: Book) => void;
  onClose: () => void;
  currentBookId?: string; // Exclude current book from list
}

const BookSelector: React.FC<Props> = ({ onSelect, onClose, currentBookId }) => {
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
        if (currentBookId) {
            list = list.filter(b => b.id !== currentBookId);
        }
        setBooks(list);
      } catch (err) {
        console.error('Failed to fetch books', err);
      } finally {
        setLoading(false);
      }
    };

    const timer = setTimeout(fetchBooks, 300);
    return () => clearTimeout(timer);
  }, [search, currentBookId]);

  return (
    <div className="fixed inset-0 z-[250] flex items-center justify-center p-4">
      <div className="absolute inset-0 bg-slate-900/60 backdrop-blur-sm" onClick={onClose}></div>
      <div className="relative w-full max-w-lg bg-white dark:bg-slate-900 rounded-2xl shadow-2xl flex flex-col animate-in zoom-in-95 duration-200 border border-slate-200 dark:border-slate-800">
        
        <div className="p-4 border-b border-slate-100 dark:border-slate-800 flex items-center gap-3">
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

        <div className="max-h-[60vh] overflow-y-auto p-2">
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
                  <div className="w-10 h-14 bg-slate-200 dark:bg-slate-700 rounded-md overflow-hidden shrink-0 relative shadow-sm">
                    {book.coverUrl ? (
                      <img src={book.coverUrl} className="w-full h-full object-cover" alt="" />
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
