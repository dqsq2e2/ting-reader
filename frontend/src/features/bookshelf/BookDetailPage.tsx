import { apiErrorMessage } from '../../core/api/errors';
import React, { useEffect, useState, useRef } from 'react';
import { useParams, useNavigate, useSearchParams } from 'react-router';
import type { Book, Chapter } from '../../core/types';
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
import DeleteLibraryItemModal from './bookDetail/DeleteLibraryItemModal';
import BookHeaderSection from './bookDetail/BookHeaderSection';
import ChapterListSection from './bookDetail/ChapterListSection';
import { useBookDetails } from '../../core/hooks/useBooks';
import type { ChapterGroupOrder, RegexGenerationResult } from '../../core/api/books';

const BookDetailPage: React.FC = () => {
  const { t } = useTranslation();
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const entryChapterId = searchParams.get('chapter_id');
  const { user } = useAuthStore();
  const {
    book, chapters, chapterTotals, chapterPageLoading, bookProgress, loading,
    isFavorite, activeTab, setActiveTab, currentGroupIndex, setCurrentGroupIndex,
    chapterAscending, setChapterAscending, chapterGroupsDescending, coverShape,
    fetchChapterPage, fetchAllChaptersForPlayback, invalidateChapters,
    operationsRef, toggleFavorite: saveFavorite, reloadBook, saveBook,
  } = useBookDetails(id, entryChapterId);
  const chapterScrollTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  useEffect(() => () => clearTimeout(chapterScrollTimer.current), [id]);

  const [chapterManagerChapters, setChapterManagerChapters] = useState<Chapter[]>([]);
  const [chapterManagerLoading, setChapterManagerLoading] = useState(false);
  const [isEditModalOpen, setIsEditModalOpen] = useState(false);
  const [isChapterManagerOpen, setIsChapterManagerOpen] = useState(false);
  const [isScrapeDiffOpen, setIsScrapeDiffOpen] = useState(false);
  const [isDeleteModalOpen, setIsDeleteModalOpen] = useState(false);
  const [deleteSourceFiles, setDeleteSourceFiles] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [editData, setEditData] = useState<Partial<Book>>({});
  const [editCoverFile, setEditCoverFile] = useState<File | null>(null);
  const [editSaving, setEditSaving] = useState(false);
  const [editSaveError, setEditSaveError] = useState<string | null>(null);
  const [editLockTouched, setEditLockTouched] = useState(false);
  const [isDescriptionExpanded, setIsDescriptionExpanded] = useState(false);
  const [isOverflowing, setIsOverflowing] = useState(false);
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
  // User Settings


  // Regex Generator State
  const [showRegexGenerator, setShowRegexGenerator] = useState(false);
  const [genFilename, setGenFilename] = useState('');
  const [genNum, setGenNum] = useState('');
  const [genTitle, setGenTitle] = useState('');
  const [genResult, setGenResult] = useState<RegexGenerationResult | null>(null);

  const handleGenerateRegex = async () => {
    if (!genFilename || !genNum || !genTitle) return;
    try {
      const result = await operationsRef.current?.generateRegex(genFilename, genNum, genTitle);
      setGenResult(result ?? null);
    } catch (error) {
      alert(apiErrorMessage(error, t('bookshelf.generateFailed')));
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
    const timer = window.setTimeout(() => {
      setHighlightedChapterId(null);
      setEditChapterGroupOrder('asc');
    }, 0);
    return () => window.clearTimeout(timer);
  }, [id, entryChapterId]);

  // Clear highlighted chapter when current chapter changes (user plays a new chapter)
  const currentChapter = usePlayerStore((state) => state.currentChapter);
  useEffect(() => {
    const timer = window.setTimeout(() => {
      if (currentChapter?.book_id === book?.id && !entryChapterId) {
        setHighlightedChapterId(null);
      }
    }, 0);
    return () => window.clearTimeout(timer);
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

  useEffect(() => {
    const timer = window.setTimeout(() => {
      if (book) { setThemeColor(book.theme_color || null); setEditChapterGroupOrder(chapterGroupsDescending ? "desc" : "asc"); }
    }, 0);
    return () => window.clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [book?.theme_color]);

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
    if (!book || book.id !== id) return;
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
      if (targetChapterId && page) {
        clearTimeout(chapterScrollTimer.current);
        chapterScrollTimer.current = setTimeout(() => scrollToChapterNode(targetChapterId, page?.groupIndex ?? currentGroupIndex), 120);
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
    const timer = window.setTimeout(() => {
      if (book?.id !== id) return;
      if (entryChapterId && chapters.some(chapter => chapter.id === entryChapterId)) {
        setHighlightedChapterId(entryChapterId);
      } else if (resumeChapter) {
        setHighlightedChapterId(resumeChapter.id);
      } else {
        setHighlightedChapterId(null);
      }
    }, 0);
    return () => window.clearTimeout(timer);
  }, [book?.id, id, resumeChapter, entryChapterId, chapters, t]);

  const doScroll = (chapterId: string, groupIndex: number) => {
      scrollToChapterNode(chapterId, groupIndex);
  };

  const scrollToChapterElement = (chapterId: string, groupIndex = currentGroupIndex) => {
      doScroll(chapterId, groupIndex);
  };

  const playChapterWithFullQueue = async (chapter: Chapter) => {
    if (!book || book.id !== id) return;
    const allChapters = await fetchAllChaptersForPlayback();
    if (!allChapters) return;
    const resolvedChapter = allChapters.find(c => c.id === chapter.id) || chapter;
    playChapter(book, allChapters, resolvedChapter);
  };

  const openChapterManager = async () => {
    if (!id) return;
    const operations = operationsRef.current;
    setChapterManagerLoading(true);
    setIsChapterManagerOpen(true);
    try {
      const allChapters = await fetchAllChaptersForPlayback();
      if (allChapters) setChapterManagerChapters(allChapters);
    } catch (err) {
      if (operationsRef.current !== operations) return;
      console.error('Failed to fetch chapters for manager', err);
      setChapterManagerChapters([]);
    } finally {
      if (operationsRef.current === operations) setChapterManagerLoading(false);
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
        if (!page) return;
        clearTimeout(chapterScrollTimer.current);
        chapterScrollTimer.current = setTimeout(() => doScroll(targetChapter.id, page?.groupIndex ?? currentGroupIndex), 120);
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
      await saveFavorite();
    } catch (err) {
      console.error('Failed to toggle favorite', err);
    }
  };

  const handleWriteMetadata = async () => {
    try {
      if (!confirm(t('bookshelf.writeMetadataConfirm'))) {
        return;
      }
      await operationsRef.current?.writeMetadata(id!);
      alert(t('bookshelf.writeMetadataStarted'));
    } catch (err) {
      console.error('Failed to write metadata', err);
      alert(apiErrorMessage(err, t('bookshelf.writeMetadataFailed')));
    }
  };

  const handleEditSave = async () => {
    if (editSaving) return;
    setEditSaving(true);
    setEditSaveError(null);
    try {
      const payload: Partial<Book> = {
        title: editData.title,
        author: editData.author,
        narrator: editData.narrator,
        tags: editData.tags,
        genre: editData.genre,
        year: editData.year,
        cover_url: editData.cover_url,
        skip_intro: editData.skip_intro,
        skip_outro: editData.skip_outro,
        chapter_regex: editData.chapter_regex,
        description: editData.description,
        // A normal manual metadata save locks the book. If the user explicitly
        // changes the toggle, preserve that choice even when it is off.
        manual_corrected: editLockTouched ? editData.manual_corrected === true : true,
      };
      
      const operations = operationsRef.current;
      if (!operations || !book) return;
      const result = await saveBook(payload, editChapterGroupOrder, editCoverFile);
      if (!result || operationsRef.current !== operations) return;
      usePlayerStore.getState().updateBook(result.book);
      if (result.rescanning) alert(t('bookshelf.regexSavedRescanning'));

      setIsEditModalOpen(false);
      setEditCoverFile(null);
      setEditLockTouched(false);
    } catch (error) {
      setEditSaveError(apiErrorMessage(error, t(editCoverFile ? 'bookshelf.coverUploadFailed' : 'common.saveFailed')));
    } finally {
      setEditSaving(false);
    }
  };

  const handleDelete = async () => {
    try {
      setDeleting(true);
      await operationsRef.current?.deleteBooks([id!], deleteSourceFiles);
      navigate('/', { replace: true });
    } catch (err) {
      console.error('Failed to delete book', err);
      alert(apiErrorMessage(err, t('bookshelf.deleteBookFailed')));
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

  // Show a Flutter-style auto-hiding scrollbar on the main content area
  // while this page is displayed: it fades in during scrolling and fades
  // out shortly after scrolling stops.
  useEffect(() => {
    const container = document.getElementById('main-content');
    if (!container) return;
    let hideTimer: number | undefined;
    const handleScroll = () => {
      container.classList.add('is-scrolling');
      window.clearTimeout(hideTimer);
      hideTimer = window.setTimeout(() => {
        container.classList.remove('is-scrolling');
      }, 800);
    };
    container.classList.add('auto-hide-scrollbar');
    container.addEventListener('scroll', handleScroll, { passive: true });
    return () => {
      window.clearTimeout(hideTimer);
      container.removeEventListener('scroll', handleScroll);
      container.classList.remove('auto-hide-scrollbar', 'is-scrolling');
    };
  }, []);

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
            setEditLockTouched(false);
            setEditCoverFile(null);
            setEditSaveError(null);
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
        <div className="-mx-4 sm:-mx-6 md:-mx-8">
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
        </div>

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
            invalidateChapters();
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
            void reloadBook().catch(error => console.error('Failed to reload scraped book', error));
          }}
        />
      )}

      {isEditModalOpen && book && (
        <EditBookModal
          editData={editData}
          coverFile={editCoverFile}
          onChangeCoverFile={setEditCoverFile}
          saving={editSaving}
          saveError={editSaveError}
          showRegexGenerator={showRegexGenerator}
          genFilename={genFilename}
          genNum={genNum}
          genTitle={genTitle}
          genResult={genResult}
          chapterGroupOrder={editChapterGroupOrder}
          onChangeEditData={setEditData}
          onChangeMetadataLock={(locked) => {
            setEditLockTouched(true);
            setEditData(prev => ({ ...prev, manual_corrected: locked }));
          }}
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
            setDeleteSourceFiles(false);
            setIsDeleteModalOpen(true);
          }}
          onSave={handleEditSave}
          onWriteMetadata={handleWriteMetadata}
          showWriteMetadata={book.library_type === 'local' || book.can_write_metadata_files === true}
        />
      )}

      {/* Delete Confirmation Modal */}
      {isDeleteModalOpen && book && (
        <DeleteLibraryItemModal
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
