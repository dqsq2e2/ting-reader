import React, { useEffect, useState, useRef } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import apiClient from '../../core/api/client';
import type { Book, Chapter, Progress } from '../../core/types';
import { usePlayerStore } from '../../core/stores/playerStore';

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

const BookDetailPage: React.FC = () => {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
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
  const [themeColor, setThemeColor] = useState<string | null>(book?.themeColor ?? null);
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
      alert('生成失败');
    }
  };

  const applyGeneratedRegex = () => {
    if (genResult?.regex) {
      setEditData({ ...editData, chapterRegex: genResult.regex });
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
    setChapterAscending(true);
  }, [id]);

  // Clear highlighted chapter when current chapter changes (user plays a new chapter)
  const currentChapter = usePlayerStore((state) => state.currentChapter);
  useEffect(() => {
    if (currentChapter?.bookId === book?.id) {
      setHighlightedChapterId(null);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentChapter?.id, book?.id]);

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
      setThemeColor(book.themeColor || null);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [book?.themeColor]);

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
        setBook(fetchedBook);
        setBookProgress(progressRes);
        setHighlightedChapterId(progressRes?.chapterId || null);
        setIsFavorite(fetchedBook.isFavorite);
        setCurrentGroupIndex(0); // Reset group index when book changes

        // Load user settings
        const settings = settingsRes.data.settingsJson || {};
        if (settings.bookshelfCoverShape) {
          setCoverShape(settings.bookshelfCoverShape);
        }
      } catch (err) {
        console.error('获取书籍详情失败', err);
      } finally {
        setLoading(false);
      }
    };
    fetchBookDetails();
  }, [id]);

  const fetchChapterPage = React.useCallback(async (options?: {
    tab?: 'main' | 'extra';
    groupIndex?: number;
    targetChapterId?: string | null;
  }) => {
    if (!id) return;

    const requestedTab = options?.tab ?? activeTab;
    const requestedGroupIndex = options?.groupIndex ?? currentGroupIndex;
    const params: Record<string, unknown> = {
      limit: chaptersPerGroup,
      offset: requestedGroupIndex * chaptersPerGroup,
      order: 'asc',
    };
    if (!(options?.targetChapterId && !options?.tab)) {
      params.chapterType = requestedTab;
    }
    if (options?.targetChapterId) {
      params.targetChapterId = options.targetChapterId;
    }

    setChapterPageLoading(true);
    try {
      const res = await apiClient.get(`/api/books/${id}/chapters`, { params });
      const data = res.data;
      setChapters(data.chapters || []);
      setChapterTotals({
        total: data.total ?? 0,
        main: data.mainTotal ?? 0,
        extra: data.extraTotal ?? 0,
      });

      const resolvedTab = data.chapterType === 'extra' ? 'extra' : 'main';
      const resolvedGroupIndex = Math.floor((data.offset || 0) / chaptersPerGroup);
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
      console.error('获取章节分页失败', err);
      setChapters([]);
      return null;
    } finally {
      setChapterPageLoading(false);
    }
  }, [id, activeTab, currentGroupIndex]);

  useEffect(() => {
    if (!book) return;
    const targetChapterId = !hasInitialScrolled.current ? bookProgress?.chapterId : null;
    fetchChapterPage(
      targetChapterId
        ? { targetChapterId }
        : { tab: activeTab, groupIndex: currentGroupIndex }
    );
    hasInitialScrolled.current = true;
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [book?.id, activeTab, currentGroupIndex]);

  // Find the chapter to resume or highlight
  const resumeChapter = React.useMemo(() => {
    if (!book) return null;

    // 1. Priority: Currently playing chapter if it belongs to this book
    if (currentChapter && currentChapter.bookId === book.id) {
      return currentChapter;
    }

    if (bookProgress?.chapterId) {
      const progressChapter = chapters.find(c => c.id === bookProgress.chapterId);
      if (progressChapter) {
        return {
          ...progressChapter,
          progressPosition: bookProgress.position,
          progressUpdatedAt: bookProgress.updatedAt,
        };
      }
    }

    return chapters[0] || null;
  }, [book, chapters, currentChapter, bookProgress]);

  // Auto-highlight current chapter logic (without scroll)
  useEffect(() => {
    if (book?.id !== id) return;

    if (resumeChapter) {
      setHighlightedChapterId(resumeChapter.id);
    }
  }, [book?.id, id, resumeChapter]);

  const doScroll = (chapterId: string, groupIndex: number) => {
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
  };

  const scrollToChapterElement = (chapterId: string, groupIndex = currentGroupIndex) => {
      doScroll(chapterId, groupIndex);
  };

  const fetchAllChaptersForPlayback = async () => {
    if (allChaptersCacheRef.current) return allChaptersCacheRef.current;
    if (book && storeChapters.length > 0 && storeChapters.some(c => c.bookId === book.id)) {
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
      console.error('获取章节管理列表失败', err);
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
        const page = await fetchChapterPage({ targetChapterId: targetChapter.id });
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
      
      const text = resumeChapter && currentChapter?.bookId === book?.id
        ? `正在播放：${resumeChapter.title}`
        : resumeChapter
        ? `继续播放：${resumeChapter.title}`
        : '立即播放';
      
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
  }, [resumeChapter, currentChapter, book?.id]);

  const toggleFavorite = async () => {
    try {
      if (isFavorite) {
        await apiClient.delete(`/api/favorites/${id}`);
      } else {
        await apiClient.post(`/api/favorites/${id}`);
      }
      setIsFavorite(!isFavorite);
    } catch (err) {
      console.error('切换收藏状态失败', err);
    }
  };

  const handleWriteMetadata = async () => {
    try {
      if (!confirm('确定要将当前元数据写入到音频文件吗？这可能需要一些时间。')) {
        return;
      }
      await apiClient.post(`/api/books/${id}/write-metadata`);
      alert('已开始后台写入元数据，请稍候查看任务进度。');
    } catch (err) {
      console.error('写入元数据失败', err);
      alert('写入失败');
    }
  };

  const handleEditSave = async () => {
    try {
      const dataToSave = { ...editData };
      // If cover changed, clear theme color so it's recalculated
    if (editData.coverUrl && editData.coverUrl !== displayCoverUrl) {
        dataToSave.themeColor = undefined;
      }
      
      // The API expects camelCase for updates (client will convert to snake_case)
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const payload: Record<string, any> = { ...dataToSave };
      
      const res = await apiClient.patch(`/api/books/${id}`, payload);
      const updatedBookData = res.data;
      
      // Update local state - merge the changes
      const updatedBook = { ...book!, ...updatedBookData };
      // Preserve existing auxiliary fields if not in response
      if (book!.libraryType) updatedBook.libraryType = book!.libraryType;
      if (book!.isFavorite !== undefined) updatedBook.isFavorite = book!.isFavorite;
      
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
      if (payload.chapterRegex) {
          apiClient.post(`/api/libraries/${book!.libraryId}/scan`);
          alert('规则已保存。正在后台重新扫描该库以应用新规则...');
      }

      setIsEditModalOpen(false);
    } catch {
      alert('保存失败');
    }
  };

  const handleDelete = async () => {
    try {
      setDeleting(true);
      await apiClient.delete(`/api/books/${id}?deleteFiles=${deleteSourceFiles}`);
      navigate('/', { replace: true });
    } catch (err) {
      console.error('删除书籍失败', err);
      alert('删除书籍失败');
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
    if (!chapter.progressPosition || !chapter.duration) return null;
    
    const percent = Math.floor((chapter.progressPosition / chapter.duration) * 100);
    if (percent === 0) return null;
    if (percent >= 95) return '已播完';
    return `已播${percent}%`;
  };

  const displayThemeColor = book ? (book.themeColor || themeColor) : themeColor;
  // If the color is too light (close to white), we ignore it and use default to ensure text readability
  const effectiveThemeColor = displayThemeColor && !isTooLight(displayThemeColor) ? displayThemeColor : undefined;

  const displayCoverUrl = book ? book.coverUrl : undefined;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const displayLibraryId = book ? (book.libraryId || (book as any).library_id) : undefined;
  const displayLibraryType = book ? book.libraryType : undefined;

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

  if (!book) return <div className="dark:text-white p-8">未找到书籍</div>;

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
          <span>返回</span>
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
          resumeChapterBookMatches={currentChapter?.bookId === book.id}
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
              coverUrl: displayCoverUrl,
              themeColor: displayThemeColor ?? undefined,
              libraryType: displayLibraryType,
              skipIntro: book.skipIntro,
              skipOutro: book.skipOutro
            });
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
          chapterPageLoading={chapterPageLoading}
          scrollRef={scrollRef}
          currentChapterId={currentChapter?.id}
          highlightedChapterId={highlightedChapterId}
          isPlaying={isPlaying}
          effectiveThemeColor={effectiveThemeColor}
          onScrollGroups={scrollGroups}
          onSetActiveTab={setActiveTab}
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
            加载章节管理...
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
          onChangeEditData={setEditData}
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
