import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
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
  getFieldLabel,
  getItemFieldValue,
  getResultExternalId,
  getResultFields,
  getResultKey,
  getSearchFields,
  getSearchFieldLabel,
  getSearchInputType,
  hasFieldValue,
  type CoverFrameProps,
  type ScrapeSearchResult,
  type SelectedField,
} from './scrapeDiffHelpers';

// ─── CoverFrame ─────────────────────────────────────────────────────────────
// Shared cover frame: value is a URL and falls back to an icon when loading fails.

export const CoverFrame: React.FC<CoverFrameProps> = ({ value, alt, book, className = '' }) => {
  const [failedSrc, setFailedSrc] = useState('');
  const rawValue = typeof value === 'string' ? value.trim() : '';
  const src = rawValue ? getCoverUrl(rawValue, book?.library_id, book?.id) : '';
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
// Step 1: select sources, fill search fields, and preview the current book.

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
}) => {
  const { t } = useTranslation();
  const booleanLabels = { trueLabel: t('scrapeDiff.yes'), falseLabel: t('scrapeDiff.no') };

  return (
  <div className="h-full overflow-y-auto p-4 sm:p-5">
    <div className="mx-auto grid max-w-6xl grid-cols-1 gap-4 lg:grid-cols-[minmax(0,1fr)_320px]">
      <section className="rounded-xl border border-slate-200 bg-white p-4 shadow-sm dark:border-slate-800 dark:bg-slate-900 sm:p-5">
        <div className="grid grid-cols-1 gap-4 lg:grid-cols-[260px_minmax(0,1fr)]">
          <div>
            <div className="mb-2 flex items-center justify-between gap-3">
              <label className="block text-xs font-bold uppercase text-slate-400">{t('scrapeDiff.enabledPlugins')}</label>
              <span className="text-xs font-bold text-primary-600">{t('scrapeDiff.pluginCount', { count: enabledSearchSources.length })}</span>
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
                        title={enabled ? t('scrapeDiff.searchEnabled') : t('scrapeDiff.searchDisabled')}
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
                          {getSearchFields(source).map((field) => getSearchFieldLabel(field, t)).join(' / ')}
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
                <div className="text-xs font-bold uppercase text-slate-400">{t('scrapeDiff.searchParams')}</div>
                <h3 className="mt-1 font-bold text-slate-950 dark:text-white">{activeSource?.name || t('scrapeDiff.noPluginSelected')}</h3>
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
                  {enabledSourceIds.has(activeSource.id) ? t('scrapeDiff.enabled') : t('scrapeDiff.disabled')}
                </button>
              ) : null}
            </div>

            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
              {searchFields.map((field) => (
                <div key={field.key} className={field.key === 'title' || field.required ? 'sm:col-span-2' : ''}>
                  <label className="mb-1.5 block text-xs font-bold text-slate-500 dark:text-slate-400">
                    {getSearchFieldLabel(field, t)}{field.required ? <span className="ml-1 text-red-500">*</span> : null}
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
              alt={book.title || t('scrapeDiff.currentCover')}
              className="h-28 w-20 shrink-0"
            />
            <div className="min-w-0 flex-1">
              <div className="text-xs font-bold uppercase text-slate-400">{t('scrapeDiff.currentBook')}</div>
              <h3 className="mt-1 line-clamp-2 font-bold text-slate-950 dark:text-white">
                {formatCurrentValue(getDraftBookFieldValue(book, selectedFields, 'title'), t('scrapeDiff.unknown'), booleanLabels)}
              </h3>
              <div className="mt-2 space-y-1 text-xs text-slate-500 dark:text-slate-400">
                <div className="truncate">{t('scrapeDiff.authorLine', { value: formatCurrentValue(getDraftBookFieldValue(book, selectedFields, 'author'), t('scrapeDiff.unknown'), booleanLabels) })}</div>
                <div className="truncate">{t('scrapeDiff.narratorLine', { value: formatCurrentValue(getDraftBookFieldValue(book, selectedFields, 'narrator'), t('scrapeDiff.unknown'), booleanLabels) })}</div>
              </div>
            </div>
          </div>
        </section>

        <section className="rounded-xl border border-slate-200 bg-white p-4 shadow-sm dark:border-slate-800 dark:bg-slate-900">
          <div className="mb-3 text-xs font-bold uppercase text-slate-400">{t('scrapeDiff.currentResultFields')}</div>
          <div className="flex flex-wrap gap-2">
            {activeResultFields.map((fieldKey) => (
              <span
                key={fieldKey}
                className="inline-flex items-center gap-1.5 rounded-lg bg-slate-100 px-2.5 py-1.5 text-xs font-bold text-slate-600 dark:bg-slate-950 dark:text-slate-300"
              >
                <span className="text-slate-400">{FIELD_DEFINITIONS[fieldKey].icon}</span>
                {getFieldLabel(fieldKey, t, activeSource)}
              </span>
            ))}
          </div>
        </section>
      </aside>
    </div>
  </div>
  );
};

// ─── ResultsStep ────────────────────────────────────────────────────────────
// Step 2: choose fields from list or detail views.

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
  const { t } = useTranslation();
  const booleanLabels = { trueLabel: t('scrapeDiff.yes'), falseLabel: t('scrapeDiff.no') };

  if (resultView === 'detail' && selectedResult && selectedResultItem && selectedResultSource) {
    return (
      <div className="h-full overflow-y-auto p-3 sm:p-5">
        <div className="mx-auto max-w-4xl space-y-4">
          <button
            onClick={() => onSetResultView('list')}
            className="inline-flex items-center gap-2 rounded-xl bg-white px-3 py-2 text-sm font-bold text-slate-600 shadow-sm ring-1 ring-slate-200 transition-colors hover:bg-slate-50 dark:bg-slate-900 dark:text-slate-300 dark:ring-slate-800 dark:hover:bg-slate-800"
          >
            <ArrowLeft size={17} />
            {t('scrapeDiff.backToResults')}
          </button>

          <section className="rounded-xl border border-slate-200 bg-white p-4 shadow-sm dark:border-slate-800 dark:bg-slate-900">
            <div className="flex flex-col gap-4 sm:flex-row sm:items-start">
              <CoverFrame
                value={getItemFieldValue(selectedResultItem, 'cover_url')}
                alt={selectedResultItem.title || t('scrapeDiff.resultCover')}
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
                {t('scrapeDiff.useAll')}
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
                      <span className="truncate">{getFieldLabel(fieldKey, t, selectedResultSource)}</span>
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
                      {selected ? t('scrapeDiff.selected') : selectedFields[fieldKey] ? t('scrapeDiff.replace') : t('scrapeDiff.use')}
                    </button>
                  </div>

                  {definition.cover ? (
                    <div className="grid grid-cols-2 gap-3">
                      <div>
                        <div className="mb-1 text-xs font-bold text-slate-400">{t('scrapeDiff.current')}</div>
                        <CoverFrame value={currentValue} book={book} alt={t('scrapeDiff.currentCover')} className={coverAspectClass} />
                      </div>
                      <div>
                        <div className="mb-1 text-xs font-bold text-primary-500">{t('scrapeDiff.applyValue')}</div>
                        <CoverFrame value={value} alt={t('scrapeDiff.pendingCover')} className={coverAspectClass} />
                      </div>
                    </div>
                  ) : (
                    <div className="space-y-3">
                      <div>
                        <div className="mb-1 text-xs font-bold text-slate-400">{t('scrapeDiff.current')}</div>
                        <div className="rounded-lg bg-slate-50 px-3 py-2 text-sm leading-relaxed text-slate-500 dark:bg-slate-950 dark:text-slate-400">
                          {formatCurrentValue(currentValue, t('scrapeDiff.unknown'), booleanLabels)}
                        </div>
                      </div>
                      <div>
                        <div className="mb-1 text-xs font-bold text-primary-500">{t('scrapeDiff.applyValue')}</div>
                        <div className={`rounded-lg bg-primary-50 px-3 py-2 text-sm font-semibold leading-relaxed text-slate-950 dark:bg-primary-950/25 dark:text-white ${
                          isDescription && !expanded ? 'line-clamp-5' : ''
                        }`}>
                          {formatFieldValue(value, t('scrapeDiff.notReturned'), booleanLabels)}
                        </div>
                        {isDescription && hasValue ? (
                          <button
                            onClick={() => onToggleDescription(expandedKey)}
                            className="mt-1.5 text-xs font-bold text-primary-600 hover:text-primary-700"
                          >
                            {expanded ? t('scrapeDiff.collapse') : t('scrapeDiff.expand')}
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
            <h3 className="text-lg font-bold text-slate-950 dark:text-white">{t('scrapeDiff.searchResults')}</h3>
            <p className="mt-1 text-sm text-slate-500 dark:text-slate-400">
              {t('scrapeDiff.resultsSummary', { plugins: enabledSearchSources.length, results: results.length, selected: selectedCount })}
            </p>
          </div>
          <button
            onClick={onBackToSearch}
            className="inline-flex items-center justify-center gap-2 rounded-xl bg-slate-100 px-4 py-2.5 text-sm font-bold text-slate-600 transition-colors hover:bg-slate-200 dark:bg-slate-950 dark:text-slate-300 dark:hover:bg-slate-800"
          >
            <ArrowLeft size={16} />
            {t('scrapeDiff.editSearch')}
          </button>
        </section>

        {searching ? (
          <div className="flex h-64 items-center justify-center rounded-xl border border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-900">
            <Loader2 className="animate-spin text-primary-600" size={32} />
          </div>
        ) : error === t('scrapeDiff.searchFailed') && results.length === 0 ? (
          <div className="flex h-64 items-center justify-center rounded-xl border border-dashed border-slate-200 bg-white text-sm font-bold text-red-500 dark:border-slate-800 dark:bg-slate-900">
            {t('scrapeDiff.searchFailed')}
          </div>
        ) : results.length === 0 ? (
          <div className="flex h-64 items-center justify-center rounded-xl border border-dashed border-slate-200 bg-white text-sm font-bold text-slate-400 dark:border-slate-800 dark:bg-slate-900">
            {resultErrorCount > 0 ? t('scrapeDiff.noDisplayResultsPartialFail') : t('scrapeDiff.noSearchResults')}
          </div>
        ) : (
          <>
            {resultErrorCount > 0 ? (
              <div className="rounded-xl border border-amber-200 bg-amber-50 px-3 py-2 text-xs font-bold text-amber-700 dark:border-amber-900/60 dark:bg-amber-950/30 dark:text-amber-300">
                {t('scrapeDiff.partialPluginFailures', { count: resultErrorCount })}
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
                      <CoverFrame value={cover} alt={item.title || t('scrapeDiff.resultCover')} className={`${compactCoverClass} shrink-0 rounded-lg`} />
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
                          {formatFieldValue(item.author || item.narrator, t('scrapeDiff.notReturned'), booleanLabels)}
                        </div>
                        <div className="mt-2 flex flex-wrap gap-1">
                          {availableFields.slice(0, 4).map((fieldKey) => (
                            <span key={fieldKey} className="rounded-md bg-slate-100 px-1.5 py-0.5 text-[10px] font-bold text-slate-500 dark:bg-slate-800 dark:text-slate-300">
                              {getFieldLabel(fieldKey, t, source)}
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
                                {selection.label || getFieldLabel(selection.key, t, source)}
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
// Step 3: review, edit, clear, or remove selected fields.

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
}) => {
  const { t } = useTranslation();

  return (
  <div className="h-full overflow-y-auto p-3 sm:p-5">
    <div className="mx-auto max-w-5xl space-y-4">
      <section className="rounded-xl border border-slate-200 bg-white p-4 shadow-sm dark:border-slate-800 dark:bg-slate-900">
        <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
          <div>
            <h3 className="text-lg font-bold text-slate-950 dark:text-white">{t('scrapeDiff.reviewTitle')}</h3>
            <p className="mt-1 text-sm text-slate-500 dark:text-slate-400">{t('scrapeDiff.fieldCount', { count: selectedCount })}</p>
          </div>
          {selectedCount > 0 ? (
            <button
              onClick={onClearSelectedFields}
              className="inline-flex items-center justify-center gap-2 rounded-xl bg-slate-100 px-4 py-2.5 text-sm font-bold text-slate-600 transition-colors hover:bg-slate-200 dark:bg-slate-950 dark:text-slate-300 dark:hover:bg-slate-800"
            >
              <Trash2 size={16} />
              {t('scrapeDiff.clear')}
            </button>
          ) : null}
        </div>
      </section>

      {selectedFieldList.length === 0 ? (
        <div className="flex h-72 items-center justify-center rounded-xl border border-dashed border-slate-200 bg-white text-sm font-bold text-slate-400 dark:border-slate-800 dark:bg-slate-900">
          {t('scrapeDiff.noSelectedFields')}
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
                    {selection.label || getFieldLabel(selection.key, t)}
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
                  <CoverFrame value={selection.value} alt={t('scrapeDiff.pendingCover')} className={`${coverAspectClass} w-40 max-w-full`} />
                  <div>
                    <label className="mb-1.5 block text-xs font-bold text-primary-500">{t('scrapeDiff.coverUrl')}</label>
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
                    {selection.key === 'tags' ? t('scrapeDiff.appliedTagsValue') : t('scrapeDiff.appliedValue')}
                  </label>
                  {isBooleanField ? (
                    <select
                      value={String(Boolean(selection.value))}
                      onChange={(event) => onUpdateSelectedFieldValue(selection.key, event.target.value)}
                      className="w-full rounded-xl border border-slate-200 bg-primary-50 px-3 py-3 text-sm font-medium text-slate-950 outline-none transition focus:ring-2 focus:ring-primary-500 dark:border-slate-800 dark:bg-primary-950/25 dark:text-white"
                    >
                      <option value="true">{t('scrapeDiff.yes')}</option>
                      <option value="false">{t('scrapeDiff.no')}</option>
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
};
