// ScrapeDiff 模态框内部用的类型、常量、字段定义与纯工具函数。
// 含 JSX 的部分仅限 FIELD_DEFINITIONS 里的 lucide 图标。
/* eslint-disable react-refresh/only-export-components */

import React from 'react';
import {
  BadgeCheck,
  BookOpen,
  Building2,
  Calendar,
  FileText,
  Globe2,
  Hash,
  Image as ImageIcon,
  Mic2,
  Tags,
  User,
} from 'lucide-react';
import type {
  Book,
  ScraperSearchField,
  ScraperSearchItem,
  ScraperSource,
} from '../../core/types';

// ─── 类型 ────────────────────────────────────────────────────────────────────

export type FieldValue = string | number | boolean | string[] | null | undefined;
export type ModalStep = 'search' | 'results' | 'review';
export type ResultView = 'list' | 'detail';

export interface FieldDefinition {
  key: string;
  label: string;
  icon: React.ReactNode;
  wide?: boolean;
  cover?: boolean;
}

export interface SelectedField {
  key: string;
  label: string;
  value: Exclude<FieldValue, null | undefined>;
  sourceId: string;
  sourceName: string;
  resultId: string;
  resultKey: string;
  resultTitle: string;
}

export interface ScrapeSearchResult {
  item: ScraperSearchItem;
  source: ScraperSource;
  resultIndex: number;
}

export interface LibraryScraperConfig {
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

export interface CoverFrameProps {
  value: FieldValue;
  alt: string;
  book?: Book;
  className?: string;
}

// ─── 字段定义 ────────────────────────────────────────────────────────────────

export const FIELD_DEFINITIONS: Record<string, FieldDefinition> = {
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

export const FIELD_ORDER = Object.keys(FIELD_DEFINITIONS);

// ─── 常量 ────────────────────────────────────────────────────────────────────

export const DEFAULT_SEARCH_FIELDS: ScraperSearchField[] = [
  { key: 'title', label: '书名', required: true, defaultFrom: 'book.title' },
  { key: 'author', label: '作者', required: false, defaultFrom: 'book.author' },
  { key: 'narrator', label: '演播', required: false, defaultFrom: 'book.narrator' },
];

export const DEFAULT_RESULT_FIELDS = [
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

export const STEP_ITEMS: Array<{ key: ModalStep; label: string }> = [
  { key: 'search', label: '搜索条件' },
  { key: 'results', label: '选择字段' },
  { key: 'review', label: '确认应用' },
];

// ─── 字段 key 归一化 + 取值 ──────────────────────────────────────────────────

export const normalizeFieldKey = (key: string) => {
  const normalized = key.replace(/[A-Z]/g, (m) => `_${m.toLowerCase()}`);
  if (normalized === 'cover_url') return 'cover_url';
  if (normalized === 'intro') return 'description';
  if (normalized === 'published_year') return 'year';
  if (normalized === 'published_date') return 'published_date';
  return normalized;
};

export const getSearchFields = (source?: ScraperSource | null) => {
  return source?.searchFields?.length ? source.searchFields : DEFAULT_SEARCH_FIELDS;
};

export const getSharedSearchFieldKind = (field: ScraperSearchField) => {
  const from = field.defaultFrom || `book.${field.key}`;
  if (field.key === 'title' || field.key === 'query' || from === 'book.title') return 'title';
  if (field.key === 'author' || from === 'book.author') return 'author';
  if (field.key === 'narrator' || from === 'book.narrator') return 'narrator';
  return null;
};

export const getResultFields = (source?: ScraperSource | null) => {
  const fields = source?.resultFields?.length ? source.resultFields : DEFAULT_RESULT_FIELDS;
  return Array.from(new Set(fields.map(normalizeFieldKey))).filter((key) => FIELD_DEFINITIONS[key]);
};

export const getBookDefaultValue = (book: Book, field: ScraperSearchField) => {
  const from = field.defaultFrom || `book.${field.key}`;
  if (from === 'book.title' || field.key === 'title' || field.key === 'query') return book.title || '';
  if (from === 'book.author' || field.key === 'author') return book.author || '';
  if (from === 'book.narrator' || field.key === 'narrator') return book.narrator || '';
  return '';
};

// ─── 书名匹配评分 ────────────────────────────────────────────────────────────

const TITLE_MATCH_PUNCTUATION_PATTERN = /[\s\u3000"'`‘’“”.,，。:：;；!?！？、·•・《》<>〈〉【】[\]（）(){}｛｝\-—–_/\\|+*=#￥$%^&~…]+/g;

export const normalizeTitleForMatch = (value?: FieldValue) => {
  if (!hasFieldValue(value) || Array.isArray(value)) return '';
  return String(value)
    .normalize('NFKC')
    .toLowerCase()
    .replace(TITLE_MATCH_PUNCTUATION_PATTERN, '');
};

export const getTitleMatchTerms = (
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

export const getTitleMatchScore = (candidate: FieldValue, terms: string[]) => {
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

// ─── 字段取值 + 草稿合并 ─────────────────────────────────────────────────────

export const getItemFieldValue = (item: ScraperSearchItem, fieldKey: string): FieldValue => {
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

export const getBookFieldValue = (book: Book, fieldKey: string): FieldValue => {
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

export const hasFieldValue = (value: FieldValue) => {
  if (Array.isArray(value)) return value.length > 0;
  if (typeof value === 'string') return value.trim().length > 0;
  return value !== null && value !== undefined;
};

export const getDraftBookFieldValue = (
  book: Book,
  selectedFields: Record<string, SelectedField>,
  fieldKey: string
): FieldValue => {
  return selectedFields[fieldKey]?.value ?? getBookFieldValue(book, fieldKey);
};

// ─── 字段值格式化 ────────────────────────────────────────────────────────────

export const formatFieldValue = (value: FieldValue, emptyLabel = '未返回') => {
  if (!hasFieldValue(value)) return emptyLabel;
  if (Array.isArray(value)) return value.join(' / ');
  if (typeof value === 'boolean') return value ? '是' : '否';
  return String(value);
};

export const formatCurrentValue = (value: FieldValue) => formatFieldValue(value, '未知');

export const fieldValueForApi = (value: Exclude<FieldValue, null | undefined>) => {
  return Array.isArray(value) || typeof value === 'boolean' || typeof value === 'number' ? value : String(value);
};

export const fieldValueForEditor = (value: FieldValue) => {
  if (!hasFieldValue(value)) return '';
  if (Array.isArray(value)) return value.join(', ');
  return String(value);
};

export const editorValueForField = (fieldKey: string, value: string): Exclude<FieldValue, null | undefined> => {
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

// ─── 媒体库刮削配置 + 结果 key ───────────────────────────────────────────────

export const getConfiguredSourceIds = (config?: LibraryScraperConfig | null) => {
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

export const getDefaultEnabledSourceIds = (sourceList: ScraperSource[], config?: LibraryScraperConfig | null) => {
  const configuredIds = getConfiguredSourceIds(config);
  if (configuredIds.size === 0) return new Set<string>();

  return new Set(
    sourceList
      .filter((source) => source.autoScrape && configuredIds.has(source.id))
      .map((source) => source.id)
  );
};

export const getResultKey = (result: ScrapeSearchResult) =>
  `${result.source.id}:${result.item.id || 'result'}:${result.resultIndex}`;

export const getResultExternalId = (result: ScrapeSearchResult) =>
  result.item.id || `result-${result.resultIndex + 1}`;

export const getSearchInputType = (field: ScraperSearchField) => {
  const fieldType = field.type || field.fieldType;
  if (fieldType === 'number') return 'number';
  return 'text';
};
