import React from 'react';
import {
  ArrowRight,
  Check,
  CheckSquare,
  CornerDownRight,
  ListOrdered,
  ListTodo,
  Loader2,
  Square,
} from 'lucide-react';

interface Props {
  selectionMode: boolean;
  allSelected: boolean;
  totalCount: number;
  selectedCount: number;
  moving: boolean;
  onToggleSelectionMode: () => void;
  onToggleAll: () => void;
  onRenumber: () => void;
  onJump: () => void;
  onMove: () => void;
}

const ChapterManagerToolbar: React.FC<Props> = ({
  selectionMode,
  allSelected,
  totalCount,
  selectedCount,
  moving,
  onToggleSelectionMode,
  onToggleAll,
  onRenumber,
  onJump,
  onMove,
}) => {
  return (
    <div className="shrink-0 border-b border-slate-100 bg-slate-50/80 px-4 py-3 dark:border-slate-800 dark:bg-slate-900/70 sm:px-6">
      {selectionMode ? (
        <div className="flex flex-wrap items-center gap-2">
          <ToolbarButton active icon={<Check size={17} />} label="完成" onClick={onToggleSelectionMode} />
          <ToolbarButton
            icon={allSelected ? <CheckSquare size={17} /> : <Square size={17} />}
            label={`全选 ${totalCount}`}
            onClick={onToggleAll}
            disabled={totalCount === 0}
          />
          <div className="inline-flex min-h-10 items-center rounded-xl border border-slate-200 bg-white px-4 text-sm font-semibold text-slate-600 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-300">
            已选 {selectedCount}
          </div>
          <ToolbarButton
            icon={moving ? <Loader2 size={17} className="animate-spin" /> : <ArrowRight size={17} />}
            label="移动"
            onClick={onMove}
            disabled={selectedCount === 0 || moving}
          />
        </div>
      ) : (
        <div className="flex flex-wrap items-center gap-2">
          <ToolbarButton
            icon={<ListTodo size={17} />}
            label="选择"
            onClick={onToggleSelectionMode}
            disabled={totalCount === 0}
          />
          <ToolbarButton icon={<ListOrdered size={17} />} label="重排" onClick={onRenumber} />
          <ToolbarButton icon={<CornerDownRight size={17} />} label="跳转" onClick={onJump} />
        </div>
      )}
    </div>
  );
};

interface ToolbarButtonProps {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
  active?: boolean;
  disabled?: boolean;
}

const ToolbarButton: React.FC<ToolbarButtonProps> = ({
  icon,
  label,
  onClick,
  active = false,
  disabled = false,
}) => (
  <button
    type="button"
    onClick={onClick}
    disabled={disabled}
    className={`inline-flex min-h-10 items-center gap-2 rounded-xl border px-3.5 text-sm font-semibold transition-all disabled:cursor-not-allowed disabled:opacity-45 ${
      active
        ? 'border-primary-600 bg-primary-600 text-white shadow-sm'
        : 'border-slate-200 bg-white text-slate-700 hover:border-primary-200 hover:text-primary-600 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-300'
    }`}
  >
    {icon}
    {label}
  </button>
);

export default ChapterManagerToolbar;
