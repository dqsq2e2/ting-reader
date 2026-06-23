import React from 'react';
import { Filter } from 'lucide-react';

export type DisplaySettingsOption = {
  value: string;
  label: string;
};

export type DisplaySettingsSection = {
  title: string;
  value: string;
  options: DisplaySettingsOption[];
  onChange: (value: string) => void;
};

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  sections: DisplaySettingsSection[];
  className?: string;
  buttonLabel?: string;
  sheetLabel?: string;
};

const DisplaySettingsMenu: React.FC<Props> = ({
  open,
  onOpenChange,
  sections,
  className = '',
  buttonLabel = '展示设置',
  sheetLabel = '关闭展示设置',
}) => {
  const handleSelect = (section: DisplaySettingsSection, value: string) => {
    section.onChange(value);
    onOpenChange(false);
  };

  const renderMenuContent = (compact = false) => (
    <>
      {sections.map((section, index) => (
        <div key={section.title}>
          <div
            className={`${compact ? 'mb-4 text-base font-black text-slate-900 dark:text-white' : `px-4 py-2 text-xs font-bold text-slate-400 uppercase tracking-widest ${index > 0 ? 'border-t ' : ''}border-b border-slate-50 dark:border-slate-800 ${index > 0 ? 'mt-2 ' : ''}mb-1`}`}
          >
            {section.title}
          </div>
          {section.options.map(option => {
            const selected = section.value === option.value;
            return (
              <button
                key={option.value}
                type="button"
                onClick={() => handleSelect(section, option.value)}
                className={compact
                  ? `flex w-full items-center justify-between py-4 text-left text-lg transition-colors ${selected ? 'font-bold text-primary-600' : 'font-medium text-slate-600 dark:text-slate-300'}`
                  : `w-full px-4 py-2.5 text-left text-sm flex items-center justify-between ${selected ? 'text-primary-600 font-bold bg-primary-50/50 dark:bg-primary-900/20' : 'text-slate-600 dark:text-slate-300 hover:bg-slate-50 dark:hover:bg-slate-800'}`}
              >
                <span>{option.label}</span>
                {selected && (
                  <span className={compact ? 'h-2.5 w-2.5 rounded-full bg-primary-600' : 'w-1.5 h-1.5 rounded-full bg-primary-600'} />
                )}
              </button>
            );
          })}
        </div>
      ))}
    </>
  );

  return (
    <div className={`relative min-w-0 ${className}`}>
      <button
        type="button"
        aria-label={buttonLabel}
        onClick={() => onOpenChange(!open)}
        className={`p-2.5 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl text-slate-600 dark:text-slate-400 hover:bg-slate-50 dark:hover:bg-slate-800 transition-colors ${open ? 'ring-2 ring-primary-500' : ''}`}
      >
        <Filter size={20} />
      </button>

      {open && (
        <div
          className="absolute right-0 top-full mt-2 w-56 max-w-[calc(100vw-2rem)] bg-white dark:bg-slate-900 border border-slate-100 dark:border-slate-800 rounded-2xl shadow-xl z-50 py-2 animate-in zoom-in-95 duration-200"
          aria-label={sheetLabel}
        >
          {renderMenuContent()}
        </div>
      )}
    </div>
  );
};

export default DisplaySettingsMenu;
