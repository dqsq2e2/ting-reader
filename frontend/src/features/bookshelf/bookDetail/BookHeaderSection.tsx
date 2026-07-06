import React from "react";
import {
  Play,
  Heart,
  ChevronDown,
  ChevronUp,
  User,
  Mic2,
  ListMusic,
  Info,
  Edit,
  RefreshCw,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import type { Book } from "../../../core/types";
import { setAlpha, toSolidColor, isLight } from "../../../core/utils/color";
import { getCoverUrl } from "../../../core/utils/image";
import PluginExtensionSlot from "../../../shared/pluginExtensions/PluginExtensionSlot";
import ExpandableTitle from "../../../shared/widgets/ExpandableTitle";

interface Props {
  book: Book;
  coverShape: "rect" | "square";
  displayCoverUrl?: string;
  displayLibraryId?: string;
  effectiveThemeColor?: string;
  chapterTotalCount: number;
  resumeChapterTitle?: string;
  resumeChapterBookMatches: boolean;
  hasResumeChapter: boolean;
  isPlayButtonTextOverflowing: boolean;
  playButtonContainerRef: React.RefObject<HTMLButtonElement | null>;
  isFavorite: boolean;
  isAdmin: boolean;
  tagsRef: React.RefObject<HTMLDivElement | null>;
  isTagsExpanded: boolean;
  isTagsOverflowing: boolean;
  descriptionRef: React.RefObject<HTMLParagraphElement | null>;
  isDescriptionExpanded: boolean;
  isDescriptionOverflowing: boolean;
  onPlayClick: () => void;
  onToggleFavorite: () => void;
  onOpenScrapeDiff: () => void;
  onOpenEditModal: () => void;
  onSetIsTagsExpanded: (expanded: boolean) => void;
  onSetIsDescriptionExpanded: (expanded: boolean) => void;
}

const BookHeaderSection: React.FC<Props> = ({
  book,
  coverShape,
  displayCoverUrl,
  displayLibraryId,
  effectiveThemeColor,
  chapterTotalCount,
  resumeChapterTitle,
  resumeChapterBookMatches,
  hasResumeChapter,
  isPlayButtonTextOverflowing,
  playButtonContainerRef,
  isFavorite,
  isAdmin,
  tagsRef,
  isTagsExpanded,
  isTagsOverflowing,
  descriptionRef,
  isDescriptionExpanded,
  isDescriptionOverflowing,
  onPlayClick,
  onToggleFavorite,
  onOpenScrapeDiff,
  onOpenEditModal,
  onSetIsTagsExpanded,
  onSetIsDescriptionExpanded,
}) => {
  const { t } = useTranslation();
  const playLabel =
    hasResumeChapter && resumeChapterTitle && resumeChapterBookMatches
      ? t("bookshelf.nowPlayingChapter", { title: resumeChapterTitle })
      : hasResumeChapter && resumeChapterTitle
        ? t("bookshelf.continuePlayingChapter", { title: resumeChapterTitle })
        : t("bookshelf.playNow");

  const marqueeLabel =
    hasResumeChapter && resumeChapterTitle && resumeChapterBookMatches
      ? `${t("bookshelf.nowPlayingChapter", { title: resumeChapterTitle })}    `
      : hasResumeChapter && resumeChapterTitle
        ? `${t("bookshelf.continuePlayingChapter", { title: resumeChapterTitle })}    `
        : t("bookshelf.playNow");
  const favoriteLabel = t(
    isFavorite ? "bookshelf.favorited" : "bookshelf.favorite",
  );
  const scrapeLabel = t("bookshelf.scrape");
  const editLabel = t("common.edit");
  const moreLabel = t("bookshelf.more");
  const actionButtonLabels = isAdmin
    ? [favoriteLabel, scrapeLabel, editLabel, moreLabel]
    : [favoriteLabel, moreLabel];
  const actionButtonWidthLabels = isAdmin
    ? [
        t("bookshelf.favorite"),
        t("bookshelf.favorited"),
        scrapeLabel,
        editLabel,
        moreLabel,
      ]
    : [t("bookshelf.favorite"), t("bookshelf.favorited"), moreLabel];
  const actionButtonCount = actionButtonLabels.length;
  const longestActionLabelWidth = Math.max(
    ...actionButtonWidthLabels.map((label) =>
      Array.from(label).reduce((width, char) => {
        if (/[\u3400-\u9fff\uf900-\ufaff]/.test(char)) return width + 14;
        if (/\s/.test(char)) return width + 4;
        return width + 8;
      }, 0),
    ),
  );
  const actionButtonWidth = Math.max(
    100,
    Math.ceil(longestActionLabelWidth + 68),
  );
  const actionButtonRowWidth =
    actionButtonWidth * actionButtonCount +
    Math.max(0, actionButtonCount - 1) * 12;
  const actionButtonGroupStyle = {
    width: "100%",
    maxWidth: `${Math.max(320, actionButtonRowWidth)}px`,
  } as React.CSSProperties;

  return (
    <div
      className={`flex flex-col md:flex-row gap-6 md:gap-8 ${coverShape === "square" ? "md:items-center" : ""}`}
    >
      <div className="w-48 md:w-72 mx-auto md:mx-0 shrink-0">
        <div
          className={`${coverShape === "square" ? "aspect-square" : "aspect-[3/4]"} rounded-3xl overflow-hidden shadow-2xl border border-slate-200 dark:border-slate-800`}
        >
          <img
            src={getCoverUrl(displayCoverUrl, displayLibraryId, book.id)}
            alt={book.title}
            className="w-full h-full object-cover rounded-lg shadow-xl"
            referrerPolicy="no-referrer"
            onError={(e) => {
              const target = e.target as HTMLImageElement;
              target.src = "https://placehold.co/300x400?text=No+Cover";
              target.onerror = null;
            }}
          />
        </div>
      </div>

      <div className="flex-1 space-y-6 text-center md:text-left flex flex-col">
        <div className="space-y-3 min-w-0">
          <ExpandableTitle
            title={book.title}
            className="font-bold text-slate-900 dark:text-white leading-tight transition-all duration-300 text-xl sm:text-2xl md:text-3xl"
            maxLines={2}
          />
          <div className="flex flex-wrap justify-center md:justify-start gap-x-4 gap-y-2 mt-4 text-sm">
            <div className="flex items-center gap-1.5 text-slate-600 dark:text-slate-400">
              <User size={16} className="text-primary-500" />
              <span className="font-bold">
                {book.author || t("bookshelf.unknownAuthor")}
              </span>
            </div>
            <div className="flex items-center gap-1.5 text-slate-600 dark:text-slate-400">
              <Mic2 size={16} className="text-primary-500" />
              <span className="font-bold">
                {book.narrator || t("bookshelf.unknownNarrator")}
              </span>
            </div>
            <div className="flex items-center gap-1.5 text-slate-600 dark:text-slate-400">
              <ListMusic size={16} className="text-primary-500" />
              <span className="font-bold">
                {t("bookshelf.chapterCount", { count: chapterTotalCount })}
              </span>
            </div>
          </div>

          {book.tags && (
            <div className="mt-3 flex items-start justify-center md:justify-start w-full gap-2">
              <div
                ref={tagsRef}
                className={`flex flex-wrap gap-2 transition-all duration-300 overflow-hidden justify-center md:justify-start ${
                  isTagsExpanded ? "max-h-[500px]" : "max-h-[32px]"
                }`}
              >
                {book.tags
                  .split(/[,，]/)
                  .filter((tag) => tag.trim())
                  .map((tag, index) => (
                    <span
                      key={index}
                      className="px-2.5 py-1 bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-400 text-xs font-bold rounded-lg border border-slate-200/50 dark:border-slate-700/50 whitespace-nowrap"
                    >
                      {tag.trim()}
                    </span>
                  ))}
                {isTagsExpanded && (
                  <button
                    onClick={() => onSetIsTagsExpanded(false)}
                    className="px-2 py-0.5 text-[10px] font-bold text-primary-500 hover:text-primary-600 flex items-center gap-0.5 bg-primary-50 dark:bg-primary-900/20 rounded-md border border-primary-100 dark:border-primary-900/30 shadow-sm self-center"
                  >
                    <ChevronUp size={10} /> {t("bookshelf.collapse")}
                  </button>
                )}
              </div>
              {isTagsOverflowing && !isTagsExpanded && (
                <button
                  onClick={() => onSetIsTagsExpanded(true)}
                  className="shrink-0 px-2 py-0.5 text-[10px] font-bold text-primary-500 hover:text-primary-600 flex items-center gap-0.5 bg-primary-50 dark:bg-primary-900/20 rounded-md border border-primary-100 dark:border-primary-900/30 shadow-sm mt-1"
                >
                  <ChevronDown size={10} /> {t("bookshelf.more")}
                </button>
              )}
            </div>
          )}
        </div>

        <div
          className="w-full flex flex-col gap-3 mx-auto md:mx-0"
          style={actionButtonGroupStyle}
        >
          <button
            ref={playButtonContainerRef}
            onClick={onPlayClick}
            className="w-full flex items-center justify-center gap-2 px-5 sm:px-8 py-3.5 sm:py-4 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-2xl shadow-xl shadow-primary-500/30 transition-all active:scale-95 group"
            style={
              effectiveThemeColor
                ? {
                    backgroundColor: toSolidColor(effectiveThemeColor),
                    boxShadow: `0 10px 20px -5px ${setAlpha(effectiveThemeColor, 0.3)}`,
                    color: isLight(effectiveThemeColor) ? "#475569" : "#ffffff",
                  }
                : {}
            }
          >
            <Play size={18} fill="currentColor" className="shrink-0" />
            {isPlayButtonTextOverflowing ? (
              <div className="flex-1 min-w-0 overflow-hidden">
                <div className="whitespace-nowrap inline-block animate-scroll-text">
                  {marqueeLabel}
                  {marqueeLabel !== t("bookshelf.playNow") ? marqueeLabel : ""}
                </div>
              </div>
            ) : (
              <span className="truncate">{playLabel}</span>
            )}
          </button>

          <div
            className="grid w-full grid-flow-col auto-cols-fr gap-1 min-[430px]:gap-1.5 md:gap-3"
          >
            <button
              onClick={onToggleFavorite}
              className={`w-full min-w-0 px-1.5 min-[430px]:px-2 lg:px-4 py-2.5 lg:py-3 rounded-xl lg:rounded-2xl border transition-all active:scale-95 flex items-center justify-center gap-1 min-[430px]:gap-1.5 lg:gap-2 font-bold text-[11px] min-[430px]:text-xs lg:text-sm whitespace-nowrap ${
                isFavorite
                  ? "bg-red-50 border-red-100 text-red-500 dark:bg-red-900/20 dark:border-red-900/30"
                  : "bg-white dark:bg-slate-900 border-slate-200 dark:border-slate-800 text-slate-500 dark:text-slate-300 hover:text-red-500"
              }`}
              aria-label={favoriteLabel}
            >
              <Heart
                size={20}
                className="h-4 w-4 shrink-0 lg:h-5 lg:w-5"
                fill={isFavorite ? "currentColor" : "none"}
              />
              <span className="hidden min-[380px]:inline">
                {favoriteLabel}
              </span>
            </button>

            {isAdmin && (
              <>
                <button
                  onClick={onOpenScrapeDiff}
                  className="w-full min-w-0 px-1.5 min-[430px]:px-2 lg:px-4 py-2.5 lg:py-3 rounded-xl lg:rounded-2xl border bg-white dark:bg-slate-900 border-slate-200 dark:border-slate-800 text-slate-500 dark:text-slate-300 hover:text-primary-600 transition-all active:scale-95 flex items-center justify-center gap-1 min-[430px]:gap-1.5 lg:gap-2 font-bold text-[11px] min-[430px]:text-xs lg:text-sm whitespace-nowrap"
                  title={t("bookshelf.scrapeMetadata")}
                >
                  <RefreshCw
                    size={20}
                    className="h-4 w-4 shrink-0 lg:h-5 lg:w-5"
                  />
                  <span className="hidden min-[380px]:inline">
                    {scrapeLabel}
                  </span>
                </button>
                <button
                  onClick={onOpenEditModal}
                  className="w-full min-w-0 px-1.5 min-[430px]:px-2 lg:px-4 py-2.5 lg:py-3 rounded-xl lg:rounded-2xl border bg-white dark:bg-slate-900 border-slate-200 dark:border-slate-800 text-slate-500 dark:text-slate-300 hover:text-primary-600 transition-all active:scale-95 flex items-center justify-center gap-1 min-[430px]:gap-1.5 lg:gap-2 font-bold text-[11px] min-[430px]:text-xs lg:text-sm whitespace-nowrap"
                >
                  <Edit
                    size={20}
                    className="h-4 w-4 shrink-0 lg:h-5 lg:w-5"
                  />
                  <span className="hidden min-[380px]:inline">
                    {editLabel}
                  </span>
                </button>
              </>
            )}
            <PluginExtensionSlot
              slot="book.detail_action"
              className="relative min-w-0"
              buttonClassName="w-full min-w-0 px-1.5 min-[430px]:px-2 lg:px-4 py-2.5 lg:py-3 rounded-xl lg:rounded-2xl border bg-white dark:bg-slate-900 border-slate-200 dark:border-slate-800 text-slate-500 dark:text-slate-300 hover:text-primary-600 transition-all active:scale-95 flex items-center justify-center gap-1 min-[430px]:gap-1.5 lg:gap-2 font-bold text-[11px] min-[430px]:text-xs lg:text-sm whitespace-nowrap"
              menuLabel={moreLabel}
              menuLabelClassName="hidden min-[380px]:inline"
              context={{
                book_id: book.id,
                book_title: book.title,
                book_path: book.path,
                library_id: book.library_id || displayLibraryId,
                author: book.author,
                narrator: book.narrator,
                chapter_count: chapterTotalCount,
              }}
            />
          </div>
        </div>

        <div
          className="mt-auto space-y-3 p-4 rounded-2xl border border-slate-100 dark:border-slate-800/50 relative group/desc"
          style={
            effectiveThemeColor
              ? {
                  backgroundColor: setAlpha(effectiveThemeColor, 0.08),
                }
              : {}
          }
        >
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2 text-slate-900 dark:text-white font-bold text-sm uppercase tracking-wider opacity-60">
              <Info size={16} />
              {t("bookshelf.descriptionTitle")}
            </div>
          </div>
          <div className="relative">
            <p
              ref={descriptionRef}
              className={`text-sm md:text-base text-slate-600 dark:text-slate-400 leading-relaxed transition-all duration-300 ${
                !isDescriptionExpanded ? "line-clamp-2" : ""
              }`}
            >
              {book.description || t("bookshelf.noDescription")}
            </p>
            {(isDescriptionOverflowing || isDescriptionExpanded) && (
              <button
                onClick={() =>
                  onSetIsDescriptionExpanded(!isDescriptionExpanded)
                }
                className="mt-2 text-primary-600 hover:text-primary-700 text-sm font-bold flex items-center gap-1 transition-colors"
              >
                {isDescriptionExpanded ? (
                  <>
                    <ChevronUp size={16} />
                    {t("bookshelf.collapseDetails")}
                  </>
                ) : (
                  <>
                    <ChevronDown size={16} />
                    {t("bookshelf.expandAll")}
                  </>
                )}
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};

export default BookHeaderSection;
