import React, { useState } from 'react';
import {
  ArrowLeft,
  Check,
  ChevronRight,
  Image as ImageIcon,
  Loader2,
  Plus,
  Trash2,
} from 'lucide-react';
import type {
  Book,
  ScraperSearchField,
  ScraperSearchItem,
  ScraperSource,
} from '../../core/types';
import { getCoverUrl } from '../../core/utils/image';
import {
  FIELD_DEFINITIONS,
  fieldValueForEditor,
  formatCurrentValue,
  formatFieldValue,
  getDraftBookFieldValue,
  getItemFieldValue,
  getResultExternalId,
  getResultFields,
  getResultKey,
  getSearchFields,
  getSearchInputType,
  hasFieldValue,
  type CoverFrameProps,
  type ScrapeSearchResult,
  type SelectedField,
} from './scrapeDiffHelpers';

// ─── CoverFrame ─────────────────────────────────────────────────────────────
// 通用封面框：value 是 URL（可能是 cover_url 字段），出错时退化为 ImageIcon 占位。

export const CoverFrame: React.FC<CoverFrameProps> = ({ value, alt, book, className = '' }) => {
  const [failedSrc, setFailedSrc] = useState('');
  const rawValue = typeof value === 'string' ? value.trim() : '';
  const src = rawValue ? getCoverUrl(rawValue, book?.libraryId, book?.id) : '';
  const failed = src === failedSrc;

  return (
    <div className={`relative overflow-hidden rounded-lg border border-slate-200 bg-slate-100 dark:border-slate-800 dark:bg-slate-900 ${className}`}>
      {src && !failed ? (
        <img
          src={src}
          alt={alt}
          className="h-full w-full object-cover"
          referrerPolicy="no-referrer"
          onError={() => setFailedSrc(src)}
        />
      ) : (
        <div className="flex h-full w-full items-center justify-center text-slate-400">
          <ImageIcon size={24} />
        </div>
      )}
    </div>
  );
};

// ─── SearchStep ─────────────────────────────────────────────────────────────
// 步骤 1：选源 + 填搜索参数 + 当前书籍预览。

interface SearchStepProps {
  book: Book;
  sources: ScraperSource[];
  activeSourceId: string;
  activeSource: ScraperSource | null;
  enabledSourceIds: Set<string>;
  enabledSearchSources: ScraperSource[];
  searchFields: ScraperSearchField[];
  activeSearchValues: Record<string, string>;
  activeResultFields: string[];
  selectedFields: Record<string, SelectedField>;
  error: string | null;
  onToggleSourceEnabled: (id: string) => void;
  onActiveSourceChange: (id: string) => void;
  onUpdateSearchValue: (sourceId: string, fieldKey: string, value: string) => void;
}

