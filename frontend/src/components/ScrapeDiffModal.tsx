import React, { useCallback, useEffect, useMemo, useState } from 'react';
import apiClient from '../api/client';
import type { Book, Library, ScraperSearchField, ScraperSearchItem, ScraperSource } from '../types';
import {
  AlertTriangle,
  ArrowLeft,
  ArrowRight,
  BadgeCheck,
  BookOpen,
  Building2,
  Calendar,
  Check,
  ChevronRight,
  FileText,
  Globe2,
  Hash,
  Image as ImageIcon,
  Loader2,
  Mic2,
  Plus,
  RefreshCw,
  Save,
  Search,
  Tags,
  Trash2,
  User,
  X,
} from 'lucide-react';
import { getCoverUrl } from '../utils/image';
import { getCoverAspectClass, useBookshelfCoverShape } from '../hooks/useBookshelfCoverShape';

interface Props {
  bookId: string;
  onClose: () => void;
  onSave: () => void;
}

type FieldValue = string | number | boolean | string[] | null | undefined;
type ModalStep = 'search' | 'results' | 'review';
type ResultView = 'list' | 'detail';

interface FieldDefinition {
  key: string;
  label: string;
  icon: React.ReactNode;
  wide?: boolean;
  cover?: boolean;
}

interface SelectedField {
  key: string;
  label: string;
  value: Exclude<FieldValue, null | undefined>;
  sourceId: string;
  sourceName: string;
  resultId: string;
  resultKey: string;
  resultTitle: string;
}

interface ScrapeSearchResult {
  item: ScraperSearchItem;
  source: ScraperSource;
  resultIndex: number;
}

interface LibraryScraperConfig {
  defaultSources?: string[];
  default_sources?: string[];
  coverSources?: string[];
  cover_sources?: string[];
  introSources?: string[];
  intro_sources?: string[];
  authorSources?: string[];
  author_sources?: string[];
  narratorSources?: string[];
  narrator_sources?: string[];
  tagsSources?: string[];
  tags_sources?: string[];
}

interface CoverFrameProps {
  value: FieldValue;
  alt: string;
  book?: Book;
  className?: string;
}

const DEFAULT_SEARCH_FIELDS: ScraperSearchField[] = [
  { key: 'title', label: '书名', required: true, defaultFrom: 'book.title' },
  { key: 'author', label: '作者', required: false, defaultFrom: 'book.author' },
  { key: 'narrator', label: '演播', required: false, defaultFrom: 'book.narrator' },
];

const DEFAULT_RESULT_FIELDS = [
  'title',
  'author',
  'narrator',
  'cover_url',
  'description',
  'tags',
  'genre',
  'year',
  'subtitle',
  'published_date',
  'publisher',
  'isbn',
  'asin',
  'language',
  'explicit',
  'abridged',
  'duration',
];

const FIELD_DEFINITIONS: Record<string, FieldDefinition> = {
  title: { key: 'title', label: '书名', icon: <BookOpen size={15} /> },
  author: { key: 'author', label: '作者', icon: <User size={15} /> },
  narrator: { key: 'narrator', label: '演播', icon: <Mic2 size={15} /> },
  cover_url: { key: 'cover_url', label: '封面', icon: <ImageIcon size={15} />, cover: true, wide: true },
  description: { key: 'description', label: '简介', icon: <FileText size={15} />, wide: true },
  tags: { key: 'tags', label: '标签', icon: <Tags size={15} />, wide: true },
  genre: { key: 'genre', label: '类型', icon: <Tags size={15} /> },
  year: { key: 'year', label: '年份', icon: <BookOpen size={15} /> },
  subtitle: { key: 'subtitle', label: '副标题', icon: <FileText size={15} /> },
  published_date: { key: 'published_date', label: '发布日期', icon: <Calendar size={15} /> },
  publisher: { key: 'publisher', label: '出版社', icon: <Building2 size={15} /> },
  isbn: { key: 'isbn', label: 'ISBN', icon: <Hash size={15} /> },
  asin: { key: 'asin', label: 'ASIN', icon: <Hash size={15} /> },
  language: { key: 'language', label: '语言', icon: <Globe2 size={15} /> },
  explicit: { key: 'explicit', label: '成人内容', icon: <BadgeCheck size={15} /> },
  abridged: { key: 'abridged', label: '删节版', icon: <BadgeCheck size={15} /> },
  duration: { key: 'duration', label: '总时长', icon: <Hash size={15} /> },
};

