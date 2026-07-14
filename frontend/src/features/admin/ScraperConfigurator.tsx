import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ArrowDown, ArrowUp, Plus, X } from 'lucide-react';
import type { ScraperSource } from '../../core/types';
import HelpHint from '../../shared/ui/HelpHint';

interface Props {
  configStr: string;
  sources: Pick<ScraperSource, 'id' | 'name' | 'auto_scrape'>[];
  onChange: (newConfigStr: string) => void;
  libraryType: string;
}

const ScraperConfigurator: React.FC<Props> = ({ configStr, sources, onChange, libraryType }) => {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState('default');

  const tabs = [
    { id: 'priority', label: t('scraperConfig.priority'), key: 'metadata_priority' },
    { id: 'default', label: t('scraperConfig.default'), key: 'default_sources' },
    { id: 'cover', label: t('scraperConfig.cover'), key: 'cover_sources' },
    { id: 'intro', label: t('scraperConfig.intro'), key: 'intro_sources' },
    { id: 'author', label: t('scraperConfig.author'), key: 'author_sources' },
    { id: 'narrator', label: t('scraperConfig.narrator'), key: 'narrator_sources' },
    { id: 'tags', label: t('scraperConfig.tags'), key: 'tags_sources' },
  ];

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let config: Record<string, any> = {};
  try {
    config = configStr ? JSON.parse(configStr) : {};
  } catch {
    config = {};
  }

  const currentTab = tabs.find(t => t.id === activeTab) || tabs[0];
  const currentKey = currentTab.key;

  // Special handling for priority tab
  const PRIORITY_SOURCES = [
    { id: 'local_metadata', name: t('scraperConfig.localMetadata') },
    { id: 'audio_metadata', name: t('scraperConfig.audioMetadata') },
    { id: 'scraper', name: t('scraperConfig.scraper') }
  ];

  let activeIds: string[] = config[currentKey] ?? [];

  // Initialize default priority if empty
  if (activeTab === 'priority' && activeIds.length === 0) {
      activeIds = ['local_metadata', 'audio_metadata', 'scraper'];
  }
  if (activeTab !== 'priority') {
    const autoSourceIds = new Set(sources.map(source => source.id));
    activeIds = activeIds.filter(id => autoSourceIds.has(id));
  }

  const nfoEnabled = config.nfo_writing_enabled ?? false;
  const metadataWritingEnabled = config.metadata_writing_enabled ?? false;
  const useFilenameAsTitle = config.use_filename_as_title ?? true;
  const extractAudioCover = config.extract_audio_cover ?? true;
  const extractExtraChapters = config.extract_extra_chapters ?? true;
  const disableWatcher = config.disable_watcher ?? false;
  const cloudMode = config.cloud_mode ?? false;

  const handleNfoChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      const newConfig: Record<string, unknown> = { ...config, nfo_writing_enabled: e.target.checked };
      onChange(JSON.stringify(newConfig, null, 2));
  };

  const handleMetadataWritingChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      const newConfig: Record<string, unknown> = { ...config, metadata_writing_enabled: e.target.checked };
      onChange(JSON.stringify(newConfig, null, 2));
  };

  const handlePreferAudioTitleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      const newConfig: Record<string, unknown> = { ...config, use_filename_as_title: e.target.checked };
      onChange(JSON.stringify(newConfig, null, 2));
  };

  const handleExtractAudioCoverChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      const newConfig: Record<string, unknown> = { ...config, extract_audio_cover: e.target.checked };
      onChange(JSON.stringify(newConfig, null, 2));
  };

  const handleExtractExtraChaptersChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      const newConfig: Record<string, unknown> = { ...config, extract_extra_chapters: e.target.checked };
      onChange(JSON.stringify(newConfig, null, 2));
  };

  const handleDisableWatcherChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      // e.target.checked is true when user wants to ENABLE watcher
      // so disable_watcher should be the opposite (!e.target.checked)
      const newConfig: Record<string, unknown> = { ...config, disable_watcher: !e.target.checked };
      onChange(JSON.stringify(newConfig, null, 2));
  };

  const handleCloudModeChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      const newConfig: Record<string, unknown> = { ...config, cloud_mode: e.target.checked };
      onChange(JSON.stringify(newConfig, null, 2));
  };

  const handleAdd = (sourceId: string) => {
    const newConfig = { ...config, [currentKey]: [...activeIds, sourceId] };
    onChange(JSON.stringify(newConfig, null, 2));
  };

  const handleRemove = (sourceId: string) => {
    const newConfig = { ...config, [currentKey]: activeIds.filter(id => id !== sourceId) };
    onChange(JSON.stringify(newConfig, null, 2));
  };

  const handleMove = (index: number, direction: 'up' | 'down') => {
    const newList = [...activeIds];
    if (direction === 'up' && index > 0) {
      [newList[index], newList[index - 1]] = [newList[index - 1], newList[index]];
    } else if (direction === 'down' && index < newList.length - 1) {
      [newList[index], newList[index + 1]] = [newList[index + 1], newList[index]];
    }
    const newConfig = { ...config, [currentKey]: newList };
    onChange(JSON.stringify(newConfig, null, 2));
  };

  const activeSources = activeIds.map(id => {
    const sourceList = activeTab === 'priority' ? PRIORITY_SOURCES : sources;
    const source = sourceList.find(s => s.id === id);
    return source || { id, name: id }; // Fallback for unknown IDs
  });

  const availableSources = activeTab === 'priority'
      ? []
      : sources.filter(s => !activeIds.includes(s.id));

  return (
    <div className="bg-slate-50 dark:bg-slate-800 rounded-xl p-4 border border-slate-200 dark:border-slate-700">
      {/* Settings Toggles */}
      <div className="space-y-2 mb-4">
        {/* NFO Toggle - Show for all libraries */}
        <div className="flex items-center gap-3 p-3 bg-white dark:bg-slate-900 rounded-lg border border-slate-200 dark:border-slate-700 shadow-sm">
          <input
            type="checkbox"
            id="nfo-writing"
            checked={nfoEnabled}
            onChange={handleNfoChange}
            className="w-4 h-4 text-primary-600 rounded focus:ring-primary-500 cursor-pointer"
          />
          <div className="flex min-w-0 items-center gap-1.5">
            <label htmlFor="nfo-writing" className="text-sm font-bold text-slate-700 dark:text-slate-300 cursor-pointer">
              {t('scraperConfig.enableNfo')}
            </label>
            <HelpHint text={t('scraperConfig.enableNfoHelp')} />
          </div>
        </div>

        {/* Metadata JSON Toggle - Show for all libraries */}
        <div className="flex items-center gap-3 p-3 bg-white dark:bg-slate-900 rounded-lg border border-slate-200 dark:border-slate-700 shadow-sm">
          <input
            type="checkbox"
            id="metadata-writing"
            checked={metadataWritingEnabled}
            onChange={handleMetadataWritingChange}
            className="w-4 h-4 text-primary-600 rounded focus:ring-primary-500 cursor-pointer"
          />
          <div className="flex min-w-0 items-center gap-1.5">
            <label htmlFor="metadata-writing" className="text-sm font-bold text-slate-700 dark:text-slate-300 cursor-pointer">
              {t('scraperConfig.writeMetadataJson')}
            </label>
            <HelpHint text={t('scraperConfig.writeMetadataJsonHelp')} />
          </div>
        </div>

        {/* Prefer Audio Title - Show for all libraries */}
        <div className="flex items-center gap-3 p-3 bg-white dark:bg-slate-900 rounded-lg border border-slate-200 dark:border-slate-700 shadow-sm">
          <input
            type="checkbox"
            id="prefer-audio-title"
            checked={useFilenameAsTitle}
            onChange={handlePreferAudioTitleChange}
            className="w-4 h-4 text-primary-600 rounded focus:ring-primary-500 cursor-pointer"
          />
          <div className="flex min-w-0 items-center gap-1.5">
            <label htmlFor="prefer-audio-title" className="text-sm font-bold text-slate-700 dark:text-slate-300 cursor-pointer">
              {t('scraperConfig.preferAudioTitle')}
            </label>
            <HelpHint text={t('scraperConfig.preferAudioTitleHelp')} />
          </div>
        </div>

        {/* Extract Audio Cover - Show for all libraries */}
        <div className="flex items-center gap-3 p-3 bg-white dark:bg-slate-900 rounded-lg border border-slate-200 dark:border-slate-700 shadow-sm">
          <input
            type="checkbox"
            id="extract-audio-cover"
            checked={extractAudioCover}
            onChange={handleExtractAudioCoverChange}
            className="w-4 h-4 text-primary-600 rounded focus:ring-primary-500 cursor-pointer"
          />
          <div className="flex min-w-0 items-center gap-1.5">
            <label htmlFor="extract-audio-cover" className="text-sm font-bold text-slate-700 dark:text-slate-300 cursor-pointer">
              {t('scraperConfig.extractAudioCover')}
            </label>
            <HelpHint text={t('scraperConfig.extractAudioCoverHelp')} />
          </div>
        </div>

        <div className="flex items-center gap-3 p-3 bg-white dark:bg-slate-900 rounded-lg border border-slate-200 dark:border-slate-700 shadow-sm">
          <input
            type="checkbox"
            id="extract-extra-chapters"
            checked={extractExtraChapters}
            onChange={handleExtractExtraChaptersChange}
            className="w-4 h-4 text-primary-600 rounded focus:ring-primary-500 cursor-pointer"
          />
          <div className="flex min-w-0 items-center gap-1.5">
            <label htmlFor="extract-extra-chapters" className="text-sm font-bold text-slate-700 dark:text-slate-300 cursor-pointer">
              {t('scraperConfig.extractExtraChapters')}
            </label>
            <HelpHint text={t('scraperConfig.extractExtraChaptersHelp')} />
          </div>
        </div>

        {/* Disable Watcher - Only relevant for local libraries but we can show it */}
        {libraryType === 'local' && (
          <div className="flex items-center gap-3 p-3 bg-white dark:bg-slate-900 rounded-lg border border-slate-200 dark:border-slate-700 shadow-sm">
            <input
              type="checkbox"
              id="disable-watcher"
              checked={!disableWatcher}
              onChange={handleDisableWatcherChange}
              className="w-4 h-4 text-primary-600 rounded focus:ring-primary-500 cursor-pointer"
            />
            <div className="flex min-w-0 items-center gap-1.5">
              <label htmlFor="disable-watcher" className="text-sm font-bold text-slate-700 dark:text-slate-300 cursor-pointer">
                {t('scraperConfig.autoDetectChanges')}
              </label>
              <HelpHint text={t('scraperConfig.autoDetectChangesHelp')} />
            </div>
          </div>
        )}

        {/* Cloud / Drive Mode - applies to both WebDAV and local libraries */}
        <div className="flex items-center gap-3 p-3 bg-white dark:bg-slate-900 rounded-lg border border-slate-200 dark:border-slate-700 shadow-sm">
          <input
            type="checkbox"
            id="cloud-mode"
            checked={cloudMode}
            onChange={handleCloudModeChange}
            className="w-4 h-4 text-primary-600 rounded focus:ring-primary-500 cursor-pointer"
          />
          <div className="flex min-w-0 items-center gap-1.5">
            <label htmlFor="cloud-mode" className="text-sm font-bold text-slate-700 dark:text-slate-300 cursor-pointer">
              {t('scraperConfig.cloudMode')}
            </label>
            <HelpHint text={t('scraperConfig.cloudModeHelp')} />
          </div>
        </div>
      </div>

      {/* Tabs */}
      <div className="flex gap-1 overflow-x-auto pb-2 mb-4 border-b border-slate-200 dark:border-slate-700 no-scrollbar">
        {tabs.map(tab => (
          <button
            key={tab.id}
            type="button"
            onClick={() => setActiveTab(tab.id)}
            className={`px-2.5 py-1 rounded-lg text-xs sm:text-sm font-bold whitespace-nowrap transition-all ${
              activeTab === tab.id
                ? 'bg-white dark:bg-slate-700 text-primary-600 shadow-sm'
                : 'text-slate-500 hover:text-slate-700 dark:hover:text-slate-300 hover:bg-slate-200/50 dark:hover:bg-slate-700/50'
            }`}
          >
            {tab.label}
          </button>
        ))}
      </div>

      <div className={`grid ${activeTab === 'priority' ? 'grid-cols-1' : 'grid-cols-1 sm:grid-cols-2'} gap-4`}>
        {/* Active List (Ordered) */}
        <div className="space-y-2">
          <div className="text-xs font-bold text-slate-500 uppercase tracking-wider flex justify-between">
            <span className="flex min-w-0 items-center gap-1.5">
              <span>{activeTab === 'priority' ? t('scraperConfig.activePriorityTitle') : t('scraperConfig.activeTitle')}</span>
              <HelpHint text={t('scraperConfig.activeHelp')} />
            </span>
            <span className="text-primary-600">{activeSources.length}</span>
          </div>
          <div className="bg-white dark:bg-slate-900 rounded-lg border border-slate-200 dark:border-slate-700 min-h-[120px] p-2 space-y-2">
            {activeSources.length > 0 ? (
              activeSources.map((source, index) => (
                <div key={source.id} className="flex items-center justify-between p-2 bg-slate-50 dark:bg-slate-800 rounded-md group">
                  <span className="text-sm font-medium truncate flex-1 mr-2 dark:text-slate-300">{source.name}</span>
                  <div className="flex items-center gap-1 opacity-60 group-hover:opacity-100 transition-opacity">
                    <button
                      type="button"
                      onClick={() => handleMove(index, 'up')}
                      disabled={index === 0}
                      className="p-1 hover:bg-slate-200 dark:hover:bg-slate-700 rounded disabled:opacity-30"
                    >
                      <ArrowUp size={14} />
                    </button>
                    <button
                      type="button"
                      onClick={() => handleMove(index, 'down')}
                      disabled={index === activeSources.length - 1}
                      className="p-1 hover:bg-slate-200 dark:hover:bg-slate-700 rounded disabled:opacity-30"
                    >
                      <ArrowDown size={14} />
                    </button>
                    <button
                      type="button"
                      onClick={() => handleRemove(source.id)}
                      disabled={activeTab === 'priority'}
                      className={`p-1 rounded ml-1 ${activeTab === 'priority' ? 'opacity-0 cursor-default' : 'hover:bg-red-100 text-slate-400 hover:text-red-500'}`}
                    >
                      <X size={14} />
                    </button>
                  </div>
                </div>
              ))
            ) : (
              <div className="h-full flex flex-col items-center justify-center text-slate-400 text-xs italic p-4">
                <span>{t('scraperConfig.noActiveSources')}</span>
                <span>{t('scraperConfig.addFromRight')}</span>
              </div>
            )}
          </div>
        </div>

        {/* Available List */}
        {activeTab !== 'priority' && (
          <div className="space-y-2">
            <div className="text-xs font-bold text-slate-500 uppercase tracking-wider flex justify-between">
              <span>{t('scraperConfig.availablePlugins')}</span>
              <span className="text-slate-400">{availableSources.length}</span>
            </div>
            <div className="bg-white dark:bg-slate-900 rounded-lg border border-slate-200 dark:border-slate-700 min-h-[120px] p-2 space-y-2">
              {availableSources.length > 0 ? (
                availableSources.map(source => (
                  <button
                    key={source.id}
                    type="button"
                    onClick={() => handleAdd(source.id)}
                    className="w-full flex items-center justify-between p-2 hover:bg-slate-50 dark:hover:bg-slate-800 rounded-md group text-left transition-colors"
                  >
                    <span className="text-sm font-medium truncate dark:text-slate-400 group-hover:text-slate-600 dark:group-hover:text-slate-200">{source.name}</span>
                    <Plus size={16} className="text-primary-500 opacity-0 group-hover:opacity-100 transition-opacity" />
                  </button>
                ))
              ) : (
                <div className="h-full flex items-center justify-center text-slate-400 text-xs italic p-4">
                  {sources.length === 0 ? t('scraperConfig.noPluginsDetected') : t('scraperConfig.allAdded')}
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

export default ScraperConfigurator;
