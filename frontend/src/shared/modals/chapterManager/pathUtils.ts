import type { Book, Library } from '../../../core/types';
import type { EditableChapter } from './types';

const safeDecode = (str: string) => {
  try {
    return decodeURIComponent(str);
  } catch {
    return str;
  }
};

const normalizePath = (path: string) => {
  return safeDecode(path)
    .replace(/\\/g, '/')
    .replace(/([^:])\/{2,}/g, '$1/')
    .replace(/\/+$/g, '');
};

const stripOuterSlashes = (path: string) => {
  return path.replace(/^\/+|\/+$/g, '');
};

const joinDisplayPath = (...parts: Array<string | undefined | null>) => {
  return parts
    .map((part) => stripOuterSlashes(part || ''))
    .filter(Boolean)
    .join('/');
};

const getPathName = (path: string) => {
  const parts = stripOuterSlashes(normalizePath(path)).split('/').filter(Boolean);
  return parts[parts.length - 1] || path;
};

const relativeFromRoot = (path: string, root: string) => {
  const normalizedPath = normalizePath(path);
  const normalizedRoot = normalizePath(root);
  if (!normalizedRoot || normalizedRoot === '/') return null;

  const lowerPath = normalizedPath.toLowerCase();
  const lowerRoot = normalizedRoot.toLowerCase();
  if (lowerPath === lowerRoot) return '';
  if (lowerPath.startsWith(`${lowerRoot}/`)) {
    return stripOuterSlashes(normalizedPath.slice(normalizedRoot.length + 1));
  }
  return null;
};

const relativeFromPathSegment = (path: string, segment: string) => {
  const pathParts = stripOuterSlashes(normalizePath(path)).split('/').filter(Boolean);
  const segmentParts = stripOuterSlashes(normalizePath(segment)).split('/').filter(Boolean);
  if (segmentParts.length === 0 || segmentParts.length > pathParts.length) return null;

  const lowerPathParts = pathParts.map((part) => part.toLowerCase());
  const lowerSegmentParts = segmentParts.map((part) => part.toLowerCase());

  for (let i = 0; i <= pathParts.length - segmentParts.length; i += 1) {
    const matched = lowerSegmentParts.every(
      (part, offset) => lowerPathParts[i + offset] === part,
    );
    if (matched) {
      return pathParts.slice(i + segmentParts.length).join('/');
    }
  }

  return null;
};

export const getRelativeChapterPath = (
  chapterPath: string,
  book: Book,
  pathLibrary: Library | null,
) => {
  const roots: string[] = [];

  if (pathLibrary) {
    if (pathLibrary.library_type === 'webdav') {
      roots.push(joinDisplayPath(pathLibrary.url, pathLibrary.root_path));
    }
    roots.push(pathLibrary.url);
    roots.push(pathLibrary.root_path);
  }

  for (const root of roots.filter(Boolean).sort((a, b) => b.length - a.length)) {
    const relativePath = relativeFromRoot(chapterPath, root);
    if (relativePath !== null) return relativePath;
  }

  if (pathLibrary?.library_type === 'local') {
    for (const segment of [pathLibrary.url, pathLibrary.root_path].filter(Boolean)) {
      const relativePath = relativeFromPathSegment(chapterPath, segment);
      if (relativePath !== null) return relativePath;
    }
  }

  if (book.path) {
    const relativeToBook = relativeFromRoot(chapterPath, book.path);
    if (relativeToBook !== null) {
      return joinDisplayPath(getPathName(book.path), relativeToBook);
    }
  }

  const normalizedPath = normalizePath(chapterPath);
  if (!normalizedPath.includes(':') && !normalizedPath.startsWith('/')) {
    return stripOuterSlashes(normalizedPath);
  }
  return getPathName(chapterPath);
};

export const formatChapterLocation = (
  chapter: EditableChapter,
  book: Book,
  pathLibrary: Library | null,
  unknownLibrary: string,
) => {
  const libraryName = pathLibrary?.name || unknownLibrary;
  const relativePath = getRelativeChapterPath(chapter.path, book, pathLibrary);
  return relativePath ? `${libraryName} / ${relativePath}` : libraryName;
};
