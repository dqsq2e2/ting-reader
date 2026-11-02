import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import apiClient from '../api/client';
import { useAuthStore } from '../stores/authStore';
import type { Book, Chapter, Progress } from '../types';
import { BookOperations, ChapterCatalogue, readChapterGroupOrders, findChapterGroupOrder } from '../api/books';
import { invalidateBookCover } from '../utils/image';
import type { ChapterGroupOrder } from '../api/books';
import type { ChapterGroupOrderEntry } from '../api/books';
import { libraryCacheIdentity, libraryViewCache } from '../api/libraryCache';

interface BookSnapshot {
  book: Book;
  chapters: Chapter[];
  totals: { total: number; main: number; extra: number };
  progress: Progress | null;
  tab: 'main' | 'extra';
  group: number;
  ascending: boolean;
  groupsDescending: boolean;
  groupOrders: ChapterGroupOrderEntry[];
  coverShape: 'rect' | 'square';
}

/** Mutations belong to this mounted consumer, scoped to server and login. */
export const useBookOperations = () => {
  const token = useAuthStore(state => state.token);
  const activeUrl = useAuthStore(state => state.activeUrl);
  const owner = useRef<BookOperations | null>(null);
  useEffect(() => {
    const operations = new BookOperations(apiClient);
    owner.current = operations;
    return () => { operations.dispose(); if (owner.current === operations) owner.current = null; };
  }, [token, activeUrl]);
  return owner;
};

/** A book's data and request lifetime, shared by playback and editing actions.
 * No DOM, scrolling, dialog state or rendered components are retained here. */
