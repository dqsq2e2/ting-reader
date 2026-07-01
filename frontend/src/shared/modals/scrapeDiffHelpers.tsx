// Types, constants, field definitions, and pure helpers used by ScrapeDiffModal.
// JSX is limited to lucide icons inside FIELD_DEFINITIONS.
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
  LocalizedText,
  ScraperSearchField,
  ScraperSearchItem,
  ScraperSource,
} from '../../core/types';
import i18n from '../../core/i18n';

// ─── Types ───────────────────────────────────────────────────────────────────

export type FieldValue = string | number | boolean | string[] | null | undefined;
export type ModalStep = 'search' | 'results' | 'review';
export type ResultView = 'list' | 'detail';

export interface FieldDefinition {
  key: string;
  label: string;
  labelKey?: string;
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
  default_sources?: string[];
  cover_sources?: string[];
  intro_sources?: string[];
  author_sources?: string[];
  narrator_sources?: string[];
  tags_sources?: string[];
}

export interface CoverFrameProps {
  value: FieldValue;
  alt: string;
  book?: Book;
  className?: string;
}

// ─── Field Definitions ───────────────────────────────────────────────────────

export const FIELD_DEFINITIONS: Record<string, FieldDefinition> = {
  title: { key: 'title', label: 'Title', labelKey: 'scrapeDiff.fields.title', icon: <BookOpen size={15} /> },
  author: { key: 'author', label: 'Author', labelKey: 'scrapeDiff.fields.author', icon: <User size={15} /> },
  narrator: { key: 'narrator', label: 'Narrator', labelKey: 'scrapeDiff.fields.narrator', icon: <Mic2 size={15} /> },
  cover_url: { key: 'cover_url', label: 'Cover', labelKey: 'scrapeDiff.fields.cover', icon: <ImageIcon size={15} />, cover: true, wide: true },
  description: { key: 'description', label: 'Description', labelKey: 'scrapeDiff.fields.description', icon: <FileText size={15} />, wide: true },
  tags: { key: 'tags', label: 'Tags', labelKey: 'scrapeDiff.fields.tags', icon: <Tags size={15} />, wide: true },
  genre: { key: 'genre', label: 'Genre', labelKey: 'scrapeDiff.fields.genre', icon: <Tags size={15} /> },
  year: { key: 'year', label: 'Year', labelKey: 'scrapeDiff.fields.year', icon: <BookOpen size={15} /> },
  subtitle: { key: 'subtitle', label: 'Subtitle', labelKey: 'scrapeDiff.fields.subtitle', icon: <FileText size={15} /> },
  published_date: { key: 'published_date', label: 'Published Date', labelKey: 'scrapeDiff.fields.publishedDate', icon: <Calendar size={15} /> },
  publisher: { key: 'publisher', label: 'Publisher', labelKey: 'scrapeDiff.fields.publisher', icon: <Building2 size={15} /> },
  isbn: { key: 'isbn', label: 'ISBN', labelKey: 'scrapeDiff.fields.isbn', icon: <Hash size={15} /> },
  asin: { key: 'asin', label: 'ASIN', labelKey: 'scrapeDiff.fields.asin', icon: <Hash size={15} /> },
  language: { key: 'language', label: 'Language', labelKey: 'scrapeDiff.fields.language', icon: <Globe2 size={15} /> },
  explicit: { key: 'explicit', label: 'Explicit', labelKey: 'scrapeDiff.fields.explicit', icon: <BadgeCheck size={15} /> },
  abridged: { key: 'abridged', label: 'Abridged', labelKey: 'scrapeDiff.fields.abridged', icon: <BadgeCheck size={15} /> },
  duration: { key: 'duration', label: 'Duration', labelKey: 'scrapeDiff.fields.duration', icon: <Hash size={15} /> },
};

export const FIELD_ORDER = Object.keys(FIELD_DEFINITIONS);

