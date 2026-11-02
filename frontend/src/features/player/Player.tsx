import { useBookOperations } from '../../core/hooks/useBooks';
import React, { useRef, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useLocation } from "react-router";
import { usePlayerStore } from "../../core/stores/playerStore";
import type { Chapter } from "../../core/types";
import { setAlpha, toSolidColor, isTooLight } from "../../core/utils/color";
import { useBookshelfCoverShape } from "../../core/hooks/useBookshelfCoverShape";
import ProgressBar from "./ProgressBar";
import { usePlayback } from "./usePlayback";
import {
  isMiniPlayerHiddenPath,
} from "./platform";
import { getRuntimePathname } from "../../core/utils/runtimeUrl";
import PlayerSettingsModal from "./PlayerSettingsModal";
import BookmarkManagerModal from "./BookmarkManagerModal";
import ChapterListDrawer from "./ChapterListDrawer";
import {
  useIsDarkMode,
  useThemeColorSync,
  useChapterGroups,
  useOutsideClickClose,
  useWidgetFullscreen,
  getPlayerCoverSizes,
  formatPlayerTime,
  getChapterProgressText,
  CHAPTERS_PER_GROUP,
} from "./playerHelpers";
import {
  ExpandedMainControls,
  ExpandedBottomControls,
  ExpandedProgressSection,
  MiniPlayerDesktopControls,
  MiniPlayerDesktopExtras,
  MiniPlayerMobileControls,
} from "./PlayerControls";
import {
  CollapsedPlayerView, ExpandedPlayerHeader, ExpandedCover, ExpandedChapterTitle, MiniPlayerBookInfo,
} from "./PlayerViews";

