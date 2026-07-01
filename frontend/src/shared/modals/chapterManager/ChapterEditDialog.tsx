import React, { useState } from 'react';
import { FileText, Folder, Hash, Save, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { ChapterEditDraft, EditableChapter } from './types';

interface Props {
  chapter: EditableChapter;
  location: string;
  onClose: () => void;
  onSave: (chapterId: string, draft: ChapterEditDraft) => void;
}

const ChapterEditDialog: React.FC<Props> = ({ chapter, location, onClose, onSave }) => {
  const { t } = useTranslation();
  const [title, setTitle] = useState(chapter.title);
  const [chapterIndex, setChapterIndex] = useState(String(chapter.chapter_index));
  const [isExtra, setIsExtra] = useState(Boolean(chapter.is_extra));

  const parsedIndex = Number.parseInt(chapterIndex, 10);
  const canSave = title.trim().length > 0 && Number.isFinite(parsedIndex) && parsedIndex > 0;

  const handleSave = () => {
    if (!canSave) return;
    onSave(chapter.id, {
      title: title.trim(),
      chapterIndex: parsedIndex,
      isExtra,
    });
  };

  return (
    <div className="fixed inset-0 z-[260] flex items-end justify-center bg-slate-950/45 p-0 sm:items-center sm:p-4">
      <div className="w-full max-w-xl rounded-t-3xl bg-white shadow-2xl dark:bg-slate-900 sm:rounded-3xl">
        <div className="flex items-center justify-between border-b border-slate-100 px-5 py-4 dark:border-slate-800">
          <div>
            <h3 className="text-lg font-bold text-slate-950 dark:text-white">{t('chapterManager.editChapter')}</h3>
            <p className="mt-1 text-sm text-slate-500">{t('chapterManager.editHint')}</p>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="rounded-full p-2 text-slate-500 hover:bg-slate-100 hover:text-slate-700 dark:hover:bg-slate-800 dark:hover:text-slate-200"
            title={t('common.close')}
          >
            <X size={22} />
          </button>
        </div>

        <div className="space-y-4 px-5 py-5">
          <label className="block">
            <span className="mb-2 flex items-center gap-2 text-sm font-semibold text-slate-600 dark:text-slate-300">
              <FileText size={16} />
              {t('chapterManager.chapterTitle')}
            </span>
            <input
              value={title}
              onChange={(event) => setTitle(event.target.value)}
              className="h-12 w-full rounded-2xl border border-slate-200 bg-white px-4 text-base font-normal text-slate-950 outline-none transition-colors focus:border-primary-400 focus:ring-2 focus:ring-primary-100 dark:border-slate-700 dark:bg-slate-800 dark:text-white dark:focus:ring-primary-900/30"
              autoFocus
            />
          </label>

          <div className="grid gap-4 sm:grid-cols-[160px_1fr]">
            <label className="block">
              <span className="mb-2 flex items-center gap-2 text-sm font-semibold text-slate-600 dark:text-slate-300">
                <Hash size={16} />
                {t('chapterManager.chapterIndex')}
              </span>
              <input
                type="number"
                min={1}
                value={chapterIndex}
                onChange={(event) => setChapterIndex(event.target.value)}
                className="h-12 w-full rounded-2xl border border-slate-200 bg-white px-4 text-base font-normal text-slate-950 outline-none transition-colors focus:border-primary-400 focus:ring-2 focus:ring-primary-100 dark:border-slate-700 dark:bg-slate-800 dark:text-white dark:focus:ring-primary-900/30"
              />
            </label>

            <div>
              <span className="mb-2 block text-sm font-semibold text-slate-600 dark:text-slate-300">
                {t('chapterManager.category')}
              </span>
              <div className="inline-flex rounded-2xl border border-slate-200 bg-slate-100 p-1 dark:border-slate-700 dark:bg-slate-800">
                <button
                  type="button"
                  onClick={() => setIsExtra(false)}
                  className={`rounded-xl px-4 py-2 text-sm font-semibold transition-colors ${
                    !isExtra
                      ? 'bg-primary-600 text-white shadow-sm'
                      : 'text-slate-500 hover:text-slate-800 dark:text-slate-300'
                  }`}
                >
                  {t('chapterManager.main')}
                </button>
                <button
                  type="button"
                  onClick={() => setIsExtra(true)}
                  className={`rounded-xl px-4 py-2 text-sm font-semibold transition-colors ${
                    isExtra
                      ? 'bg-primary-600 text-white shadow-sm'
                      : 'text-slate-500 hover:text-slate-800 dark:text-slate-300'
                  }`}
                >
                  {t('chapterManager.extra')}
                </button>
              </div>
            </div>
          </div>

          <div className="rounded-2xl border border-slate-200 bg-slate-50 p-4 dark:border-slate-700 dark:bg-slate-800/60">
            <div className="mb-2 flex items-center gap-2 text-sm font-semibold text-slate-600 dark:text-slate-300">
              <Folder size={16} />
              {t('chapterManager.storageLocation')}
            </div>
            <p className="break-all text-sm font-normal leading-6 text-slate-500 dark:text-slate-400">
              {location}
            </p>
          </div>
        </div>

        <div className="flex items-center justify-end gap-3 border-t border-slate-100 px-5 py-4 dark:border-slate-800">
          <button
            type="button"
            onClick={onClose}
            className="rounded-xl px-4 py-2 text-sm font-semibold text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800"
          >
            {t('common.cancel')}
          </button>
          <button
            type="button"
            onClick={handleSave}
            disabled={!canSave}
            className="inline-flex items-center gap-2 rounded-xl bg-primary-600 px-5 py-2 text-sm font-semibold text-white shadow-sm transition-colors hover:bg-primary-700 disabled:cursor-not-allowed disabled:bg-slate-200 disabled:text-slate-500 dark:disabled:bg-slate-800"
          >
            <Save size={16} />
            {t('chapterManager.applyChanges')}
          </button>
        </div>
      </div>
    </div>
  );
};

export default ChapterEditDialog;