export type Translate = (key: string, options?: Record<string, unknown>) => string;

export const getLocalizedPluginText = (value?: LocalizedText | string | null) => {
  if (typeof value === 'string') {
    const text = value.trim();
    return text || undefined;
  }
  if (!value || typeof value !== 'object') return undefined;

  const language = (i18n.resolvedLanguage || i18n.language || 'zh-CN').toLowerCase();
  const preferredKeys = language.startsWith('en') ? ['en', 'enUS', 'en-US'] : ['zh', 'zhCN', 'zh-CN', 'zhHans', 'zh-Hans'];
  const fallbackKeys = language.startsWith('en') ? ['zh', 'zhCN', 'zh-CN', 'zhHans', 'zh-Hans'] : ['en', 'enUS', 'en-US'];
  for (const key of [...preferredKeys, ...fallbackKeys]) {
    const text = value[key]?.trim();
    if (text) return text;
  }
  return Object.values(value).find((text) => typeof text === 'string' && text.trim())?.trim();
};

const getResultFieldPluginLabel = (fieldKey: string, source?: ScraperSource | null) => {
  const labels = source?.result_field_labels;
  if (!labels) return undefined;
  const normalized = normalizeFieldKey(fieldKey);
  return (
    getLocalizedPluginText(labels[fieldKey]) ||
    getLocalizedPluginText(labels[normalized])
  );
};

export const getFieldLabel = (fieldKey: string, t?: Translate, source?: ScraperSource | null) => {
  const pluginLabel = getResultFieldPluginLabel(fieldKey, source);
  if (pluginLabel) return pluginLabel;

  const definition = FIELD_DEFINITIONS[fieldKey];
  if (!definition) return fieldKey;
  return t && definition.labelKey ? t(definition.labelKey) : definition.label;
};

const DEFAULT_FIELD_LABELS_BY_KEY: Record<string, string[]> = {
  title: ['Title', '书名'],
  author: ['Author', '作者'],
  narrator: ['Narrator', '演播', '演播者'],
};

export const getSearchFieldLabel = (field: ScraperSearchField, t?: Translate) => {
  const declaredLabel = getLocalizedPluginText(field.label_i18n);
  if (declaredLabel) return declaredLabel;

  const normalizedKey = normalizeFieldKey(field.key);
  const knownLabels = DEFAULT_FIELD_LABELS_BY_KEY[normalizedKey] || [];
  if (knownLabels.includes(field.label)) {
    return getFieldLabel(normalizedKey, t);
  }
  return field.label || getFieldLabel(normalizedKey, t);
};

export interface FormatLabels {
  trueLabel: string;
  falseLabel: string;
}

const DEFAULT_BOOLEAN_LABELS: FormatLabels = {
  trueLabel: 'Yes',
  falseLabel: 'No',
};

// ─── Constants ───────────────────────────────────────────────────────────────

