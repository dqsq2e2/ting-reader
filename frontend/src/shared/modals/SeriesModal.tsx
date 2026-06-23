import React, { useState } from 'react';
import apiClient from '../../core/api/client';
import type { Book } from '../../core/types';

interface SeriesModalProps {
  isOpen: boolean;
  onClose: () => void;
  selectedBooks: Book[];
  onSuccess: () => void;
}

const SeriesModal: React.FC<SeriesModalProps> = ({ isOpen, onClose, selectedBooks, onSuccess }) => {
  const firstBook = selectedBooks[0];
  const [title, setTitle] = useState('');
  const [author, setAuthor] = useState('');
  const [narrator, setNarrator] = useState('');
  const [coverUrl, setCoverUrl] = useState('');
  const [description, setDescription] = useState('');
  const [loading, setLoading] = useState(false);

  React.useEffect(() => {
    if (isOpen && firstBook) {
      setAuthor(firstBook.author || '');
      setNarrator(firstBook.narrator || '');
      setCoverUrl(firstBook.coverUrl || '');
      setDescription(firstBook.description || '');
      // Reset title when opening
      setTitle('');
    }
  }, [isOpen, firstBook]);

  if (!isOpen) return null;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    try {
      await apiClient.post('/api/v1/series', {
        library_id: firstBook.libraryId,
        title,
        author: author || undefined, // Send undefined if empty to let backend decide or stick to empty
        narrator: narrator || undefined,
        cover_url: coverUrl || undefined,
        description: description || undefined,
        book_ids: selectedBooks.map(b => b.id)
      });
      onSuccess();
      onClose();
    } catch (err) {
      console.error('创建系列失败', err);
      alert('创建系列失败');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
      <div className="bg-white dark:bg-slate-900 rounded-2xl w-full max-w-lg p-6 shadow-xl animate-in zoom-in-95 duration-200">
        <h2 className="text-xl font-bold mb-4 text-slate-900 dark:text-white">创建新系列</h2>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-slate-700 dark:text-slate-300 mb-1">系列名称</label>
            <input
              type="text"
              required
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              className="w-full px-3 py-2 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-lg focus:ring-2 focus:ring-primary-500 outline-none text-slate-900 dark:text-white"
              placeholder="输入系列名称"
            />
          </div>
          
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-slate-700 dark:text-slate-300 mb-1">作者</label>
              <input
                type="text"
                value={author}
                onChange={(e) => setAuthor(e.target.value)}
                className="w-full px-3 py-2 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-lg focus:ring-2 focus:ring-primary-500 outline-none text-slate-900 dark:text-white"
                placeholder="可选"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-slate-700 dark:text-slate-300 mb-1">演播</label>
              <input
                type="text"
                value={narrator}
                onChange={(e) => setNarrator(e.target.value)}
                className="w-full px-3 py-2 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-lg focus:ring-2 focus:ring-primary-500 outline-none text-slate-900 dark:text-white"
                placeholder="可选"
              />
            </div>
          </div>

          <div>
            <label className="block text-sm font-medium text-slate-700 dark:text-slate-300 mb-1">简介</label>
            <textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              rows={3}
              className="w-full px-3 py-2 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-lg focus:ring-2 focus:ring-primary-500 outline-none text-slate-900 dark:text-white resize-none"
              placeholder="可选"
            />
          </div>

          <div className="flex justify-end gap-3 pt-4">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-lg transition-colors"
            >
              取消
            </button>
            <button
              type="submit"
              disabled={loading || !title}
              className="px-4 py-2 bg-primary-600 text-white rounded-lg hover:bg-primary-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {loading ? '创建中...' : '创建系列'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};

export default SeriesModal;
