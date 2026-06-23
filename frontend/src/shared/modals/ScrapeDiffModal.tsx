import React, { useCallback, useEffect, useMemo, useState } from 'react';
import apiClient from '../../core/api/client';
import type { Book, Library, ScraperSearchItem, ScraperSource } from '../../core/types';
import {
  AlertTriangle,
  ArrowLeft,
  ArrowRight,
  ChevronRight,
  Loader2,
  RefreshCw,
  Save,
  Search,
  X,
} from 'lucide-react';
import { getCoverAspectClass, useBookshelfCoverShape } from '../../core/hooks/useBookshelfCoverShape';
import {
  FIELD_DEFINITIONS,
  FIELD_ORDER,
  STEP_ITEMS,
  editorValueForField,
  fieldValueForApi,
  getBookDefaultValue,
  getDefaultEnabledSourceIds,
  getItemFieldValue,
  getResultExternalId,
  getResultFields,
  getResultKey,
  getSearchFields,
  getSharedSearchFieldKind,
  getTitleMatchScore,
  getTitleMatchTerms,
  hasFieldValue,
  type FieldValue,
  type LibraryScraperConfig,
  type ModalStep,
  type ResultView,
  type ScrapeSearchResult,
  type SelectedField,
} from './scrapeDiffHelpers';
import { SearchStep, ResultsStep, ReviewStep } from './ScrapeDiffPieces';

interface Props {
  bookId: string;
  onClose: () => void;
  onSave: () => void;
}

