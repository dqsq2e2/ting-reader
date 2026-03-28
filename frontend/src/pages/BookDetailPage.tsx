import React, { useEffect, useState, useRef } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import apiClient from '../api/client';
import type { Book, Chapter } from '../types';
import { usePlayerStore } from '../store/playerStore';

import ChapterManagerModal from '../components/ChapterManagerModal';
import ScrapeDiffModal from '../components/ScrapeDiffModal';
import { 
  Play, 
  Heart, 
  ChevronLeft, 
  ChevronDown, 
  ChevronUp, 
  Clock, 
  User, 
  Mic2,
  ListMusic,
  Info,
  Edit,
  Save,
  X,
  Loader2,
  Trash2,
  AlertTriangle,
  Settings,
  RefreshCw,
  Wand2,
  FileSignature
} from 'lucide-react';
import { getCoverUrl } from '../utils/image';
import { useAuthStore } from '../store/authStore';
import ExpandableTitle from '../components/ExpandableTitle';
import { setAlpha, toSolidColor, isLight, isTooLight } from '../utils/color';

const BookDetailPage: React.FC = () => {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { user } = useAuthStore();
  const [book, setBook] = useState<Book | null>(null);
  const [chapters, setChapters] = useState<Chapter[]>([]);
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
  const [themeColor, setThemeColor] = useState<string | null>(book?.theme_color || null);
  const [isTagsExpanded, setIsTagsExpanded] = useState(false);
  const tagsRef = useRef<HTMLDivElement>(null);
  const [isTagsOverflowing, setIsTagsOverflowing] = useState(false);
  const descriptionRef = useRef<HTMLParagraphElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const hasInitialScrolled = useRef(false);
  const [highlightedChapterId, setHighlightedChapterId] = useState<string | null>(null);

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

  const { mainChapters, extraChapters } = React.useMemo(() => {
    return {
      mainChapters: chapters.filter(c => !c.isExtra),
      extraChapters: chapters.filter(c => c.isExtra)
    };
  }, [chapters]);

  const currentChapters = activeTab === 'main' ? mainChapters : extraChapters;

  const chaptersPerGroup = 100;
  const groups = React.useMemo(() => {
    const g = [];
    for (let i = 0; i < currentChapters.length; i += chaptersPerGroup) {
      const slice = currentChapters.slice(i, i + chaptersPerGroup);
      g.push({
        start: i + 1,
        end: i + slice.length,
        chapters: slice
      });
    }
    return g;
  }, [currentChapters]);

  const playBook = usePlayerStore((state) => state.playBook);
  const isPlaying = usePlayerStore((state) => state.isPlaying);
  const playChapter = usePlayerStore((state) => state.playChapter);

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
        const [bookRes, chaptersRes, settingsRes] = await Promise.all([
          apiClient.get(`/api/books/${id}`),
          apiClient.get(`/api/books/${id}/chapters`),
          apiClient.get('/api/settings')
        ]);
        const fetchedBook = bookRes.data;
        setBook(fetchedBook);
        setChapters(chaptersRes.data);
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

  // Find the chapter to resume or highlight
  const resumeChapter = React.useMemo(() => {
    if (!book || chapters.length === 0) return null;

    // 1. Priority: Currently playing chapter if it belongs to this book
    if (currentChapter && currentChapter.bookId === book.id) {
      return currentChapter;
    } 
    
    // 2. Fallback: Most recently played chapter from history
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const playedChapters = [...chapters].filter(c => (c as any).progressUpdatedAt);
    if (playedChapters.length > 0) {
      playedChapters.sort((a, b) => {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        return new Date((b as any).progressUpdatedAt).getTime() - new Date((a as any).progressUpdatedAt).getTime();
      });
      return playedChapters[0];
    }
    return null;
  }, [book, chapters, currentChapter]);

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

  const scrollToChapterElement = (chapterId: string, list: Chapter[]) => {
      // Calculate group index
      const index = list.findIndex(c => c.id === chapterId);
      if (index !== -1) {
        const groupIndex = Math.floor(index / chaptersPerGroup);
        if (currentGroupIndex !== groupIndex) {
          setCurrentGroupIndex(groupIndex);
          // Wait for group switch
          setTimeout(() => doScroll(chapterId, groupIndex), 100);
          return;
        }
        doScroll(chapterId, groupIndex);
      }
  };

  const handlePlayClick = () => {
    if (resumeChapter) {
      // If we have a resume chapter, play it and scroll to it
      playChapter(book!, currentChapters, resumeChapter);
      
      // Scroll logic
      const targetChapter = resumeChapter;
      
      // Determine if target chapter is in main or extra
      const inMain = mainChapters.find(c => c.id === targetChapter.id);
      const inExtra = extraChapters.find(c => c.id === targetChapter.id);
      
      let targetList = currentChapters;
      
      if (inMain) {
        if (activeTab !== 'main') {
          setActiveTab('main');
          // Wait for tab switch then continue
          setTimeout(() => scrollToChapterElement(targetChapter.id, mainChapters), 100);
          return;
        }
        targetList = mainChapters;
      } else if (inExtra) {
        if (activeTab !== 'extra') {
          setActiveTab('extra');
          setTimeout(() => scrollToChapterElement(targetChapter.id, extraChapters), 100);
          return;
        }
        targetList = extraChapters;
      }
      
      scrollToChapterElement(targetChapter.id, targetList);
    } else {
      // Default play behavior
      playBook(book!, currentChapters);
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
      <div className="flex items-center justify-center h-full">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-primary-600"></div>
      </div>
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
        <div className={`flex flex-col md:flex-row gap-6 md:gap-8 ${coverShape === 'square' ? 'md:items-center' : ''}`}>
          <div className="w-48 md:w-72 mx-auto md:mx-0 shrink-0">
            <div className={`${coverShape === 'square' ? 'aspect-square' : 'aspect-[3/4]'} rounded-3xl overflow-hidden shadow-2xl border border-slate-200 dark:border-slate-800`}>
              <img 
                src={getCoverUrl(displayCoverUrl, displayLibraryId, book.id)} 
                alt={book.title}
                className="w-full h-full object-cover rounded-lg shadow-xl"
                referrerPolicy="no-referrer"
                onError={(e) => {
                  const target = e.target as HTMLImageElement;
                  target.src = 'https://placehold.co/300x400?text=No+Cover';
                  target.onerror = null;
                }}
              />
            </div>
          </div>

          <div className="flex-1 space-y-6 text-center md:text-left flex flex-col">
            <div className="space-y-3 min-w-0">
              <ExpandableTitle 
                title={book.title} 
                className={`font-bold text-slate-900 dark:text-white leading-tight transition-all duration-300 text-xl sm:text-2xl md:text-3xl`}
                maxLines={2}
              />
              <div className="flex flex-wrap justify-center md:justify-start gap-x-4 gap-y-2 mt-4 text-sm">
                <div className="flex items-center gap-1.5 text-slate-600 dark:text-slate-400">
                  <User size={16} className="text-primary-500" />
                  <span className="font-bold">{book.author || '未知作者'}</span>
                </div>
                <div className="flex items-center gap-1.5 text-slate-600 dark:text-slate-400">
                  <Mic2 size={16} className="text-primary-500" />
                  <span className="font-bold">{book.narrator || '未知演播'}</span>
                </div>
                <div className="flex items-center gap-1.5 text-slate-600 dark:text-slate-400">
                  <ListMusic size={16} className="text-primary-500" />
                  <span className="font-bold">{chapters.length} 章节</span>
                </div>
              </div>

              {book.tags && (
                <div className="mt-3 flex items-start justify-center md:justify-start w-full gap-2">
                  <div 
                    ref={tagsRef}
                    className={`flex flex-wrap gap-2 transition-all duration-300 overflow-hidden justify-center md:justify-start ${
                      isTagsExpanded ? 'max-h-[500px]' : 'max-h-[32px]'
                    }`}
                  >
                    {book.tags.split(/[,，]/).filter(tag => tag.trim()).map((tag, index) => (
                      <span 
                        key={index}
                        className="px-2.5 py-1 bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-400 text-xs font-bold rounded-lg border border-slate-200/50 dark:border-slate-700/50 whitespace-nowrap"
                      >
                        {tag.trim()}
                      </span>
                    ))}
                    {isTagsExpanded && (
                      <button 
                        onClick={() => setIsTagsExpanded(false)}
                        className="px-2 py-0.5 text-[10px] font-bold text-primary-500 hover:text-primary-600 flex items-center gap-0.5 bg-primary-50 dark:bg-primary-900/20 rounded-md border border-primary-100 dark:border-primary-900/30 shadow-sm self-center"
                      >
                        <ChevronUp size={10} /> 收起
                      </button>
                    )}
                  </div>
                  {isTagsOverflowing && !isTagsExpanded && (
                    <button 
                      onClick={() => setIsTagsExpanded(true)}
                      className="shrink-0 px-2 py-0.5 text-[10px] font-bold text-primary-500 hover:text-primary-600 flex items-center gap-0.5 bg-primary-50 dark:bg-primary-900/20 rounded-md border border-primary-100 dark:border-primary-900/30 shadow-sm mt-1"
                    >
                      <ChevronDown size={10} /> 更多
                    </button>
                  )}
                </div>
              )}
            </div>

            <div className="w-full flex flex-col gap-3 md:max-w-md mx-auto md:mx-0">
              <button 
                onClick={handlePlayClick}
                className="w-full flex items-center justify-center gap-2 px-5 sm:px-8 py-3.5 sm:py-4 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-2xl shadow-xl shadow-primary-500/30 transition-all active:scale-95 group"
                style={effectiveThemeColor ? { 
                  backgroundColor: toSolidColor(effectiveThemeColor),
                  boxShadow: `0 10px 20px -5px ${setAlpha(effectiveThemeColor, 0.3)}`,
                  color: isLight(effectiveThemeColor) ? '#475569' : '#ffffff'
                } : {}}
              >
                <Play size={18} fill="currentColor" />
                {resumeChapter ? '继续播放' : '立即播放'}
              </button>

              <div className="w-full flex gap-2 sm:gap-3">
                <button 
                  onClick={toggleFavorite}
                  className={`flex-1 min-w-0 px-3 sm:px-4 py-3 rounded-2xl border transition-all active:scale-95 flex items-center justify-center gap-2 font-bold text-sm ${
                    isFavorite 
                      ? 'bg-red-50 border-red-100 text-red-500 dark:bg-red-900/20 dark:border-red-900/30' 
                      : 'bg-white dark:bg-slate-900 border-slate-200 dark:border-slate-800 text-slate-500 dark:text-slate-300 hover:text-red-500'
                  }`}
                >
                  <Heart size={20} fill={isFavorite ? "currentColor" : "none"} />
                  收藏
                </button>
                
                {user?.role === 'admin' && (
                  <>
                    <button 
                      onClick={() => setIsScrapeDiffOpen(true)}
                      className="flex-1 min-w-0 px-3 sm:px-4 py-3 rounded-2xl border bg-white dark:bg-slate-900 border-slate-200 dark:border-slate-800 text-slate-500 dark:text-slate-300 hover:text-primary-600 transition-all active:scale-95 flex items-center justify-center gap-2 font-bold text-sm"
                      title="刮削元数据"
                    >
                      <RefreshCw size={20} />
                      刮削
                    </button>
                    <button 
                      onClick={() => {
                        setEditData({ 
                          ...book,
                          coverUrl: displayCoverUrl,
                          themeColor: displayThemeColor,
                          libraryType: displayLibraryType,
                          skipIntro: book.skipIntro,
                          skipOutro: book.skipOutro
                        });
                        setIsEditModalOpen(true);
                      }}
                      className="flex-1 min-w-0 px-3 sm:px-4 py-3 rounded-2xl border bg-white dark:bg-slate-900 border-slate-200 dark:border-slate-800 text-slate-500 dark:text-slate-300 hover:text-primary-600 transition-all active:scale-95 flex items-center justify-center gap-2 font-bold text-sm"
                    >
                      <Edit size={20} />
                      编辑
                    </button>
                  </>
                )}
              </div>
            </div>

            <div 
              className="mt-auto space-y-3 p-4 rounded-2xl border border-slate-100 dark:border-slate-800/50 relative group/desc"
              style={effectiveThemeColor ? { 
                  backgroundColor: setAlpha(effectiveThemeColor, 0.08)
                } : {}}
              >
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2 text-slate-900 dark:text-white font-bold text-sm uppercase tracking-wider opacity-60">
                  <Info size={16} />
                  简介内容
                </div>
              </div>
              <div className="relative">
                <p 
                  ref={descriptionRef}
                  className={`text-sm md:text-base text-slate-600 dark:text-slate-400 leading-relaxed transition-all duration-300 ${
                    !isDescriptionExpanded ? 'line-clamp-2' : ''
                  }`}
                >
                  {book.description || '暂无简介'}
                </p>
              {(isOverflowing || isDescriptionExpanded) && (
                <button 
                  onClick={() => setIsDescriptionExpanded(!isDescriptionExpanded)}
                  className="mt-2 text-primary-600 hover:text-primary-700 text-sm font-bold flex items-center gap-1 transition-colors"
                >
                  {isDescriptionExpanded ? (
                    <><ChevronUp size={16} />收起详情</>
                  ) : (
                    <><ChevronDown size={16} />展开全部</>
                  )}
                </button>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* Chapters List */}
      <div className="bg-white dark:bg-slate-900 rounded-3xl p-4 md:p-6 shadow-sm border border-slate-100 dark:border-slate-800">
        <div className="flex flex-col sm:flex-row sm:items-center justify-between mb-6 gap-4">
          <h2 className="text-xl md:text-2xl font-bold dark:text-white flex items-center gap-2">
            <ListMusic size={24} className="text-primary-600" />
            章节列表
            {user?.role === 'admin' && (
              <button 
                onClick={() => setIsChapterManagerOpen(true)}
                className="ml-2 p-1.5 text-slate-400 hover:text-primary-600 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-lg transition-colors"
                title="管理章节"
              >
                <Settings size={20} />
              </button>
            )}
          </h2>
          
          {extraChapters.length > 0 && (
            <div className="flex bg-slate-100 dark:bg-slate-800 p-1 rounded-xl self-start">
              <button 
                onClick={() => { setActiveTab('main'); setCurrentGroupIndex(0); }}
                className={`px-4 py-1.5 rounded-lg text-sm font-bold transition-all ${
                  activeTab === 'main' 
                    ? 'bg-white dark:bg-slate-700 text-primary-600 shadow-sm' 
                    : 'text-slate-500 hover:text-slate-700 dark:hover:text-slate-300'
                }`}
              >
                正文 ({mainChapters.length})
              </button>
              <button 
                onClick={() => { setActiveTab('extra'); setCurrentGroupIndex(0); }}
                className={`px-4 py-1.5 rounded-lg text-sm font-bold transition-all ${
                  activeTab === 'extra' 
                    ? 'bg-white dark:bg-slate-700 text-primary-600 shadow-sm' 
                    : 'text-slate-500 hover:text-slate-700 dark:hover:text-slate-300'
                }`}
              >
                番外 ({extraChapters.length})
              </button>
            </div>
          )}
        </div>

        {/* Chapter Groups Selector */}
        {groups.length > 1 && (
          <div className="relative group/nav mb-6 flex items-center">
            <button 
              onClick={() => scrollGroups('left')}
              className="absolute -left-4 sm:-left-7 top-1/2 -translate-y-1/2 z-10 p-1 bg-white/90 dark:bg-slate-800/90 backdrop-blur shadow-md rounded-full opacity-0 group-hover/nav:opacity-100 transition-opacity hidden sm:block border border-slate-100 dark:border-slate-700"
            >
              <ChevronLeft size={20} className="text-slate-600 dark:text-slate-400" />
            </button>
            <div 
              ref={scrollRef}
              className="flex gap-2 overflow-x-auto no-scrollbar scroll-smooth snap-x pb-2 px-1 mx-1 w-full"
            >
              {groups.map((group, index) => (
                <button
                  key={index}
                  id={`group-tab-${index}`}
                  onClick={() => setCurrentGroupIndex(index)}
                className={`px-4 py-2 rounded-xl text-sm font-bold transition-all border shrink-0 snap-start ${
                  currentGroupIndex === index
                    ? `text-white shadow-lg shadow-black/10 ${!effectiveThemeColor ? 'bg-primary-600 border-primary-600' : ''}`
                    : 'bg-white dark:bg-slate-800 text-slate-600 dark:text-slate-400 border-slate-200 dark:border-slate-700 hover:bg-slate-50'
                }`}
                style={currentGroupIndex === index && effectiveThemeColor ? { 
                  backgroundColor: toSolidColor(effectiveThemeColor),
                  borderColor: toSolidColor(effectiveThemeColor),
                  color: isLight(effectiveThemeColor) ? '#475569' : '#ffffff'
                } : {}}
              >
                第 {group.start}-{group.end} 章
              </button>
              ))}
            </div>
            <button 
              onClick={() => scrollGroups('right')}
              className="absolute -right-4 sm:-right-7 top-1/2 -translate-y-1/2 z-10 p-1 bg-white/90 dark:bg-slate-800/90 backdrop-blur shadow-md rounded-full opacity-0 group-hover/nav:opacity-100 transition-opacity hidden sm:block border border-slate-100 dark:border-slate-700"
            >
              <ChevronLeft size={20} className="rotate-180 text-slate-600 dark:text-slate-400" />
            </button>
          </div>
        )}

        <div className="space-y-3">
          {(groups[currentGroupIndex]?.chapters || currentChapters).map((chapter, index) => {
            const actualIndex = currentGroupIndex * chaptersPerGroup + index;
            const isCurrent = currentChapter?.id === chapter.id;
            const isActive = isCurrent || highlightedChapterId === chapter.id;

            return (
              <div 
                key={chapter.id}
                id={`chapter-${chapter.id}`}
                onClick={() => playChapter(book!, currentChapters, chapter)}
                className={`group flex items-center justify-between p-4 rounded-2xl cursor-pointer transition-all border ${
                  isActive 
                    ? 'bg-opacity-10 border-opacity-20' 
                    : 'bg-white dark:bg-slate-900 border-slate-100 dark:border-slate-800 hover:border-primary-200 dark:hover:border-primary-800'
                }`}
                style={isActive && effectiveThemeColor ? { 
                  backgroundColor: setAlpha(effectiveThemeColor, 0.1),
                  borderColor: setAlpha(effectiveThemeColor, 0.3),
                } : {}}
              >
                <div 
                  className="flex items-center gap-3 sm:gap-4 min-w-0 flex-1 cursor-pointer"
                >
                  <div 
                    className={`w-10 h-10 sm:w-12 sm:h-12 rounded-xl flex items-center justify-center font-bold text-base sm:text-lg shrink-0 ${
                      isActive ? `text-white ${!effectiveThemeColor ? 'bg-primary-600' : ''}` : 'bg-slate-100 dark:bg-slate-800 text-slate-500 dark:text-slate-400'
                    }`}
                    style={isActive && effectiveThemeColor ? { 
                      backgroundColor: toSolidColor(effectiveThemeColor),
                      color: isLight(effectiveThemeColor) ? '#475569' : '#ffffff'
                    } : {}}
                  >
                    {chapter.chapter_index || (actualIndex + 1)}
                  </div>
                  <div className="min-w-0">
                    <p 
                      className={`font-bold truncate ${isActive ? '' : 'text-slate-900 dark:text-white'}`}
                      style={isActive && effectiveThemeColor ? { color: toSolidColor(effectiveThemeColor) } : {}}
                    >
                      {chapter.title}
                    </p>
                    <div className="flex items-center gap-2 mt-1">
                      <div className="flex items-center gap-1 text-xs text-slate-400 font-medium">
                        <Clock size={12} />
                        {formatDuration(chapter.duration)}
                      </div>
                      {getChapterProgressText(chapter) && (
                        <div 
                          className={`text-[10px] font-bold px-1.5 py-0.5 rounded-md whitespace-nowrap ${
                            getChapterProgressText(chapter) === '已播完' 
                              ? 'bg-green-50 text-green-500 dark:bg-green-900/20' 
                              : 'bg-primary-50 text-primary-600 dark:bg-primary-900/20'
                          }`}
                        >
                          {getChapterProgressText(chapter)}
                        </div>
                      )}
                    </div>
                  </div>
                </div>
                
                <div className="flex items-center gap-4">
                  {isCurrent && isPlaying ? (
                    <div className="flex gap-1 items-end h-5">
                      <div className={`w-1 animate-music-bar-1 rounded-full ${!effectiveThemeColor ? 'bg-primary-600' : ''}`} style={effectiveThemeColor ? { backgroundColor: toSolidColor(effectiveThemeColor) } : {}}></div>
                      <div className={`w-1 animate-music-bar-2 rounded-full ${!effectiveThemeColor ? 'bg-primary-600' : ''}`} style={effectiveThemeColor ? { backgroundColor: toSolidColor(effectiveThemeColor) } : {}}></div>
                      <div className={`w-1 animate-music-bar-3 rounded-full ${!effectiveThemeColor ? 'bg-primary-600' : ''}`} style={effectiveThemeColor ? { backgroundColor: toSolidColor(effectiveThemeColor) } : {}}></div>
                    </div>
                  ) : (
                    <div 
                      className="w-10 h-10 rounded-full bg-slate-50 dark:bg-slate-800 flex items-center justify-center opacity-0 group-hover:opacity-100 transition-all cursor-pointer hover:scale-105"
                      onClick={(e) => {
                        e.stopPropagation();
                        playChapter(book!, currentChapters, chapter);
                      }}
                    >
                      <Play size={16} className="text-primary-600 ml-1" fill="currentColor" style={effectiveThemeColor ? { color: toSolidColor(effectiveThemeColor) } : {}} />
                    </div>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      </div>

      {/* Chapter Manager Modal */}
      {isChapterManagerOpen && book && (
        <ChapterManagerModal
          bookId={book.id}
          initialChapters={chapters}
          onClose={() => setIsChapterManagerOpen(false)}
          onSave={() => {
            // Reload chapters
            apiClient.get(`/api/books/${id}/chapters`).then(res => setChapters(res.data));
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
            apiClient.get(`/api/books/${id}/chapters`).then(res => setChapters(res.data));
          }}
        />
      )}

      {/* Edit Modal */}
      {isEditModalOpen && (
        <div className="fixed inset-0 z-[200] flex items-center justify-center p-4">
          <div className="absolute inset-0 bg-slate-900/60 backdrop-blur-sm" onClick={() => setIsEditModalOpen(false)}></div>
          <div className="relative w-full max-w-2xl max-h-[90vh] bg-white dark:bg-slate-900 rounded-3xl shadow-2xl overflow-y-auto animate-in zoom-in-95 duration-200 no-scrollbar">
            {showRegexGenerator ? (
                <div className="p-4 sm:p-6 md:p-8">
                    <div className="flex items-center justify-between mb-6">
                        <h2 className="text-xl font-bold dark:text-white flex items-center gap-2">
                            <Wand2 className="text-primary-600" /> 正则生成器
                        </h2>
                        <button onClick={() => setShowRegexGenerator(false)}><X size={24} className="text-slate-400" /></button>
                    </div>
                    
                    <div className="space-y-4">
                        <div className="space-y-1">
                            <label className="text-xs font-bold text-slate-500">示例文件名 (不含后缀)</label>
                            <input 
                                type="text" 
                                value={genFilename}
                                onChange={e => setGenFilename(e.target.value)}
                                placeholder="例如：书名 第1集 章节名"
                                className="w-full px-4 py-2 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                            />
                        </div>
                        <div className="grid grid-cols-2 gap-4">
                            <div className="space-y-1">
                                <label className="text-xs font-bold text-slate-500">提取章节号</label>
                                <input 
                                    type="text" 
                                    value={genNum}
                                    onChange={e => setGenNum(e.target.value)}
                                    placeholder="例如：1"
                                    className="w-full px-4 py-2 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                                />
                            </div>
                            <div className="space-y-1">
                                <label className="text-xs font-bold text-slate-500">提取章节名</label>
                                <input 
                                    type="text" 
                                    value={genTitle}
                                    onChange={e => setGenTitle(e.target.value)}
                                    placeholder="例如：章节名"
                                    className="w-full px-4 py-2 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                                />
                            </div>
                        </div>
                        
                        <button 
                            onClick={handleGenerateRegex}
                            disabled={!genFilename || !genNum || !genTitle}
                            className="w-full py-3 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-xl shadow-lg shadow-primary-500/30 transition-all disabled:opacity-50"
                        >
                            生成规则
                        </button>
                        
                        {genResult && (
                            <div className="mt-6 p-4 bg-slate-50 dark:bg-slate-800/50 rounded-xl border border-slate-200 dark:border-slate-700 space-y-3">
                                <div>
                                    <div className="text-xs font-bold text-slate-500 mb-1">生成正则</div>
                                    <code className="block p-2 bg-white dark:bg-slate-900 rounded-lg border border-slate-200 dark:border-slate-800 font-mono text-sm text-primary-600 break-all">
                                        {genResult.regex}
                                    </code>
                                </div>
                                
                                <div className="grid grid-cols-2 gap-4 text-sm">
                                    <div>
                                        <span className="text-slate-500 text-xs">提取序号:</span>
                                        <div className={genResult.capturedIndex === genNum ? "text-green-600 font-bold" : "text-red-500"}>
                                            {genResult.capturedIndex || "未匹配"}
                                        </div>
                                    </div>
                                    <div>
                                        <span className="text-slate-500 text-xs">提取标题:</span>
                                        <div className={genResult.capturedTitle === genTitle ? "text-green-600 font-bold" : "text-red-500"}>
                                            {genResult.capturedTitle || "未匹配"}
                                        </div>
                                    </div>
                                </div>
                                
                                <button 
                                    onClick={applyGeneratedRegex}
                                    className="w-full py-2 border-2 border-primary-600 text-primary-600 hover:bg-primary-50 dark:hover:bg-primary-900/20 font-bold rounded-xl transition-all"
                                >
                                    使用此规则
                                </button>
                            </div>
                        )}
                    </div>
                </div>
            ) : (
            <div className="p-4 sm:p-6 md:p-8">
              <div className="flex items-center justify-between mb-4 sm:mb-6">
                <h2 className="text-xl sm:text-2xl font-bold dark:text-white">编辑书籍元数据</h2>
              </div>

              <div className="grid grid-cols-1 md:grid-cols-2 gap-4 sm:gap-6">
                <div className="space-y-3 sm:space-y-4">
                  <div className="space-y-1">
                    <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">书名</label>
                    <input 
                      type="text" 
                      value={editData.title || ''}
                      onChange={e => setEditData({...editData, title: e.target.value})}
                      className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                    />
                  </div>
                  <div className="space-y-1">
                    <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">作者</label>
                    <input 
                      type="text" 
                      value={editData.author || ''}
                      onChange={e => setEditData({...editData, author: e.target.value})}
                      className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                    />
                  </div>
                  <div className="space-y-1">
                    <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">演播者</label>
                    <input 
                      type="text" 
                      value={editData.narrator || ''}
                      onChange={e => setEditData({...editData, narrator: e.target.value})}
                      className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                    />
                  </div>
                  <div className="space-y-1">
                    <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">标签 (逗号分隔)</label>
                    <input 
                      type="text" 
                      value={editData.tags || ''}
                      onChange={e => setEditData({...editData, tags: e.target.value})}
                      className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                    />
                  </div>

                  <div className="space-y-1">
                    <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">流派</label>
                    <input 
                      type="text" 
                      value={editData.genre || ''}
                      onChange={e => setEditData({...editData, genre: e.target.value})}
                      className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                    />
                  </div>
                  
                  <div className="space-y-1">
                    <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider flex justify-between items-center">
                        <span>章节正则清洗规则</span>
                        <button 
                            onClick={() => setShowRegexGenerator(true)}
                            className="text-primary-600 hover:text-primary-700 flex items-center gap-1"
                        >
                            <Wand2 size={12} /> 自动生成
                        </button>
                    </label>
                    <input 
                      type="text" 
                      value={editData.chapterRegex || ''}
                      onChange={e => setEditData({...editData, chapterRegex: e.target.value})}
                      placeholder="^...(\d+)...(.+)$"
                      className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base font-mono bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                    />
                    <p className="text-[10px] text-slate-400">用于从文件名提取章节号和标题。修改后需重新扫描生效。</p>
                  </div>
                </div>
                
                <div className="space-y-3 sm:space-y-4">
                  <div className="space-y-1">
                    <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">封面 URL</label>
                    <input 
                      type="text" 
                      value={editData.coverUrl || ''}
                      onChange={e => setEditData({...editData, coverUrl: e.target.value})}
                      className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                    />
                  </div>
                  <div className="grid grid-cols-2 gap-3 sm:gap-4">
                    <div className="space-y-1">
                      <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">跳过片头 (秒)</label>
                      <input 
                        type="number" 
                        value={editData.skipIntro || 0}
                        onChange={e => setEditData({...editData, skipIntro: parseInt(e.target.value) || 0})}
                        className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                      />
                    </div>
                    <div className="space-y-1">
                      <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">跳过片尾 (秒)</label>
                      <input 
                        type="number" 
                        value={editData.skipOutro || 0}
                        onChange={e => setEditData({...editData, skipOutro: parseInt(e.target.value) || 0})}
                        className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                      />
                    </div>
                  </div>
                </div>
              </div>

              <div className="mt-4 sm:mt-6 space-y-1">
                <label className="text-[10px] sm:text-xs font-bold text-slate-500 uppercase tracking-wider">简介</label>
                <textarea 
                  rows={4}
                  value={editData.description || ''}
                  onChange={e => setEditData({...editData, description: e.target.value})}
                  className="w-full px-3 py-2 sm:px-4 sm:py-2.5 text-sm sm:text-base bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white resize-none"
                />
              </div>

              <div className="flex flex-col-reverse sm:flex-row gap-3 sm:gap-4 mt-6 sm:mt-8">
                <button 
                  onClick={() => {
                    setIsEditModalOpen(false);
                    setIsDeleteModalOpen(true);
                  }}
                  className="px-4 py-2.5 sm:py-3 font-bold text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-xl transition-all flex items-center justify-center gap-2 sm:justify-start"
                >
                  <Trash2 size={18} className="sm:w-5 sm:h-5" />
                  删除书籍
                </button>
                <div className="flex-1" />
                <div className="flex gap-2 sm:gap-3">
                  <button 
                    onClick={handleWriteMetadata}
                    className="flex-1 sm:flex-none px-2.5 sm:px-6 py-2.5 sm:py-3 font-bold text-primary-600 bg-primary-50 hover:bg-primary-100 dark:bg-primary-900/20 dark:hover:bg-primary-900/30 rounded-xl transition-all flex items-center justify-center gap-1.5 sm:gap-2 text-xs sm:text-base whitespace-nowrap"
                    title="将元数据写入音频文件"
                  >
                    <FileSignature size={16} className="sm:w-5 sm:h-5" />
                    写入文件
                  </button>
                  <button 
                    onClick={() => setIsEditModalOpen(false)}
                    className="flex-1 sm:flex-none px-3 sm:px-6 py-2.5 sm:py-3 font-bold text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-xl transition-all text-xs sm:text-base whitespace-nowrap"
                  >
                    取消
                  </button>
                  <button 
                    onClick={handleEditSave}
                    className="flex-1 sm:flex-none px-3 sm:px-8 py-2.5 sm:py-3 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-xl shadow-lg shadow-primary-500/30 flex items-center justify-center gap-1.5 sm:gap-2 transition-all text-xs sm:text-base whitespace-nowrap"
                  >
                    <Save size={16} className="sm:w-5 sm:h-5" />
                    <span>保存<span className="hidden min-[380px]:inline">更改</span></span>
                  </button>
                </div>
              </div>
            </div>
            )}
          </div>
        </div>
      )}

      {/* Delete Confirmation Modal */}
      {isDeleteModalOpen && (
        <div className="fixed inset-0 z-[300] flex items-center justify-center p-4">
          <div className="absolute inset-0 bg-slate-900/60 backdrop-blur-sm" onClick={() => setIsDeleteModalOpen(false)}></div>
          <div className="relative w-full max-w-md bg-white dark:bg-slate-900 rounded-3xl shadow-2xl overflow-hidden animate-in zoom-in-95 duration-200">
            <div className="p-8">
              <div className="w-16 h-16 bg-red-50 dark:bg-red-900/20 rounded-2xl flex items-center justify-center text-red-500 mx-auto mb-6">
                <AlertTriangle size={32} />
              </div>
              
              <h3 className="text-xl font-bold text-center dark:text-white mb-2">确认删除书籍？</h3>
              <p className="text-slate-500 dark:text-slate-400 text-center mb-8">
                此操作将从书架中移除《{book.title}》，并清除所有相关的播放进度。
              </p>

              {book.libraryType === 'local' && (
                <div 
                  className="flex items-center gap-3 p-4 bg-slate-50 dark:bg-slate-800 rounded-2xl mb-8 cursor-pointer group"
                  onClick={() => setDeleteSourceFiles(!deleteSourceFiles)}
                >
                  <div className={`w-6 h-6 rounded-lg border-2 flex items-center justify-center transition-all ${
                    deleteSourceFiles 
                      ? 'bg-red-500 border-red-500 text-white' 
                      : 'border-slate-300 dark:border-slate-600 group-hover:border-red-400'
                  }`}>
                    {deleteSourceFiles && <X size={16} strokeWidth={3} />}
                  </div>
                  <div className="flex-1">
                    <p className="text-sm font-bold dark:text-white">同时删除本地源文件</p>
                    <p className="text-xs text-slate-500">警告：此操作不可撤销</p>
                  </div>
                </div>
              )}

              <div className="flex gap-4">
                <button 
                  onClick={() => setIsDeleteModalOpen(false)}
                  className="flex-1 py-3 font-bold text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-xl transition-all"
                >
                  取消
                </button>
                <button 
                  onClick={handleDelete}
                  disabled={deleting}
                  className="flex-1 py-3 bg-red-500 hover:bg-red-600 text-white font-bold rounded-xl shadow-lg shadow-red-500/30 flex items-center justify-center gap-2 transition-all disabled:opacity-50"
                >
                  {deleting ? <Loader2 className="animate-spin" size={20} /> : <Trash2 size={20} />}
                  确认删除
                </button>
              </div>
            </div>
          </div>
        </div>
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
