import React from 'react';
import { Check, Folder, Pencil, SearchX } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { Book, Library } from '../../../core/types';
import AutoSizer from '../../widgets/AutoSizer';
import FixedSizeList from '../../widgets/VirtualList';
import { formatChapterLocation } from './pathUtils';
import type { EditableChapter } from './types';

interface Props {
  book: Book;
  chapters: EditableChapter[];
  selectedIds: Set<string>;
  changedIds: Set<string>;
  selectionMode: boolean;
  pathLibrary: Library | null;
  onToggleSelection: (id: string) => void;
  onEdit: (chapter: EditableChapter) => void;
}

const ChapterManagerList: React.FC<Props> = ({
  book,
  chapters,
  selectedIds,
  changedIds,
  selectionMode,
  pathLibrary,
  onToggleSelection,
  onEdit,
}) => {
  const { t } = useTranslation();
  if (chapters.length === 0) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 px-6 text-center text-slate-400">
        <SearchX size={42} className="text-slate-300 dark:text-slate-600" />
        <div>
          <p className="text-base font-semibold text-slate-600 dark:text-slate-300">{t('chapterManager.noMatchedChapters')}</p>
          <p className="mt-1 text-sm">{t('chapterManager.noMatchedChaptersHint')}</p>
        </div>
      </div>
    );
  }

  return (
    <AutoSizer>
      {({ height, width }) => {
        const compact = width < 640;
        const itemSize = compact ? 64 : 74;

        return (
          <FixedSizeList height={height} width={width} itemCount={chapters.length} itemSize={itemSize}>
            {({ index, style }) => {
              const chapter = chapters[index];
              return (
                <ChapterRow
                  key={chapter.id}
                  style={style}
                  book={book}
                  chapter={chapter}
                  selected={selectedIds.has(chapter.id)}
                  changed={changedIds.has(chapter.id)}
                  selectionMode={selectionMode}
                  pathLibrary={pathLibrary}
                  onToggleSelection={onToggleSelection}
                  onEdit={onEdit}
                />
              );
            }}
          </FixedSizeList>
        );
      }}
    </AutoSizer>
  );
};

interface ChapterRowProps {
  style: React.CSSProperties;
  book: Book;
  chapter: EditableChapter;
  selected: boolean;
  changed: boolean;
  selectionMode: boolean;
  pathLibrary: Library | null;
  onToggleSelection: (id: string) => void;
  onEdit: (chapter: EditableChapter) => void;
}

const ChapterRow: React.FC<ChapterRowProps> = ({
  style,
  book,
  chapter,
  selected,
  changed,
  selectionMode,
  pathLibrary,
  onToggleSelection,
  onEdit,
}) => {
  const { t } = useTranslation();
  const location = formatChapterLocation(chapter, book, pathLibrary, t('chapterManager.unknownLibrary'));

  const handleRowClick = () => {
    if (selectionMode) {
      onToggleSelection(chapter.id);
    } else {
      onEdit(chapter);
    }
  };

  return (
    <div style={style} className="px-3 py-1 sm:px-5">
      <button
        type="button"
        onClick={handleRowClick}
        className={`flex h-full w-full items-center gap-2 rounded-2xl border px-3 text-left transition-colors sm:gap-3 sm:px-4 ${
          selected
            ? 'border-primary-200 bg-primary-50 dark:border-primary-800 dark:bg-primary-900/20'
            : changed
              ? 'border-amber-200 bg-amber-50/80 dark:border-amber-800 dark:bg-amber-900/10'
              : 'border-slate-200 bg-white hover:border-primary-200 dark:border-slate-800 dark:bg-slate-900/70 dark:hover:border-primary-900'
        }`}
      >
        {selectionMode && (
          <span
            className={`flex h-7 w-7 shrink-0 items-center justify-center rounded-lg border-2 transition-colors sm:h-8 sm:w-8 ${
              selected
                ? 'border-primary-600 bg-primary-600 text-white'
                : 'border-slate-300 bg-white text-transparent dark:border-slate-600 dark:bg-slate-800'
            }`}
          >
            <Check size={18} strokeWidth={3} />
          </span>
        )}

        <span className="flex min-w-9 shrink-0 items-center justify-center rounded-xl bg-primary-50 px-2 py-2 text-xs font-semibold leading-none text-primary-600 dark:bg-slate-800 sm:min-w-11">
          #{chapter.chapter_index}
        </span>

        <span className="min-w-0 flex-1">
          <span className="block truncate text-sm font-normal leading-snug text-slate-950 dark:text-white sm:text-[15px]">
            {chapter.title}
          </span>
          <span className="mt-1 hidden min-w-0 items-center gap-1.5 text-xs font-normal text-slate-400 lg:flex">
            <Folder size={13} className="shrink-0" />
            <span className="truncate">{location}</span>
          </span>
        </span>

        {changed && (
          <span className="hidden shrink-0 rounded-lg bg-cyan-50 px-2 py-1 text-xs font-semibold leading-none text-primary-600 dark:bg-primary-900/20 sm:inline-flex">
            {t('chapterManager.changed')}
          </span>
        )}

        {!selectionMode && (
          <span
            className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-slate-500 transition-colors hover:bg-slate-100 hover:text-primary-600 dark:text-slate-400 dark:hover:bg-slate-800"
            title={t('chapterManager.editChapter')}
            onClick={(event) => {
              event.stopPropagation();
              onEdit(chapter);
            }}
          >
            <Pencil size={17} />
          </span>
        )}
      </button>
    </div>
  );
};

export default ChapterManagerList;
