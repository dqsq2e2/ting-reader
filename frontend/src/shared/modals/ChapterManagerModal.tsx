import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { Loader2, Save } from 'lucide-react';
import type { Book, Chapter, Library } from '../../core/types';
import apiClient from '../../core/api/client';
import BookSelector from './BookSelector';
import ChapterEditDialog from './chapterManager/ChapterEditDialog';
import ChapterManagerHeader from './chapterManager/ChapterManagerHeader';
import ChapterManagerList from './chapterManager/ChapterManagerList';
import ChapterManagerToolbar from './chapterManager/ChapterManagerToolbar';
import { formatChapterLocation } from './chapterManager/pathUtils';
import type { ChapterEditDraft, ChapterGroup, ChapterTab, EditableChapter } from './chapterManager/types';

interface Props {
  book: Book;
  bookId: string;
  initialChapters: Chapter[];
  onClose: () => void;
  onSave: () => void;
}

const groupSize = 100;

const ChapterManagerModal: React.FC<Props> = ({ book, bookId, initialChapters, onClose, onSave }) => {
  const [chapters, setChapters] = useState<EditableChapter[]>(() => sortChapters(initialChapters));
  const [search, setSearch] = useState('');
  const [activeTab, setActiveTab] = useState<ChapterTab>('main');
  const [groupIndex, setGroupIndex] = useState(0);
  const [saving, setSaving] = useState(false);
  const [changedIds, setChangedIds] = useState<Set<string>>(new Set());
  const [selectionMode, setSelectionMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [showBookSelector, setShowBookSelector] = useState(false);
  const [moving, setMoving] = useState(false);
  const [pathLibrary, setPathLibrary] = useState<Library | null>(null);
  const [editingChapter, setEditingChapter] = useState<EditableChapter | null>(null);

  useEffect(() => {
    setChapters(sortChapters(initialChapters));
    setChangedIds(new Set());
    setSelectedIds(new Set());
    setSelectionMode(false);
    setGroupIndex(0);
    setSearch('');
  }, [initialChapters]);

  useEffect(() => {
    let cancelled = false;

    const loadPathContext = async () => {
      try {
        const librariesRes = await apiClient.get('/api/libraries');
        if (cancelled) return;

        const libraries = librariesRes.data as Library[];
        setPathLibrary(libraries.find((library) => library.id === book.libraryId) || null);
      } catch (err) {
        console.error('Failed to load chapter path context', err);
      }
    };

    loadPathContext();

    return () => {
      cancelled = true;
    };
  }, [book.libraryId]);

  const mainCount = useMemo(() => chapters.filter((chapter) => !chapter.isExtra).length, [chapters]);
  const extraCount = useMemo(() => chapters.filter((chapter) => Boolean(chapter.isExtra)).length, [chapters]);

  useEffect(() => {
    if (activeTab === 'extra' && extraCount === 0) {
      setActiveTab('main');
      setGroupIndex(0);
    } else if (activeTab === 'main' && mainCount === 0 && extraCount > 0) {
      setActiveTab('extra');
      setGroupIndex(0);
    }
  }, [activeTab, extraCount, mainCount]);

  const tabChapters = useMemo(
    () => chapters.filter((chapter) => (activeTab === 'extra' ? Boolean(chapter.isExtra) : !chapter.isExtra)),
    [activeTab, chapters],
  );

  const filteredChapters = useMemo(() => {
    const query = search.trim().toLowerCase();
    if (!query) return tabChapters;

    return tabChapters.filter((chapter) => {
      const location = formatChapterLocation(chapter, book, pathLibrary).toLowerCase();
      return (
        chapter.title.toLowerCase().includes(query) ||
        String(chapter.chapterIndex).includes(query) ||
        location.includes(query)
      );
    });
  }, [book, pathLibrary, search, tabChapters]);

  const groups = useMemo<ChapterGroup[]>(() => {
    const count = Math.ceil(filteredChapters.length / groupSize);
    return Array.from({ length: count }, (_, index) => ({
      index,
      start: index * groupSize + 1,
      end: Math.min((index + 1) * groupSize, filteredChapters.length),
    }));
  }, [filteredChapters.length]);

  useEffect(() => {
    setGroupIndex((current) => Math.min(current, Math.max(groups.length - 1, 0)));
  }, [groups.length]);

  const visibleChapters = useMemo(() => {
    if (groups.length <= 1) return filteredChapters;
    const start = groupIndex * groupSize;
    return filteredChapters.slice(start, start + groupSize);
  }, [filteredChapters, groupIndex, groups.length]);

  const allFilteredSelected =
    filteredChapters.length > 0 && filteredChapters.every((chapter) => selectedIds.has(chapter.id));

  const requestClose = useCallback(() => {
    if (saving || moving) return;
    if (changedIds.size > 0 && !window.confirm('有未保存更改，确定关闭吗？')) return;
    onClose();
  }, [changedIds.size, moving, onClose, saving]);

  const handleSearchChange = (value: string) => {
    setSearch(value);
    setGroupIndex(0);
  };

  const handleTabChange = (tab: ChapterTab) => {
    setActiveTab(tab);
    setSearch('');
    setGroupIndex(0);
    setSelectedIds(new Set());
    setSelectionMode(false);
  };

  const toggleSelectionMode = () => {
    setSelectionMode((current) => {
      if (current) setSelectedIds(new Set());
      return !current;
    });
  };

  const toggleSelection = (id: string) => {
    if (!selectionMode) return;
    setSelectedIds((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const toggleAll = () => {
    if (!selectionMode) return;
    setSelectedIds((current) => {
      if (filteredChapters.length > 0 && filteredChapters.every((chapter) => current.has(chapter.id))) {
        const next = new Set(current);
        filteredChapters.forEach((chapter) => next.delete(chapter.id));
        return next;
      }
      const next = new Set(current);
      filteredChapters.forEach((chapter) => next.add(chapter.id));
      return next;
    });
  };

  const applyChapterEdit = (chapterId: string, draft: ChapterEditDraft) => {
    setChapters((current) =>
      current.map((chapter) =>
        chapter.id === chapterId
          ? {
              ...chapter,
              title: draft.title,
              chapterIndex: draft.chapterIndex,
              isExtra: draft.isExtra ? 1 : 0,
            }
          : chapter,
      ),
    );
    setChangedIds((current) => new Set(current).add(chapterId));
    setEditingChapter(null);
  };

  const handleRenumber = () => {
    if (!window.confirm('确定要按当前列表顺序重新生成章节序号（从 1 开始）吗？')) return;
    setChapters((current) => current.map((chapter, index) => ({ ...chapter, chapterIndex: index + 1 })));
    setChangedIds(new Set(chapters.map((chapter) => chapter.id)));
  };

  const handleJump = () => {
    const value = window.prompt('输入章节序号');
    const target = Number.parseInt((value || '').trim(), 10);
    if (!Number.isFinite(target) || tabChapters.length === 0) return;

    let targetIndex = tabChapters.findIndex((chapter) => chapter.chapterIndex >= target);
    if (targetIndex < 0) targetIndex = tabChapters.length - 1;
    setSearch('');
    setGroupIndex(Math.floor(targetIndex / groupSize));
  };

  const handleSave = async () => {
    try {
      setSaving(true);
      const updates = chapters
        .filter((chapter) => changedIds.has(chapter.id))
        .map((chapter) => ({
          id: chapter.id,
          title: chapter.title,
          chapter_index: chapter.chapterIndex,
          is_extra: chapter.isExtra ? 1 : 0,
        }));

      if (updates.length > 0) {
        await apiClient.put(`/api/books/${bookId}/chapters/batch`, { updates });
      }

      onSave();
      onClose();
    } catch (err) {
      console.error('保存章节失败', err);
      alert('保存失败');
    } finally {
      setSaving(false);
    }
  };

  const handleMoveChapters = async (targetBook: Book) => {
    try {
      setMoving(true);
      await apiClient.post('/api/books/chapters/move', {
        target_book_id: targetBook.id,
        chapter_ids: Array.from(selectedIds),
      });
      setShowBookSelector(false);
      onSave();
      onClose();
    } catch (err) {
      console.error('移动章节失败', err);
      alert('移动章节失败');
    } finally {
      setMoving(false);
    }
  };

  const editingLocation = editingChapter ? formatChapterLocation(editingChapter, book, pathLibrary) : '';

  return (
    <div className="fixed inset-0 z-[200] flex items-center justify-center p-0 sm:p-4">
      <div className="absolute inset-0 bg-slate-900/60 backdrop-blur-sm" onClick={requestClose} />
      <div
        className="absolute inset-x-2 bottom-2 top-2 flex min-w-0 animate-in flex-col overflow-hidden rounded-3xl bg-white shadow-2xl duration-200 zoom-in-95 dark:bg-slate-900 sm:relative sm:inset-auto sm:h-[90vh] sm:w-full sm:max-w-5xl"
      >
        <ChapterManagerHeader
          search={search}
          activeTab={activeTab}
          mainCount={mainCount}
          extraCount={extraCount}
          groups={groups}
          currentGroupIndex={groupIndex}
          onSearchChange={handleSearchChange}
          onTabChange={handleTabChange}
          onGroupChange={setGroupIndex}
          onClose={requestClose}
        />

        <ChapterManagerToolbar
          selectionMode={selectionMode}
          allSelected={allFilteredSelected}
          totalCount={filteredChapters.length}
          selectedCount={selectedIds.size}
          moving={moving}
          onToggleSelectionMode={toggleSelectionMode}
          onToggleAll={toggleAll}
          onRenumber={handleRenumber}
          onJump={handleJump}
          onMove={() => setShowBookSelector(true)}
        />

        <div className="min-h-0 flex-1 bg-slate-50/50 py-2 dark:bg-slate-950/20">
          <ChapterManagerList
            book={book}
            chapters={visibleChapters}
            selectedIds={selectedIds}
            changedIds={changedIds}
            selectionMode={selectionMode}
            pathLibrary={pathLibrary}
            onToggleSelection={toggleSelection}
            onEdit={setEditingChapter}
          />
        </div>

        <div className="flex min-w-0 shrink-0 items-center gap-3 border-t border-slate-100 bg-white px-4 py-3 dark:border-slate-800 dark:bg-slate-900 sm:px-6 sm:py-4">
          <button
            type="button"
            onClick={requestClose}
            className="rounded-xl px-4 py-2 text-sm font-semibold text-slate-500 transition-colors hover:bg-slate-100 dark:hover:bg-slate-800"
          >
            取消
          </button>
          <button
            type="button"
            onClick={handleSave}
            disabled={saving || changedIds.size === 0}
            className="ml-auto inline-flex min-h-10 min-w-0 flex-1 items-center justify-center gap-2 truncate rounded-xl bg-primary-600 px-4 py-2 text-sm font-semibold text-white shadow-sm transition-colors hover:bg-primary-700 disabled:cursor-not-allowed disabled:bg-slate-100 disabled:text-slate-500 dark:disabled:bg-slate-800 sm:flex-none sm:px-5 sm:min-w-[210px]"
          >
            {saving ? <Loader2 className="animate-spin" size={18} /> : <Save size={18} />}
            保存更改 ({changedIds.size})
          </button>
        </div>

        {showBookSelector && (
          <BookSelector
            excludeIds={[bookId]}
            onClose={() => setShowBookSelector(false)}
            onSelect={handleMoveChapters}
          />
        )}

        {editingChapter && (
          <ChapterEditDialog
            key={editingChapter.id}
            chapter={editingChapter}
            location={editingLocation}
            onClose={() => setEditingChapter(null)}
            onSave={applyChapterEdit}
          />
        )}
      </div>
    </div>
  );
};

const sortChapters = (chapters: Chapter[]): EditableChapter[] => {
  return [...chapters].sort((a, b) => a.chapterIndex - b.chapterIndex);
};

export default ChapterManagerModal;