export const DEFAULT_SEARCH_FIELDS: ScraperSearchField[] = [
  { key: 'title', label: 'Title', required: true, default_from: 'book.title' },
  { key: 'author', label: 'Author', required: false, default_from: 'book.author' },
  { key: 'narrator', label: 'Narrator', required: false, default_from: 'book.narrator' },
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

export const STEP_ITEMS: Array<{ key: ModalStep; label: string; labelKey: string }> = [
  { key: 'search', label: 'Search Conditions', labelKey: 'scrapeDiff.searchConditions' },
  { key: 'results', label: 'Select Fields', labelKey: 'scrapeDiff.selectFieldsStep' },
  { key: 'review', label: 'Review & Apply', labelKey: 'scrapeDiff.reviewApply' },
];

// ─── Field key normalization and values ──────────────────────────────────────

export const normalizeFieldKey = (key: string) => {
  const normalized = key.replace(/[A-Z]/g, (m) => `_${m.toLowerCase()}`);
  if (normalized === 'cover_url') return 'cover_url';
  if (normalized === 'intro') return 'description';
  if (normalized === 'published_year') return 'year';
  if (normalized === 'published_date') return 'published_date';
  return normalized;
};

export const getSearchFields = (source?: ScraperSource | null) => {
  return source?.search_fields?.length ? source.search_fields : DEFAULT_SEARCH_FIELDS;
};

export const getSharedSearchFieldKind = (field: ScraperSearchField) => {
  const from = field.default_from || `book.${field.key}`;
  if (field.key === 'title' || field.key === 'query' || from === 'book.title') return 'title';
  if (field.key === 'author' || from === 'book.author') return 'author';
  if (field.key === 'narrator' || from === 'book.narrator') return 'narrator';
  return null;
};

export const getResultFields = (source?: ScraperSource | null) => {
  const fields = source?.result_fields?.length ? source.result_fields : DEFAULT_RESULT_FIELDS;
  return Array.from(new Set(fields.map(normalizeFieldKey))).filter((key) => FIELD_DEFINITIONS[key]);
};

export const getBookDefaultValue = (book: Book, field: ScraperSearchField) => {
  const from = field.default_from || `book.${field.key}`;
  if (from === 'book.title' || field.key === 'title' || field.key === 'query') return book.title || '';
  if (from === 'book.author' || field.key === 'author') return book.author || '';
  if (from === 'book.narrator' || field.key === 'narrator') return book.narrator || '';
  return '';
};

// ─── Title match scoring ─────────────────────────────────────────────────────

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

// ─── Field values and draft merge ────────────────────────────────────────────

export const getItemFieldValue = (item: ScraperSearchItem, fieldKey: string): FieldValue => {
  switch (fieldKey) {
    case 'title':
      return item.title;
    case 'author':
      return item.author;
    case 'narrator':
      return item.narrator;
    case 'cover_url':
      return item.cover_url;
    case 'description':
      return item.description || item.intro;
    case 'tags':
      return Array.isArray(item.tags) ? item.tags : undefined;
    case 'genre':
      return item.genre;
    case 'year':
      return item.published_year;
    case 'subtitle':
      return item.subtitle;
    case 'published_date':
      return item.published_date;
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
      return book.cover_url;
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

// ─── Field value formatting ──────────────────────────────────────────────────

export const formatFieldValue = (
  value: FieldValue,
  emptyLabel = 'Not returned',
  booleanLabels: FormatLabels = DEFAULT_BOOLEAN_LABELS
) => {
  if (!hasFieldValue(value)) return emptyLabel;
  if (Array.isArray(value)) return value.join(' / ');
  if (typeof value === 'boolean') return value ? booleanLabels.trueLabel : booleanLabels.falseLabel;
  return String(value);
};

export const formatCurrentValue = (
  value: FieldValue,
  emptyLabel = 'Unknown',
  booleanLabels: FormatLabels = DEFAULT_BOOLEAN_LABELS
) => formatFieldValue(value, emptyLabel, booleanLabels);

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

// ─── Library scraper config and result keys ──────────────────────────────────

export const getConfiguredSourceIds = (config?: LibraryScraperConfig | null) => {
  if (!config) return new Set<string>();

  const keys: Array<keyof LibraryScraperConfig> = [
    'default_sources',
    'cover_sources',
    'intro_sources',
    'author_sources',
    'narrator_sources',
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
      .filter((source) => source.auto_scrape && configuredIds.has(source.id))
      .map((source) => source.id)
  );
};

export const getResultKey = (result: ScrapeSearchResult) =>
  `${result.source.id}:${result.item.id || 'result'}:${result.resultIndex}`;

export const getResultExternalId = (result: ScrapeSearchResult) =>
  result.item.id || `result-${result.resultIndex + 1}`;

export const getSearchInputType = (field: ScraperSearchField) => {
  const fieldType = field.type || field.field_type;
  if (fieldType === 'number') return 'number';
  return 'text';
};