export const SearchStep: React.FC<SearchStepProps> = ({
  book,
  sources,
  activeSourceId,
  activeSource,
  enabledSourceIds,
  enabledSearchSources,
  searchFields,
  activeSearchValues,
  activeResultFields,
  selectedFields,
  error,
  onToggleSourceEnabled,
  onActiveSourceChange,
  onUpdateSearchValue,
}) => (
  <div className="h-full overflow-y-auto p-4 sm:p-5">
    <div className="mx-auto grid max-w-6xl grid-cols-1 gap-4 lg:grid-cols-[minmax(0,1fr)_320px]">
      <section className="rounded-xl border border-slate-200 bg-white p-4 shadow-sm dark:border-slate-800 dark:bg-slate-900 sm:p-5">
        <div className="grid grid-cols-1 gap-4 lg:grid-cols-[260px_minmax(0,1fr)]">
          <div>
            <div className="mb-2 flex items-center justify-between gap-3">
              <label className="block text-xs font-bold uppercase text-slate-400">本次启用插件</label>
              <span className="text-xs font-bold text-primary-600">{enabledSearchSources.length} 个</span>
            </div>
            <div className="space-y-2">
              {sources.map((source) => {
                const enabled = enabledSourceIds.has(source.id);
                const active = activeSourceId === source.id;

                return (
                  <div
                    key={source.id}
                    className={`rounded-xl border p-2.5 transition-colors ${
                      active
                        ? 'border-primary-300 bg-primary-50 dark:border-primary-900 dark:bg-primary-950/30'
                        : 'border-slate-200 bg-slate-50 dark:border-slate-800 dark:bg-slate-950'
                    }`}
                  >
                    <div className="flex items-center gap-2">
                      <button
                        onClick={() => onToggleSourceEnabled(source.id)}
                        className={`flex h-6 w-10 shrink-0 items-center rounded-full p-0.5 transition-colors ${
                          enabled ? 'bg-primary-600' : 'bg-slate-300 dark:bg-slate-700'
                        }`}
                        title={enabled ? '本次搜索启用' : '本次搜索停用'}
                      >
                        <span className={`h-5 w-5 rounded-full bg-white shadow transition-transform ${
                          enabled ? 'translate-x-4' : ''
                        }`} />
                      </button>
                      <button
                        onClick={() => onActiveSourceChange(source.id)}
                        className="min-w-0 flex-1 text-left"
                      >
                        <div className="truncate text-sm font-bold text-slate-800 dark:text-slate-100">
                          {source.name}
                        </div>
                        <div className="mt-0.5 truncate text-[11px] text-slate-400">
                          {getSearchFields(source).map((field) => field.label).join(' / ')}
                        </div>
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          </div>

          <div className="min-w-0">
            <div className="mb-3 flex items-start justify-between gap-3">
              <div>
                <div className="text-xs font-bold uppercase text-slate-400">搜索参数</div>
                <h3 className="mt-1 font-bold text-slate-950 dark:text-white">{activeSource?.name || '未选择插件'}</h3>
              </div>
              {activeSource ? (
                <button
                  onClick={() => onToggleSourceEnabled(activeSource.id)}
                  className={`rounded-lg px-3 py-1.5 text-xs font-bold transition-colors ${
                    enabledSourceIds.has(activeSource.id)
                      ? 'bg-primary-600 text-white'
                      : 'bg-slate-100 text-slate-500 dark:bg-slate-950 dark:text-slate-300'
                  }`}
                >
                  {enabledSourceIds.has(activeSource.id) ? '已启用' : '未启用'}
                </button>
              ) : null}
            </div>

            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
              {searchFields.map((field) => (
                <div key={field.key} className={field.key === 'title' || field.required ? 'sm:col-span-2' : ''}>
                  <label className="mb-1.5 block text-xs font-bold text-slate-500 dark:text-slate-400">
                    {field.label}{field.required ? <span className="ml-1 text-red-500">*</span> : null}
                  </label>
                  <input
                    type={getSearchInputType(field)}
                    value={activeSearchValues[field.key] || ''}
                    onChange={(event) => onUpdateSearchValue(activeSourceId, field.key, event.target.value)}
                    placeholder={field.placeholder || ''}
                    disabled={!activeSource}
                    className="w-full rounded-xl border border-slate-200 bg-slate-50 px-3 py-3 text-sm outline-none transition focus:ring-2 focus:ring-primary-500 disabled:opacity-60 dark:border-slate-800 dark:bg-slate-950 dark:text-white"
                  />
                </div>
              ))}
            </div>
          </div>
        </div>

        {error ? (
          <div className="mt-4 rounded-lg bg-red-50 px-3 py-2 text-sm font-bold text-red-600 dark:bg-red-900/20">
            {error}
          </div>
        ) : null}
      </section>

      <aside className="space-y-4">
        <section className="rounded-xl border border-slate-200 bg-white p-4 shadow-sm dark:border-slate-800 dark:bg-slate-900">
          <div className="flex gap-3">
            <CoverFrame
              value={getDraftBookFieldValue(book, selectedFields, 'cover_url')}
              book={book}
              alt={book.title || '当前封面'}
              className="h-28 w-20 shrink-0"
            />
            <div className="min-w-0 flex-1">
              <div className="text-xs font-bold uppercase text-slate-400">当前书籍</div>
              <h3 className="mt-1 line-clamp-2 font-bold text-slate-950 dark:text-white">
                {formatCurrentValue(getDraftBookFieldValue(book, selectedFields, 'title'))}
              </h3>
              <div className="mt-2 space-y-1 text-xs text-slate-500 dark:text-slate-400">
                <div className="truncate">作者：{formatCurrentValue(getDraftBookFieldValue(book, selectedFields, 'author'))}</div>
                <div className="truncate">演播：{formatCurrentValue(getDraftBookFieldValue(book, selectedFields, 'narrator'))}</div>
              </div>
            </div>
          </div>
        </section>

        <section className="rounded-xl border border-slate-200 bg-white p-4 shadow-sm dark:border-slate-800 dark:bg-slate-900">
          <div className="mb-3 text-xs font-bold uppercase text-slate-400">当前插件返回字段</div>
          <div className="flex flex-wrap gap-2">
            {activeResultFields.map((fieldKey) => (
              <span
                key={fieldKey}
                className="inline-flex items-center gap-1.5 rounded-lg bg-slate-100 px-2.5 py-1.5 text-xs font-bold text-slate-600 dark:bg-slate-950 dark:text-slate-300"
              >
                <span className="text-slate-400">{FIELD_DEFINITIONS[fieldKey].icon}</span>
                {FIELD_DEFINITIONS[fieldKey].label}
              </span>
            ))}
          </div>
        </section>
      </aside>
    </div>
  </div>
);

// ─── ResultsStep ────────────────────────────────────────────────────────────
// 步骤 2：列表/详情两态，挑字段。

interface ResultsStepProps {
  book: Book;
  results: ScrapeSearchResult[];
  resultView: 'list' | 'detail';
  selectedResult: ScrapeSearchResult | null;
  selectedResultItem: ScraperSearchItem | null;
  selectedResultSource: ScraperSource | null;
  selectedResultKey: string | null;
  selectedResultFields: string[];
  selectedFields: Record<string, SelectedField>;
  selectedFieldList: SelectedField[];
  selectedCount: number;
  expandedDescriptions: Set<string>;
  searching: boolean;
  error: string | null;
  resultErrorCount: number;
  enabledSearchSources: ScraperSource[];
  coverAspectClass: string;
  compactCoverClass: string;
  onSetResultView: (view: 'list' | 'detail') => void;
  onSelectAllAvailableFields: (result: ScrapeSearchResult) => void;
  onSelectField: (result: ScrapeSearchResult, fieldKey: string) => void;
  onToggleDescription: (key: string) => void;
  onOpenResultDetail: (index: number) => void;
  onBackToSearch: () => void;
}

export const ResultsStep: React.FC<ResultsStepProps> = ({
  book,
  results,
  resultView,
  selectedResult,
  selectedResultItem,
  selectedResultSource,
  selectedResultKey,
  selectedResultFields,
  selectedFields,
  selectedFieldList,
  selectedCount,
  expandedDescriptions,
  searching,
  error,
  resultErrorCount,
  enabledSearchSources,
  coverAspectClass,
  compactCoverClass,
  onSetResultView,
  onSelectAllAvailableFields,
  onSelectField,
  onToggleDescription,
  onOpenResultDetail,
  onBackToSearch,
}) => {
  if (resultView === 'detail' && selectedResult && selectedResultItem && selectedResultSource) {
    return (
      <div className="h-full overflow-y-auto p-3 sm:p-5">
        <div className="mx-auto max-w-4xl space-y-4">
          <button
            onClick={() => onSetResultView('list')}
            className="inline-flex items-center gap-2 rounded-xl bg-white px-3 py-2 text-sm font-bold text-slate-600 shadow-sm ring-1 ring-slate-200 transition-colors hover:bg-slate-50 dark:bg-slate-900 dark:text-slate-300 dark:ring-slate-800 dark:hover:bg-slate-800"
          >
            <ArrowLeft size={17} />
            返回搜索结果
          </button>

          <section className="rounded-xl border border-slate-200 bg-white p-4 shadow-sm dark:border-slate-800 dark:bg-slate-900">
            <div className="flex flex-col gap-4 sm:flex-row sm:items-start">
              <CoverFrame
                value={getItemFieldValue(selectedResultItem, 'cover_url')}
                alt={selectedResultItem.title || '搜索结果封面'}
                className="h-40 w-28 shrink-0 self-center sm:self-start"
              />
              <div className="min-w-0 flex-1 text-center sm:text-left">
                <div className="inline-flex max-w-full rounded-lg bg-slate-100 px-2 py-1 text-xs font-bold text-slate-500 dark:bg-slate-950 dark:text-slate-300">
                  <span className="truncate">{selectedResultSource.name}</span>
                </div>
                <h3 className="mt-2 text-xl font-bold leading-tight text-slate-950 dark:text-white sm:text-2xl">
                  {selectedResultItem.title || getResultExternalId(selectedResult)}
                </h3>
                <div className="mt-2 text-sm text-slate-500 dark:text-slate-400">
                  {getResultExternalId(selectedResult)}
                </div>
              </div>
              <button
                onClick={() => onSelectAllAvailableFields(selectedResult)}
                className="inline-flex items-center justify-center gap-2 rounded-xl bg-slate-950 px-4 py-2.5 text-sm font-bold text-white transition-opacity hover:opacity-90 dark:bg-white dark:text-slate-950"
              >
                <Check size={16} />
                采用全部
              </button>
            </div>
          </section>

          <section className="grid grid-cols-1 gap-3 lg:grid-cols-2">
            {selectedResultFields.map((fieldKey) => {
              const definition = FIELD_DEFINITIONS[fieldKey];
              const value = getItemFieldValue(selectedResultItem, fieldKey);
              const currentValue = getDraftBookFieldValue(book, selectedFields, fieldKey);
              const hasValue = hasFieldValue(value);
              const selected = selectedFields[fieldKey]?.resultKey === selectedResultKey
                && selectedFields[fieldKey]?.sourceId === selectedResultSource.id;
              const expandedKey = `${selectedResultKey}:${fieldKey}`;
              const expanded = expandedDescriptions.has(expandedKey);
              const isDescription = fieldKey === 'description';

              return (
                <div
                  key={fieldKey}
                  className={`rounded-xl border border-slate-200 bg-white p-4 shadow-sm dark:border-slate-800 dark:bg-slate-900 ${
                    definition.wide ? 'lg:col-span-2' : ''
                  }`}
                >
                  <div className="mb-3 flex items-start justify-between gap-3">
                    <div className="flex min-w-0 items-center gap-2 font-bold text-slate-800 dark:text-slate-100">
                      <span className="shrink-0 text-slate-400">{definition.icon}</span>
                      <span className="truncate">{definition.label}</span>
                    </div>
                    <button
                      onClick={() => onSelectField(selectedResult, fieldKey)}
                      disabled={!hasValue}
                      className={`inline-flex shrink-0 items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-bold transition-colors ${
                        selected
                          ? 'bg-primary-600 text-white'
                          : hasValue
                            ? 'bg-slate-100 text-slate-600 hover:bg-primary-600 hover:text-white dark:bg-slate-950 dark:text-slate-300'
                            : 'bg-slate-50 text-slate-300 dark:bg-slate-950 dark:text-slate-600'
                      }`}
                    >
                      {selected ? <Check size={14} /> : <Plus size={14} />}
                      {selected ? '已采用' : selectedFields[fieldKey] ? '替换' : '采用'}
                    </button>
                  </div>

                  {definition.cover ? (
                    <div className="grid grid-cols-2 gap-3">
                      <div>
                        <div className="mb-1 text-xs font-bold text-slate-400">当前</div>
                        <CoverFrame value={currentValue} book={book} alt="当前封面" className={coverAspectClass} />
                      </div>
                      <div>
                        <div className="mb-1 text-xs font-bold text-primary-500">应用</div>
                        <CoverFrame value={value} alt="待应用封面" className={coverAspectClass} />
                      </div>
                    </div>
                  ) : (
                    <div className="space-y-3">
                      <div>
                        <div className="mb-1 text-xs font-bold text-slate-400">当前</div>
                        <div className="rounded-lg bg-slate-50 px-3 py-2 text-sm leading-relaxed text-slate-500 dark:bg-slate-950 dark:text-slate-400">
                          {formatCurrentValue(currentValue)}
                        </div>
                      </div>
                      <div>
                        <div className="mb-1 text-xs font-bold text-primary-500">应用</div>
                        <div className={`rounded-lg bg-primary-50 px-3 py-2 text-sm font-semibold leading-relaxed text-slate-950 dark:bg-primary-950/25 dark:text-white ${
                          isDescription && !expanded ? 'line-clamp-5' : ''
                        }`}>
                          {formatFieldValue(value)}
                        </div>
                        {isDescription && hasValue ? (
                          <button
                            onClick={() => onToggleDescription(expandedKey)}
                            className="mt-1.5 text-xs font-bold text-primary-600 hover:text-primary-700"
                          >
                            {expanded ? '收起' : '展开'}
                          </button>
                        ) : null}
                      </div>
                    </div>
                  )}
                </div>
              );
            })}
          </section>
        </div>
      </div>
    );
  }

  return (
    <div className="h-full overflow-y-auto p-3 sm:p-5">
      <div className="mx-auto max-w-6xl space-y-4">
        <section className="flex flex-col gap-3 rounded-xl border border-slate-200 bg-white p-4 shadow-sm dark:border-slate-800 dark:bg-slate-900 sm:flex-row sm:items-center sm:justify-between">
          <div>
            <h3 className="text-lg font-bold text-slate-950 dark:text-white">搜索结果</h3>
            <p className="mt-1 text-sm text-slate-500 dark:text-slate-400">
              {enabledSearchSources.length} 个插件 · {results.length} 条结果 · 已选择 {selectedCount} 个字段
            </p>
          </div>
          <button
            onClick={onBackToSearch}
            className="inline-flex items-center justify-center gap-2 rounded-xl bg-slate-100 px-4 py-2.5 text-sm font-bold text-slate-600 transition-colors hover:bg-slate-200 dark:bg-slate-950 dark:text-slate-300 dark:hover:bg-slate-800"
          >
            <ArrowLeft size={16} />
            修改搜索
          </button>
        </section>

        {searching ? (
          <div className="flex h-64 items-center justify-center rounded-xl border border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-900">
            <Loader2 className="animate-spin text-primary-600" size={32} />
          </div>
        ) : error === '搜索失败' && results.length === 0 ? (
          <div className="flex h-64 items-center justify-center rounded-xl border border-dashed border-slate-200 bg-white text-sm font-bold text-red-500 dark:border-slate-800 dark:bg-slate-900">
            搜索失败
          </div>
        ) : results.length === 0 ? (
          <div className="flex h-64 items-center justify-center rounded-xl border border-dashed border-slate-200 bg-white text-sm font-bold text-slate-400 dark:border-slate-800 dark:bg-slate-900">
            {resultErrorCount > 0 ? '没有可展示结果，部分插件搜索失败' : '暂无搜索结果'}
          </div>
        ) : (
          <>
            {resultErrorCount > 0 ? (
              <div className="rounded-xl border border-amber-200 bg-amber-50 px-3 py-2 text-xs font-bold text-amber-700 dark:border-amber-900/60 dark:bg-amber-950/30 dark:text-amber-300">
                {resultErrorCount} 个插件搜索失败，其余结果已展示
              </div>
            ) : null}

            <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
              {results.map((result, index) => {
                const item = result.item;
                const source = result.source;
                const resultKey = getResultKey(result);
                const cover = getItemFieldValue(item, 'cover_url');
                const selectedFromThisResult = selectedFieldList.filter(
                  (selection) => selection.sourceId === source.id && selection.resultKey === resultKey
                );
                const availableFields = getResultFields(source).filter((fieldKey) => hasFieldValue(getItemFieldValue(item, fieldKey)));

                return (
                  <button
                    key={resultKey}
                    onClick={() => onOpenResultDetail(index)}
                    className="group rounded-xl border border-slate-200 bg-white p-3 text-left shadow-sm transition-all hover:-translate-y-0.5 hover:border-primary-300 hover:shadow-md dark:border-slate-800 dark:bg-slate-900 dark:hover:border-primary-900"
                  >
                    <div className="flex gap-3">
                      <CoverFrame value={cover} alt={item.title || '搜索结果封面'} className={`${compactCoverClass} shrink-0 rounded-lg`} />
                      <div className="min-w-0 flex-1">
                        <div className="flex items-start justify-between gap-2">
                          <span className="inline-flex max-w-[10rem] rounded-md bg-slate-100 px-1.5 py-0.5 text-[10px] font-bold text-slate-500 dark:bg-slate-800 dark:text-slate-300">
                            <span className="truncate">{source.name}</span>
                          </span>
                          <ChevronRight size={16} className="shrink-0 text-slate-300 transition-transform group-hover:translate-x-0.5 group-hover:text-primary-500" />
                        </div>
                        <div className="mt-2 line-clamp-2 text-sm font-bold leading-snug text-slate-950 dark:text-white">
                          {item.title || getResultExternalId(result)}
                        </div>
                        <div className="mt-1 line-clamp-1 text-xs text-slate-500 dark:text-slate-400">
                          {formatFieldValue(item.author || item.narrator)}
                        </div>
                        <div className="mt-2 flex flex-wrap gap-1">
                          {availableFields.slice(0, 4).map((fieldKey) => (
                            <span key={fieldKey} className="rounded-md bg-slate-100 px-1.5 py-0.5 text-[10px] font-bold text-slate-500 dark:bg-slate-800 dark:text-slate-300">
                              {FIELD_DEFINITIONS[fieldKey].label}
                            </span>
                          ))}
                          {availableFields.length > 4 ? (
                            <span className="rounded-md bg-slate-100 px-1.5 py-0.5 text-[10px] font-bold text-slate-500 dark:bg-slate-800 dark:text-slate-300">
                              +{availableFields.length - 4}
                            </span>
                          ) : null}
                        </div>
                        {selectedFromThisResult.length > 0 ? (
                          <div className="mt-2 flex flex-wrap gap-1">
                            {selectedFromThisResult.slice(0, 3).map((selection) => (
                              <span key={selection.key} className="rounded-md bg-primary-600 px-1.5 py-0.5 text-[10px] font-bold text-white">
                                {selection.label}
                              </span>
                            ))}
                            {selectedFromThisResult.length > 3 ? (
                              <span className="rounded-md bg-primary-100 px-1.5 py-0.5 text-[10px] font-bold text-primary-700 dark:bg-primary-950 dark:text-primary-300">
                                +{selectedFromThisResult.length - 3}
                              </span>
                            ) : null}
                          </div>
                        ) : null}
                      </div>
                    </div>
                  </button>
                );
              })}
            </div>
          </>
        )}
      </div>
    </div>
  );
};

// ─── ReviewStep ─────────────────────────────────────────────────────────────
// 步骤 3：审阅已选字段，允许手改、清空、删除单项。

interface ReviewStepProps {
  selectedFieldList: SelectedField[];
  selectedCount: number;
  coverAspectClass: string;
  onClearSelectedFields: () => void;
  onRemoveSelectedField: (key: string) => void;
  onUpdateSelectedFieldValue: (key: string, value: string) => void;
}

export const ReviewStep: React.FC<ReviewStepProps> = ({
  selectedFieldList,
  selectedCount,
  coverAspectClass,
  onClearSelectedFields,
  onRemoveSelectedField,
  onUpdateSelectedFieldValue,
}) => (
  <div className="h-full overflow-y-auto p-3 sm:p-5">
    <div className="mx-auto max-w-5xl space-y-4">
      <section className="rounded-xl border border-slate-200 bg-white p-4 shadow-sm dark:border-slate-800 dark:bg-slate-900">
        <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
          <div>
            <h3 className="text-lg font-bold text-slate-950 dark:text-white">待应用字段</h3>
            <p className="mt-1 text-sm text-slate-500 dark:text-slate-400">{selectedCount} 个字段</p>
          </div>
          {selectedCount > 0 ? (
            <button
              onClick={onClearSelectedFields}
              className="inline-flex items-center justify-center gap-2 rounded-xl bg-slate-100 px-4 py-2.5 text-sm font-bold text-slate-600 transition-colors hover:bg-slate-200 dark:bg-slate-950 dark:text-slate-300 dark:hover:bg-slate-800"
            >
              <Trash2 size={16} />
              清空
            </button>
          ) : null}
        </div>
      </section>

      {selectedFieldList.length === 0 ? (
        <div className="flex h-72 items-center justify-center rounded-xl border border-dashed border-slate-200 bg-white text-sm font-bold text-slate-400 dark:border-slate-800 dark:bg-slate-900">
          未选择字段
        </div>
      ) : (
        selectedFieldList.map((selection) => {
          const definition = FIELD_DEFINITIONS[selection.key];
          const editorValue = fieldValueForEditor(selection.value);
          const isLongText = selection.key === 'description' || selection.key === 'tags';
          const isBooleanField = selection.key === 'explicit' || selection.key === 'abridged';

          return (
            <section key={selection.key} className="rounded-xl border border-slate-200 bg-white p-4 shadow-sm dark:border-slate-800 dark:bg-slate-900">
              <div className="mb-3 flex items-start justify-between gap-3">
                <div>
                  <div className="flex items-center gap-2 font-bold text-slate-950 dark:text-white">
                    <span className="text-slate-400">{definition.icon}</span>
                    {selection.label}
                  </div>
                  <div className="mt-1 text-xs text-slate-400">
                    {selection.sourceName} · {selection.resultTitle}
                  </div>
                </div>
                <button
                  onClick={() => onRemoveSelectedField(selection.key)}
                  className="rounded-lg p-2 text-slate-400 transition-colors hover:bg-red-50 hover:text-red-500 dark:hover:bg-red-900/20"
                >
                  <Trash2 size={16} />
                </button>
              </div>

              {definition.cover ? (
                <div className="grid grid-cols-1 gap-4 md:grid-cols-[10rem_1fr]">
                  <CoverFrame value={selection.value} alt="待应用封面" className={`${coverAspectClass} w-40 max-w-full`} />
                  <div>
                    <label className="mb-1.5 block text-xs font-bold text-primary-500">封面 URL</label>
                    <input
                      type="url"
                      value={editorValue}
                      onChange={(event) => onUpdateSelectedFieldValue(selection.key, event.target.value)}
                      placeholder="https://example.com/cover.jpg"
                      className="w-full rounded-xl border border-slate-200 bg-primary-50 px-3 py-3 text-sm font-medium text-slate-950 outline-none transition focus:ring-2 focus:ring-primary-500 dark:border-slate-800 dark:bg-primary-950/25 dark:text-white"
                    />
                  </div>
                </div>
              ) : (
                <div>
                  <label className="mb-1.5 block text-xs font-bold text-primary-500">
                    {selection.key === 'tags' ? '应用值（逗号分隔）' : '应用值'}
                  </label>
                  {isBooleanField ? (
                    <select
                      value={String(Boolean(selection.value))}
                      onChange={(event) => onUpdateSelectedFieldValue(selection.key, event.target.value)}
                      className="w-full rounded-xl border border-slate-200 bg-primary-50 px-3 py-3 text-sm font-medium text-slate-950 outline-none transition focus:ring-2 focus:ring-primary-500 dark:border-slate-800 dark:bg-primary-950/25 dark:text-white"
                    >
                      <option value="true">是</option>
                      <option value="false">否</option>
                    </select>
                  ) : isLongText ? (
                    <textarea
                      value={editorValue}
                      onChange={(event) => onUpdateSelectedFieldValue(selection.key, event.target.value)}
                      rows={selection.key === 'description' ? 6 : 3}
                      className="w-full resize-y rounded-xl border border-slate-200 bg-primary-50 px-3 py-3 text-sm font-medium leading-relaxed text-slate-950 outline-none transition focus:ring-2 focus:ring-primary-500 dark:border-slate-800 dark:bg-primary-950/25 dark:text-white"
                    />
                  ) : (
                    <input
                      type={selection.key === 'year' ? 'number' : 'text'}
                      value={editorValue}
                      onChange={(event) => onUpdateSelectedFieldValue(selection.key, event.target.value)}
                      className="w-full rounded-xl border border-slate-200 bg-primary-50 px-3 py-3 text-sm font-medium text-slate-950 outline-none transition focus:ring-2 focus:ring-primary-500 dark:border-slate-800 dark:bg-primary-950/25 dark:text-white"
                    />
                  )}
                </div>
              )}
            </section>
          );
        })
      )}
    </div>
  </div>
);
