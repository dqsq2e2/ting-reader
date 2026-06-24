import React, { useEffect, useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import apiClient from '../../core/api/client';
import type { Book, Playlist, Progress, Series } from '../../core/types';
import BookCard from '../../shared/cards/BookCard';
import { getCoverUrl } from '../../core/utils/image';
import { usePlayerStore } from '../../core/stores/playerStore';
import { getCoverAspectClass, useBookshelfCoverShape, type CoverShape } from '../../core/hooks/useBookshelfCoverShape';
import { DEFAULT_HOME_LAYOUT, normalizeHomeLayout } from '../../core/utils/homeLayout';
import {
  collectPlaylistCovers,
  getPlaylistBookCount,
  type PlaylistCoverItem,
} from '../../core/utils/playlist';
import LoadingSpinner from '../../shared/ui/LoadingSpinner';
import {
  Calendar,
  ChevronRight,
  Clock,
  Headphones,
  Heart,
  History,
  Library,
  ListMusic,
  Play,
  Search,
  Sparkles,
  TrendingUp,
  RefreshCw,
} from 'lucide-react';

type HeroItem = {
  id: string;
  title: string;
  subtitle: string;
  description: string;
  coverUrl?: string;
  libraryId?: string;
  book?: Book;
  progress?: Progress;
};

const getSeriesCover = (series: Series): PlaylistCoverItem[] => {
  const coverBook = series.books?.[0];
  return [{
    id: series.id,
    title: series.title,
    coverUrl: series.coverUrl || coverBook?.coverUrl,
    libraryId: series.libraryId || coverBook?.libraryId,
    bookId: coverBook?.id,
  }];
};

const HomePage: React.FC = () => {
  const currentChapter = usePlayerStore((state) => state.currentChapter);
  const coverShape = useBookshelfCoverShape();
  const [recentPlays, setRecentPlays] = useState<Progress[]>([]);
  const [books, setBooks] = useState<Book[]>([]);
  const [favorites, setFavorites] = useState<Book[]>([]);
  const [series, setSeries] = useState<Series[]>([]);
  const [playlists, setPlaylists] = useState<Playlist[]>([]);
  const [playlistCoverSeed, setPlaylistCoverSeed] = useState(() => Date.now());
  const [homeLayout, setHomeLayout] = useState(DEFAULT_HOME_LAYOUT);
  const [loading, setLoading] = useState(true);
  const [activeHeroBookId, setActiveHeroBookId] = useState<string | null>(null);

  useEffect(() => {
    const fetchData = async () => {
      setLoading(true);
      const [recentRes, booksRes, favoritesRes, seriesRes, playlistsRes] = await Promise.allSettled([
        apiClient.get('/api/progress/recent'),
        apiClient.get('/api/books'),
        apiClient.get('/api/favorites'),
        apiClient.get('/api/v1/series'),
        apiClient.get('/api/playlists'),
      ]);

      if (recentRes.status === 'fulfilled') setRecentPlays(recentRes.value.data || []);
      if (booksRes.status === 'fulfilled') setBooks(booksRes.value.data || []);
      if (favoritesRes.status === 'fulfilled') setFavorites(favoritesRes.value.data || []);
      if (seriesRes.status === 'fulfilled') setSeries(seriesRes.value.data || []);
      if (playlistsRes.status === 'fulfilled') {
        setPlaylists(playlistsRes.value.data || []);
        setPlaylistCoverSeed(Date.now());
      }
      setLoading(false);
    };

    fetchData();
    window.addEventListener('focus', fetchData);
    return () => window.removeEventListener('focus', fetchData);
  }, []);

  useEffect(() => {
    const loadHomeLayout = async () => {
      try {
        const res = await apiClient.get('/api/settings');
        setHomeLayout(normalizeHomeLayout(res.data.settingsJson?.homeLayout ?? res.data.homeLayout));
      } catch (err) {
        console.error('加载首页设置失败', err);
      }
    };

    loadHomeLayout();
    window.addEventListener('focus', loadHomeLayout);
    return () => window.removeEventListener('focus', loadHomeLayout);
  }, []);

  const bookMap = useMemo(() => {
    const map = new Map<string, Book>();
    books.forEach(book => map.set(book.id, book));
    return map;
  }, [books]);

  const recentlyAddedBooks = useMemo(() => {
    return [...books]
      .sort((a, b) => new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime())
      .slice(0, 10);
  }, [books]);

  const heroItems = useMemo<HeroItem[]>(() => {
    const seen = new Set<string>();
    const items: HeroItem[] = [];

    recentPlays.forEach(progress => {
      const book = bookMap.get(progress.bookId);
      const id = book?.id || progress.bookId;
      if (!id || seen.has(id)) return;
      seen.add(id);
      items.push({
        id,
        title: book?.title || progress.bookTitle || '未知书籍',
        subtitle: progress.chapterTitle ? `上次听到 ${progress.chapterTitle}` : (book?.author || '继续收听'),
        description: book?.description || (progress.chapterTitle ? `继续从「${progress.chapterTitle}」开始，接上你的听书进度。` : '继续上次没有听完的内容。'),
        coverUrl: book?.coverUrl || progress.coverUrl,
        libraryId: book?.libraryId || progress.libraryId,
        book,
        progress,
      });
    });

    [...favorites, ...recentlyAddedBooks].forEach(book => {
      if (!book?.id || seen.has(book.id)) return;
      seen.add(book.id);
      items.push({
        id: book.id,
        title: book.title,
        subtitle: book.author || '今日推荐',
        description: book.description || '从书架里挑一本作品，开启今天的听书时间。',
        coverUrl: book.coverUrl,
        libraryId: book.libraryId,
        book,
      });
    });

    return items.slice(0, 10);
  }, [bookMap, favorites, recentPlays, recentlyAddedBooks]);

  const activeHeroItem = useMemo(() => (
    heroItems.find(item => item.id === activeHeroBookId) || heroItems[0]
  ), [activeHeroBookId, heroItems]);

  const activeHeroIndex = useMemo(() => (
    activeHeroItem ? heroItems.findIndex(item => item.id === activeHeroItem.id) : -1
  ), [activeHeroItem, heroItems]);

  const nextHeroItem = useMemo(() => {
    if (heroItems.length <= 1) return undefined;
    const nextIndex = activeHeroIndex >= 0
      ? (activeHeroIndex + 1) % heroItems.length
      : 0;
    return heroItems[nextIndex];
  }, [activeHeroIndex, heroItems]);

  const heroProgress = activeHeroItem?.progress;

  const recommendedBooks = useMemo(() => {
    const seen = new Set<string>();
    const source = [
      activeHeroItem?.book,
      ...favorites,
      ...recentPlays.map(progress => bookMap.get(progress.bookId)),
      ...recentlyAddedBooks,
    ].filter(Boolean) as Book[];

    return source.filter(book => {
      if (seen.has(book.id)) return false;
      seen.add(book.id);
      return true;
    }).slice(0, 8);
  }, [activeHeroItem?.book, bookMap, favorites, recentPlays, recentlyAddedBooks]);

  const listenMinutes = useMemo(() => {
    const seconds = recentPlays.reduce((sum, progress) => sum + Math.max(0, progress.position || 0), 0);
    return Math.round(seconds / 60);
  }, [recentPlays]);

  const getGreeting = () => {
    const hour = new Date().getHours();
    if (hour >= 5 && hour < 12) return '早上好';
    if (hour >= 12 && hour < 14) return '中午好';
    if (hour >= 14 && hour < 18) return '下午好';
    return '晚上好';
  };

  const coverAspectClass = getCoverAspectClass(coverShape);

  const handleCycleHero = () => {
    if (!nextHeroItem) return;
    setActiveHeroBookId(nextHeroItem.id);
  };

  if (loading) {
    return <LoadingSpinner />;
  }

  return (
    <div className="flex-1 min-h-full flex flex-col p-4 sm:p-6 md:p-8 animate-in fade-in duration-500">
      <div className="flex-1 space-y-8">
        <div className="flex flex-col lg:flex-row lg:items-center justify-between gap-4">
          <div>
            <p className="text-sm font-bold text-primary-600">{getGreeting()}</p>
            <h1 className="text-2xl md:text-4xl font-bold text-slate-900 dark:text-white mt-1">今天听点什么</h1>
            <p className="text-sm md:text-base text-slate-500 dark:text-slate-400 mt-2">推荐、最近、书单和你的听书节奏都在这里。</p>
          </div>

          <div className="flex items-center gap-2">
            <div className="hidden md:flex h-12 items-center gap-2 text-sm text-slate-500 bg-white dark:bg-slate-900 px-4 py-2.5 rounded-xl shadow-sm border border-slate-100 dark:border-slate-800">
              <Calendar size={16} />
              <span>{new Date().toLocaleDateString('zh-CN', { weekday: 'long', month: 'long', day: 'numeric' })}</span>
            </div>
            <Link
              to="/search"
              className="inline-flex h-12 items-center gap-2 px-4 py-2.5 bg-slate-900 dark:bg-white text-white dark:text-slate-900 rounded-xl text-sm font-bold shadow-lg shadow-slate-900/10 dark:shadow-white/5 hover:opacity-90 transition-opacity"
            >
              <Search size={18} />
              搜索内容
            </Link>
          </div>
        </div>

        {(homeLayout.showHero || homeLayout.showStats) && (
        <section className="grid grid-cols-1 gap-6">
          {homeLayout.showHero && (
          <div className="relative overflow-hidden rounded-3xl border border-white/70 dark:border-white/10 bg-[linear-gradient(135deg,#f8fafc_0%,#dbeafe_42%,#f0f9ff_100%)] dark:bg-[linear-gradient(135deg,#020617_0%,#172554_46%,#2e1065_100%)] text-slate-950 dark:text-white shadow-2xl shadow-sky-200/70 dark:shadow-slate-950/40">
            {activeHeroItem && (
              <img
                src={getCoverUrl(activeHeroItem.coverUrl, activeHeroItem.libraryId, activeHeroItem.id)}
                alt=""
                aria-hidden="true"
                referrerPolicy="no-referrer"
                className="absolute -right-12 -top-16 w-[58%] h-[140%] object-cover opacity-20 dark:opacity-24 blur-3xl scale-110"
                onError={(event) => {
                  (event.target as HTMLImageElement).style.display = 'none';
                }}
              />
            )}
            <div className="absolute inset-0 bg-[radial-gradient(circle_at_12%_12%,rgba(14,165,233,0.26),transparent_34%),radial-gradient(circle_at_75%_24%,rgba(168,85,247,0.2),transparent_30%),linear-gradient(115deg,rgba(255,255,255,0.72)_0%,rgba(255,255,255,0.2)_38%,rgba(15,23,42,0.1)_100%)] dark:bg-[radial-gradient(circle_at_12%_12%,rgba(56,189,248,0.2),transparent_32%),radial-gradient(circle_at_74%_24%,rgba(168,85,247,0.28),transparent_30%),linear-gradient(115deg,rgba(15,23,42,0.1)_0%,rgba(15,23,42,0.25)_45%,rgba(2,6,23,0.58)_100%)]" />
            <div className="absolute -left-32 top-12 h-28 w-[62%] rotate-[-12deg] bg-white/45 dark:bg-white/10 blur-2xl" />
            <div className="absolute inset-x-0 bottom-0 h-40 bg-gradient-to-t from-white/70 via-white/20 to-transparent dark:from-slate-950/60 dark:via-slate-950/10 dark:to-transparent" />
            <div className="absolute inset-y-0 right-0 w-1/2 bg-[linear-gradient(90deg,transparent,rgba(15,23,42,0.08))] dark:bg-[linear-gradient(90deg,transparent,rgba(15,23,42,0.35))]" />
            <div className="relative p-4 sm:p-5 md:p-7 min-h-0 sm:min-h-[420px] flex flex-col gap-4 sm:gap-6">
              <div className="flex-1 min-w-0 flex flex-col justify-between gap-5 sm:gap-8">
                <div className="flex flex-col lg:flex-row gap-5 sm:gap-6 lg:gap-8">
                  <div className="flex-1 min-w-0">
                    <div className="inline-flex items-center gap-2 px-3 py-1.5 rounded-full bg-slate-900/5 dark:bg-white/10 border border-slate-900/10 dark:border-white/10 text-xs font-bold text-slate-700 dark:text-white backdrop-blur-sm">
                    <Sparkles size={14} className="text-amber-500 dark:text-amber-300" />
                    {heroProgress ? '继续收听' : '今日推荐'}
                    </div>
                    <h2 className="text-3xl sm:text-4xl md:text-5xl font-black mt-4 sm:mt-5 leading-tight line-clamp-2 sm:line-clamp-none">
                      {activeHeroItem?.title || '打开一本新的声音'}
                    </h2>
                    <p className="text-sm md:text-base text-slate-500 dark:text-white/72 mt-2 sm:mt-3 line-clamp-1">
                      {activeHeroItem?.subtitle || '从书架里挑一本作品开始播放'}
                    </p>
                    <p className="text-sm md:text-base text-slate-700 dark:text-slate-100/90 mt-3 sm:mt-4 max-w-3xl line-clamp-3 sm:line-clamp-4 leading-6 sm:leading-8">
                      {activeHeroItem?.description || '从书架里挑一本最近添加的作品，或者去搜索页面发现新的内容。'}
                    </p>
                  </div>

                  <div className="w-full lg:w-[290px] xl:w-[320px] flex items-center justify-center py-1 sm:py-0">
                    <button
                      type="button"
                      onClick={handleCycleHero}
                      disabled={heroItems.length <= 1}
                      className={`relative w-[min(62vw,13.5rem)] sm:w-56 md:w-64 lg:w-72 text-left ${heroItems.length > 1 ? 'cursor-pointer group/hero-cover' : 'cursor-default'}`}
                      aria-label={heroItems.length > 1 ? '点击切换下一本作品' : '当前作品封面'}
                    >
                      <div className={`absolute -left-2 sm:-left-4 top-3 sm:top-4 w-full ${coverAspectClass} rounded-[22px] sm:rounded-[28px] bg-white/45 dark:bg-white/10 shadow-2xl shadow-sky-900/10 dark:shadow-black/25 -rotate-6 backdrop-blur-sm`} />
                      {nextHeroItem && (
                        <div className={`absolute -right-3 sm:-right-5 top-5 sm:top-8 w-full ${coverAspectClass} rounded-[22px] sm:rounded-[28px] overflow-hidden bg-white/20 dark:bg-white/10 border border-white/25 dark:border-white/10 shadow-2xl shadow-sky-900/20 dark:shadow-black/30 rotate-6 opacity-80`}>
                          <img
                            src={getCoverUrl(nextHeroItem.coverUrl, nextHeroItem.libraryId, nextHeroItem.id)}
                            alt={nextHeroItem.title}
                            referrerPolicy="no-referrer"
                            className="w-full h-full object-cover scale-105 saturate-75"
                            onError={(event) => {
                              (event.target as HTMLImageElement).src = 'https://placehold.co/300x400?text=No+Cover';
                            }}
                          />
                          <div className="absolute inset-0 bg-slate-950/32 dark:bg-slate-950/38 backdrop-blur-[1px]" />
                        </div>
                      )}
                      <div className={`relative ${coverAspectClass} rounded-[22px] sm:rounded-[28px] overflow-hidden shadow-[0_22px_60px_-30px_rgba(15,23,42,0.85)] sm:shadow-[0_30px_90px_-36px_rgba(15,23,42,0.85)] dark:shadow-[0_28px_76px_-34px_rgba(0,0,0,0.92)] bg-slate-900/60 border border-white/40 dark:border-white/15 backdrop-blur-sm transition-transform duration-300 ${heroItems.length > 1 ? 'group-hover/hero-cover:-translate-y-1' : ''}`}>
                        {activeHeroItem ? (
                          <img
                            src={getCoverUrl(activeHeroItem.coverUrl, activeHeroItem.libraryId, activeHeroItem.id)}
                            alt={activeHeroItem.title}
                            referrerPolicy="no-referrer"
                            className="w-full h-full object-cover"
                            onError={(event) => {
                              (event.target as HTMLImageElement).src = 'https://placehold.co/300x400?text=No+Cover';
                            }}
                          />
                        ) : (
                          <div className="w-full h-full flex items-center justify-center text-slate-300">
                            <Headphones size={72} />
                          </div>
                        )}
                        {heroItems.length > 1 && (
                          <div className="absolute inset-x-2 bottom-2 sm:inset-x-3 sm:bottom-3 flex items-center justify-between gap-2 sm:gap-3 rounded-xl sm:rounded-2xl border border-white/12 bg-slate-950/45 px-2.5 sm:px-3 py-2 backdrop-blur-sm">
                            <div className="min-w-0">
                              <p className="hidden sm:block text-[10px] uppercase tracking-wide text-white/55">下一本</p>
                              <p className="text-[11px] sm:text-xs font-bold text-white truncate">下一本：{nextHeroItem?.title || '继续切换'}</p>
                            </div>
                            <div className="w-7 h-7 sm:w-8 sm:h-8 rounded-full bg-white/12 flex items-center justify-center text-white/85 shrink-0">
                              <RefreshCw size={13} />
                            </div>
                          </div>
                        )}
                      </div>
                    </button>
                  </div>
                </div>

                <div className="grid grid-cols-2 sm:flex sm:flex-wrap items-center gap-3">
                  {activeHeroItem ? (
                    <Link
                      to={`/book/${activeHeroItem.id}`}
                      className="inline-flex items-center justify-center gap-2 px-3 sm:px-5 py-3 rounded-2xl bg-slate-950 dark:bg-white text-white dark:text-slate-950 text-sm font-black hover:opacity-90 transition-opacity shadow-xl shadow-slate-950/20 dark:shadow-black/25"
                    >
                      <Play size={18} fill="currentColor" />
                      {heroProgress ? '继续播放' : '查看详情'}
                    </Link>
                  ) : (
                    <Link
                      to="/bookshelf"
                      className="inline-flex items-center justify-center gap-2 px-3 sm:px-5 py-3 rounded-2xl bg-slate-950 dark:bg-white text-white dark:text-slate-950 text-sm font-black hover:opacity-90 transition-opacity shadow-xl shadow-slate-950/20 dark:shadow-black/25"
                    >
                      <Library size={18} />
                      去书架
                    </Link>
                  )}
                  <Link
                    to="/playlists"
                    className="inline-flex items-center justify-center gap-2 px-3 sm:px-5 py-3 rounded-2xl bg-white/45 dark:bg-white/10 border border-white/60 dark:border-white/10 text-slate-800 dark:text-white text-sm font-bold hover:bg-white/70 dark:hover:bg-white/15 transition-colors backdrop-blur-sm"
                  >
                    <ListMusic size={18} />
                    管理书单
                  </Link>
                </div>
              </div>

            </div>
          </div>
          )}

          {homeLayout.showStats && (
          <div className="grid grid-cols-2 gap-4">
            <DataCard icon={<Headphones size={20} />} label="最近已听" value={listenMinutes > 0 ? `${listenMinutes}` : '0'} unit="分钟" tone="text-primary-600 bg-primary-50 dark:bg-primary-900/20" />
            <DataCard icon={<Heart size={20} />} label="收藏作品" value={favorites.length} unit="本" tone="text-red-500 bg-red-50 dark:bg-red-900/20" />
            <DataCard icon={<ListMusic size={20} />} label="我的书单" value={playlists.length} unit="个" tone="text-amber-600 bg-amber-50 dark:bg-amber-900/20" />
            <DataCard icon={<History size={20} />} label="收听记录" value={recentPlays.length} unit="条" tone="text-emerald-600 bg-emerald-50 dark:bg-emerald-900/20" />
            <div className="col-span-2 bg-white dark:bg-slate-900 border border-slate-100 dark:border-slate-800 rounded-3xl p-5 shadow-sm">
              <div className="flex items-center gap-3">
                <div className="w-11 h-11 rounded-2xl bg-violet-50 dark:bg-violet-900/20 text-violet-600 flex items-center justify-center">
                  <Clock size={20} />
                </div>
                <div className="min-w-0">
                  <p className="text-xs text-slate-500 font-bold">当前播放</p>
                  <p className="text-lg md:text-xl font-bold dark:text-white truncate">{currentChapter?.title || '暂无播放'}</p>
                </div>
              </div>
            </div>
          </div>
          )}
        </section>
        )}

        {homeLayout.showRecommended && (
        <section className="space-y-4">
          <SectionTitle icon={<Sparkles size={22} className="text-amber-500" />} title="为你推荐" to="/bookshelf" action="查看书架" />
          {recommendedBooks.length > 0 ? (
            <div className="grid grid-cols-3 sm:grid-cols-4 lg:grid-cols-6 2xl:grid-cols-8 gap-x-4 gap-y-7">
              {recommendedBooks.map(book => (
                <BookCard key={book.id} book={book} coverShape={coverShape} />
              ))}
            </div>
          ) : (
            <EmptyBand icon={<Library size={34} />} title="还没有可推荐的内容" action="去书架看看" to="/bookshelf" />
          )}
        </section>
        )}

        {(homeLayout.showRecent || homeLayout.showRecentlyAdded) && (
        <section className="grid grid-cols-1 gap-8">
          {homeLayout.showRecent && (
          <div className="space-y-4">
            <SectionTitle icon={<History size={22} className="text-primary-600" />} title="最近收听" to="/history" action="查看历史" />
            {recentPlays.length > 0 ? (
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                {recentPlays.slice(0, 4).map(progress => (
                  <RecentListenTile key={progress.id || `${progress.bookId}-${progress.chapterId}`} progress={progress} coverShape={coverShape} />
                ))}
              </div>
            ) : (
              <EmptyBand icon={<Play size={34} />} title="暂无播放记录" action="开始听书" to="/bookshelf" />
            )}
          </div>
          )}

          {homeLayout.showRecentlyAdded && (
          <div className="space-y-4">
            <SectionTitle icon={<TrendingUp size={22} className="text-emerald-600" />} title="最近上新" to="/bookshelf" action="更多" />
            {recentlyAddedBooks.length > 0 ? (
              <div className="grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-3 gap-4">
                {recentlyAddedBooks.slice(0, 6).map(book => (
                  <RecentlyAddedTile key={book.id} book={book} coverShape={coverShape} />
                ))}
              </div>
            ) : (
              <EmptyBand icon={<TrendingUp size={34} />} title="暂无上新内容" action="去书架看看" to="/bookshelf" />
            )}
          </div>
          )}
        </section>
        )}

        {homeLayout.showCollections && (
        <section className="space-y-4">
          <SectionTitle icon={<ListMusic size={22} className="text-orange-500" />} title="书单与系列" to="/playlists" action="管理书单" />
          <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-4">
            {playlists.slice(0, 4).map(playlist => (
              <CollectionCard
                key={playlist.id}
                title={playlist.title}
                subtitle={`${getPlaylistBookCount(playlist)} 本书`}
                to={`/playlists/${playlist.id}`}
                items={collectPlaylistCovers(playlist, playlistCoverSeed)}
              />
            ))}
            {series.slice(0, Math.max(0, 4 - playlists.length)).map(item => (
              <CollectionCard
                key={item.id}
                title={item.title}
                subtitle={`${item.books?.length || 0} 本系列作品`}
                to={`/series/${item.id}`}
                items={getSeriesCover(item)}
              />
            ))}
            {playlists.length === 0 && series.length === 0 && (
              <div className="md:col-span-2 xl:col-span-4">
                <EmptyBand icon={<ListMusic size={34} />} title="还没有书单或系列" action="创建书单" to="/playlists" />
              </div>
            )}
          </div>
        </section>
        )}
      </div>

      <div
        className="shrink-0 transition-all duration-300"
        style={{ height: currentChapter ? 'var(--safe-bottom-with-player)' : 'var(--safe-bottom-base)' }}
      />
    </div>
  );
};

const SectionTitle = ({ icon, title, to, action }: { icon: React.ReactNode; title: string; to: string; action: string }) => (
  <div className="flex items-center justify-between gap-4">
    <div className="flex items-center gap-2">
      {icon}
      <h2 className="text-xl md:text-2xl font-bold dark:text-white">{title}</h2>
    </div>
    <Link to={to} className="inline-flex items-center gap-1 text-sm font-bold text-primary-600 hover:text-primary-700">
      {action}
      <ChevronRight size={16} />
    </Link>
  </div>
);

const DataCard = ({
  icon,
  label,
  value,
  unit,
  tone,
}: {
  icon: React.ReactNode;
  label: string;
  value: string | number;
  unit?: string;
  tone: string;
}) => (
  <div className="bg-white dark:bg-slate-900 border border-slate-100 dark:border-slate-800 rounded-3xl p-4 shadow-sm min-w-0">
    <div className={`w-11 h-11 rounded-2xl flex items-center justify-center mb-4 ${tone}`}>
      {icon}
    </div>
    <p className="text-xs text-slate-500 font-bold">{label}</p>
    <p className="text-2xl font-black text-slate-900 dark:text-white truncate">
      {value}
      {unit && <span className="text-xs font-bold text-slate-400 ml-1">{unit}</span>}
    </p>
  </div>
);

const RecentListenTile = ({ progress, coverShape }: { progress: Progress; coverShape: CoverShape }) => {
  const percent = Math.min(100, Math.round((progress.position / (progress.chapterDuration || 1)) * 100));

  return (
    <Link
      to={`/book/${progress.bookId}`}
      className="bg-white dark:bg-slate-900 rounded-3xl p-3 md:p-4 shadow-sm border border-slate-100 dark:border-slate-800 flex gap-3 md:gap-4 hover:shadow-md transition-shadow group"
    >
      <div className={`w-20 md:w-24 ${getCoverAspectClass(coverShape)} rounded-2xl overflow-hidden shrink-0 shadow-sm`}>
        <img
          src={getCoverUrl(progress.coverUrl, progress.libraryId, progress.bookId)}
          alt={progress.bookTitle}
          referrerPolicy="no-referrer"
          className="w-full h-full object-cover group-hover:scale-105 transition-transform duration-300"
          onError={(event) => {
            (event.target as HTMLImageElement).src = 'https://placehold.co/300x400?text=No+Cover';
          }}
        />
      </div>
      <div className="flex-1 min-w-0 flex flex-col justify-between py-0.5">
        <div className="min-w-0">
          <p className="font-bold text-sm md:text-base dark:text-white group-hover:text-primary-600 transition-colors truncate">
            {progress.bookTitle || '未知书籍'}
          </p>
          <p className="text-xs text-slate-500 truncate mt-0.5">正在播放: {progress.chapterTitle}</p>
        </div>
        <div className="flex items-center justify-between mt-2">
          <div className="flex-1 h-1.5 bg-slate-100 dark:bg-slate-800 rounded-full mr-3 overflow-hidden">
            <div className="h-full bg-primary-500 rounded-full" style={{ width: `${percent}%` }} />
          </div>
          <span className="text-[10px] text-slate-400 shrink-0">{percent}%</span>
        </div>
      </div>
    </Link>
  );
};

const RecentlyAddedTile = ({ book, coverShape }: { book: Book; coverShape: CoverShape }) => (
  <Link
    to={`/book/${book.id}`}
    className="group bg-white dark:bg-slate-900 rounded-3xl p-3 md:p-4 shadow-sm border border-slate-100 dark:border-slate-800 flex items-center gap-4 hover:shadow-md transition-shadow min-w-0"
  >
    <div className={`w-20 md:w-24 ${getCoverAspectClass(coverShape)} rounded-2xl overflow-hidden shrink-0 shadow-sm bg-slate-100 dark:bg-slate-800`}>
      <img
        src={getCoverUrl(book.coverUrl, book.libraryId, book.id)}
        alt={book.title}
        referrerPolicy="no-referrer"
        className="w-full h-full object-cover group-hover:scale-105 transition-transform duration-300"
        onError={(event) => {
          (event.target as HTMLImageElement).src = 'https://placehold.co/300x400?text=No+Cover';
        }}
      />
    </div>
    <div className="min-w-0 flex-1">
      <p className="text-[11px] font-black text-emerald-600 dark:text-emerald-400 mb-1">最近入库</p>
      <p className="font-bold text-sm md:text-base dark:text-white truncate group-hover:text-primary-600 transition-colors">{book.title}</p>
      <p className="text-xs text-slate-500 truncate mt-1">{book.author || '未知作者'}</p>
    </div>
    <ChevronRight size={16} className="text-slate-300 shrink-0" />
  </Link>
);

const CollectionCard = ({
  title,
  subtitle,
  to,
  color,
  items,
}: {
  title: string;
  subtitle: string;
  to: string;
  color?: string;
  items?: PlaylistCoverItem[];
}) => (
  <Link
    to={to}
    className="group bg-white dark:bg-slate-900 border border-slate-100 dark:border-slate-800 rounded-3xl p-4 shadow-sm hover:shadow-md transition-shadow"
  >
    <div className="relative h-[178px] rounded-2xl overflow-hidden bg-slate-100 dark:bg-slate-800">
      {items && items.length > 0 ? (
        <img
          src={getCoverUrl(items[0].coverUrl, items[0].libraryId, items[0].bookId)}
          alt={items[0].title || title}
          referrerPolicy="no-referrer"
          className="absolute inset-0 w-full h-full object-cover group-hover:scale-105 transition-transform duration-300"
          onError={(event) => {
            (event.target as HTMLImageElement).src = 'https://placehold.co/300x400?text=No+Cover';
          }}
        />
      ) : (
        <div className="h-full flex items-center justify-center text-white" style={{ backgroundColor: color || '#0ea5e9' }}>
          <ListMusic size={48} />
        </div>
      )}
      <div className="absolute inset-0 bg-gradient-to-t from-slate-950/82 via-slate-950/30 to-transparent" />
      <div className="absolute inset-0 p-4 flex flex-col justify-between text-white">
        <ListMusic size={24} className="opacity-95" />
        <div className="min-w-0">
          <h3 className="font-bold text-lg truncate">
            {title}
          </h3>
          <p className="text-sm text-white/80 mt-1 truncate">
            {subtitle}
          </p>
        </div>
      </div>
    </div>
  </Link>
);

const EmptyBand = ({ icon, title, action, to }: { icon: React.ReactNode; title: string; action: string; to: string }) => (
  <div className="rounded-3xl bg-white dark:bg-slate-900 border border-dashed border-slate-200 dark:border-slate-800 p-10 text-center">
    <div className="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-slate-100 dark:bg-slate-800 text-slate-400 mb-4">
      {icon}
    </div>
    <p className="text-slate-500 mb-5">{title}</p>
    <Link to={to} className="inline-flex items-center justify-center px-5 py-2.5 rounded-xl bg-primary-600 text-white text-sm font-bold hover:bg-primary-700 transition-colors">
      {action}
    </Link>
  </div>
);

export default HomePage;
