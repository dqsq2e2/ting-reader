import React from 'react';
import { FileSignature, Save, Trash2, Wand2, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { Book } from '../../../core/types';

interface RegexResult {
  regex?: string;
  captured_index?: string;
  captured_title?: string;
}

interface Props {
  editData: Partial<Book>;
  showRegexGenerator: boolean;
  genFilename: string;
  genNum: string;
  genTitle: string;
  genResult: RegexResult | null;
  chapterGroupOrder: 'asc' | 'desc';
  onChangeEditData: (next: Partial<Book>) => void;
  onChangeChapterGroupOrder: (order: 'asc' | 'desc') => void;
  onShowRegexGenerator: () => void;
  onHideRegexGenerator: () => void;
  onChangeGenFilename: (v: string) => void;
  onChangeGenNum: (v: string) => void;
  onChangeGenTitle: (v: string) => void;
  onGenerateRegex: () => void;
  onApplyRegex: () => void;
  onClose: () => void;
  onDelete: () => void;
  onSave: () => void;
  onWriteMetadata: () => void;
}

const EditBookModal: React.FC<Props> = ({
  editData,
  showRegexGenerator,
  genFilename,
  genNum,
  genTitle,
  genResult,
  chapterGroupOrder,
  onChangeEditData,
  onChangeChapterGroupOrder,
  onShowRegexGenerator,
  onHideRegexGenerator,
  onChangeGenFilename,
  onChangeGenNum,
  onChangeGenTitle,
  onGenerateRegex,
  onApplyRegex,
  onClose,
  onDelete,
  onSave,
  onWriteMetadata,
}) => {
  const { t } = useTranslation();
  const [locationExpanded, setLocationExpanded] = React.useState(false);
  const update = (patch: Partial<Book>) => onChangeEditData({ ...editData, ...patch });
  const bookLocation = editData.path || '';

  return (
    <div className="fixed inset-0 z-[200] flex items-center justify-center p-4">
      <div className="absolute inset-0 bg-slate-900/60 backdrop-blur-sm" onClick={onClose}></div>
      <div className="relative w-full max-w-2xl max-h-[90vh] bg-white dark:bg-slate-900 rounded-3xl shadow-2xl overflow-y-auto animate-in zoom-in-95 duration-200 no-scrollbar">
        {showRegexGenerator ? (
            <div className="p-4 sm:p-6 md:p-8">
                <div className="flex items-center justify-between mb-6">
                    <h2 className="text-xl font-bold dark:text-white flex items-center gap-2">
                        <Wand2 className="text-primary-600" /> {t('bookshelf.regexGenerator')}
                    </h2>
                    <button onClick={onHideRegexGenerator}><X size={24} className="text-slate-400" /></button>
                </div>

                <div className="space-y-4">
                    <div className="space-y-1">
                        <label className="text-xs font-bold text-slate-500">{t('bookshelf.sampleFilename')}</label>
                        <input
                            type="text"
                            value={genFilename}
                            onChange={e => onChangeGenFilename(e.target.value)}
                            placeholder={t('bookshelf.sampleFilenamePlaceholder')}
                            className="w-full px-4 py-2 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                        />
                    </div>
                    <div className="grid grid-cols-2 gap-4">
                        <div className="space-y-1">
                            <label className="text-xs font-bold text-slate-500">{t('bookshelf.extractChapterNumber')}</label>
                            <input
                                type="text"
                                value={genNum}
                                onChange={e => onChangeGenNum(e.target.value)}
                                placeholder={t('bookshelf.exampleOne')}
                                className="w-full px-4 py-2 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                            />
                        </div>
                        <div className="space-y-1">
                            <label className="text-xs font-bold text-slate-500">{t('bookshelf.extractChapterTitle')}</label>
                            <input
                                type="text"
                                value={genTitle}
                                onChange={e => onChangeGenTitle(e.target.value)}
                                placeholder={t('bookshelf.exampleChapterTitle')}
                                className="w-full px-4 py-2 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                            />
                        </div>
                    </div>

                    <button
                        onClick={onGenerateRegex}
                        disabled={!genFilename || !genNum || !genTitle}
                        className="w-full py-3 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-xl shadow-lg shadow-primary-500/30 transition-all disabled:opacity-50"
                    >
                        {t('bookshelf.generateRule')}
                    </button>

                    {genResult && (
                        <div className="mt-6 p-4 bg-slate-50 dark:bg-slate-800/50 rounded-xl border border-slate-200 dark:border-slate-700 space-y-3">
                            <div>
                                <div className="text-xs font-bold text-slate-500 mb-1">{t('bookshelf.generatedRegex')}</div>
                                <code className="block p-2 bg-white dark:bg-slate-900 rounded-lg border border-slate-200 dark:border-slate-800 font-mono text-sm text-primary-600 break-all">
                                    {genResult.regex}
                                </code>
                            </div>

                            <div className="grid grid-cols-2 gap-4 text-sm">
                                <div>
                                    <span className="text-slate-500 text-xs">{t('bookshelf.extractedIndex')}</span>
                                    <div className={genResult.captured_index === genNum ? "text-green-600 font-bold" : "text-red-500"}>
                                        {genResult.captured_index || t('bookshelf.noMatch')}
                                    </div>
                                </div>
                                <div>
                                    <span className="text-slate-500 text-xs">{t('bookshelf.extractedTitle')}</span>
                                    <div className={genResult.captured_title === genTitle ? "text-green-600 font-bold" : "text-red-500"}>
                                        {genResult.captured_title || t('bookshelf.noMatch')}
                                    </div>
                                </div>
                            </div>

                            <button
                                onClick={onApplyRegex}
                                className="w-full py-2 border-2 border-primary-600 text-primary-600 hover:bg-primary-50 dark:hover:bg-primary-900/20 font-bold rounded-xl transition-all"
                            >
                                {t('bookshelf.useThisRule')}
                            </button>
                        </div>
                    )}
                </div>
            </div>
        ) : (
        <div className="p-4 sm:p-6 md:p-8">
          <div className="flex items-center justify-between mb-4 sm:mb-6">
            <h2 className="text-xl sm:text-2xl font-bold dark:text-white">{t('bookshelf.editBookMetadata')}</h2>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-4 sm:gap-6">
            <div className="space-y-3 sm:space-y-4">
              <div className="space-y-1">
                <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">{t('bookshelf.titleField')}</label>
                <input
                  type="text"
                  value={editData.title || ''}
                  onChange={e => update({ title: e.target.value })}
                  className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                />
              </div>
              <div className="space-y-1">
                <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">{t('bookshelf.authorField')}</label>
                <input
                  type="text"
                  value={editData.author || ''}
                  onChange={e => update({ author: e.target.value })}
                  className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                />
              </div>
              <div className="space-y-1">
                <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">{t('bookshelf.narratorField')}</label>
                <input
                  type="text"
                  value={editData.narrator || ''}
                  onChange={e => update({ narrator: e.target.value })}
                  className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                />
              </div>
              <div className="space-y-1">
                <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">{t('bookshelf.tagsCommaSeparated')}</label>
                <input
                  type="text"
                  value={editData.tags || ''}
                  onChange={e => update({ tags: e.target.value })}
                  className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                />
              </div>

              <div className="space-y-1">
                <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">{t('bookshelf.genreField')}</label>
                <input
                  type="text"
                  value={editData.genre || ''}
                  onChange={e => update({ genre: e.target.value })}
                  className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                />
              </div>

              <div className="space-y-1">
                <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">{t('bookshelf.yearField')}</label>
                <input
                  type="number"
                  value={editData.year || ''}
                  onChange={e => update({ year: e.target.value ? parseInt(e.target.value) : undefined })}
                  placeholder={t('bookshelf.exampleYear')}
                  className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                />
              </div>

              <div className="space-y-1">
                <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider flex justify-between items-center">
                    <span>{t('bookshelf.chapterRegexRule')}</span>
                    <button
                        onClick={onShowRegexGenerator}
                        className="text-primary-600 hover:text-primary-700 flex items-center gap-1 whitespace-nowrap text-xs sm:text-sm"
                    >
                        <Wand2 size={12} /> {t('bookshelf.autoGenerate')}
                    </button>
                </label>
                <input
                  type="text"
                  value={editData.chapter_regex || ''}
                  onChange={e => update({ chapter_regex: e.target.value })}
                  placeholder="^...(\d+)...(.+)$"
                  className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base font-mono bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                />
                <p className="text-[10px] text-slate-400">{t('bookshelf.chapterRegexHelp')}</p>
              </div>
            </div>

            <div className="space-y-3 sm:space-y-4">
              <div className="space-y-1">
                <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">{t('bookshelf.coverUrl')}</label>
                <input
                  type="text"
                  value={editData.cover_url || ''}
                  onChange={e => update({ cover_url: e.target.value })}
                  className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                />
              </div>
              <div className="space-y-1">
                <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">{t('bookshelf.bookLocation')}</label>
                <button
                  type="button"
                  title={bookLocation}
                  onClick={() => setLocationExpanded(value => !value)}
                  className={`w-full px-3 py-2 sm:px-4 sm:py-2.5 text-left text-xs sm:text-sm font-mono bg-slate-100 dark:bg-slate-800/70 border border-slate-200 dark:border-slate-700 rounded-xl outline-none text-slate-600 dark:text-slate-300 cursor-pointer ${
                    locationExpanded ? 'whitespace-pre-wrap break-all' : 'truncate'
                  }`}
                >
                  {bookLocation}
                </button>
              </div>
              <div className="grid grid-cols-2 gap-3 sm:gap-4">
                <div className="space-y-1">
                  <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">{t('bookshelf.skipIntroSeconds')}</label>
                  <input
                    type="number"
                    value={editData.skip_intro || 0}
                    onChange={e => update({ skip_intro: parseInt(e.target.value) || 0 })}
                    className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                  />
                </div>
                <div className="space-y-1">
                  <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">{t('bookshelf.skipOutroSeconds')}</label>
                  <input
                    type="number"
                    value={editData.skip_outro || 0}
                    onChange={e => update({ skip_outro: parseInt(e.target.value) || 0 })}
                    className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                  />
                </div>
              </div>

              <div className="space-y-1">
                <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">{t('bookshelf.groupDisplayOrder')}</label>
                <div className="grid grid-cols-2 gap-2 rounded-xl bg-slate-100 dark:bg-slate-800 p-1">
                  <button
                    type="button"
                    onClick={() => onChangeChapterGroupOrder('asc')}
                    className={`px-3 py-2 text-xs sm:text-sm font-bold rounded-lg transition-all ${
                      chapterGroupOrder === 'asc'
                        ? 'bg-white dark:bg-slate-700 text-primary-600 shadow-sm'
                        : 'text-slate-500 hover:text-slate-700 dark:hover:text-slate-300'
                    }`}
                  >
                    {t('bookshelf.frontToBack')}
                  </button>
                  <button
                    type="button"
                    onClick={() => onChangeChapterGroupOrder('desc')}
                    className={`px-3 py-2 text-xs sm:text-sm font-bold rounded-lg transition-all ${
                      chapterGroupOrder === 'desc'
                        ? 'bg-white dark:bg-slate-700 text-primary-600 shadow-sm'
                        : 'text-slate-500 hover:text-slate-700 dark:hover:text-slate-300'
                    }`}
                  >
                    {t('bookshelf.backToFront')}
                  </button>
                </div>
              </div>
            </div>
          </div>

          <div className="mt-4 sm:mt-6 space-y-1">
            <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">{t('bookshelf.descriptionField')}</label>
            <textarea
              rows={4}
              value={editData.description || ''}
              onChange={e => update({ description: e.target.value })}
              className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white resize-none"
            />
          </div>

          <div className="flex flex-col-reverse sm:flex-row gap-3 sm:gap-4 mt-6 sm:mt-8">
            <button
              onClick={onDelete}
              className="px-4 py-2.5 sm:py-3 font-bold text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-xl transition-all flex items-center justify-center gap-2 sm:justify-start whitespace-nowrap text-xs sm:text-base"
            >
              <Trash2 size={18} className="sm:w-5 sm:h-5" />
              {t('bookshelf.deleteBook')}
            </button>
            <div className="flex-1" />
            <div className="flex gap-2 sm:gap-3">
              <button
                onClick={onWriteMetadata}
                className="flex-1 sm:flex-none px-2.5 sm:px-6 py-2.5 sm:py-3 font-bold text-primary-600 bg-primary-50 hover:bg-primary-100 dark:bg-primary-900/20 dark:hover:bg-primary-900/30 rounded-xl transition-all flex items-center justify-center gap-1.5 sm:gap-2 text-xs sm:text-base whitespace-nowrap"
                title={t('bookshelf.writeMetadataTitle')}
              >
                <FileSignature size={16} className="sm:w-5 sm:h-5" />
                {t('bookshelf.writeFile')}
              </button>
              <button
                onClick={onClose}
                className="flex-1 sm:flex-none px-3 sm:px-6 py-2.5 sm:py-3 font-bold text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-xl transition-all text-xs sm:text-base whitespace-nowrap"
              >
                {t('common.cancel')}
              </button>
              <button
                onClick={onSave}
                className="flex-1 sm:flex-none px-3 sm:px-8 py-2.5 sm:py-3 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-xl shadow-lg shadow-primary-500/30 flex items-center justify-center gap-1.5 sm:gap-2 transition-all text-xs sm:text-base whitespace-nowrap"
              >
                <Save size={16} className="sm:w-5 sm:h-5" />
                <span>{t('common.save')}</span>
              </button>
            </div>
          </div>
        </div>
        )}
      </div>
    </div>
  );
};

export default EditBookModal;
