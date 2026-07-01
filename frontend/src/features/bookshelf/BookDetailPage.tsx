import React, { useEffect, useState, useRef } from 'react';
import { useParams, useNavigate, useSearchParams } from 'react-router-dom';
import apiClient from '../../core/api/client';
import type { Book, Chapter, Progress } from '../../core/types';
import { usePlayerStore } from '../../core/stores/playerStore';
import { useTranslation } from 'react-i18next';

import ChapterManagerModal from '../../shared/modals/ChapterManagerModal';
import ScrapeDiffModal from '../../shared/modals/ScrapeDiffModal';
import {
  ChevronLeft,
  Loader2,
} from 'lucide-react';
import { useAuthStore } from '../../core/stores/authStore';
import { setAlpha, isTooLight } from '../../core/utils/color';
import LoadingSpinner from '../../shared/ui/LoadingSpinner';
import EditBookModal from './bookDetail/EditBookModal';
import DeleteBookModal from './bookDetail/DeleteBookModal';
import BookHeaderSection from './bookDetail/BookHeaderSection';
import ChapterListSection from './bookDetail/ChapterListSection';

type ChapterGroupOrder = 'asc' | 'desc';

interface ChapterGroupOrderEntry {
  book_id: string;
  order: ChapterGroupOrder;
}

const normalizeChapterGroupOrder = (value: unknown): ChapterGroupOrder | null => {
  if (value === 'asc' || value === 'desc') {
    return value;
  }
  return null;
};

const readChapterGroupOrders = (settings: Record<string, unknown>): ChapterGroupOrderEntry[] => {
  const raw = settings.chapter_group_orders;

  if (Array.isArray(raw)) {
    return raw.flatMap((entry) => {
      if (!entry || typeof entry !== 'object') return [];
      const item = entry as Record<string, unknown>;
      const bookId = typeof item.book_id === 'string' ? item.book_id : '';
      const order = normalizeChapterGroupOrder(item.order);
      return bookId && order ? [{ book_id: bookId, order }] : [];
    });
  }

  if (raw && typeof raw === 'object') {
    return Object.entries(raw as Record<string, unknown>).flatMap(([bookId, value]) => {
      const order = normalizeChapterGroupOrder(value);
      return order ? [{ book_id: bookId, order }] : [];
    });
  }

  return [];
};

const findChapterGroupOrder = (
  orders: ChapterGroupOrderEntry[],
  bookId: string
): ChapterGroupOrder | null => orders.find((item) => item.book_id === bookId)?.order ?? null;

const upsertChapterGroupOrder = (
  orders: ChapterGroupOrderEntry[],
  bookId: string,
  order: ChapterGroupOrder
): ChapterGroupOrderEntry[] => {
  const exists = orders.some((item) => item.book_id === bookId);
  if (!exists) {
    return [...orders, { book_id: bookId, order }];
  }
  return orders.map((item) => item.book_id === bookId ? { ...item, order } : item);
};