const FIELD_ORDER = Object.keys(FIELD_DEFINITIONS);

const STEP_ITEMS: Array<{ key: ModalStep; label: string }> = [
  { key: 'search', label: '搜索条件' },
  { key: 'results', label: '选择字段' },
  { key: 'review', label: '确认应用' },
];

const normalizeFieldKey = (key: string) => {
  const normalized = key.replace(/[A-Z]/g, (m) => `_${m.toLowerCase()}`);
  if (normalized === 'cover_url') return 'cover_url';
  if (normalized === 'intro') return 'description';
  if (normalized === 'published_year') return 'year';
  if (normalized === 'published_date') return 'published_date';
  return normalized;
};

const getSearchFields = (source?: ScraperSource | null) => {
  return source?.searchFields?.length ? source.searchFields : DEFAULT_SEARCH_FIELDS;
};

const getSharedSearchFieldKind = (field: ScraperSearchField) => {
  const from = field.defaultFrom || `book.${field.key}`;
  if (field.key === 'title' || field.key === 'query' || from === 'book.title') return 'title';
  if (field.key === 'author' || from === 'book.author') return 'author';
  if (field.key === 'narrator' || from === 'book.narrator') return 'narrator';
  return null;
};

const getResultFields = (source?: ScraperSource | null) => {
  const fields = source?.resultFields?.length ? source.resultFields : DEFAULT_RESULT_FIELDS;
  return Array.from(new Set(fields.map(normalizeFieldKey))).filter((key) => FIELD_DEFINITIONS[key]);
};

const getBookDefaultValue = (book: Book, field: ScraperSearchField) => {
  const from = field.defaultFrom || `book.${field.key}`;
  if (from === 'book.title' || field.key === 'title' || field.key === 'query') return book.title || '';
  if (from === 'book.author' || field.key === 'author') return book.author || '';
  if (from === 'book.narrator' || field.key === 'narrator') return book.narrator || '';
  return '';
};

