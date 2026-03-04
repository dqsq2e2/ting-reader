import React, { useState, useEffect, useCallback } from 'react';
import apiClient from '../api/client';
import type { ScrapeDiff, ChapterChange } from '../types';
import { X, Save, AlertTriangle, Loader2, RefreshCw } from 'lucide-react';

interface Props {
  bookId: string;
  onClose: () => void;
  onSave: () => void;
}

const ScrapeDiffModal: React.FC<Props> = ({ bookId, onClose, onSave }) => {
  const [diff, setDiff] = useState<ScrapeDiff | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [selectedChanges, setSelectedChanges] = useState<Set<number>>(new Set());
  
  // We need to know the libraryId to use the proxy correctly if needed.
  // Ideally, this should be passed as a prop, but for now we can try to infer it 
  // or fetch it. Since we only have bookId, we might need to fetch the book details first.
  // const [libraryId, setLibraryId] = useState<string>("");

  const fetchDiff = useCallback(async () => {
    try {
      setLoading(true);
      
      // Fetch current book details to get the title AND library_id
      const bookRes = await apiClient.get(`/api/books/${bookId}`);
      const bookTitle = bookRes.data.title;
      // if (bookRes.data.library_id) {
      //    setLibraryId(bookRes.data.library_id);
      // }
      
      const res = await apiClient.post(`/api/books/${bookId}/scrape-diff`, {
        query: bookTitle
      });

      // Fix camelCase vs snake_case issue if needed
      if (res.data.scraped) {
          if (res.data.scraped.coverUrl && !res.data.scraped.cover_url) {
              res.data.scraped.cover_url = res.data.scraped.coverUrl;
          }
          if (res.data.scraped.chapterChanges && !res.data.scraped.chapter_changes) {
              res.data.scraped.chapter_changes = res.data.scraped.chapterChanges;
          }
          if (res.data.chapterChanges && !res.data.chapter_changes) {
              res.data.chapter_changes = res.data.chapterChanges;
          }
          
          // Ensure URL is clean
          if (res.data.scraped.cover_url) {
              res.data.scraped.cover_url = String(res.data.scraped.cover_url).replace(/[`\s]/g, '').trim();
          }
      }

      setDiff(res.data);
      // Default select all "update" and "new" changes
      const initialSelected = new Set<number>();
      const changes = res.data.chapter_changes || [];
      changes.forEach((c: ChapterChange) => {
        if (c.status === 'update' || c.status === 'new') {
          initialSelected.add(c.index);
        }
      });
      setSelectedChanges(initialSelected);
    } catch (err) {
      console.error('Failed to fetch scrape diff', err);
      // alert('获取元数据失败');
    } finally {
      setLoading(false);
    }
  }, [bookId]);

  useEffect(() => {
    fetchDiff();
  }, [fetchDiff]);

  // const [imageError, setImageError] = useState<Record<string, boolean>>({});

  const cleanUrl = (url?: string) => {
    if (!url) return '';
    let cleaned = url.replace(/[`\s]/g, '');
    
    // Remove query parameters starting with ! or ?
    const queryIndex = cleaned.indexOf('!');
    if (queryIndex !== -1) {
        cleaned = cleaned.substring(0, queryIndex);
    }
    const qIndex = cleaned.indexOf('?');
    if (qIndex !== -1) {
        cleaned = cleaned.substring(0, qIndex);
    }

    // Upgrade HTTP to HTTPS for better compatibility
    if (cleaned.startsWith('http:')) {
      cleaned = cleaned.replace('http:', 'https:');
    }
    
    // Explicitly trim again to be safe
    return cleaned.trim();
  };

  /*
  const handleImageError = (type: 'current' | 'scraped') => {
    setImageError(prev => ({ ...prev, [type]: true }));
  };
  */

  /*
  useEffect(() => {
    // Reset image errors when diff changes
    setImageError({});
  }, [diff]);
  */

  const handleApply = async () => {
    if (!diff) return;
    try {
      setSaving(true);
      
      // Construct metadata matching BookDetail structure
      const metadata = {
        id: '', // Not used for update
        title: diff.scraped.title,
        author: diff.scraped.author,
        narrator: diff.scraped.narrator || null,
        cover_url: cleanUrl(diff.scraped.cover_url) || null,
        intro: diff.scraped.description,
        tags: diff.scraped.tags || [],
        chapter_count: 0, // Not used for update
        duration: 0
      };

      await apiClient.post(`/api/books/${bookId}/scrape-apply`, {
        metadata: metadata,
        apply_metadata: true,
        apply_chapters: Array.from(selectedChanges)
      });
      onSave();
      onClose();
    } catch (err) {
      console.error('Failed to apply scrape result', err);
      alert('应用失败');
    } finally {
      setSaving(false);
    }
  };

  /*
  const toggleChange = (index: number) => {
    const newSelected = new Set(selectedChanges);
    if (newSelected.has(index)) {
      newSelected.delete(index);
    } else {
      newSelected.add(index);
    }
    setSelectedChanges(newSelected);
  };
  */

  if (loading) {
    return (
      <div className="fixed inset-0 z-[200] flex items-center justify-center p-4">
        <div className="absolute inset-0 bg-slate-900/60 backdrop-blur-sm"></div>
        <div className="relative bg-white dark:bg-slate-900 p-8 rounded-3xl shadow-2xl flex flex-col items-center gap-4">
          <Loader2 className="animate-spin text-primary-600" size={40} />
          <p className="font-bold text-slate-600 dark:text-slate-400">正在获取元数据...</p>
        </div>
      </div>
    );
  }

  if (!diff) {
    return (
      <div className="fixed inset-0 z-[200] flex items-center justify-center p-4">
        <div className="absolute inset-0 bg-slate-900/60 backdrop-blur-sm" onClick={onClose}></div>
        <div className="relative bg-white dark:bg-slate-900 p-8 rounded-3xl shadow-2xl flex flex-col items-center gap-4 max-w-sm text-center">
          <AlertTriangle className="text-yellow-500" size={40} />
          <h3 className="text-xl font-bold dark:text-white">未找到元数据</h3>
          <p className="text-slate-500 text-sm">无法从在线源找到此书籍的匹配信息。</p>
          <button onClick={onClose} className="px-6 py-2 bg-slate-100 dark:bg-slate-800 font-bold rounded-xl mt-4">关闭</button>
        </div>
      </div>
    );
  }

  return (
    <div className="fixed inset-0 z-[200] flex items-center justify-center p-4">
      <div className="absolute inset-0 bg-slate-900/60 backdrop-blur-sm" onClick={onClose}></div>
      <div className="relative w-full max-w-2xl max-h-[90vh] bg-white dark:bg-slate-900 rounded-3xl shadow-2xl flex flex-col animate-in zoom-in-95 duration-200">
        
        {/* Header */}
        <div className="flex items-center justify-between p-6 border-b border-slate-100 dark:border-slate-800">
          <h2 className="text-xl font-bold dark:text-white flex items-center gap-2">
            <RefreshCw size={24} className="text-primary-600" />
            更新元数据
          </h2>
          <button onClick={onClose} className="p-2 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-full transition-colors">
            <X size={24} className="text-slate-500" />
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-6 min-h-0">
          <div className="flex flex-col gap-6">
            
            <div className="flex gap-6">
              {/* Cover Image */}
              <div className="shrink-0">
                <div className="w-32 h-48 rounded-xl overflow-hidden shadow-lg border border-slate-200 dark:border-slate-700 bg-slate-100 dark:bg-slate-800 relative group">
                  {diff.scraped.cover_url ? (
                      <img 
                        src={diff.scraped.cover_url} 
                        className="w-full h-full object-cover" 
                        alt="Cover"
                        referrerPolicy="no-referrer"
                        onError={(e) => {
                             // Hide on error
                             e.currentTarget.style.display = 'none';
                        }} 
                      />
                  ) : (
                      <div className="w-full h-full flex flex-col items-center justify-center text-slate-400">
                          <span className="text-sm">无封面</span>
                      </div>
                  )}
                </div>
              </div>

              {/* Metadata Fields */}
              <div className="flex-1 space-y-4">
                <div>
                  <label className="block text-xs font-bold text-slate-400 uppercase tracking-wider mb-1">标题</label>
                  <div className="text-lg font-bold text-slate-900 dark:text-white leading-tight">
                    {diff.scraped.title || <span className="text-slate-400 italic">未获取</span>}
                  </div>
                </div>

                <div>
                  <label className="block text-xs font-bold text-slate-400 uppercase tracking-wider mb-1">作者</label>
                  <div className="text-base text-slate-700 dark:text-slate-300">
                    {diff.scraped.author || <span className="text-slate-400 italic">Unknown</span>}
                  </div>
                </div>

                <div>
                  <label className="block text-xs font-bold text-slate-400 uppercase tracking-wider mb-1">演播</label>
                  <div className="text-base text-slate-700 dark:text-slate-300">
                    {diff.scraped.narrator || <span className="text-slate-400 italic">-</span>}
                  </div>
                </div>
                <div>
                  <label className="block text-xs font-bold text-slate-400 uppercase tracking-wider mb-1">标签</label>
                  <div className="flex flex-wrap gap-2">
                    {diff.scraped.tags && diff.scraped.tags.length > 0 ? (
                        diff.scraped.tags.map((tag, idx) => (
                            <span key={idx} className="px-2 py-1 bg-slate-100 dark:bg-slate-800 text-xs font-medium text-slate-600 dark:text-slate-400 rounded-md">
                                {tag}
                            </span>
                        ))
                    ) : (
                        <span className="text-slate-400 italic text-sm">-</span>
                    )}
                  </div>
                </div>

              </div>
            </div>

            {/* Description */}
            <div className="bg-slate-50 dark:bg-slate-800/50 rounded-xl p-4 border border-slate-100 dark:border-slate-800">
              <label className="block text-xs font-bold text-slate-400 uppercase tracking-wider mb-2">简介</label>
              <div className="text-sm text-slate-600 dark:text-slate-400 leading-relaxed whitespace-pre-wrap max-h-60 overflow-y-auto pr-2 custom-scrollbar">
                {diff.scraped.description || <span className="text-slate-400 italic">无简介</span>}
              </div>
            </div>

          </div>
        </div>

        {/* Footer */}
        <div className="p-6 border-t border-slate-100 dark:border-slate-800 flex flex-col sm:flex-row justify-between items-center gap-4">
          <div className="text-sm text-slate-500 text-center sm:text-left">
            确认更新后将覆盖现有元数据
          </div>
          <div className="flex gap-3 w-full sm:w-auto">
            <button 
              onClick={onClose}
              className="flex-1 sm:flex-none px-6 py-2 font-bold text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-xl transition-all"
            >
              取消
            </button>
            <button 
              onClick={handleApply}
              disabled={saving}
              className="flex-1 sm:flex-none px-8 py-2 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-xl shadow-lg shadow-primary-500/30 flex items-center justify-center gap-2 transition-all disabled:opacity-50 disabled:shadow-none whitespace-nowrap"
            >
              {saving ? <Loader2 className="animate-spin" size={20} /> : <Save size={20} />}
              确认更新
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};

export default ScrapeDiffModal;