const BookDetailPage: React.FC = () => {
  const { t } = useTranslation();
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const entryChapterId = searchParams.get('chapter_id');
  const { user } = useAuthStore();
  const [book, setBook] = useState<Book | null>(null);
  const [chapters, setChapters] = useState<Chapter[]>([]);
  const [chapterTotals, setChapterTotals] = useState({ total: 0, main: 0, extra: 0 });
  const [chapterPageLoading, setChapterPageLoading] = useState(false);
  const [bookProgress, setBookProgress] = useState<Progress | null>(null);
  const [chapterManagerChapters, setChapterManagerChapters] = useState<Chapter[]>([]);
  const [chapterManagerLoading, setChapterManagerLoading] = useState(false);
  const [loading, setLoading] = useState(true);
  const [isFavorite, setIsFavorite] = useState(false);
  const [isEditModalOpen, setIsEditModalOpen] = useState(false);
  const [isChapterManagerOpen, setIsChapterManagerOpen] = useState(false);
  const [isScrapeDiffOpen, setIsScrapeDiffOpen] = useState(false);
  const [isDeleteModalOpen, setIsDeleteModalOpen] = useState(false);
  const [deleteSourceFiles, setDeleteSourceFiles] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [editData, setEditData] = useState<Partial<Book>>({});
  const [isDescriptionExpanded, setIsDescriptionExpanded] = useState(false);
  const [isOverflowing, setIsOverflowing] = useState(false);
  const [activeTab, setActiveTab] = useState<'main' | 'extra'>('main');
  const [currentGroupIndex, setCurrentGroupIndex] = useState(0);
  const [chapterAscending, setChapterAscending] = useState(true);
  const [chapterGroupsDescending, setChapterGroupsDescending] = useState(false);
  const [chapterGroupOrders, setChapterGroupOrders] = useState<ChapterGroupOrderEntry[]>([]);
  const [editChapterGroupOrder, setEditChapterGroupOrder] = useState<ChapterGroupOrder>('asc');
  const [themeColor, setThemeColor] = useState<string | null>(book?.theme_color ?? null);
  const [isTagsExpanded, setIsTagsExpanded] = useState(false);
  const tagsRef = useRef<HTMLDivElement>(null);
  const [isTagsOverflowing, setIsTagsOverflowing] = useState(false);
  const descriptionRef = useRef<HTMLParagraphElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const hasInitialScrolled = useRef(false);
  const [highlightedChapterId, setHighlightedChapterId] = useState<string | null>(null);
  const playButtonContainerRef = useRef<HTMLButtonElement>(null);
  const [isPlayButtonTextOverflowing, setIsPlayButtonTextOverflowing] = useState(false);
  const allChaptersCacheRef = useRef<Chapter[] | null>(null);

  // User Settings
  const [coverShape, setCoverShape] = useState<'rect' | 'square'>('rect');

  // Regex Generator State
  const [showRegexGenerator, setShowRegexGenerator] = useState(false);
  const [genFilename, setGenFilename] = useState('');
  const [genNum, setGenNum] = useState('');
  const [genTitle, setGenTitle] = useState('');
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const [genResult, setGenResult] = useState<any>(null);

  const handleGenerateRegex = async () => {
    if (!genFilename || !genNum || !genTitle) return;
    try {
      const res = await apiClient.post('/api/tools/regex/generate', {
        filename: genFilename,
        chapter_number: genNum,
        chapter_title: genTitle
      });
      setGenResult(res.data);
    } catch {
      alert(t('bookshelf.generateFailed'));
    }
  };

  const applyGeneratedRegex = () => {
    if (genResult?.regex) {
      setEditData({ ...editData, chapter_regex: genResult.regex });
      setShowRegexGenerator(false);
      setGenResult(null);
    }
  };

  // Reset scroll state when book ID changes
  useEffect(() => {
    hasInitialScrolled.current = false;
    setHighlightedChapterId(null);
    allChaptersCacheRef.current = null;
    setChapters([]);
    setChapterTotals({ total: 0, main: 0, extra: 0 });
    setBookProgress(null);
    setActiveTab('main');
    setCurrentGroupIndex(0);
    setChapterAscending(true);
    setChapterGroupsDescending(false);
    setChapterGroupOrders([]);
    setEditChapterGroupOrder('asc');
  }, [id, entryChapterId]);

  // Clear highlighted chapter when current chapter changes (user plays a new chapter)
  const currentChapter = usePlayerStore((state) => state.currentChapter);
  useEffect(() => {
    if (currentChapter?.book_id === book?.id && !entryChapterId) {
      setHighlightedChapterId(null);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentChapter?.id, book?.id, entryChapterId]);

  const scrollGroups = (direction: 'left' | 'right') => {
    if (scrollRef.current) {
      const scrollAmount = 200;
      scrollRef.current.scrollBy({
        left: direction === 'left' ? -scrollAmount : scrollAmount,
        behavior: 'smooth'
      });
    }
  };

  const activeTotal = activeTab === 'main' ? chapterTotals.main : chapterTotals.extra;

  const chaptersPerGroup = 100;
  const groups = React.useMemo(() => {
    const g = [];
    for (let i = 0; i < activeTotal; i += chaptersPerGroup) {
      g.push({
        start: i + 1,
        end: Math.min(i + chaptersPerGroup, activeTotal),
        offset: i,
        index: g.length,
      });
    }
    return g;
  }, [activeTotal]);

  const visibleChapters = React.useMemo(
    () => (chapterAscending ? chapters : [...chapters].reverse()),
    [chapterAscending, chapters]
  );

  const isPlaying = usePlayerStore((state) => state.isPlaying);
  const playChapter = usePlayerStore((state) => state.playChapter);
  const storeChapters = usePlayerStore((state) => state.chapters);

  useEffect(() => {
    if (book) {
      setThemeColor(book.theme_color || null);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [book?.theme_color]);

  useEffect(() => {
    const fetchBookDetails = async () => {
      try {
        setLoading(true);
        const progressRequest = apiClient
          .get(`/api/progress/${id}`)
          .then(res => res.data as Progress)
          .catch(() => null);
        const [bookRes, progressRes, settingsRes] = await Promise.all([
          apiClient.get(`/api/books/${id}`),
          progressRequest,
          apiClient.get('/api/settings')
        ]);
        const fetchedBook = bookRes.data;
        const initialChapterId = entryChapterId || progressRes?.chapter_id || null;
        const rssLibrary = fetchedBook.library_type === 'rss';
        setBook(fetchedBook);
        setBookProgress(progressRes);
        setHighlightedChapterId(initialChapterId);
        setIsFavorite(fetchedBook.is_favorite);
        // Load user settings
        const settings = settingsRes.data.settings_json || {};
        const savedGroupOrders = readChapterGroupOrders(settings);
        const effectiveGroupOrder = findChapterGroupOrder(savedGroupOrders, fetchedBook.id)
          ?? (rssLibrary ? 'desc' : 'asc');
        setChapterGroupOrders(savedGroupOrders);
        setEditChapterGroupOrder(effectiveGroupOrder);
        if (settings.bookshelf_cover_shape) {
          setCoverShape(settings.bookshelf_cover_shape);
        }
        setActiveTab('main');
        setCurrentGroupIndex(0);
        setChapterGroupsDescending(effectiveGroupOrder === 'desc');
        setChapterAscending(effectiveGroupOrder === 'asc');
      } catch (err) {
        console.error('Failed to fetch book details', err);
      } finally {
        setLoading(false);
      }
    };
    fetchBookDetails();
  }, [id, entryChapterId]);

  const fetchChapterPage = React.useCallback(async (options?: {
    tab?: 'main' | 'extra';
    groupIndex?: number;
    target_chapter_id?: string | null;
    preferLastGroup?: boolean;
  }) => {
    if (!id) return;

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

    setChapterPageLoading(true);
    try {
      const res = await apiClient.get(`/api/books/${id}/chapters`, { params });
      const data = res.data;
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

      if (options?.preferLastGroup && !options?.target_chapter_id && requestedGroupIndex !== lastGroupIndex) {
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
      return {
        tab: resolvedTab,
        groupIndex: resolvedGroupIndex,
      };
    } catch (err) {
      console.error('Failed to fetch chapter page', err);
      setChapters([]);
      return null;
    } finally {
      setChapterPageLoading(false);
    }
  }, [id, activeTab, currentGroupIndex]);

  const scrollToChapterNode = React.useCallback((chapterId: string, groupIndex: number) => {
    const el = document.getElementById(`chapter-${chapterId}`);
    if (el) {
      el.scrollIntoView({ block: 'center', behavior: 'smooth' });
    }

    const groupTab = document.getElementById(`group-tab-${groupIndex}`);
    const container = scrollRef.current;
    if (groupTab && container) {
      const containerWidth = container.offsetWidth;
      const tabWidth = groupTab.offsetWidth;
      const tabLeft = groupTab.offsetLeft;

      container.scrollTo({
        left: tabLeft - containerWidth / 2 + tabWidth / 2,
        behavior: 'smooth'
      });
    }
  }, []);

  useEffect(() => {
    if (!book) return;
    const targetChapterId = !hasInitialScrolled.current
      ? entryChapterId
      : null;
    const preferLastGroup = !hasInitialScrolled.current
      && !targetChapterId
      && chapterGroupsDescending;
    fetchChapterPage(
      targetChapterId
        ? { target_chapter_id: targetChapterId }
        : { tab: activeTab, groupIndex: currentGroupIndex, preferLastGroup }
    ).then((page) => {
      if (targetChapterId) {
        setTimeout(() => scrollToChapterNode(targetChapterId, page?.groupIndex ?? currentGroupIndex), 120);
      }
    });
    hasInitialScrolled.current = true;
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [book?.id, activeTab, currentGroupIndex, entryChapterId, chapterGroupsDescending]);

  // Find the chapter to resume or highlight
  const resumeChapter = React.useMemo(() => {
    if (!book) return null;

    // 1. Priority: Currently playing chapter if it belongs to this book
    if (currentChapter && currentChapter.book_id === book.id) {
      return currentChapter;
    }

    if (entryChapterId) {
      const entryChapter = chapters.find(c => c.id === entryChapterId);
      if (entryChapter) {
        return entryChapter;
      }
    }

    if (bookProgress?.chapter_id) {
      const progressChapter = chapters.find(c => c.id === bookProgress.chapter_id);
      if (progressChapter) {
        return {
          ...progressChapter,
          progress_position: bookProgress.position,
          progress_updated_at: bookProgress.updated_at,
        };
      }

      return {
        id: bookProgress.chapter_id,
        book_id: book.id,
        title: bookProgress.chapter_title || t('bookshelf.lastListenedChapter'),
        path: '',
        duration: bookProgress.chapter_duration || bookProgress.duration || 0,
        chapter_index: 0,
        progress_position: bookProgress.position,
        progress_updated_at: bookProgress.updated_at,
      };
    }

    return null;
  }, [book, chapters, currentChapter, bookProgress, entryChapterId, t]);

  // Auto-highlight current chapter logic (without scroll)
  useEffect(() => {
    if (book?.id !== id) return;

    if (entryChapterId && chapters.some(chapter => chapter.id === entryChapterId)) {
      setHighlightedChapterId(entryChapterId);
      return;
    }

    if (resumeChapter) {
      setHighlightedChapterId(resumeChapter.id);
      return;
    }

    setHighlightedChapterId(null);
  }, [book?.id, id, resumeChapter, entryChapterId, chapters, t]);

  const doScroll = (chapterId: string, groupIndex: number) => {
      scrollToChapterNode(chapterId, groupIndex);
  };

  const scrollToChapterElement = (chapterId: string, groupIndex = currentGroupIndex) => {
      doScroll(chapterId, groupIndex);
  };

  const fetchAllChaptersForPlayback = async () => {
    if (allChaptersCacheRef.current) return allChaptersCacheRef.current;
    if (book && storeChapters.length > 0 && storeChapters.some(c => c.book_id === book.id)) {
      allChaptersCacheRef.current = storeChapters;
      return storeChapters;
    }
    const res = await apiClient.get(`/api/books/${id}/chapters`);
    const allChapters = (res.data || []) as Chapter[];
    allChaptersCacheRef.current = allChapters;
    return allChapters;
  };

  const playChapterWithFullQueue = async (chapter: Chapter) => {
    if (!book) return;
    const allChapters = await fetchAllChaptersForPlayback();
    const resolvedChapter = allChapters.find(c => c.id === chapter.id) || chapter;
    playChapter(book, allChapters, resolvedChapter);
  };

  const openChapterManager = async () => {
    if (!id) return;
    setChapterManagerLoading(true);
    setIsChapterManagerOpen(true);
    try {
      const allChapters = await fetchAllChaptersForPlayback();
      setChapterManagerChapters(allChapters);
    } catch (err) {
      console.error('Failed to fetch chapters for manager', err);
      setChapterManagerChapters([]);
    } finally {
      setChapterManagerLoading(false);
    }
  };

  const handlePlayClick = async () => {
    if (resumeChapter) {
      // If we have a resume chapter, play it and scroll to it
      await playChapterWithFullQueue(resumeChapter);

      // Scroll logic
      const targetChapter = resumeChapter;

      // Determine if target chapter is in main or extra
      const inMain = activeTab === 'main' && chapters.some(c => c.id === targetChapter.id);
      const inExtra = activeTab === 'extra' && chapters.some(c => c.id === targetChapter.id);

      if (inMain) {
        scrollToChapterElement(targetChapter.id);
      } else if (inExtra) {
        scrollToChapterElement(targetChapter.id);
      } else {
        const page = await fetchChapterPage({ target_chapter_id: targetChapter.id });
        setTimeout(() => doScroll(targetChapter.id, page?.groupIndex ?? currentGroupIndex), 120);
      }
    } else {
      // No play history - play first main chapter
      if (chapters.length > 0) {
        await playChapterWithFullQueue(chapters[0]);
      }
    }
  };

  useEffect(() => {
    const checkOverflow = () => {
      if (descriptionRef.current) {
        const { scrollHeight, clientHeight } = descriptionRef.current;
        setIsOverflowing(scrollHeight > clientHeight);
      }
    };

    checkOverflow();
    window.addEventListener('resize', checkOverflow);
    return () => window.removeEventListener('resize', checkOverflow);
  }, [book?.description]);

  useEffect(() => {
    const checkTagsOverflow = () => {
      if (tagsRef.current) {
        // Measure real content height without max-height constraint
        const originalMaxHeight = tagsRef.current.style.maxHeight;
        tagsRef.current.style.maxHeight = 'none';
        const fullHeight = tagsRef.current.scrollHeight;
        tagsRef.current.style.maxHeight = originalMaxHeight;
        
        // 36px is approximately the height of one row of tags including gap
        setIsTagsOverflowing(fullHeight > 36);
      }
    };

    checkTagsOverflow();
    const timer = setTimeout(checkTagsOverflow, 500);
    window.addEventListener('resize', checkTagsOverflow);
    return () => {
      window.removeEventListener('resize', checkTagsOverflow);
      clearTimeout(timer);
    };
  }, [book?.tags]);

  // Check if play button text is overflowing
  useEffect(() => {
    const checkPlayButtonOverflow = () => {
      if (!playButtonContainerRef.current) return;
      
      const button = playButtonContainerRef.current;
      const computedStyle = window.getComputedStyle(button);
      
      // Get button dimensions
      const buttonWidth = button.offsetWidth;
      
      // Parse padding from computed style
      const paddingLeft = parseFloat(computedStyle.paddingLeft);
      const paddingRight = parseFloat(computedStyle.paddingRight);
      
      // Icon width (18px) + gap (8px from gap-2 class)
      const iconAndGapWidth = 18 + 8;
      
      // Calculate available width for text
      const availableWidth = buttonWidth - paddingLeft - paddingRight - iconAndGapWidth;
      
      // Create temporary element to measure text width
      const tempSpan = document.createElement('span');
      tempSpan.style.visibility = 'hidden';
      tempSpan.style.position = 'absolute';
      tempSpan.style.whiteSpace = 'nowrap';
      tempSpan.style.fontWeight = computedStyle.fontWeight;
      tempSpan.style.fontSize = computedStyle.fontSize;
      tempSpan.style.fontFamily = computedStyle.fontFamily;
      
      const text = resumeChapter && currentChapter?.book_id === book?.id
        ? t('bookshelf.nowPlayingChapter', { title: resumeChapter.title })
        : resumeChapter
        ? t('bookshelf.continuePlayingChapter', { title: resumeChapter.title })
        : t('bookshelf.playNow');
      
      tempSpan.textContent = text;
      document.body.appendChild(tempSpan);
      const textWidth = tempSpan.offsetWidth;
      document.body.removeChild(tempSpan);
      
      // Set overflow state with a small buffer (2px) to prevent edge cases
      setIsPlayButtonTextOverflowing(textWidth > availableWidth - 2);
    };

    // Check immediately and after a delay to ensure layout is stable
    checkPlayButtonOverflow();
    const timer = setTimeout(checkPlayButtonOverflow, 100);
    
    // Check on resize with debounce
    let resizeTimer: ReturnType<typeof setTimeout>;
    const handleResize = () => {
      clearTimeout(resizeTimer);
      resizeTimer = setTimeout(checkPlayButtonOverflow, 50);
    };
    
    window.addEventListener('resize', handleResize);
    
    return () => {
      window.removeEventListener('resize', handleResize);
      clearTimeout(timer);
      clearTimeout(resizeTimer);
    };
  }, [resumeChapter, currentChapter, book?.id, t]);

  const toggleFavorite = async () => {
    try {
      if (isFavorite) {
        await apiClient.delete(`/api/favorites/${id}`);
      } else {
        await apiClient.post(`/api/favorites/${id}`);
      }
      setIsFavorite(!isFavorite);
    } catch (err) {
      console.error('Failed to toggle favorite', err);
    }
  };

  const handleWriteMetadata = async () => {
    try {
      if (!confirm(t('bookshelf.writeMetadataConfirm'))) {
        return;
      }
      await apiClient.post(`/api/books/${id}/write-metadata`);
      alert(t('bookshelf.writeMetadataStarted'));
    } catch (err) {
      console.error('Failed to write metadata', err);
      alert(t('bookshelf.writeMetadataFailed'));
    }
  };

  const handleEditSave = async () => {
    try {
      const dataToSave = { ...editData };
      // If cover changed, clear theme color so it's recalculated
    if (editData.cover_url && editData.cover_url !== displayCoverUrl) {
        dataToSave.theme_color = undefined;
      }
      
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const payload: Record<string, any> = { ...dataToSave };
      
      const res = await apiClient.patch(`/api/books/${id}`, payload);
      const updatedBookData = res.data;

      if (book && findChapterGroupOrder(chapterGroupOrders, book.id) !== editChapterGroupOrder) {
        const nextOrders = upsertChapterGroupOrder(chapterGroupOrders, book.id, editChapterGroupOrder);
        await apiClient.post('/api/settings', { chapter_group_orders: nextOrders });
        setChapterGroupOrders(nextOrders);
      }

      if (editChapterGroupOrder !== (chapterGroupsDescending ? 'desc' : 'asc')) {
        const nextDescending = editChapterGroupOrder === 'desc';
        setChapterGroupsDescending(nextDescending);
        setChapterAscending(!nextDescending);
        setCurrentGroupIndex(nextDescending ? lastGroupIndexFor(activeTab) : 0);
      }
      
      // Update local state - merge the changes
      const updatedBook = { ...book!, ...updatedBookData };
      // Preserve existing auxiliary fields if not in response
      if (book!.library_type) updatedBook.library_type = book!.library_type;
      if (book!.is_favorite !== undefined) updatedBook.is_favorite = book!.is_favorite;
      
      setBook(updatedBook);
      
      // If the edited book is currently playing, update the player store
      const currentPlayerState = usePlayerStore.getState();
      if (currentPlayerState.currentBook?.id === updatedBook.id) {
        usePlayerStore.setState({
          currentBook: {
            ...currentPlayerState.currentBook,
            ...updatedBook
          }
        });
      }
      
      // If chapterRegex changed, trigger a re-scan of this book
      if (payload.chapter_regex) {
          apiClient.post(`/api/libraries/${book!.library_id}/scan`);
          alert(t('bookshelf.regexSavedRescanning'));
      }

      setIsEditModalOpen(false);
    } catch {
      alert(t('common.saveFailed'));
    }
  };

  const handleDelete = async () => {
    try {
      setDeleting(true);
      await apiClient.delete(`/api/books/${id}?delete_files=${deleteSourceFiles}`);
      navigate('/', { replace: true });
    } catch (err) {
      console.error('Failed to delete book', err);
      alert(t('bookshelf.deleteBookFailed'));
    } finally {
      setDeleting(false);
      setIsDeleteModalOpen(false);
    }
  };

  const formatDuration = (seconds: number) => {
    if (!seconds) return '0:00';
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    const s = Math.floor(seconds % 60);
    
    if (h > 0) {
      return `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
    }
    return `${m}:${s.toString().padStart(2, '0')}`;
  };

  const getChapterProgressText = (chapter: Chapter) => {
    if (!chapter.progress_position || !chapter.duration) return null;
    
    const percent = Math.floor((chapter.progress_position / chapter.duration) * 100);
    if (percent === 0) return null;
    if (percent >= 95) return t('bookshelf.progressComplete');
    return t('bookshelf.progressPercent', { percent });
  };

  const lastGroupIndexFor = (tab: 'main' | 'extra') => {
    const total = tab === 'extra' ? chapterTotals.extra : chapterTotals.main;
    return Math.max(0, Math.ceil(total / chaptersPerGroup) - 1);
  };

  const handleSetActiveTab = (tab: 'main' | 'extra') => {
    setActiveTab(tab);
    setCurrentGroupIndex(chapterGroupsDescending ? lastGroupIndexFor(tab) : 0);
  };

  const displayThemeColor = book ? (book.theme_color || themeColor) : themeColor;
  // If the color is too light (close to white), we ignore it and use default to ensure text readability
  const effectiveThemeColor = displayThemeColor && !isTooLight(displayThemeColor) ? displayThemeColor : undefined;

  const displayCoverUrl = book ? book.cover_url : undefined;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const displayLibraryId = book ? (book.library_id || (book as any).library_id) : undefined;
  const displayLibraryType = book ? book.library_type : undefined;

  useEffect(() => {
    if (effectiveThemeColor) {
      const bgColor = setAlpha(effectiveThemeColor, 0.05);
      document.documentElement.style.setProperty('--page-background', bgColor);
    }
    return () => {
      document.documentElement.style.removeProperty('--page-background');
    };
  }, [effectiveThemeColor]);

  if (loading && !book) {
    return (
      <LoadingSpinner />
    );
  }

  if (!book) return <div className="dark:text-white p-8">{t('bookshelf.notFound')}</div>;

  return (
    <div 
      className="flex-1 min-h-full flex flex-col p-4 sm:p-6 md:p-8 animate-in slide-in-from-bottom-4 duration-500"
    >
      <div className="flex-1 max-w-6xl mx-auto space-y-8 w-full">
        {/* Header */}
        <button 
          type="button"
          onClick={() => navigate(-1)}
          className="flex items-center gap-2 text-slate-500 hover:text-primary-600 transition-colors"
        >
          <ChevronLeft size={20} />
          <span>{t('common.back')}</span>
        </button>

        {/* Book Info Section */}
        <BookHeaderSection
          book={book}
          coverShape={coverShape}
          displayCoverUrl={displayCoverUrl}
          displayLibraryId={displayLibraryId}
          effectiveThemeColor={effectiveThemeColor}
          chapterTotalCount={chapterTotals.total}
          resumeChapterTitle={resumeChapter?.title}
          resumeChapterBookMatches={currentChapter?.book_id === book.id}
          hasResumeChapter={!!resumeChapter}
          isPlayButtonTextOverflowing={isPlayButtonTextOverflowing}
          playButtonContainerRef={playButtonContainerRef}
          isFavorite={isFavorite}
          isAdmin={user?.role === 'admin'}
          tagsRef={tagsRef}
          isTagsExpanded={isTagsExpanded}
          isTagsOverflowing={isTagsOverflowing}
          descriptionRef={descriptionRef}
          isDescriptionExpanded={isDescriptionExpanded}
          isDescriptionOverflowing={isOverflowing}
          onPlayClick={handlePlayClick}
          onToggleFavorite={toggleFavorite}
          onOpenScrapeDiff={() => setIsScrapeDiffOpen(true)}
          onOpenEditModal={() => {
            setEditData({
              ...book,
              cover_url: displayCoverUrl,
              theme_color: displayThemeColor ?? undefined,
              library_type: displayLibraryType,
              skip_intro: book.skip_intro,
              skip_outro: book.skip_outro
            });
            setEditChapterGroupOrder(chapterGroupsDescending ? 'desc' : 'asc');
            setIsEditModalOpen(true);
          }}
          onSetIsTagsExpanded={setIsTagsExpanded}
          onSetIsDescriptionExpanded={setIsDescriptionExpanded}
        />

        {/* Chapters List */}
        <ChapterListSection
          isAdmin={user?.role === 'admin'}
          chapters={chapters}
          visibleChapters={visibleChapters}
          chapterTotals={chapterTotals}
          groups={groups}
          currentGroupIndex={currentGroupIndex}
          activeTab={activeTab}
          chapterAscending={chapterAscending}
          chapterGroupsDescending={chapterGroupsDescending}
          chapterPageLoading={chapterPageLoading}
          scrollRef={scrollRef}
          currentChapterId={currentChapter?.id}
          highlightedChapterId={highlightedChapterId}
          isPlaying={isPlaying}
          effectiveThemeColor={effectiveThemeColor}
          onScrollGroups={scrollGroups}
          onSetActiveTab={handleSetActiveTab}
          onSetCurrentGroupIndex={setCurrentGroupIndex}
          onToggleAscending={() => setChapterAscending(prev => !prev)}
          onPlayChapter={playChapterWithFullQueue}
          onOpenChapterManager={openChapterManager}
          formatDuration={formatDuration}
          getChapterProgressText={getChapterProgressText}
        />

      {/* Chapter Manager Modal */}
      {isChapterManagerOpen && book && chapterManagerLoading && (
        <div className="fixed inset-0 z-[200] flex items-center justify-center p-4">
          <div className="absolute inset-0 bg-slate-900/60 backdrop-blur-sm"></div>
          <div className="relative bg-white dark:bg-slate-900 rounded-3xl shadow-2xl px-8 py-6 flex items-center gap-3 text-slate-600 dark:text-slate-300">
            <Loader2 className="w-5 h-5 animate-spin text-primary-600" />
            {t('bookshelf.loadingChapterManager')}
          </div>
        </div>
      )}
      {isChapterManagerOpen && book && !chapterManagerLoading && (
        <ChapterManagerModal
          book={book}
          bookId={book.id}
          initialChapters={chapterManagerChapters}
          onClose={() => setIsChapterManagerOpen(false)}
          onSave={() => {
            allChaptersCacheRef.current = null;
            setChapterManagerChapters([]);
            fetchChapterPage();
          }}
        />
      )}

      {/* Scrape Diff Modal */}
      {isScrapeDiffOpen && book && (
        <ScrapeDiffModal
          bookId={book.id}
          onClose={() => setIsScrapeDiffOpen(false)}
          onSave={() => {
            // Reload book details
            apiClient.get(`/api/books/${id}`).then(res => setBook(res.data));
            allChaptersCacheRef.current = null;
            fetchChapterPage();
          }}
        />
      )}

      {isEditModalOpen && (
        <EditBookModal
          editData={editData}
          showRegexGenerator={showRegexGenerator}
          genFilename={genFilename}
          genNum={genNum}
          genTitle={genTitle}
          genResult={genResult}
          chapterGroupOrder={editChapterGroupOrder}
          onChangeEditData={setEditData}
          onChangeChapterGroupOrder={setEditChapterGroupOrder}
          onShowRegexGenerator={() => setShowRegexGenerator(true)}
          onHideRegexGenerator={() => setShowRegexGenerator(false)}
          onChangeGenFilename={setGenFilename}
          onChangeGenNum={setGenNum}
          onChangeGenTitle={setGenTitle}
          onGenerateRegex={handleGenerateRegex}
          onApplyRegex={applyGeneratedRegex}
          onClose={() => setIsEditModalOpen(false)}
          onDelete={() => {
            setIsEditModalOpen(false);
            setIsDeleteModalOpen(true);
          }}
          onSave={handleEditSave}
          onWriteMetadata={handleWriteMetadata}
        />
      )}

      {/* Delete Confirmation Modal */}
      {isDeleteModalOpen && book && (
        <DeleteBookModal
          book={book}
          deleting={deleting}
          deleteSourceFiles={deleteSourceFiles}
          onToggleDeleteSourceFiles={() => setDeleteSourceFiles(!deleteSourceFiles)}
          onClose={() => setIsDeleteModalOpen(false)}
          onConfirm={handleDelete}
        />
      )}
      </div>

      {/* Dynamic Safe Bottom Spacer */}
      <div
        className="shrink-0 transition-all duration-300"
        style={{ height: currentChapter ? 'var(--safe-bottom-with-player)' : 'var(--safe-bottom-base)' }}
      />
    </div>
  );
};

export default BookDetailPage;
