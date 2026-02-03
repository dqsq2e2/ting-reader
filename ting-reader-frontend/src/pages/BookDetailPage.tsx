import React, { useEffect, useState, useRef } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import apiClient from '../api/client';
import type { Book, Chapter } from '../types';
import { usePlayerStore } from '../store/playerStore';

import { 
  Play, 
  Heart, 
  Share2, 
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
  Sparkles,
  Loader2,
  Trash2,
  AlertTriangle
} from 'lucide-react';
import { getCoverUrl } from '../utils/image';
import { useAuthStore } from '../store/authStore';
import ExpandableTitle from '../components/ExpandableTitle';
import { setAlpha, toSolidColor } from '../utils/color';

const BookDetailPage: React.FC = () => {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { user } = useAuthStore();
  const [book, setBook] = useState<Book | null>(null);
  const [chapters, setChapters] = useState<Chapter[]>([]);
  const [loading, setLoading] = useState(true);
  const [isFavorite, setIsFavorite] = useState(false);
  const [isEditModalOpen, setIsEditModalOpen] = useState(false);
  const [isDeleteModalOpen, setIsDeleteModalOpen] = useState(false);
  const [deleteSourceFiles, setDeleteSourceFiles] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [editData, setEditData] = useState<Partial<Book>>({});
  const [scraping, setScraping] = useState(false);
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
      mainChapters: chapters.filter(c => !c.is_extra),
      extraChapters: chapters.filter(c => c.is_extra)
    };
  }, [chapters]);

  const currentChapters = activeTab === 'main' ? mainChapters : extraChapters;

  const chaptersPerGroup = 100;
  const groups = React.useMemo(() => {
    const g = [];
    for (let i = 0; i < currentChapters.length; i += chaptersPerGroup) {
      const slice = currentChapters.slice(i, i + chaptersPerGroup);
      g.push({
        start: slice[0]?.chapter_index || (i + 1),
        end: slice[slice.length - 1]?.chapter_index || (i + slice.length),
        chapters: slice
      });
    }
    return g;
  }, [currentChapters]);

  const playBook = usePlayerStore((state) => state.playBook);
  const currentChapter = usePlayerStore((state) => state.currentChapter);
  const isPlaying = usePlayerStore((state) => state.isPlaying);
  const playChapter = usePlayerStore((state) => state.playChapter);

  useEffect(() => {
    if (book?.theme_color) {
      setThemeColor(book.theme_color);
    }
  }, [book?.theme_color]);

  useEffect(() => {
    const fetchBookDetails = async () => {
      try {
        setLoading(true);
        const [bookRes, chaptersRes] = await Promise.all([
          apiClient.get(`/api/books/${id}`),
          apiClient.get(`/api/books/${id}/chapters`)
        ]);
        const fetchedBook = bookRes.data;
        setBook(fetchedBook);
        if (fetchedBook.theme_color) {
          setThemeColor(fetchedBook.theme_color);
        }
        setChapters(chaptersRes.data);
        setIsFavorite(bookRes.data.is_favorite);
        setCurrentGroupIndex(0); // Reset group index when book changes
      } catch (err) {
        console.error('Failed to fetch book details', err);
      } finally {
        setLoading(false);
      }
    };
    fetchBookDetails();
  }, [id]);

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
      console.error('Failed to toggle favorite', err);
    }
  };

  const handleEditSave = async () => {
    try {
      const dataToSave = { ...editData };
      // If cover changed, clear theme color so it's recalculated
      if (editData.cover_url && editData.cover_url !== book?.cover_url) {
        dataToSave.theme_color = undefined; // Will be handled by COALESCE or we can pass null
      }
      
      await apiClient.patch(`/api/books/${id}`, dataToSave);
      setBook({ ...book!, ...dataToSave });
      setIsEditModalOpen(false);
    } catch (err) {
      alert('保存失败');
    }
  };

  const handleScrape = async () => {
    setScraping(true);
    try {
      const response = await apiClient.get(`/api/scrape/ximalaya?keyword=${encodeURIComponent(book?.title || '')}`);
      setEditData({
        ...editData,
        title: response.data.title,
        author: response.data.author,
        cover_url: response.data.cover_url,
        description: response.data.description,
        tags: response.data.tags
      });
    } catch (err) {
      alert('刮削失败');
    } finally {
      setScraping(false);
    }
  };

  const handleDelete = async () => {
    try {
      setDeleting(true);
      await apiClient.delete(`/api/books/${id}?deleteFiles=${deleteSourceFiles}`);
      navigate('/', { replace: true });
    } catch (err) {
      console.error('Failed to delete book', err);
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
    if (!chapter.progress_position || !chapter.duration) return null;
    
    const percent = Math.floor((chapter.progress_position / chapter.duration) * 100);
    if (percent === 0) return null;
    if (percent >= 95) return '已播完';
    return `已播${percent}%`;
  };

  const getTitleFontSize = (title: string) => {
    // Responsive font size: subtle scaling from mobile to desktop
    return 'text-xl sm:text-2xl md:text-3xl';
  };

  const displayThemeColor = themeColor || book?.theme_color;

  useEffect(() => {
    if (displayThemeColor) {
      const bgColor = setAlpha(displayThemeColor, 0.08);
      document.documentElement.style.setProperty('--page-background', bgColor);
    }
    return () => {
      document.documentElement.style.setProperty('--page-background', 'transparent');
    };
  }, [displayThemeColor]);

  if (loading && !book) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-primary-600"></div>
      </div>
    );
  }

  if (!book) return <div className="dark:text-white p-8">未找到书籍</div>;

  return (
    <div className="flex-1 min-h-full flex flex-col p-4 sm:p-6 md:p-8 animate-in slide-in-from-bottom-4 duration-500">
      <div className="flex-1 max-w-6xl mx-auto space-y-8 w-full">
        {/* Header */}
        <button 
          onClick={() => navigate(-1)}
          className="flex items-center gap-2 text-slate-500 hover:text-primary-600 transition-colors"
        >
          <ChevronLeft size={20} />
          <span>返回</span>
        </button>

        {/* Book Info Section */}
        <div className="flex flex-col md:flex-row gap-6 md:gap-8">
          <div className="w-48 md:w-72 mx-auto md:mx-0 shrink-0">
            <div className="aspect-[3/4] rounded-3xl overflow-hidden shadow-2xl border border-slate-200 dark:border-slate-800">
              <img 
                src={getCoverUrl(book.cover_url, book.library_id, book.id)} 
                alt={book.title}
                crossOrigin="anonymous"
                className="w-full h-full object-cover rounded-lg shadow-xl"
                onError={(e) => {
                  (e.target as HTMLImageElement).src = 'https://placehold.co/300x400?text=No+Cover';
                }}
              />
            </div>
          </div>

          <div className="flex-1 space-y-6 text-center md:text-left flex flex-col">
            <div className="space-y-3 min-w-0">
              <ExpandableTitle 
                title={book.title} 
                className={`font-bold text-slate-900 dark:text-white leading-tight transition-all duration-300 ${getTitleFontSize(book.title || '')}`}
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

            <div className="flex gap-3 max-w-md mx-auto md:mx-0">
              <button 
                onClick={() => playBook(book, chapters)}
                className="flex-1 flex items-center justify-center gap-2 px-8 py-4 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-2xl shadow-xl shadow-primary-500/30 transition-all active:scale-95 group"
                style={themeColor ? { 
                  backgroundColor: toSolidColor(themeColor),
                  boxShadow: `0 10px 20px -5px ${setAlpha(themeColor, 0.3)}`
                } : {}}
              >
                <Play size={20} fill="currentColor" />
                立即播放
              </button>
              <button 
                onClick={toggleFavorite}
                className={`p-3.5 rounded-2xl border transition-all active:scale-95 ${
                  isFavorite 
                    ? 'bg-red-50 border-red-100 text-red-500 dark:bg-red-900/20 dark:border-red-900/30' 
                    : 'bg-white dark:bg-slate-900 border-slate-200 dark:border-slate-800 text-slate-400 hover:text-red-500'
                }`}
              >
                <Heart size={24} fill={isFavorite ? "currentColor" : "none"} />
              </button>
              {user?.role === 'admin' && (
                <button 
                  onClick={() => {
                    setEditData({ ...book });
                    setIsEditModalOpen(true);
                  }}
                  className="p-3.5 rounded-2xl border bg-white dark:bg-slate-900 border-slate-200 dark:border-slate-800 text-slate-400 hover:text-primary-600 transition-all active:scale-95"
                >
                  <Edit size={24} />
                </button>
              )}
            </div>

            <div 
              className="mt-auto space-y-3 p-4 rounded-2xl border border-slate-100 dark:border-slate-800/50 relative group/desc"
              style={themeColor ? { backgroundColor: themeColor } : {}}
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
          </h2>
          
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
            {extraChapters.length > 0 && (
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
            )}
          </div>
        </div>

        {/* Chapter Groups Selector */}
        {groups.length > 1 && (
          <div className="relative group/nav mb-6">
            <button 
              onClick={() => scrollGroups('left')}
              className="absolute left-0 top-1/2 -translate-y-1/2 z-10 p-1 bg-white/80 dark:bg-slate-800/80 backdrop-blur shadow-md rounded-r-xl opacity-0 group-hover/nav:opacity-100 transition-opacity hidden sm:block"
            >
              <ChevronLeft size={20} className="text-slate-600 dark:text-slate-400" />
            </button>
            <div 
              ref={scrollRef}
              className="flex gap-2 overflow-x-auto no-scrollbar scroll-smooth snap-x pb-2"
            >
              {groups.map((group, index) => (
                <button
                  key={index}
                  onClick={() => setCurrentGroupIndex(index)}
                  className={`px-4 py-2 rounded-xl text-sm font-bold transition-all border shrink-0 snap-start ${
                    currentGroupIndex === index
                      ? 'text-white shadow-lg shadow-black/10'
                      : 'bg-white dark:bg-slate-800 text-slate-600 dark:text-slate-400 border-slate-200 dark:border-slate-700 hover:bg-slate-50'
                  }`}
                  style={currentGroupIndex === index && themeColor ? { 
                    backgroundColor: toSolidColor(themeColor),
                    borderColor: toSolidColor(themeColor)
                  } : {}}
                >
                  第 {group.start}-{group.end} 章
                </button>
              ))}
            </div>
            <button 
              onClick={() => scrollGroups('right')}
              className="absolute right-0 top-1/2 -translate-y-1/2 z-10 p-1 bg-white/80 dark:bg-slate-800/80 backdrop-blur shadow-md rounded-l-xl opacity-0 group-hover/nav:opacity-100 transition-opacity hidden sm:block"
            >
              <ChevronLeft size={20} className="rotate-180 text-slate-600 dark:text-slate-400" />
            </button>
          </div>
        )}

        <div className="space-y-3">
          {(groups[currentGroupIndex]?.chapters || currentChapters).map((chapter, index) => {
            const actualIndex = currentGroupIndex * chaptersPerGroup + index;
            const isCurrent = currentChapter?.id === chapter.id;
            return (
              <div 
                key={chapter.id}
                onClick={() => playChapter(book!, chapters, chapter)}
                className={`group flex items-center justify-between p-4 rounded-2xl cursor-pointer transition-all border ${
                  isCurrent 
                    ? 'bg-opacity-10 border-opacity-20' 
                    : 'bg-white dark:bg-slate-900 border-slate-100 dark:border-slate-800 hover:border-primary-200 dark:hover:border-primary-800'
                }`}
                style={isCurrent && themeColor ? { 
                  backgroundColor: setAlpha(themeColor, 0.1),
                  borderColor: setAlpha(themeColor, 0.3),
                } : {}}
              >
                <div className="flex items-center gap-4 min-w-0">
                  <div 
                    className={`w-12 h-12 rounded-xl flex items-center justify-center font-bold text-lg shrink-0 ${
                      isCurrent ? 'text-white' : 'bg-slate-100 dark:bg-slate-800 text-slate-500 dark:text-slate-400'
                    }`}
                    style={isCurrent && themeColor ? { backgroundColor: toSolidColor(themeColor) } : {}}
                  >
                    {chapter.chapter_index || (actualIndex + 1)}
                  </div>
                  <div className="min-w-0">
                    <p 
                      className={`font-bold truncate ${isCurrent ? '' : 'text-slate-900 dark:text-white'}`}
                      style={isCurrent && themeColor ? { color: toSolidColor(themeColor) } : {}}
                    >
                      {chapter.title}
                    </p>
                    <div className="flex items-center gap-3 mt-1">
                      <div className="flex items-center gap-1 text-xs text-slate-400 font-medium">
                        <Clock size={12} />
                        {formatDuration(chapter.duration)}
                      </div>
                      {getChapterProgressText(chapter) && (
                        <div 
                          className={`text-[10px] font-bold px-1.5 py-0.5 rounded-md ${
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
                      <div className="w-1 animate-music-bar-1 rounded-full" style={themeColor ? { backgroundColor: toSolidColor(themeColor) } : {}}></div>
                      <div className="w-1 animate-music-bar-2 rounded-full" style={themeColor ? { backgroundColor: toSolidColor(themeColor) } : {}}></div>
                      <div className="w-1 animate-music-bar-3 rounded-full" style={themeColor ? { backgroundColor: toSolidColor(themeColor) } : {}}></div>
                    </div>
                  ) : (
                    <div className="w-10 h-10 rounded-full bg-slate-50 dark:bg-slate-800 flex items-center justify-center opacity-0 group-hover:opacity-100 transition-all">
                      <Play size={16} className="text-primary-600 ml-1" fill="currentColor" style={themeColor ? { color: toSolidColor(themeColor) } : {}} />
                    </div>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      </div>

      {/* Edit Modal */}
      {isEditModalOpen && (
        <div className="fixed inset-0 z-[200] flex items-center justify-center p-4">
          <div className="absolute inset-0 bg-slate-900/60 backdrop-blur-sm" onClick={() => setIsEditModalOpen(false)}></div>
          <div className="relative w-full max-w-2xl max-h-[90vh] bg-white dark:bg-slate-900 rounded-3xl shadow-2xl overflow-y-auto animate-in zoom-in-95 duration-200 no-scrollbar">
            <div className="p-6 md:p-8">
              <div className="flex items-center justify-between mb-6">
                <h2 className="text-2xl font-bold dark:text-white">编辑书籍元数据</h2>
                <button 
                  onClick={handleScrape}
                  disabled={scraping}
                  className="flex items-center gap-2 px-4 py-2 bg-primary-50 dark:bg-primary-900/20 text-primary-600 rounded-xl text-sm font-bold hover:bg-primary-100 transition-all disabled:opacity-50"
                >
                  {scraping ? <Loader2 className="animate-spin" size={16} /> : <Sparkles size={16} />}
                  自动刮削 (喜马拉雅)
                </button>
              </div>

              <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                <div className="space-y-4">
                  <div className="space-y-1">
                    <label className="text-xs font-bold text-slate-500 uppercase tracking-wider">书名</label>
                    <input 
                      type="text" 
                      value={editData.title || ''}
                      onChange={e => setEditData({...editData, title: e.target.value})}
                      className="w-full px-4 py-2.5 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                    />
                  </div>
                  <div className="space-y-1">
                    <label className="text-xs font-bold text-slate-500 uppercase tracking-wider">作者</label>
                    <input 
                      type="text" 
                      value={editData.author || ''}
                      onChange={e => setEditData({...editData, author: e.target.value})}
                      className="w-full px-4 py-2.5 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                    />
                  </div>
                  <div className="space-y-1">
                    <label className="text-xs font-bold text-slate-500 uppercase tracking-wider">演播者</label>
                    <input 
                      type="text" 
                      value={editData.narrator || ''}
                      onChange={e => setEditData({...editData, narrator: e.target.value})}
                      className="w-full px-4 py-2.5 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                    />
                  </div>
                  <div className="space-y-1">
                    <label className="text-xs font-bold text-slate-500 uppercase tracking-wider">标签 (逗号分隔)</label>
                    <input 
                      type="text" 
                      value={editData.tags || ''}
                      onChange={e => setEditData({...editData, tags: e.target.value})}
                      className="w-full px-4 py-2.5 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                    />
                  </div>
                </div>
                
                <div className="space-y-4">
                  <div className="space-y-1">
                    <label className="text-xs font-bold text-slate-500 uppercase tracking-wider">封面 URL</label>
                    <input 
                      type="text" 
                      value={editData.cover_url || ''}
                      onChange={e => setEditData({...editData, cover_url: e.target.value})}
                      className="w-full px-4 py-2.5 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                    />
                  </div>
                  <div className="grid grid-cols-2 gap-4">
                    <div className="space-y-1">
                      <label className="text-xs font-bold text-slate-500 uppercase tracking-wider">跳过片头 (秒)</label>
                      <input 
                        type="number" 
                        value={editData.skip_intro || 0}
                        onChange={e => setEditData({...editData, skip_intro: parseInt(e.target.value) || 0})}
                        className="w-full px-4 py-2.5 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                      />
                    </div>
                    <div className="space-y-1">
                      <label className="text-xs font-bold text-slate-500 uppercase tracking-wider">跳过片尾 (秒)</label>
                      <input 
                        type="number" 
                        value={editData.skip_outro || 0}
                        onChange={e => setEditData({...editData, skip_outro: parseInt(e.target.value) || 0})}
                        className="w-full px-4 py-2.5 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                      />
                    </div>
                  </div>
                </div>
              </div>

              <div className="mt-6 space-y-1">
                <label className="text-xs font-bold text-slate-500 uppercase tracking-wider">简介</label>
                <textarea 
                  rows={4}
                  value={editData.description || ''}
                  onChange={e => setEditData({...editData, description: e.target.value})}
                  className="w-full px-4 py-2.5 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white resize-none"
                />
              </div>

              <div className="flex gap-4 mt-8">
                <button 
                  onClick={() => {
                    setIsEditModalOpen(false);
                    setIsDeleteModalOpen(true);
                  }}
                  className="px-4 py-3 font-bold text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-xl transition-all flex items-center gap-2"
                >
                  <Trash2 size={20} />
                  删除书籍
                </button>
                <div className="flex-1" />
                <button 
                  onClick={() => setIsEditModalOpen(false)}
                  className="px-6 py-3 font-bold text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-xl transition-all"
                >
                  取消
                </button>
                <button 
                  onClick={handleEditSave}
                  className="px-8 py-3 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-xl shadow-lg shadow-primary-500/30 flex items-center justify-center gap-2 transition-all"
                >
                  <Save size={20} />
                  保存更改
                </button>
              </div>
            </div>
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

              {book.library_type === 'local' && (
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