const ScrapeDiffModal: React.FC<Props> = ({ bookId, onClose, onSave }) => {
  const coverShape = useBookshelfCoverShape();
  const coverAspectClass = getCoverAspectClass(coverShape);
  const compactCoverClass = coverShape === 'square' ? 'h-24 w-24' : 'h-28 w-20';
  const [book, setBook] = useState<Book | null>(null);
  const [sources, setSources] = useState<ScraperSource[]>([]);
  const [activeSourceId, setActiveSourceId] = useState('');
  const [enabledSourceIds, setEnabledSourceIds] = useState<Set<string>>(new Set());
  const [searchValuesBySourceId, setSearchValuesBySourceId] = useState<Record<string, Record<string, string>>>({});
  const [results, setResults] = useState<ScrapeSearchResult[]>([]);
  const [selectedResultIndex, setSelectedResultIndex] = useState<number | null>(null);
  const [selectedFields, setSelectedFields] = useState<Record<string, SelectedField>>({});
  const [expandedDescriptions, setExpandedDescriptions] = useState<Set<string>>(new Set());
  const [searchErrors, setSearchErrors] = useState<Record<string, string>>({});
  const [step, setStep] = useState<ModalStep>('search');
  const [resultView, setResultView] = useState<ResultView>('list');
  const [loading, setLoading] = useState(true);
  const [searching, setSearching] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');

  const activeSource = useMemo(
    () => sources.find((source) => source.id === activeSourceId) || null,
    [sources, activeSourceId]
  );

  const enabledSearchSources = useMemo(
    () => sources.filter((source) => enabledSourceIds.has(source.id)),
    [sources, enabledSourceIds]
  );
  const searchFields = useMemo(() => getSearchFields(activeSource), [activeSource]);
  const activeResultFields = useMemo(() => getResultFields(activeSource), [activeSource]);
  const activeSearchValues = activeSourceId ? searchValuesBySourceId[activeSourceId] || {} : {};
  const selectedResult = selectedResultIndex !== null ? results[selectedResultIndex] || null : null;
  const selectedResultItem = selectedResult?.item || null;
  const selectedResultSource = selectedResult?.source || null;
  const selectedResultFields = useMemo(() => getResultFields(selectedResultSource), [selectedResultSource]);
  const selectedCount = Object.keys(selectedFields).length;

  const selectedFieldList = useMemo(() => {
    return Object.values(selectedFields).sort((a, b) => {
      const aIndex = FIELD_ORDER.indexOf(a.key);
      const bIndex = FIELD_ORDER.indexOf(b.key);
      return (aIndex === -1 ? 999 : aIndex) - (bIndex === -1 ? 999 : bIndex);
    });
  }, [selectedFields]);
  const filledSelectedCount = useMemo(
    () => selectedFieldList.filter((selection) => hasFieldValue(selection.value)).length,
    [selectedFieldList]
  );

  const buildSearchValues = useCallback((source: ScraperSource | null, currentBook: Book) => {
    const values: Record<string, string> = {};
    getSearchFields(source).forEach((field) => {
      values[field.key] = getBookDefaultValue(currentBook, field);
    });
    return values;
  }, []);

  const buildSearchValuesBySource = useCallback((sourceList: ScraperSource[], currentBook: Book) => {
    return Object.fromEntries(
      sourceList.map((source) => [source.id, buildSearchValues(source, currentBook)])
    );
  }, [buildSearchValues]);

  const loadInitialData = useCallback(async () => {
    try {
      setLoading(true);
      setError('');

      const [bookRes, sourcesRes, librariesRes] = await Promise.all([
        apiClient.get(`/api/books/${bookId}`),
        apiClient.get('/api/scraper/sources'),
        apiClient.get('/api/libraries'),
      ]);

      const currentBook = bookRes.data as Book;
      const enabledSources = ((sourcesRes.data.sources || []) as ScraperSource[])
        .filter((source) => source.enabled);
      const libraries = (librariesRes.data || []) as Library[];
      const currentLibrary = libraries.find((library) => library.id === currentBook.libraryId);
      const defaultEnabledIds = getDefaultEnabledSourceIds(
        enabledSources,
        currentLibrary?.scraperConfig as LibraryScraperConfig | undefined
      );
      const firstSource = enabledSources.find((source) => defaultEnabledIds.has(source.id)) || enabledSources[0] || null;

      setBook(currentBook);
      setSources(enabledSources);
      setActiveSourceId(firstSource?.id || '');
      setEnabledSourceIds(defaultEnabledIds);
      setSearchValuesBySourceId(buildSearchValuesBySource(enabledSources, currentBook));
    } catch (err) {
      console.error('获取刮削信息失败', err);
      setError('加载失败');
    } finally {
      setLoading(false);
    }
  }, [bookId, buildSearchValuesBySource]);

  useEffect(() => {
    loadInitialData();
  }, [loadInitialData]);

  const clearSearchResults = () => {
    setResults([]);
    setSelectedResultIndex(null);
    setSearchErrors({});
    setExpandedDescriptions(new Set());
    setResultView('list');
  };

  const fillEmptyValuesFromPreviousSource = (
    sourceId: string,
    previousValues: Record<string, string>,
    currentValuesBySource: Record<string, Record<string, string>>
  ) => {
    const source = sources.find((item) => item.id === sourceId) || null;
    if (!source || !book) return currentValuesBySource;

    const existingValues = currentValuesBySource[sourceId] || buildSearchValues(source, book);
    const nextValues = { ...existingValues };
    getSearchFields(source).forEach((field) => {
      if (!nextValues[field.key]?.trim() && previousValues[field.key]?.trim()) {
        nextValues[field.key] = previousValues[field.key];
      }
    });

    return {
      ...currentValuesBySource,
      [sourceId]: nextValues,
    };
  };

  const handleActiveSourceChange = (sourceId: string) => {
    const previousValues = searchValuesBySourceId[activeSourceId] || {};
    setActiveSourceId(sourceId);
    setSearchValuesBySourceId((prev) => fillEmptyValuesFromPreviousSource(sourceId, previousValues, prev));
  };

  const toggleSourceEnabled = (sourceId: string) => {
    const previousValues = searchValuesBySourceId[activeSourceId] || {};

    setEnabledSourceIds((prev) => {
      const next = new Set(prev);
      if (next.has(sourceId)) {
        next.delete(sourceId);
      } else {
        next.add(sourceId);
      }
      return next;
    });

    setActiveSourceId(sourceId);
    setSearchValuesBySourceId((prev) => fillEmptyValuesFromPreviousSource(sourceId, previousValues, prev));
    clearSearchResults();
    setStep('search');
  };

  const updateSearchValue = (sourceId: string, fieldKey: string, value: string) => {
    const source = sources.find((item) => item.id === sourceId) || null;
    const field = getSearchFields(source).find((item) => item.key === fieldKey);
    const sharedKind = field ? getSharedSearchFieldKind(field) : null;

    setSearchValuesBySourceId((prev) => {
      if (!sharedKind) {
        return {
          ...prev,
          [sourceId]: {
            ...(prev[sourceId] || {}),
            [fieldKey]: value,
          },
        };
      }

      const next = { ...prev };
      sources.forEach((item) => {
        const currentValues = next[item.id] || (book ? buildSearchValues(item, book) : {});
        let nextValues = currentValues;

        getSearchFields(item).forEach((searchField) => {
          if (getSharedSearchFieldKind(searchField) !== sharedKind) return;
          if (nextValues === currentValues) {
            nextValues = { ...currentValues };
          }
          nextValues[searchField.key] = value;
        });

        if (nextValues !== currentValues) {
          next[item.id] = nextValues;
        }
      });

      if (!next[sourceId]?.[fieldKey] && fieldKey) {
        next[sourceId] = {
          ...(next[sourceId] || {}),
          [fieldKey]: value,
        };
      }

      return next;
    });
    clearSearchResults();
  };

  const openResultDetail = (index: number) => {
    setSelectedResultIndex(index);
    setResultView('detail');
  };

  const handleSearch = async () => {
    if (enabledSearchSources.length === 0) {
      alert('请至少启用一个插件');
      return;
    }

    for (const source of enabledSearchSources) {
      const values = searchValuesBySourceId[source.id] || {};
      const missingRequired = getSearchFields(source).find((field) => field.required && !values[field.key]?.trim());
      if (missingRequired) {
        setActiveSourceId(source.id);
        setStep('search');
        alert(`${source.name} 的 ${missingRequired.label}不能为空`);
        return;
      }
    }

    try {
      setSearching(true);
      setError('');
      setSearchErrors({});
      const responses = await Promise.all(
        enabledSearchSources.map(async (source) => {
          try {
            const res = await apiClient.post('/api/scraper/search', {
              source: source.id,
              searchParams: searchValuesBySourceId[source.id] || {},
              page: 1,
              pageSize: 20,
            });
            return {
              source,
              items: (res.data.items || []) as ScraperSearchItem[],
              error: '',
            };
          } catch (err) {
            console.error(`${source.name} 搜索刮削结果失败`, err);
            return {
              source,
              items: [] as ScraperSearchItem[],
              error: '搜索失败',
            };
          }
        })
      );

      const titleMatchTerms = getTitleMatchTerms(book, enabledSearchSources, searchValuesBySourceId);
      const nextResults = responses
        .flatMap((response, sourceIndex) =>
          response.items.map((item, resultIndex) => ({
            item,
            source: response.source,
            resultIndex,
            matchScore: getTitleMatchScore(getItemFieldValue(item, 'title'), titleMatchTerms),
            originalOrder: sourceIndex * 10000 + resultIndex,
          }))
        )
        .sort((a, b) => b.matchScore - a.matchScore || a.originalOrder - b.originalOrder)
        .map((result) => ({
          item: result.item,
          source: result.source,
          resultIndex: result.resultIndex,
        }));
      const nextErrors = Object.fromEntries(
        responses
          .filter((response) => response.error)
          .map((response) => [response.source.id, response.error])
      );

      setResults(nextResults);
      setSearchErrors(nextErrors);
      setSelectedResultIndex(null);
      setResultView('list');
      setExpandedDescriptions(new Set());
      setStep('results');
    } catch (err) {
      console.error('搜索刮削结果失败', err);
      setResults([]);
      setSelectedResultIndex(null);
      setResultView('list');
      setError('搜索失败');
      setSearchErrors({});
      setStep('results');
    } finally {
      setSearching(false);
    }
  };

  const selectField = (result: ScrapeSearchResult, fieldKey: string) => {
    const item = result.item;
    const source = result.source;

    const value = getItemFieldValue(item, fieldKey);
    if (!hasFieldValue(value)) return;

    const definition = FIELD_DEFINITIONS[fieldKey];
    const resultKey = getResultKey(result);
    const resultId = getResultExternalId(result);
    setSelectedFields((prev) => ({
      ...prev,
      [fieldKey]: {
        key: fieldKey,
        label: definition.label,
        value: value as Exclude<FieldValue, null | undefined>,
        sourceId: source.id,
        sourceName: source.name,
        resultId,
        resultKey,
        resultTitle: item.title || resultId,
      },
    }));
  };

  const selectAllAvailableFields = (result: ScrapeSearchResult) => {
    const item = result.item;
    const source = result.source;

    setSelectedFields((prev) => {
      const next = { ...prev };
      getResultFields(source).forEach((fieldKey) => {
        const value = getItemFieldValue(item, fieldKey);
        if (!hasFieldValue(value)) return;

        const definition = FIELD_DEFINITIONS[fieldKey];
        const resultKey = getResultKey(result);
        const resultId = getResultExternalId(result);
        next[fieldKey] = {
          key: fieldKey,
          label: definition.label,
          value: value as Exclude<FieldValue, null | undefined>,
          sourceId: source.id,
          sourceName: source.name,
          resultId,
          resultKey,
          resultTitle: item.title || resultId,
        };
      });
      return next;
    });
  };

  const removeSelectedField = (fieldKey: string) => {
    setSelectedFields((prev) => {
      const next = { ...prev };
      delete next[fieldKey];
      return next;
    });
  };

  const updateSelectedFieldValue = (fieldKey: string, value: string) => {
    setSelectedFields((prev) => {
      const current = prev[fieldKey];
      if (!current) return prev;

      return {
        ...prev,
        [fieldKey]: {
          ...current,
          value: editorValueForField(fieldKey, value),
        },
      };
    });
  };

  const toggleDescription = (key: string) => {
    setExpandedDescriptions((prev) => {
      const next = new Set(prev);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  };

  const isStepEnabled = (target: ModalStep) => {
    if (target === step || target === 'search') return true;
    if (target === 'results') return results.length > 0 || error === '搜索失败' || Object.keys(searchErrors).length > 0;
    return selectedCount > 0;
  };

  const handleApply = async () => {
    if (!book || filledSelectedCount === 0) return;

    try {
      setSaving(true);
      const fields = Object.fromEntries(
        Object.entries(selectedFields)
          .filter(([, selection]) => hasFieldValue(selection.value))
          .map(([key, selection]) => [
            key,
            {
              value: fieldValueForApi(selection.value),
              source: selection.sourceId,
              externalId: selection.resultId,
            },
          ])
      );

      await apiClient.post(`/api/books/${bookId}/scrape-apply`, {
        fields,
        applyMetadata: true,
      });
      onSave();
      onClose();
    } catch (err) {
      console.error('应用刮削结果失败', err);
      alert('应用失败');
    } finally {
      setSaving(false);
    }
  };

  if (loading) {
    return (
      <div className="fixed inset-0 z-[200] flex items-center justify-center p-4">
        <div className="absolute inset-0 bg-slate-900/60 backdrop-blur-sm" />
        <div className="relative flex flex-col items-center gap-4 rounded-2xl bg-white p-8 shadow-2xl dark:bg-slate-900">
          <Loader2 className="animate-spin text-primary-600" size={40} />
          <p className="font-bold text-slate-600 dark:text-slate-400">正在加载...</p>
        </div>
      </div>
    );
  }

  if (!book || sources.length === 0) {
    return (
      <div className="fixed inset-0 z-[200] flex items-center justify-center p-4">
        <div className="absolute inset-0 bg-slate-900/60 backdrop-blur-sm" onClick={onClose} />
        <div className="relative flex max-w-sm flex-col items-center gap-4 rounded-2xl bg-white p-8 text-center shadow-2xl dark:bg-slate-900">
          <AlertTriangle className="text-yellow-500" size={40} />
          <h3 className="text-xl font-bold dark:text-white">{error || '没有可用插件'}</h3>
          <button onClick={onClose} className="mt-2 rounded-xl bg-slate-100 px-6 py-2 font-bold dark:bg-slate-800">
            关闭
          </button>
        </div>
      </div>
    );
  }

  const selectedResultKey = selectedResult ? getResultKey(selectedResult) : '';
  const resultErrorCount = Object.keys(searchErrors).length;

  return (
    <div className="fixed inset-0 z-[200] flex items-center justify-center p-2 sm:p-4">
      <div className="absolute inset-0 bg-slate-900/60 backdrop-blur-sm" onClick={onClose} />
      <div className="relative flex h-[92vh] w-full max-w-6xl flex-col overflow-hidden rounded-2xl bg-white shadow-2xl animate-in zoom-in-95 duration-200 dark:bg-slate-950">
        <div className="border-b border-slate-100 px-4 py-3 dark:border-slate-800 sm:px-5">
          <div className="flex items-center justify-between gap-3">
            <h2 className="flex items-center gap-2 text-lg font-bold text-slate-950 dark:text-white sm:text-xl">
              <RefreshCw size={22} className="text-primary-600" />
              手动刮削
            </h2>
            <button onClick={onClose} className="rounded-full p-2 transition-colors hover:bg-slate-100 dark:hover:bg-slate-800">
              <X size={22} className="text-slate-500" />
            </button>
          </div>

          <div className="mt-3 flex items-center gap-1 overflow-x-auto pb-1">
            {STEP_ITEMS.map((item, index) => {
              const active = step === item.key;
              const enabled = isStepEnabled(item.key);

              return (
                <React.Fragment key={item.key}>
                  <button
                    onClick={() => enabled && setStep(item.key)}
                    disabled={!enabled}
                    className={`flex shrink-0 items-center gap-2 rounded-lg px-3 py-2 text-xs font-bold transition-colors ${
                      active
                        ? 'bg-primary-600 text-white'
                        : enabled
                          ? 'bg-slate-100 text-slate-600 hover:bg-slate-200 dark:bg-slate-900 dark:text-slate-300 dark:hover:bg-slate-800'
                          : 'bg-slate-50 text-slate-300 dark:bg-slate-900/50 dark:text-slate-600'
                    }`}
                  >
                    <span className={`flex h-5 w-5 items-center justify-center rounded-full text-[11px] ${
                      active ? 'bg-white/20' : 'bg-white dark:bg-slate-800'
                    }`}>
                      {index + 1}
                    </span>
                    {item.label}
                  </button>
                  {index < STEP_ITEMS.length - 1 ? <ChevronRight size={16} className="shrink-0 text-slate-300" /> : null}
                </React.Fragment>
              );
            })}
          </div>
        </div>

        <div className="min-h-0 flex-1 overflow-hidden bg-slate-50/70 dark:bg-slate-950">
          {step === 'search' ? (
            <SearchStep
              book={book}
              sources={sources}
              activeSourceId={activeSourceId}
              activeSource={activeSource}
              enabledSourceIds={enabledSourceIds}
              enabledSearchSources={enabledSearchSources}
              searchFields={searchFields}
              activeSearchValues={activeSearchValues}
              activeResultFields={activeResultFields}
              selectedFields={selectedFields}
              error={error}
              onToggleSourceEnabled={toggleSourceEnabled}
              onActiveSourceChange={handleActiveSourceChange}
              onUpdateSearchValue={updateSearchValue}
            />
          ) : null}

          {step === 'results' ? (
            <ResultsStep
              book={book}
              results={results}
              resultView={resultView}
              selectedResult={selectedResult}
              selectedResultItem={selectedResultItem}
              selectedResultSource={selectedResultSource}
              selectedResultKey={selectedResultKey}
              selectedResultFields={selectedResultFields}
              selectedFields={selectedFields}
              selectedFieldList={selectedFieldList}
              selectedCount={selectedCount}
              expandedDescriptions={expandedDescriptions}
              searching={searching}
              error={error}
              resultErrorCount={resultErrorCount}
              enabledSearchSources={enabledSearchSources}
              coverAspectClass={coverAspectClass}
              compactCoverClass={compactCoverClass}
              onSetResultView={setResultView}
              onSelectAllAvailableFields={selectAllAvailableFields}
              onSelectField={selectField}
              onToggleDescription={toggleDescription}
              onOpenResultDetail={openResultDetail}
              onBackToSearch={() => setStep('search')}
            />
          ) : null}

          {step === 'review' ? (
            <ReviewStep
              selectedFieldList={selectedFieldList}
              selectedCount={selectedCount}
              coverAspectClass={coverAspectClass}
              onClearSelectedFields={() => setSelectedFields({})}
              onRemoveSelectedField={removeSelectedField}
              onUpdateSelectedFieldValue={updateSelectedFieldValue}
            />
          ) : null}
        </div>

        <div className="flex flex-col gap-3 border-t border-slate-100 px-4 py-3 dark:border-slate-800 sm:flex-row sm:items-center sm:justify-between sm:px-5">
          <div className="text-sm font-bold text-slate-500">
            已选择 {selectedCount} 个字段
          </div>

          <div className="flex flex-wrap justify-end gap-2">
            {step === 'search' ? (
              <>
                <button
                  onClick={onClose}
                  className="rounded-xl px-5 py-2.5 font-bold text-slate-500 transition-colors hover:bg-slate-100 dark:hover:bg-slate-800"
                >
                  取消
                </button>
                <button
                  onClick={handleSearch}
                  disabled={searching || enabledSearchSources.length === 0}
                  className="inline-flex items-center justify-center gap-2 rounded-xl bg-primary-600 px-6 py-2.5 font-bold text-white transition-colors hover:bg-primary-700 disabled:opacity-60"
                >
                  {searching ? <Loader2 className="animate-spin" size={18} /> : <Search size={18} />}
                  搜索 {enabledSearchSources.length} 个插件
                </button>
              </>
            ) : null}

            {step === 'results' ? (
              <>
                <button
                  onClick={() => {
                    if (resultView === 'detail') {
                      setResultView('list');
                    } else {
                      setStep('search');
                    }
                  }}
                  className="inline-flex items-center justify-center gap-2 rounded-xl px-5 py-2.5 font-bold text-slate-500 transition-colors hover:bg-slate-100 dark:hover:bg-slate-800"
                >
                  <ArrowLeft size={17} />
                  {resultView === 'detail' ? '搜索结果' : '搜索条件'}
                </button>
                <button
                  onClick={() => setStep('review')}
                  disabled={selectedCount === 0}
                  className="inline-flex items-center justify-center gap-2 rounded-xl bg-primary-600 px-6 py-2.5 font-bold text-white transition-colors hover:bg-primary-700 disabled:opacity-50"
                >
                  确认应用
                  <ArrowRight size={17} />
                </button>
              </>
            ) : null}

            {step === 'review' ? (
              <>
                <button
                  onClick={() => setStep(results.length > 0 ? 'results' : 'search')}
                  className="inline-flex items-center justify-center gap-2 rounded-xl px-5 py-2.5 font-bold text-slate-500 transition-colors hover:bg-slate-100 dark:hover:bg-slate-800"
                >
                  <ArrowLeft size={17} />
                  返回
                </button>
                <button
                  onClick={handleApply}
                  disabled={saving || filledSelectedCount === 0}
                  className="inline-flex items-center justify-center gap-2 rounded-xl bg-primary-600 px-6 py-2.5 font-bold text-white transition-colors hover:bg-primary-700 disabled:opacity-50"
                >
                  {saving ? <Loader2 className="animate-spin" size={18} /> : <Save size={18} />}
                  应用
                </button>
              </>
            ) : null}
          </div>
        </div>
      </div>
    </div>
  );
};

export default ScrapeDiffModal;
