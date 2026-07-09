import type { Book, Library } from '../../../core/types';
import type { EditableChapter } from './types';

const decodeSegment = (segment: string) => {
  try {
    return decodeURIComponent(segment);
  } catch {
    return segment;
  }
};

const decodePathBySegment = (path: string) => (
  path.split('/').map(decodeSegment).join('/')
);

const stripWindowsVerbatimPrefix = (path: string) => {
  if (path.startsWith('\\\\?\\UNC\\')) return `//${path.slice(8)}`;
  if (path.startsWith('//?/UNC/')) return `//${path.slice(8)}`;
  if (path.startsWith('\\\\?\\')) return path.slice(4);
  if (path.startsWith('//?/')) return path.slice(4);
  return path;
};

const normalizePath = (path: string) => (
  stripWindowsVerbatimPrefix(path)
    .replace(/\\/g, '/')
    .replace(/([^:])\/{2,}/g, '$1/')
    .replace(/\/+$/g, '')
);

const formatRemoteDisplayPath = (path: string) => {
  const trimmed = path.trim();
  if (!trimmed) return '';

  try {
    const url = new URL(trimmed);
    const decodedPath = decodePathBySegment(url.pathname);
    return normalizePath(`${url.origin}${decodedPath}${url.search}${url.hash}`);
  } catch {
    return normalizePath(decodePathBySegment(trimmed.replace(/\\/g, '/')));
  }
};

export const formatChapterDisplayPath = (
  chapter: EditableChapter,
  _book: Book,
  pathLibrary: Library | null,
) => {
  const path = chapter.path || '';
  if (pathLibrary?.library_type === 'local') {
    return normalizePath(path);
  }

  return formatRemoteDisplayPath(path);
};

export const getRelativeChapterPath = (
  chapterPath: string,
  book: Book,
  pathLibrary: Library | null,
) => formatChapterDisplayPath({ path: chapterPath } as EditableChapter, book, pathLibrary);

export const formatChapterLocation = (
  chapter: EditableChapter,
  book: Book,
  pathLibrary: Library | null,
) => formatChapterDisplayPath(chapter, book, pathLibrary);