const Player: React.FC = () => {
  const bookOperations = useBookOperations();
  const { t } = useTranslation();
  const coverShape = useBookshelfCoverShape();
  const {
    currentBook,
    currentChapter,
    isPlaying,
    togglePlay,
    currentTime,
    duration,
    nextChapter,
    prevChapter,
    playbackSpeed,
    setPlaybackSpeed,
    volume,
    setVolume,
    themeColor,
    playChapter,
    isExpanded,
    setIsExpanded,
    isCollapsed,
    setIsCollapsed,
    isSeriesEditing,
  } = usePlayerStore();

  const {
    audioProps, chapters, isMuted, setIsMuted, error, bufferedTime, isSeeking,
    seekTime, handleSeek, handleSeekStart, handleSeekEnd, seekToTime, sleepTimer,
    sleepEpisodes, startSleepTimer, startEpisodeSleepTimer, cancelSleepTimer,
  } = usePlayback();
  const location = useLocation();
  const [showChapters, setShowChapters] = useState(false);
  const [showSleepTimer, setShowSleepTimer] = useState(false);
  const [showVolumeControl, setShowVolumeControl] = useState(false);
  const [showSpeedControl, setShowSpeedControl] = useState(false);
  const [showBookmarks, setShowBookmarks] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [currentGroupIndex, setCurrentGroupIndex] = useState(0);
  const [activeTab, setActiveTab] = useState<"main" | "extra">("main");
  const [chapterAscending, setChapterAscending] = useState(true);
  const scrollRef = useRef<HTMLDivElement>(null);
  const volumeControlRef = useRef<HTMLDivElement>(null);
  const speedControlRef = useRef<HTMLDivElement>(null);

  const scrollGroups = (direction: "left" | "right") => {
    if (scrollRef.current) {
      const scrollAmount = 200;
      scrollRef.current.scrollBy({
        left: direction === "left" ? -scrollAmount : scrollAmount,
        behavior: "smooth",
      });
    }
  };
  const [customMinutes, setCustomMinutes] = useState("");
  const [customEpisodes, setCustomEpisodes] = useState("");
  const [editSkipIntro, setEditSkipIntro] = useState(0);
  const [editSkipOutro, setEditSkipOutro] = useState(0);

  const isDark = useIsDarkMode();

  const effectiveThemeColor =
    themeColor && !isTooLight(themeColor) ? themeColor : undefined;
  const {
    collapsed: collapsedCoverSizeClass,
    mini: miniCoverSizeClass,
    expanded: expandedCoverSizeClass,
  } = getPlayerCoverSizes(coverShape);
  // Always use the theme color for the mini player progress bar, even in dark mode
  const miniPlayerThemeColor = effectiveThemeColor;
  // Determine if we should use dark mode text colors (white/gray) for controls
  // In dark mode, we always want bright white/gray for contrast
  const useDarkControls = isDark;

  const authorName = currentBook?.author?.trim();
  const narratorName = currentBook?.narrator?.trim();
  const authorText = [authorName, narratorName]
    .filter(Boolean)
    .join(' / ') || t('player.unknownAuthor');

  // Sync theme color from current book (stored value preferred, fallback to FAC extraction).
  useThemeColorSync(currentBook);

  useEffect(() => {
    if (currentBook) {
      setTimeout(() => {
        setEditSkipIntro(currentBook.skip_intro || 0);
        setEditSkipOutro(currentBook.skip_outro || 0);
      }, 0);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentBook?.id]);

  const handleSaveSettings = async () => {
    if (!currentBook) return;
    try {
      const book = currentBook;
      const updated = await bookOperations.current?.save(book, { skip_intro: editSkipIntro, skip_outro: editSkipOutro });
      if (!updated) return;
      usePlayerStore.getState().updateBook(updated);
      setShowSettings(false);
    } catch (err) {
      console.error("Failed to save player settings", err);
    }
  };

  const { extraChapters, currentChapters, groups } = useChapterGroups(
    chapters,
    activeTab,
  );
  const chaptersPerGroup = CHAPTERS_PER_GROUP;

  useEffect(() => {
    const timer = window.setTimeout(() => setCurrentGroupIndex(0), 0);
    return () => window.clearTimeout(timer);
  }, [currentBook?.id]);
  const timerMenuRef = useRef<HTMLDivElement>(null);

  // Close timer / volume menus when clicking outside.
  useOutsideClickClose([timerMenuRef, volumeControlRef, speedControlRef], (ref) => {
    if (ref === timerMenuRef) setShowSleepTimer(false);
    if (ref === volumeControlRef) setShowVolumeControl(false);
    if (ref === speedControlRef) setShowSpeedControl(false);
  });

  const jumpToBookmark = (chapterId: string, position: number) => {
    if (!currentBook) return;
    const chapter = chapters.find((item) => item.id === chapterId);
    if (!chapter) {
      alert(t('bookmarks.chapterUnavailable', '当前书籍章节尚未加载'));
      return;
    }
    if (currentChapter?.id === chapterId) {
      seekToTime(position);
      usePlayerStore.setState({ isPlaying: true });
    } else {
      playChapter(currentBook, chapters, chapter, position);
    }
    setShowBookmarks(false);
  };

  const formatTime = formatPlayerTime;
  const getLocalizedChapterProgressText = React.useCallback(
    (chapter: Chapter) =>
      getChapterProgressText(chapter, {
        complete: t("player.playedComplete"),
        percent: (percent) => t("player.playedPercent", { percent }),
      }),
    [t],
  );

  const isHiddenPage = isMiniPlayerHiddenPath(location.pathname);
  const isWidgetMode = getRuntimePathname().startsWith("/widget");

  // Auto collapse player when navigating to hidden pages
  useEffect(() => {
    if (isHiddenPage && isExpanded) {
      setTimeout(() => setIsExpanded(false), 0);
    }
  }, [location.pathname, isExpanded, isHiddenPage, setIsExpanded]);

  useEffect(() => {
    const timer = window.setTimeout(() => setShowVolumeControl(false), 0);
    return () => window.clearTimeout(timer);
  }, [isExpanded]);

  // Fullscreen Logic for Widget
  const { toggleFullscreen, exitExpanded: handleExitExpanded } =
    useWidgetFullscreen({
      isWidgetMode,
      setIsExpanded,
    });

  if (!currentChapter) return null;

  const miniPlayerStyle = !isExpanded
    ? {
        bottom: isWidgetMode ? "0" : "var(--mini-player-offset)",
        height: isWidgetMode
          ? "100%"
          : isCollapsed
            ? "64px"
            : "var(--player-h)",
        left: isWidgetMode ? "0" : undefined,
        right: isWidgetMode ? "0" : undefined,
      }
    : {};

  const openChapterList = () => {
    if (currentChapter && chapters.length > 0) {
      const isExtra = !!currentChapter.is_extra;
      const targetTab = isExtra ? "extra" : "main";
      if (activeTab !== targetTab) setActiveTab(targetTab);

      const targetList = chapters.filter((chapter) => {
        const chapterIsExtra = !!chapter.is_extra;
        return chapterIsExtra === isExtra;
      });

      const index = targetList.findIndex(
        (chapter) => chapter.id === currentChapter.id,
      );
      if (index !== -1) {
        const groupIndex = Math.floor(index / chaptersPerGroup);
        setCurrentGroupIndex(groupIndex);
        const groupCount = Math.ceil(targetList.length / chaptersPerGroup);
        // 从第一组进入默认正序；从最后一组（从后往前）进入默认逆序。
        setChapterAscending(!(groupCount > 1 && groupIndex === groupCount - 1));

        setTimeout(() => {
          const chapterEl = document.getElementById(
            `player-chapter-${currentChapter.id}`,
          );
          if (chapterEl) {
            chapterEl.scrollIntoView({ block: "center", behavior: "smooth" });
          }

          const groupTab = document.getElementById(
            `player-group-tab-${groupIndex}`,
          );
          const container = scrollRef.current;
          if (groupTab && container) {
            const containerWidth = container.offsetWidth;
            const tabWidth = groupTab.offsetWidth;
            const tabLeft = groupTab.offsetLeft;

            container.scrollTo({
              left: tabLeft - containerWidth / 2 + tabWidth / 2,
              behavior: "smooth",
            });
          }
        }, 100);
      }
    }

    setShowChapters(true);
  };

  const handleToggleChapterOrder = () => {
    setChapterAscending((prev) => !prev);
  };

  return (
    <div
      className={`
        transition-all duration-500 ease-in-out
        ${(isHiddenPage || isSeriesEditing) && !isExpanded ? "translate-y-full opacity-0 pointer-events-none" : ""}
        ${
          isExpanded
            ? "fixed inset-0 z-[110] bg-white dark:bg-slate-950"
            : "absolute left-0 right-0 z-[30] bg-transparent pointer-events-none"
        }
      `}
      style={miniPlayerStyle}
    >
      <audio key={currentChapter.id} {...audioProps} />

      {error && !isExpanded && (
        <div className="absolute top-0 left-4 right-4 bg-red-500 text-white text-[10px] py-1 px-2 text-center rounded-t-lg animate-pulse z-[101]">
          {error}
        </div>
      )}
      {/* Mini Player - Floating Card Style on Mobile */}
      {!isExpanded && (
        <div
          className={`h-full ${isWidgetMode ? "px-0" : "px-2 sm:px-4"} pointer-events-none`}
        >
          {isCollapsed ? (
            <CollapsedPlayerView
              book={currentBook}
              coverSizeClass={collapsedCoverSizeClass}
              themeColor={miniPlayerThemeColor}
              onExpandCollapsed={() => setIsCollapsed(false)}
            />
          ) : (
            /* Normal Mini Player */
            <div
              className={`
              h-full ${isWidgetMode ? "max-w-none rounded-none border-none shadow-none" : "max-w-7xl mx-auto rounded-2xl sm:rounded-3xl shadow-2xl shadow-black/10 border border-slate-200/50 dark:border-slate-800/50"}
              bg-white/95 dark:bg-slate-900/95 backdrop-blur-md 
              flex items-center justify-between gap-3 sm:gap-4 ${isWidgetMode ? "px-3 max-[380px]:flex-col max-[380px]:justify-center max-[380px]:gap-1.5 max-[380px]:py-2" : "px-3 sm:px-6"} pointer-events-auto
              transition-all duration-300
            `}
              style={{
                backgroundColor: isWidgetMode
                  ? undefined
                  : miniPlayerThemeColor
                    ? setAlpha(miniPlayerThemeColor, 0.05)
                    : undefined,
                borderColor: isWidgetMode
                  ? undefined
                  : miniPlayerThemeColor
                    ? setAlpha(miniPlayerThemeColor, 0.2)
                    : undefined,
              }}
            >
              {/* Info */}
              <MiniPlayerBookInfo
                book={currentBook}
                chapterTitle={currentChapter.title}
                coverSizeClass={miniCoverSizeClass}
                isWidgetMode={isWidgetMode}
                onCoverClick={toggleFullscreen}
              />

              {/* Widget Vertical Layout: Progress Bar (Visible only on small widget) */}
              {isWidgetMode && (
                <div className="hidden max-[380px]:block w-full px-1 py-1">
                  <ProgressBar
                    isMini={true}
                    isSeeking={isSeeking}
                    seekTime={seekTime}
                    currentTime={currentTime}
                    duration={duration}
                    bufferedTime={bufferedTime}
                    themeColor={miniPlayerThemeColor}
                    onSeek={handleSeek}
                    onSeekStart={handleSeekStart}
                    onSeekEnd={handleSeekEnd}
                  />
                </div>
              )}

              {/* Controls (Desktop) */}
              <MiniPlayerDesktopControls
                isPlaying={isPlaying}
                currentTime={currentTime}
                duration={duration}
                themeColor={miniPlayerThemeColor}
                effectiveThemeColor={effectiveThemeColor}
                useDarkControls={useDarkControls}
                formatTime={formatTime}
                onPrev={prevChapter}
                onNext={nextChapter}
                onTogglePlay={togglePlay}
                onSeekTo={seekToTime}
                isSeeking={isSeeking}
                seekTime={seekTime}
                bufferedTime={bufferedTime}
                onSeek={handleSeek}
                onSeekStart={handleSeekStart}
                onSeekEnd={handleSeekEnd}
              />

              {/* Mobile Controls - Only visible on small screens */}
              <MiniPlayerMobileControls
                isPlaying={isPlaying}
                isWidgetMode={isWidgetMode}
                themeColor={miniPlayerThemeColor}
                effectiveThemeColor={effectiveThemeColor}
                useDarkControls={useDarkControls}
                currentTime={currentTime}
                duration={duration}
                bufferedTime={bufferedTime}
                isSeeking={isSeeking}
                seekTime={seekTime}
                onSeek={handleSeek}
                onSeekStart={handleSeekStart}
                onSeekEnd={handleSeekEnd}
                onTogglePlay={togglePlay}
                onPrev={prevChapter}
                onNext={nextChapter}
                onSeekTo={seekToTime}
                onCollapse={() => setIsCollapsed(true)}
                onOpenChapterList={openChapterList}
              />

              {/* Desktop Extra Controls - Visible on Tablet and Desktop */}
              <MiniPlayerDesktopExtras
                volume={volume}
                isMuted={isMuted}
                showVolumeControl={showVolumeControl}
                volumeControlRef={
                  !isExpanded ? volumeControlRef : { current: null }
                }
                themeColor={miniPlayerThemeColor}
                useDarkControls={useDarkControls}
                playbackSpeed={playbackSpeed}
                onToggleVolumeControl={() =>
                  setShowVolumeControl(!showVolumeControl)
                }
                onChangeVolume={setVolume}
                onToggleMuted={() => setIsMuted(!isMuted)}
                onCyclePlaybackSpeed={() =>
                  setPlaybackSpeed(
                    playbackSpeed === 2 ? 1 : playbackSpeed + 0.25,
                  )
                }
                onCollapse={() => setIsCollapsed(true)}
                onExpand={() => setIsExpanded(true)}
                onOpenChapterList={openChapterList}
              />
            </div>
          )}
        </div>
      )}

      {/* Expanded Player View */}
      {isExpanded && (
        <div
          className="absolute inset-0 flex flex-col p-4 sm:p-8 md:p-12 overflow-y-auto animate-in slide-in-from-bottom duration-500 bg-white dark:bg-slate-950"
            style={{
              paddingTop: "max(env(safe-area-inset-top, 0px), clamp(1rem, 3vw, 3rem))",
              paddingBottom: "max(env(safe-area-inset-bottom, 0px), clamp(1rem, 3vw, 3rem))",
            backgroundColor: isWidgetMode
              ? effectiveThemeColor
                ? toSolidColor(effectiveThemeColor)
                : "#1e293b"
              : effectiveThemeColor
                ? setAlpha(effectiveThemeColor, 0.05)
                : undefined,
          }}
        >
          <ExpandedPlayerHeader
            bookTitle={currentBook?.title || currentChapter.title}
            authorText={authorText}
            onExit={handleExitExpanded}
            onOpenSettings={() => setShowSettings(true)}
          />

          <div className="flex-1 flex flex-col items-center justify-center max-w-[520px] mx-auto w-full gap-5 sm:gap-7 min-[700px]:landscape:max-w-[1280px] min-[700px]:landscape:grid min-[700px]:landscape:grid-cols-[minmax(0,5fr)_minmax(0,6fr)] min-[700px]:landscape:items-center min-[700px]:landscape:gap-16 xl:landscape:gap-24">
            <div className="flex w-full min-w-0 flex-col items-center gap-5 sm:gap-7">
              <ExpandedCover
                book={currentBook}
                expandedCoverSizeClass={expandedCoverSizeClass}
              />
              <div className="w-full min-[700px]:landscape:hidden">
                <ExpandedChapterTitle
                  chapterTitle={currentChapter.title}
                  error={error}
                />
              </div>
            </div>

            <div className="w-full flex flex-col gap-7 sm:gap-8">
              <div className="hidden w-full min-[700px]:landscape:block">
                <ExpandedChapterTitle
                  chapterTitle={currentChapter.title}
                  error={error}
                />
              </div>

              {/* Progress Bar Section */}
              <ExpandedProgressSection
                currentTime={currentTime}
                duration={duration}
                bufferedTime={bufferedTime}
                isSeeking={isSeeking}
                seekTime={seekTime}
                themeColor={effectiveThemeColor || "#60a5fa"}
                formatTime={formatTime}
                onSeek={handleSeek}
                onSeekStart={handleSeekStart}
                onSeekEnd={handleSeekEnd}
                onSeekTo={seekToTime}
              />

              <ExpandedMainControls
                isPlaying={isPlaying}
                themeColor={effectiveThemeColor}
                onPrev={prevChapter}
                onTogglePlay={togglePlay}
                onNext={nextChapter}
              />

              <ExpandedBottomControls
                playbackSpeed={playbackSpeed}
                showSpeedControl={showSpeedControl}
                speedControlRef={speedControlRef}
                onToggleSpeedControl={() => setShowSpeedControl((value) => !value)}
                onChangePlaybackSpeed={setPlaybackSpeed}
                onOpenBookmarks={() => setShowBookmarks(true)}
                sleepTimer={sleepTimer}
                showSleepTimer={showSleepTimer}
                customMinutes={customMinutes}
                timerMenuRef={timerMenuRef}
                onToggleShowSleepTimer={() =>
                  setShowSleepTimer(!showSleepTimer)
                }
                onSetCustomMinutes={setCustomMinutes}
                onStartSleepTimer={startSleepTimer}
                onCancelSleepTimer={cancelSleepTimer}
                sleepEpisodes={sleepEpisodes}
                customEpisodes={customEpisodes}
                onSetCustomEpisodes={setCustomEpisodes}
                onStartEpisodeSleepTimer={startEpisodeSleepTimer}
                onCloseSleepTimer={() => setShowSleepTimer(false)}
                onOpenChapterList={openChapterList}
              />
            </div>
          </div>

          {/* Settings Modal */}
          {showSettings && (
            <PlayerSettingsModal
              editSkipIntro={editSkipIntro}
              editSkipOutro={editSkipOutro}
              volume={volume}
              onChangeVolume={(value) => {
                setVolume(value);
                if (value > 0) setIsMuted(false);
              }}
              onChangeSkipIntro={setEditSkipIntro}
              onChangeSkipOutro={setEditSkipOutro}
              onClose={() => setShowSettings(false)}
              onSave={handleSaveSettings}
            />
          )}

          {showBookmarks && currentBook && currentChapter && <BookmarkManagerModal
            key={currentBook.id}
            bookId={currentBook.id}
            bookTitle={currentBook.title}
            chapterId={currentChapter.id}
            position={currentTime}
            onClose={() => setShowBookmarks(false)}
            onJump={jumpToBookmark}
          />}
        </div>
      )}
      <ChapterListDrawer
        show={showChapters}
        currentBook={currentBook}
        currentChapter={currentChapter}
        currentChapters={currentChapters}
        groups={groups}
        chaptersPerGroup={chaptersPerGroup}
        currentGroupIndex={currentGroupIndex}
        activeTab={activeTab}
        extraChapters={extraChapters}
        isPlaying={isPlaying}
        chapterAscending={chapterAscending}
        onToggleOrder={handleToggleChapterOrder}
        effectiveThemeColor={effectiveThemeColor}
        scrollRef={scrollRef}
        onClose={() => setShowChapters(false)}
        onSetActiveTab={setActiveTab}
        onSetCurrentGroupIndex={setCurrentGroupIndex}
        onScrollGroups={scrollGroups}
        onPlayChapter={(chapter) =>
          playChapter(currentBook!, currentChapters, chapter)
        }
        formatTime={formatTime}
        getChapterProgressText={getLocalizedChapterProgressText}
      />
    </div>
  );
};

export default Player;
