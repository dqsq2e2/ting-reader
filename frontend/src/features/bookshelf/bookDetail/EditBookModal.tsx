import React from 'react';
import { FileSignature, Save, Trash2, Wand2, X } from 'lucide-react';
import type { Book } from '../../../core/types';

interface RegexResult {
  regex?: string;
  capturedIndex?: string;
  capturedTitle?: string;
}

interface Props {
  editData: Partial<Book>;
  showRegexGenerator: boolean;
  genFilename: string;
  genNum: string;
  genTitle: string;
  genResult: RegexResult | null;
  onChangeEditData: (next: Partial<Book>) => void;
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
  onChangeEditData,
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
  const update = (patch: Partial<Book>) => onChangeEditData({ ...editData, ...patch });

  return (
    <div className="fixed inset-0 z-[200] flex items-center justify-center p-4">
      <div className="absolute inset-0 bg-slate-900/60 backdrop-blur-sm" onClick={onClose}></div>
      <div className="relative w-full max-w-2xl max-h-[90vh] bg-white dark:bg-slate-900 rounded-3xl shadow-2xl overflow-y-auto animate-in zoom-in-95 duration-200 no-scrollbar">
        {showRegexGenerator ? (
            <div className="p-4 sm:p-6 md:p-8">
                <div className="flex items-center justify-between mb-6">
                    <h2 className="text-xl font-bold dark:text-white flex items-center gap-2">
                        <Wand2 className="text-primary-600" /> 正则生成器
                    </h2>
                    <button onClick={onHideRegexGenerator}><X size={24} className="text-slate-400" /></button>
                </div>

                <div className="space-y-4">
                    <div className="space-y-1">
                        <label className="text-xs font-bold text-slate-500">示例文件名 (不含后缀)</label>
                        <input
                            type="text"
                            value={genFilename}
                            onChange={e => onChangeGenFilename(e.target.value)}
                            placeholder="例如：书名 第1集 章节名"
                            className="w-full px-4 py-2 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                        />
                    </div>
                    <div className="grid grid-cols-2 gap-4">
                        <div className="space-y-1">
                            <label className="text-xs font-bold text-slate-500">提取章节号</label>
                            <input
                                type="text"
                                value={genNum}
                                onChange={e => onChangeGenNum(e.target.value)}
                                placeholder="例如：1"
                                className="w-full px-4 py-2 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                            />
                        </div>
                        <div className="space-y-1">
                            <label className="text-xs font-bold text-slate-500">提取章节名</label>
                            <input
                                type="text"
                                value={genTitle}
                                onChange={e => onChangeGenTitle(e.target.value)}
                                placeholder="例如：章节名"
                                className="w-full px-4 py-2 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                            />
                        </div>
                    </div>

                    <button
                        onClick={onGenerateRegex}
                        disabled={!genFilename || !genNum || !genTitle}
                        className="w-full py-3 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-xl shadow-lg shadow-primary-500/30 transition-all disabled:opacity-50"
                    >
                        生成规则
                    </button>

                    {genResult && (
                        <div className="mt-6 p-4 bg-slate-50 dark:bg-slate-800/50 rounded-xl border border-slate-200 dark:border-slate-700 space-y-3">
                            <div>
                                <div className="text-xs font-bold text-slate-500 mb-1">生成正则</div>
                                <code className="block p-2 bg-white dark:bg-slate-900 rounded-lg border border-slate-200 dark:border-slate-800 font-mono text-sm text-primary-600 break-all">
                                    {genResult.regex}
                                </code>
                            </div>

                            <div className="grid grid-cols-2 gap-4 text-sm">
                                <div>
                                    <span className="text-slate-500 text-xs">提取序号:</span>
                                    <div className={genResult.capturedIndex === genNum ? "text-green-600 font-bold" : "text-red-500"}>
                                        {genResult.capturedIndex || "未匹配"}
                                    </div>
                                </div>
                                <div>
                                    <span className="text-slate-500 text-xs">提取标题:</span>
                                    <div className={genResult.capturedTitle === genTitle ? "text-green-600 font-bold" : "text-red-500"}>
                                        {genResult.capturedTitle || "未匹配"}
                                    </div>
                                </div>
                            </div>

                            <button
                                onClick={onApplyRegex}
                                className="w-full py-2 border-2 border-primary-600 text-primary-600 hover:bg-primary-50 dark:hover:bg-primary-900/20 font-bold rounded-xl transition-all"
                            >
                                使用此规则
                            </button>
                        </div>
                    )}
                </div>
            </div>
        ) : (
        <div className="p-4 sm:p-6 md:p-8">
          <div className="flex items-center justify-between mb-4 sm:mb-6">
            <h2 className="text-xl sm:text-2xl font-bold dark:text-white">编辑书籍元数据</h2>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-4 sm:gap-6">
            <div className="space-y-3 sm:space-y-4">
              <div className="space-y-1">
                <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">书名</label>
                <input
                  type="text"
                  value={editData.title || ''}
                  onChange={e => update({ title: e.target.value })}
                  className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                />
              </div>
              <div className="space-y-1">
                <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">作者</label>
                <input
                  type="text"
                  value={editData.author || ''}
                  onChange={e => update({ author: e.target.value })}
                  className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                />
              </div>
              <div className="space-y-1">
                <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">演播者</label>
                <input
                  type="text"
                  value={editData.narrator || ''}
                  onChange={e => update({ narrator: e.target.value })}
                  className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                />
              </div>
              <div className="space-y-1">
                <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">标签 (逗号分隔)</label>
                <input
                  type="text"
                  value={editData.tags || ''}
                  onChange={e => update({ tags: e.target.value })}
                  className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                />
              </div>

              <div className="space-y-1">
                <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">流派</label>
                <input
                  type="text"
                  value={editData.genre || ''}
                  onChange={e => update({ genre: e.target.value })}
                  className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                />
              </div>

              <div className="space-y-1">
                <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">年份</label>
                <input
                  type="number"
                  value={editData.year || ''}
                  onChange={e => update({ year: e.target.value ? parseInt(e.target.value) : undefined })}
                  placeholder="例如: 2024"
                  className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                />
              </div>

              <div className="space-y-1">
                <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider flex justify-between items-center">
                    <span>章节正则清洗规则</span>
                    <button
                        onClick={onShowRegexGenerator}
                        className="text-primary-600 hover:text-primary-700 flex items-center gap-1"
                    >
                        <Wand2 size={12} /> 自动生成
                    </button>
                </label>
                <input
                  type="text"
                  value={editData.chapterRegex || ''}
                  onChange={e => update({ chapterRegex: e.target.value })}
                  placeholder="^...(\d+)...(.+)$"
                  className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base font-mono bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                />
                <p className="text-[10px] text-slate-400">用于从文件名提取章节号和标题。修改后需重新扫描生效。</p>
              </div>
            </div>

            <div className="space-y-3 sm:space-y-4">
              <div className="space-y-1">
                <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">封面 URL</label>
                <input
                  type="text"
                  value={editData.coverUrl || ''}
                  onChange={e => update({ coverUrl: e.target.value })}
                  className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                />
              </div>
              <div className="grid grid-cols-2 gap-3 sm:gap-4">
                <div className="space-y-1">
                  <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">跳过片头 (秒)</label>
                  <input
                    type="number"
                    value={editData.skipIntro || 0}
                    onChange={e => update({ skipIntro: parseInt(e.target.value) || 0 })}
                    className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                  />
                </div>
                <div className="space-y-1">
                  <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">跳过片尾 (秒)</label>
                  <input
                    type="number"
                    value={editData.skipOutro || 0}
                    onChange={e => update({ skipOutro: parseInt(e.target.value) || 0 })}
                    className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                  />
                </div>
              </div>
            </div>
          </div>

          <div className="mt-4 sm:mt-6 space-y-1">
            <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">简介</label>
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
              className="px-4 py-2.5 sm:py-3 font-bold text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-xl transition-all flex items-center justify-center gap-2 sm:justify-start"
            >
              <Trash2 size={18} className="sm:w-5 sm:h-5" />
              删除书籍
            </button>
            <div className="flex-1" />
            <div className="flex gap-2 sm:gap-3">
              <button
                onClick={onWriteMetadata}
                className="flex-1 sm:flex-none px-2.5 sm:px-6 py-2.5 sm:py-3 font-bold text-primary-600 bg-primary-50 hover:bg-primary-100 dark:bg-primary-900/20 dark:hover:bg-primary-900/30 rounded-xl transition-all flex items-center justify-center gap-1.5 sm:gap-2 text-xs sm:text-base whitespace-nowrap"
                title="将元数据写入音频文件"
              >
                <FileSignature size={16} className="sm:w-5 sm:h-5" />
                写入文件
              </button>
              <button
                onClick={onClose}
                className="flex-1 sm:flex-none px-3 sm:px-6 py-2.5 sm:py-3 font-bold text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-xl transition-all text-xs sm:text-base whitespace-nowrap"
              >
                取消
              </button>
              <button
                onClick={onSave}
                className="flex-1 sm:flex-none px-3 sm:px-8 py-2.5 sm:py-3 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-xl shadow-lg shadow-primary-500/30 flex items-center justify-center gap-1.5 sm:gap-2 transition-all text-xs sm:text-base whitespace-nowrap"
              >
                <Save size={16} className="sm:w-5 sm:h-5" />
                <span>保存<span className="hidden min-[380px]:inline">更改</span></span>
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
