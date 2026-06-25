import React, { useRef } from 'react';
import { ChevronLeft, Search, X } from 'lucide-react';
import type { ChapterGroup, ChapterTab } from './types';

interface Props {
  search: string;
  activeTab: ChapterTab;
  mainCount: number;
  extraCount: number;
  groups: ChapterGroup[];
  currentGroupIndex: number;
  onSearchChange: (value: string) => void;
  onTabChange: (tab: ChapterTab) => void;
  onGroupChange: (index: number) => void;
  onClose: () => void;
}

const ChapterManagerHeader: React.FC<Props> = ({
  search,
  activeTab,
  mainCount,
  extraCount,
  groups,
  currentGroupIndex,
  onSearchChange,
  onTabChange,
  onGroupChange,
  onClose,
}) => {
  const scrollRef = useRef<HTMLDivElement>(null);
  const showTypeSwitch = mainCount > 0 && extraCount > 0;
  const showGroups = groups.length > 1;

  const scrollGroups = (direction: 'left' | 'right') => {
    const element = scrollRef.current;
    if (!element) return;
    element.scrollBy({
      left: direction === 'left' ? -element.clientWidth * 0.75 : element.clientWidth * 0.75,
      behavior: 'smooth',
    });
  };

  return (
    <div className="shrink-0 border-b border-slate-100 dark:border-slate-800">
      <div className="px-4 pt-4 pb-3 sm:px-6 sm:pt-5 sm:pb-4">
        <div className="flex items-center gap-3">
          <div className="flex min-w-0 flex-1 items-center gap-2 sm:gap-3">
            <h2 className="shrink-0 text-xl font-bold leading-none text-slate-950 dark:text-white sm:text-2xl">
              章节管理
            </h2>
            {showTypeSwitch && (
              <div className="inline-flex min-w-0 items-center rounded-2xl border border-slate-200 bg-slate-100/80 p-1 dark:border-slate-700 dark:bg-slate-800">
                <TypeSwitchButton
                  selected={activeTab === 'main'}
                  label="正文"
                  count={mainCount}
                  onClick={() => onTabChange('main')}
                />
                <TypeSwitchButton
                  selected={activeTab === 'extra'}
                  label="番外"
                  count={extraCount}
                  onClick={() => onTabChange('extra')}
                />
              </div>
            )}
          </div>
          <button
            type="button"
            onClick={onClose}
            className="rounded-full p-2 text-slate-500 transition-colors hover:bg-slate-100 hover:text-slate-700 dark:hover:bg-slate-800 dark:hover:text-slate-200"
            title="关闭"
          >
            <X size={24} />
          </button>
        </div>

        <label className="mt-4 flex h-12 items-center gap-3 rounded-2xl border border-slate-200 bg-slate-50 px-4 text-slate-500 transition-colors focus-within:border-primary-300 focus-within:bg-white dark:border-slate-700 dark:bg-slate-800 dark:focus-within:border-primary-700">
          <Search size={18} className="shrink-0" />
          <input
            value={search}
            onChange={(event) => onSearchChange(event.target.value)}
            placeholder="搜索章节、序号"
            className="h-full min-w-0 flex-1 border-none bg-transparent p-0 text-sm font-normal text-slate-900 outline-none focus:ring-0 dark:text-white sm:text-base"
          />
        </label>

        {showGroups && (
          <div className="group/nav relative mt-3 flex items-center">
            <button
              type="button"
              onClick={() => scrollGroups('left')}
              className="absolute -left-2 top-1/2 z-10 hidden -translate-y-1/2 rounded-full border border-slate-200 bg-white/95 p-1.5 text-slate-600 opacity-0 shadow-md backdrop-blur transition-opacity group-hover/nav:opacity-100 dark:border-slate-700 dark:bg-slate-800/95 dark:text-slate-300 sm:block"
              title="向左"
            >
              <ChevronLeft size={18} />
            </button>
            <div
              ref={scrollRef}
              className="no-scrollbar flex w-full gap-2 overflow-x-auto scroll-smooth pr-8"
            >
              {groups.map((group) => (
                <button
                  key={group.index}
                  type="button"
                  onClick={() => onGroupChange(group.index)}
                  className={`shrink-0 rounded-xl border px-3.5 py-2 text-sm font-semibold transition-all ${
                    currentGroupIndex === group.index
                      ? 'border-primary-600 bg-primary-600 text-white shadow-sm'
                      : 'border-slate-200 bg-white text-slate-600 hover:border-primary-200 hover:text-primary-600 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-300'
                  }`}
                >
                  第 {group.start}-{group.end} 章
                </button>
              ))}
            </div>
            <button
              type="button"
              onClick={() => scrollGroups('right')}
              className="absolute -right-2 top-1/2 z-10 hidden -translate-y-1/2 rounded-full border border-slate-200 bg-white/95 p-1.5 text-slate-600 opacity-0 shadow-md backdrop-blur transition-opacity group-hover/nav:opacity-100 dark:border-slate-700 dark:bg-slate-800/95 dark:text-slate-300 sm:block"
              title="向右"
            >
              <ChevronLeft size={18} className="rotate-180" />
            </button>
          </div>
        )}
      </div>
    </div>
  );
};

interface TypeSwitchButtonProps {
  selected: boolean;
  label: string;
  count: number;
  onClick: () => void;
}

const TypeSwitchButton: React.FC<TypeSwitchButtonProps> = ({ selected, label, count, onClick }) => (
  <button
    type="button"
    onClick={onClick}
    className={`rounded-xl px-3 py-1.5 text-xs font-semibold leading-none transition-all sm:text-sm ${
      selected
        ? 'bg-primary-600 text-white shadow-sm'
        : 'text-slate-500 hover:text-slate-800 dark:text-slate-300 dark:hover:text-white'
    }`}
  >
    {label}
    <span className="hidden sm:inline"> {count}</span>
  </button>
);

export default ChapterManagerHeader;
