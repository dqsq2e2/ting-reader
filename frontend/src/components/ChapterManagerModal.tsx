import React, { useState, useMemo } from 'react';
import type { Chapter, Book } from '../types';
import apiClient from '../api/client';
import { X, Save, Loader2, Search, ArrowRight, Folder, CheckSquare, Square, ListOrdered } from 'lucide-react';
import FixedSizeList from './VirtualList';
const List = FixedSizeList;
import AutoSizer from './AutoSizer';
import BookSelector from './BookSelector';

interface Props {
  bookId: string;
  initialChapters: Chapter[];
  onClose: () => void;
  onSave: () => void;
}

const ChapterManagerModal: React.FC<Props> = ({ bookId, initialChapters, onClose, onSave }) => {
  const [chapters, setChapters] = useState<Chapter[]>(initialChapters);
  const [search, setSearch] = useState('');
  const [saving, setSaving] = useState(false);
  const [changedIds, setChangedIds] = useState<Set<string>>(new Set());
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [showBookSelector, setShowBookSelector] = useState(false);
  const [moving, setMoving] = useState(false);

  // Filter chapters
  const filteredChapters = useMemo(() => {
    if (!search) return chapters;
    const lower = search.toLowerCase();
    return chapters.filter(c => c.title.toLowerCase().includes(lower));
  }, [chapters, search]);

  const handleTitleChange = (id: string, newTitle: string) => {
    setChapters(prev => prev.map(c => 
      c.id === id ? { ...c, title: newTitle } : c
    ));
    setChangedIds(prev => new Set(prev).add(id));
  };

  const handleIndexChange = (id: string, newIndex: string) => {
    const num = parseInt(newIndex);
    if (isNaN(num)) return;
    setChapters(prev => prev.map(c => 
      c.id === id ? { ...c, chapterIndex: num } : c
    ));
    setChangedIds(prev => new Set(prev).add(id));
  };

  const toggleSelection = (id: string) => {
    const newSet = new Set(selectedIds);
    if (newSet.has(id)) newSet.delete(id);
    else newSet.add(id);
    setSelectedIds(newSet);
  };

  const toggleAll = () => {
    if (selectedIds.size === filteredChapters.length) {
      setSelectedIds(new Set());
    } else {
      setSelectedIds(new Set(filteredChapters.map(c => c.id)));
    }
  };

  const handleSave = async () => {
    try {
      setSaving(true);
      const updates = chapters
        .filter(c => changedIds.has(c.id))
        .map(c => ({
          id: c.id,
          title: c.title,
          chapter_index: c.chapterIndex,
          is_extra: c.isExtra ? 1 : 0
        }));

      if (updates.length > 0) {
        await apiClient.put(`/api/books/${bookId}/chapters/batch`, { updates });
      }
      
      onSave();
      onClose();
    } catch (err) {
      console.error('Failed to save chapters', err);
      alert('保存失败');
    } finally {
      setSaving(false);
    }
  };

  const handleRenumber = () => {
    if (confirm('确定要按当前列表顺序重新生成章节序号（从1开始）吗？')) {
      const newChapters = chapters.map((c, idx) => ({
        ...c,
        chapterIndex: idx + 1
      }));
      setChapters(newChapters);
      // Mark all as changed
      const allIds = new Set(chapters.map(c => c.id));
      setChangedIds(allIds);
    }
  };

  const handleMoveChapters = async (targetBook: Book) => {
    try {
      setMoving(true);
      await apiClient.post('/api/books/chapters/move', {
        target_book_id: targetBook.id,
        chapter_ids: Array.from(selectedIds)
      });
      setShowBookSelector(false);
      onSave(); // Reload parent
      onClose(); // Close modal
    } catch (err) {
      console.error('Failed to move chapters', err);
      alert('移动章节失败');
    } finally {
      setMoving(false);
    }
  };

  const safeDecode = (str: string) => {
    try {
      return decodeURIComponent(str);
    } catch {
      return str;
    }
  };

  const Row = ({ index, style }: { index: number, style: React.CSSProperties }) => {
    const chapter = filteredChapters[index];
    const isSelected = selectedIds.has(chapter.id);
    const isChanged = changedIds.has(chapter.id);

    return (
      <div style={style} className="px-6 py-1">
        <div className={`flex items-center gap-3 p-2 rounded-lg border transition-colors ${
            isSelected 
              ? 'bg-primary-50 dark:bg-primary-900/20 border-primary-200 dark:border-primary-800' 
              : isChanged
                ? 'bg-amber-50 dark:bg-amber-900/10 border-amber-200 dark:border-amber-800'
                : 'bg-white dark:bg-slate-800/50 border-slate-100 dark:border-slate-800 hover:border-slate-300 dark:hover:border-slate-600'
        }`}>
          {/* Checkbox */}
          <button onClick={() => toggleSelection(chapter.id)} className="shrink-0 text-slate-400 hover:text-primary-600">
            {isSelected ? <CheckSquare size={20} className="text-primary-600" /> : <Square size={20} />}
          </button>

          {/* Index */}
          <input
            type="number"
            value={chapter.chapterIndex}
            onChange={(e) => handleIndexChange(chapter.id, e.target.value)}
            className="w-16 bg-transparent text-center font-mono text-sm text-slate-500 border-b border-transparent focus:border-primary-500 focus:outline-none focus:bg-white dark:focus:bg-slate-800 rounded px-1"
          />

          {/* Title */}
          <div className="flex-1 min-w-0">
            <input
              type="text"
              value={chapter.title}
              onChange={(e) => handleTitleChange(chapter.id, e.target.value)}
              className="w-full bg-transparent border-none p-0 text-slate-900 dark:text-white font-medium focus:ring-0"
              placeholder="章节标题"
            />
          </div>

          {/* Extra Toggle */}
          <button
            onClick={() => {
              setChapters(prev => prev.map(c => 
                c.id === chapter.id ? { ...c, isExtra: !c.isExtra } : c
              ));
              setChangedIds(prev => new Set(prev).add(chapter.id));
            }}
            className={`px-2 py-0.5 text-xs font-medium rounded-md border transition-colors ${
              chapter.isExtra 
                ? 'bg-purple-50 text-purple-600 border-purple-200 dark:bg-purple-900/20 dark:text-purple-300 dark:border-purple-800' 
                : 'text-slate-400 border-transparent hover:bg-slate-100 dark:hover:bg-slate-800'
            }`}
          >
            番外
          </button>

          {/* Path (Tooltip on hover) */}
          <div className="hidden md:flex items-center gap-2 max-w-[200px] text-xs text-slate-400 shrink-0" title={safeDecode(chapter.path)}>
            <Folder size={14} />
            <span className="truncate direction-rtl">{safeDecode(chapter.path)}</span>
          </div>

        </div>
      </div>
    );
  };

  return (
    <div className="fixed inset-0 z-[200] flex items-center justify-center p-4">
      <div className="absolute inset-0 bg-slate-900/60 backdrop-blur-sm" onClick={onClose}></div>
      <div className="relative w-full max-w-5xl h-[90vh] bg-white dark:bg-slate-900 rounded-3xl shadow-2xl flex flex-col animate-in zoom-in-95 duration-200">
        
        {/* Header */}
        <div className="flex items-center justify-between p-6 border-b border-slate-100 dark:border-slate-800 shrink-0">
          <div className="flex items-center gap-4">
            <h2 className="text-2xl font-bold dark:text-white">章节管理</h2>
            <div className="flex items-center gap-2 bg-slate-100 dark:bg-slate-800 px-3 py-1.5 rounded-xl">
              <Search size={18} className="text-slate-400" />
              <input
                type="text"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder="搜索章节..."
                className="bg-transparent border-none p-0 text-sm w-40 focus:ring-0 text-slate-900 dark:text-white"
              />
            </div>
          </div>
          <button onClick={onClose} className="p-2 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-full transition-colors">
            <X size={24} className="text-slate-500" />
          </button>
        </div>

        {/* Toolbar */}
        <div className="px-6 py-3 border-b border-slate-100 dark:border-slate-800 flex items-center justify-between bg-slate-50/50 dark:bg-slate-900/50 shrink-0">
            <div className="flex items-center gap-4">
                <button 
                    onClick={toggleAll}
                    className="flex items-center gap-2 text-sm font-bold text-slate-600 dark:text-slate-400 hover:text-primary-600 transition-colors"
                >
                    {selectedIds.size > 0 && selectedIds.size === filteredChapters.length ? <CheckSquare size={18} /> : <Square size={18} />}
                    全选 ({filteredChapters.length})
                </button>
                
                {selectedIds.size > 0 && (
                    <span className="text-sm text-slate-500">
                        已选 {selectedIds.size} 项
                    </span>
                )}
            </div>

            <div className="flex gap-2">
            <button
              onClick={handleRenumber}
              className="flex items-center gap-2 px-4 py-2 bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl text-slate-600 dark:text-slate-300 font-bold text-sm hover:border-primary-500 hover:text-primary-600 transition-all shadow-sm"
              title="按列表顺序重排（从1开始）"
            >
              <ListOrdered size={16} />
              重排序号
            </button>

            <button
              onClick={() => setShowBookSelector(true)}
                    disabled={selectedIds.size === 0 || moving}
                    className="flex items-center gap-2 px-4 py-2 bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl text-slate-600 dark:text-slate-300 font-bold text-sm hover:border-primary-500 hover:text-primary-600 disabled:opacity-50 disabled:hover:border-slate-200 transition-all shadow-sm"
                >
                    {moving ? <Loader2 size={16} className="animate-spin" /> : <ArrowRight size={16} />}
                    移动到...
                </button>
            </div>
        </div>

        {/* List Content */}
        <div className="flex-1 min-h-0 w-full">
            <AutoSizer>
                {({ height, width }: { height: number, width: number }) => (
                    <List
                        height={height}
                        width={width}
                        itemCount={filteredChapters.length}
                        itemSize={60} // Row height
                    >
                        {Row}
                    </List>
                )}
            </AutoSizer>
        </div>

        {/* Footer */}
        <div className="p-6 border-t border-slate-100 dark:border-slate-800 flex justify-end gap-3 shrink-0">
          <button 
            onClick={onClose}
            className="px-6 py-3 font-bold text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-xl transition-all"
          >
            取消
          </button>
          <button 
            onClick={handleSave}
            disabled={saving || changedIds.size === 0}
            className="px-8 py-3 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-xl shadow-lg shadow-primary-500/30 flex items-center gap-2 transition-all disabled:opacity-50 disabled:shadow-none"
          >
            {saving ? <Loader2 className="animate-spin" size={20} /> : <Save size={20} />}
            保存更改 ({changedIds.size})
          </button>
        </div>

        {/* Book Selector Modal */}
        {showBookSelector && (
            <BookSelector
                currentBookId={bookId}
                onClose={() => setShowBookSelector(false)}
                onSelect={handleMoveChapters}
            />
        )}
      </div>
    </div>
  );
};

export default ChapterManagerModal;