export const useBookDetails = (id: string | undefined, entryChapterId: string | null) => {
  const token = useAuthStore(state => state.token);
  const activeUrl = useAuthStore(state => state.activeUrl);
  const identity = useAuthStore(state => libraryCacheIdentity(state.serverUrl || state.activeUrl, state.token ? state.user?.id : undefined));
  const key = JSON.stringify([identity, id]);
  const initial = useMemo(() => id ? libraryViewCache.get<BookSnapshot>(identity, `book:${id}`) : null, [id, identity]);
  const [dataKey, setDataKey] = useState(key);
  const [book, setBook] = useState<Book | null>(initial?.book ?? null);
  const [chapters, setChapters] = useState<Chapter[]>(initial?.chapters ?? []);
  const [chapterTotals, setChapterTotals] = useState(initial?.totals ?? { total: 0, main: 0, extra: 0 });
  const [chapterPageLoading, setChapterPageLoading] = useState(false);
  const [bookProgress, setBookProgress] = useState<Progress | null>(initial?.progress ?? null);
  const [loading, setLoading] = useState(!initial);
  const [isFavorite, setIsFavorite] = useState(!!initial?.book.is_favorite);
  const [activeTab, setActiveTab] = useState<'main' | 'extra'>(initial?.tab ?? 'main');
  const [currentGroupIndex, setCurrentGroupIndex] = useState(initial?.group ?? 0);
  const [chapterAscending, setChapterAscending] = useState(initial?.ascending ?? true);
  const [chapterGroupsDescending, setChapterGroupsDescending] = useState(initial?.groupsDescending ?? false);
  const [chapterGroupOrders, setChapterGroupOrders] = useState<ChapterGroupOrderEntry[]>(initial?.groupOrders ?? []);
  const operationsRef = useRef<BookOperations | null>(null);
  const [coverShape, setCoverShape] = useState<'rect' | 'square'>(initial?.coverShape ?? 'square');
  const chaptersPerGroup = 100;
  const catalogueRef = useRef<ChapterCatalogue | null>(null);
  const chapterRequestRevision = useRef(0);
  const favoritePending = useRef(false);
  const displayedPage = useRef({ key, tab: initial?.tab ?? 'main', group: initial?.group ?? 0 });
  const visibleBookRef = useRef<{ key: string; book: Book | null }>({ key, book: initial?.book ?? null });
  useEffect(() => { visibleBookRef.current = { key: dataKey, book }; }, [dataKey, book]);

  useEffect(() => {
    if (!id) return;
    const operations = new BookOperations(apiClient);
    operationsRef.current = operations;
    const catalogue = new ChapterCatalogue(async (params, signal) => {
      const response = await apiClient.get(`/api/books/${id}/chapters`, { params, signal });
      return response.data;
    });
    catalogueRef.current = catalogue;
    return () => {
      catalogue.dispose();
      operations.dispose();
      if (operationsRef.current === operations) operationsRef.current = null;
      if (catalogueRef.current === catalogue) catalogueRef.current = null;
    };
  }, [id, token, activeUrl, identity]);

  useEffect(() => {
    const request = new AbortController();
    const fetchBookDetails = async () => {
      try {
        const retain = visibleBookRef.current.key === key && !!visibleBookRef.current.book;
        setLoading(!retain && !initial);
        setDataKey(key);
        chapterRequestRevision.current++;
        catalogueRef.current?.invalidate();
        if (!retain) {
          displayedPage.current = { key, tab: initial?.tab ?? 'main', group: initial?.group ?? 0 };
          setBook(initial?.book ?? null);
          setChapters(initial?.chapters ?? []);
          setChapterTotals(initial?.totals ?? { total: 0, main: 0, extra: 0 });
          setBookProgress(initial?.progress ?? null);
          setActiveTab(initial?.tab ?? 'main');
          setCurrentGroupIndex(initial?.group ?? 0);
        }
        const operations = operationsRef.current;
        if (!operations || !id) return;
        const result = await operations.details(id, request.signal);
        if (request.signal.aborted) return;
        const fetchedBook = result.book;
        const progressRes = result.progress;
        const rssLibrary = fetchedBook.library_type === 'rss';
        setBook(fetchedBook);
        setBookProgress(progressRes);

        setIsFavorite(!!fetchedBook.is_favorite);
        // Load user settings
        const settings = result.settings;
        const savedGroupOrders = readChapterGroupOrders(settings);
        const effectiveGroupOrder = findChapterGroupOrder(savedGroupOrders, fetchedBook.id)
          ?? (rssLibrary ? 'desc' : 'asc');
        setChapterGroupOrders(savedGroupOrders);

        if (settings.bookshelf_cover_shape) {
          setCoverShape(settings.bookshelf_cover_shape === 'rect' ? 'rect' : 'square');
        }
        setChapterGroupsDescending(effectiveGroupOrder === 'desc');
        setChapterAscending(effectiveGroupOrder === 'asc');
      } catch (err) {
        if (!request.signal.aborted) console.error('Failed to fetch book details', err);
      } finally {
        if (!request.signal.aborted) setLoading(false);
      }
    };
    void fetchBookDetails();
    return () => request.abort();
  }, [id, entryChapterId, token, activeUrl, identity, initial, key]);

  useEffect(() => {
    if (!id || !book || loading || chapterPageLoading || dataKey !== key || book.id !== id
      || displayedPage.current.key !== key || displayedPage.current.tab !== activeTab || displayedPage.current.group !== currentGroupIndex) return;
    libraryViewCache.set(identity, `book:${id}`, {
      book, chapters, totals: chapterTotals, progress: bookProgress,
      tab: activeTab, group: currentGroupIndex, ascending: chapterAscending,
      groupsDescending: chapterGroupsDescending, groupOrders: chapterGroupOrders, coverShape,
    } satisfies BookSnapshot);
  }, [id, identity, key, dataKey, book, chapters, chapterTotals, bookProgress, loading, chapterPageLoading,
    activeTab, currentGroupIndex, chapterAscending, chapterGroupsDescending, chapterGroupOrders, coverShape]);

  const fetchChapterPage = useCallback(async (options?: {
    tab?: 'main' | 'extra';
    groupIndex?: number;
    target_chapter_id?: string | null;
    preferLastGroup?: boolean;
  }) => {
    const catalogue = catalogueRef.current;
    if (!id || !catalogue) return;
    const revision = ++chapterRequestRevision.current;

    const requestedTab = options?.tab ?? activeTab;
    const requestedGroupIndex = options?.groupIndex ?? currentGroupIndex;
    const params: Record<string, unknown> = {
      limit: chaptersPerGroup,
      offset: requestedGroupIndex * chaptersPerGroup,
      order: 'asc',
    };
    if (!(options?.target_chapter_id && !options?.tab)) {
      params.chapter_type = requestedTab;
    }
    if (options?.target_chapter_id) {
      params.target_chapter_id = options.target_chapter_id;
    }

    const keepVisible = chapters.length > 0 && displayedPage.current.key === key
      && displayedPage.current.tab === requestedTab && displayedPage.current.group === requestedGroupIndex;
    setChapterPageLoading(!keepVisible);
    try {
      const data = await catalogue.page(params);
      if (!data || catalogueRef.current !== catalogue || revision !== chapterRequestRevision.current) return null;
      setChapters(data.chapters || []);
      setChapterTotals({
        total: data.total ?? 0,
        main: data.main_total ?? 0,
        extra: data.extra_total ?? 0,
      });

      const resolvedTab = data.chapter_type === 'extra' ? 'extra' : 'main';
      const resolvedGroupIndex = Math.floor((data.offset || 0) / chaptersPerGroup);
      const filteredTotal = resolvedTab === 'extra'
        ? (data.extra_total ?? 0)
        : (data.main_total ?? 0);
      const lastGroupIndex = Math.max(0, Math.ceil(filteredTotal / chaptersPerGroup) - 1);

      if (!options?.target_chapter_id && (requestedGroupIndex > lastGroupIndex || options?.preferLastGroup && requestedGroupIndex !== lastGroupIndex)) {
        setChapterTotals({
          total: data.total ?? 0,
          main: data.main_total ?? 0,
          extra: data.extra_total ?? 0,
        });
        if (resolvedTab !== activeTab) {
          setActiveTab(resolvedTab);
        }
        setCurrentGroupIndex(lastGroupIndex);
        setChapters([]);
        return {
          tab: resolvedTab,
          groupIndex: lastGroupIndex,
        };
      }

      if (resolvedTab !== activeTab) {
        setActiveTab(resolvedTab);
      }
      if (resolvedGroupIndex !== currentGroupIndex) {
        setCurrentGroupIndex(resolvedGroupIndex);
      }
      displayedPage.current = { key, tab: resolvedTab, group: resolvedGroupIndex };
      return {
        tab: resolvedTab,
        groupIndex: resolvedGroupIndex,
      };
    } catch (err) {
      if (catalogueRef.current !== catalogue || revision !== chapterRequestRevision.current) return null;
      console.error('Failed to fetch chapter page', err);
      if (!keepVisible) setChapters([]);
      return null;
    } finally {
      if (catalogueRef.current === catalogue && revision === chapterRequestRevision.current) setChapterPageLoading(false);
    }
  }, [id, activeTab, currentGroupIndex, chapters.length, key]);

  const fetchAllChaptersForPlayback = async () => {
    const catalogue = catalogueRef.current;
    const allChapters = await catalogue?.all() ?? null;
    if (catalogueRef.current !== catalogue || !allChapters) return null;
    const extra = allChapters.filter(chapter => Boolean(chapter.is_extra)).length;
    setChapterTotals(current => {
      // A source-backed book may already expose a remote total while the
      // non-paged catalogue still contains only the pages loaded so far.
      // Never replace that authoritative total with a smaller partial count.
      if (allChapters.length < current.total) return current;
      return {
        total: allChapters.length,
        main: allChapters.length - extra,
        extra,
      };
    });
    return allChapters;
  };

  const invalidateChapters = () => catalogueRef.current?.invalidate();
  const toggleFavorite = async () => {
    const operations = operationsRef.current;
    if (!operations || !book || book.id !== id || favoritePending.current) return;
    favoritePending.current = true;
    try {
      await operations.favorite(book.id, !isFavorite);
      if (operationsRef.current !== operations) return;
      setIsFavorite(!isFavorite);
      setBook(current => current?.id === book.id ? { ...current, is_favorite: !isFavorite } : current);
    } finally {
      favoritePending.current = false;
    }
  };
  const reloadBook = async () => {
    const operations = operationsRef.current;
    if (!operations || !id) return;
    const updated = await operations.book(id);
    if (operationsRef.current !== operations) return;
    setBook(updated);
    setIsFavorite(!!updated.is_favorite);
    invalidateChapters();
    await fetchChapterPage();
  };
  const saveBook = async (metadata: Partial<Book>, order: ChapterGroupOrder, cover: File | null) => {
    const operations = operationsRef.current;
    if (!operations || !book || book.id !== id) return null;
    const saved = await operations.save(book, metadata, cover);
    if (operationsRef.current !== operations) return null;
    if (cover) invalidateBookCover(book.id);
    setBook(saved);
    if (findChapterGroupOrder(chapterGroupOrders, book.id) !== order) {
      const next = await operations.saveGroupOrder(chapterGroupOrders, book.id, order);
      if (operationsRef.current !== operations) return null;
      setChapterGroupOrders(next);
    }
    if (order !== (chapterGroupsDescending ? 'desc' : 'asc')) {
      setChapterGroupsDescending(order === 'desc');
      setChapterAscending(order === 'asc');
      const count = activeTab === 'extra' ? chapterTotals.extra : chapterTotals.main;
      setCurrentGroupIndex(order === 'desc' ? Math.max(0, Math.ceil(count / chaptersPerGroup) - 1) : 0);
    }
    const rescanning = metadata.chapter_regex !== undefined && metadata.chapter_regex !== book.chapter_regex;
    if (rescanning) await operations.rescan(book.library_id);
    if (operationsRef.current !== operations) return null;
    invalidateChapters();
    await fetchChapterPage();
    return operationsRef.current === operations ? { book: saved, rescanning } : null;
  };
  const currentIdentity = dataKey === key;
  return { book: currentIdentity ? book : null, chapters: currentIdentity ? chapters : [], chapterTotals,
    chapterPageLoading, bookProgress: currentIdentity ? bookProgress : null, loading: loading || !currentIdentity, isFavorite,
    activeTab, setActiveTab, currentGroupIndex, setCurrentGroupIndex, chapterAscending,
    setChapterAscending, chapterGroupsDescending,
    coverShape, fetchChapterPage, fetchAllChaptersForPlayback,
    invalidateChapters, operationsRef, toggleFavorite, reloadBook, saveBook };
};