const TITLE_MATCH_PUNCTUATION_PATTERN = /[\s\u3000"'`‘’“”.,，。:：;；!?！？、·•・《》<>〈〉【】[\]（）(){}｛｝\-—–_/\\|+*=#￥$%^&~…]+/g;

const normalizeTitleForMatch = (value?: FieldValue) => {
  if (!hasFieldValue(value) || Array.isArray(value)) return '';
  return String(value)
    .normalize('NFKC')
    .toLowerCase()
    .replace(TITLE_MATCH_PUNCTUATION_PATTERN, '');
};

const getTitleMatchTerms = (
  currentBook: Book | null,
  sourceList: ScraperSource[],
  valuesBySourceId: Record<string, Record<string, string>>
) => {
  const terms: string[] = [];
  const seen = new Set<string>();
  const addTerm = (value?: FieldValue) => {
    if (!hasFieldValue(value) || Array.isArray(value)) return;
    String(value)
      .split(/[|丨]/)
      .map((item) => item.trim())
      .filter(Boolean)
      .forEach((item) => {
        const normalized = normalizeTitleForMatch(item);
        if (!normalized || seen.has(normalized)) return;
        seen.add(normalized);
        terms.push(item);
      });
  };

  sourceList.forEach((source) => {
    const values = valuesBySourceId[source.id] || {};
    getSearchFields(source).forEach((field) => {
      if (getSharedSearchFieldKind(field) === 'title') {
        addTerm(values[field.key]);
      }
    });
  });
  addTerm(currentBook?.title);

  return terms;
};

const getCommonPrefixLength = (a: string, b: string) => {
  const max = Math.min(a.length, b.length);
  let length = 0;
  while (length < max && a[length] === b[length]) {
    length += 1;
  }
  return length;
};

const getCharacterOverlap = (a: string, b: string) => {
  const counts = new Map<string, number>();
  for (const char of a) {
    counts.set(char, (counts.get(char) || 0) + 1);
  }

  let overlap = 0;
  for (const char of b) {
    const count = counts.get(char) || 0;
    if (count > 0) {
      overlap += 1;
      counts.set(char, count - 1);
    }
  }

  return overlap;
};

const getTitleMatchScore = (candidate: FieldValue, terms: string[]) => {
  const normalizedCandidate = normalizeTitleForMatch(candidate);
  if (!normalizedCandidate) return 0;

  return Math.max(
    0,
    ...terms.map((term) => {
      const normalizedTerm = normalizeTitleForMatch(term);
      if (!normalizedTerm) return 0;
      if (normalizedCandidate === normalizedTerm) return 100000;

      const lengthRatio = Math.min(normalizedCandidate.length, normalizedTerm.length)
        / Math.max(normalizedCandidate.length, normalizedTerm.length);
      if (normalizedCandidate.includes(normalizedTerm) || normalizedTerm.includes(normalizedCandidate)) {
        return 80000 + Math.round(lengthRatio * 10000);
      }

      const overlap = getCharacterOverlap(normalizedCandidate, normalizedTerm);
      const diceScore = Math.round((2 * overlap / (normalizedCandidate.length + normalizedTerm.length)) * 70000);
      const prefixScore = Math.round(
        (getCommonPrefixLength(normalizedCandidate, normalizedTerm) / Math.max(normalizedCandidate.length, normalizedTerm.length)) * 10000
      );
      return diceScore + prefixScore;
    })
  );
};

const getItemFieldValue = (item: ScraperSearchItem, fieldKey: string): FieldValue => {
  switch (fieldKey) {
    case 'title':
      return item.title;
    case 'author':
      return item.author;
    case 'narrator':
      return item.narrator;
    case 'cover_url':
      return item.coverUrl || item.cover_url;
    case 'description':
      return item.description || item.intro;
    case 'tags':
      return Array.isArray(item.tags) ? item.tags : undefined;
    case 'genre':
      return item.genre;
    case 'year':
      return item.publishedYear || item.published_year;
    case 'subtitle':
      return item.subtitle;
    case 'published_date':
      return item.publishedDate || item.published_date;
    case 'publisher':
      return item.publisher;
    case 'isbn':
      return item.isbn;
    case 'asin':
      return item.asin;
    case 'language':
      return item.language;
    case 'explicit':
      return item.explicit;
    case 'abridged':
      return item.abridged;
    case 'duration':
      return item.duration;
    default:
      return item[fieldKey] as FieldValue;
  }
};

const getBookFieldValue = (book: Book, fieldKey: string): FieldValue => {
  switch (fieldKey) {
    case 'title':
      return book.title;
    case 'author':
      return book.author;
    case 'narrator':
      return book.narrator;
    case 'cover_url':
      return book.coverUrl;
    case 'description':
      return book.description;
    case 'tags':
      return book.tags ? book.tags.split(',').map((tag) => tag.trim()).filter(Boolean) : undefined;
    case 'genre':
      return book.genre;
    case 'year':
      return book.year;
    default:
      return undefined;
  }
};

const hasFieldValue = (value: FieldValue) => {
  if (Array.isArray(value)) return value.length > 0;
  if (typeof value === 'string') return value.trim().length > 0;
  return value !== null && value !== undefined;
};

const getDraftBookFieldValue = (
  book: Book,
  selectedFields: Record<string, SelectedField>,
  fieldKey: string
): FieldValue => {
  return selectedFields[fieldKey]?.value ?? getBookFieldValue(book, fieldKey);
};

const formatFieldValue = (value: FieldValue, emptyLabel = '未返回') => {
  if (!hasFieldValue(value)) return emptyLabel;
  if (Array.isArray(value)) return value.join(' / ');
  if (typeof value === 'boolean') return value ? '是' : '否';
  return String(value);
};

const formatCurrentValue = (value: FieldValue) => formatFieldValue(value, '未知');

const fieldValueForApi = (value: Exclude<FieldValue, null | undefined>) => {
  return Array.isArray(value) || typeof value === 'boolean' || typeof value === 'number' ? value : String(value);
};

const fieldValueForEditor = (value: FieldValue) => {
  if (!hasFieldValue(value)) return '';
  if (Array.isArray(value)) return value.join(', ');
  return String(value);
};

const editorValueForField = (fieldKey: string, value: string): Exclude<FieldValue, null | undefined> => {
  if (fieldKey === 'tags') {
    return value.split(',').map((tag) => tag.trim()).filter(Boolean);
  }
  if (fieldKey === 'explicit' || fieldKey === 'abridged') {
    return value === 'true';
  }
  if (fieldKey === 'duration') {
    return value;
  }
  return value;
};

const getConfiguredSourceIds = (config?: LibraryScraperConfig | null) => {
  if (!config) return new Set<string>();

  const keys: Array<keyof LibraryScraperConfig> = [
    'defaultSources',
    'default_sources',
    'coverSources',
    'cover_sources',
    'introSources',
    'intro_sources',
    'authorSources',
    'author_sources',
    'narratorSources',
    'narrator_sources',
    'tagsSources',
    'tags_sources',
  ];

  return new Set(
    keys.flatMap((key) => {
      const value = config[key];
      return Array.isArray(value) ? value.filter((id): id is string => typeof id === 'string' && id.trim().length > 0) : [];
    })
  );
};

const getDefaultEnabledSourceIds = (sourceList: ScraperSource[], config?: LibraryScraperConfig | null) => {
  const configuredIds = getConfiguredSourceIds(config);
  if (configuredIds.size === 0) return new Set<string>();

  return new Set(
    sourceList
      .filter((source) => source.autoScrape && configuredIds.has(source.id))
      .map((source) => source.id)
  );
};

const getResultKey = (result: ScrapeSearchResult) =>
  `${result.source.id}:${result.item.id || 'result'}:${result.resultIndex}`;

const getResultExternalId = (result: ScrapeSearchResult) =>
  result.item.id || `result-${result.resultIndex + 1}`;

const getSearchInputType = (field: ScraperSearchField) => {
  const fieldType = field.type || field.fieldType;
  if (fieldType === 'number') return 'number';
  return 'text';
};

const CoverFrame: React.FC<CoverFrameProps> = ({ value, alt, book, className = '' }) => {
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
                                  onClick={() => toggleSourceEnabled(source.id)}
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
                                  onClick={() => handleActiveSourceChange(source.id)}
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
                            onClick={() => toggleSourceEnabled(activeSource.id)}
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
                              onChange={(event) => updateSearchValue(activeSourceId, field.key, event.target.value)}
                              placeholder={field.placeholder || ''}
                              disabled={!activeSource}
                              className="w-full rounded-xl border border-slate-200 bg-slate-50 px-3 py-3 text-sm outline-none transition focus:ring-2 focus:ring-primary-500 disabled:opacity-60 dark:border-slate-800 dark:bg-slate-950 dark:text-white"
                            />
                          </div>
                        ))}
                      </div>
                    </div>
                  </div>

                  {error && step === 'search' ? (
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
          ) : null}

          {step === 'results' ? (
            resultView === 'detail' && selectedResult && selectedResultItem && selectedResultSource ? (
              <div className="h-full overflow-y-auto p-3 sm:p-5">
                <div className="mx-auto max-w-4xl space-y-4">
                  <button
                    onClick={() => setResultView('list')}
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
                        onClick={() => selectAllAvailableFields(selectedResult)}
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
                              onClick={() => selectField(selectedResult, fieldKey)}
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
                                    onClick={() => toggleDescription(expandedKey)}
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
            ) : (
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
                      onClick={() => setStep('search')}
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
                              onClick={() => openResultDetail(index)}
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
            )
          ) : null}

          {step === 'review' ? (
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
                        onClick={() => setSelectedFields({})}
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
                            onClick={() => removeSelectedField(selection.key)}
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
                                onChange={(event) => updateSelectedFieldValue(selection.key, event.target.value)}
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
                                onChange={(event) => updateSelectedFieldValue(selection.key, event.target.value)}
                                className="w-full rounded-xl border border-slate-200 bg-primary-50 px-3 py-3 text-sm font-medium text-slate-950 outline-none transition focus:ring-2 focus:ring-primary-500 dark:border-slate-800 dark:bg-primary-950/25 dark:text-white"
                              >
                                <option value="true">是</option>
                                <option value="false">否</option>
                              </select>
                            ) : isLongText ? (
                              <textarea
                                value={editorValue}
                                onChange={(event) => updateSelectedFieldValue(selection.key, event.target.value)}
                                rows={selection.key === 'description' ? 6 : 3}
                                className="w-full resize-y rounded-xl border border-slate-200 bg-primary-50 px-3 py-3 text-sm font-medium leading-relaxed text-slate-950 outline-none transition focus:ring-2 focus:ring-primary-500 dark:border-slate-800 dark:bg-primary-950/25 dark:text-white"
                              />
                            ) : (
                              <input
                                type={selection.key === 'year' ? 'number' : 'text'}
                                value={editorValue}
                                onChange={(event) => updateSelectedFieldValue(selection.key, event.target.value)}
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
