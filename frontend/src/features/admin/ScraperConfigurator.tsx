import React, { useState } from 'react';
import { ArrowDown, ArrowUp, Plus, X } from 'lucide-react';
import type { ScraperSource } from '../../core/types';
import HelpHint from '../../shared/ui/HelpHint';

interface Props {
  configStr: string;
  sources: Pick<ScraperSource, 'id' | 'name' | 'autoScrape'>[];
  onChange: (newConfigStr: string) => void;
  libraryType: string;
}

const ScraperConfigurator: React.FC<Props> = ({ configStr, sources, onChange, libraryType }) => {
  const [activeTab, setActiveTab] = useState('default');

  const tabs = [
    { id: 'priority', label: '优先级', key: 'metadataPriority' },
    { id: 'default', label: '默认', key: 'defaultSources' },
    { id: 'cover', label: '封面', key: 'coverSources' },
    { id: 'intro', label: '简介', key: 'introSources' },
    { id: 'author', label: '作者', key: 'authorSources' },
    { id: 'narrator', label: '演播', key: 'narratorSources' },
    { id: 'tags', label: '标签', key: 'tagsSources' },
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

  // Handle snake_case vs camelCase from legacy data
  const snakeCaseMap: Record<string, string> = {
    'metadataPriority': 'metadata_priority',
    'defaultSources': 'default_sources',
    'coverSources': 'cover_sources',
    'introSources': 'intro_sources',
    'authorSources': 'author_sources',
    'narratorSources': 'narrator_sources',
    'tagsSources': 'tags_sources',
  };

  // Special handling for priority tab
  const PRIORITY_SOURCES = [
    { id: 'local_metadata', name: '本地元数据 (JSON/NFO)' },
    { id: 'audio_metadata', name: '音频文件元数据 (ID3)' },
    { id: 'scraper', name: '刮削器 (Plugins)' }
  ];

  let activeIds: string[] = config[currentKey] ?? config[snakeCaseMap[currentKey]] ?? [];

  // Initialize default priority if empty
  if (activeTab === 'priority' && activeIds.length === 0) {
      activeIds = ['local_metadata', 'audio_metadata', 'scraper'];
  }
  if (activeTab !== 'priority') {
    const autoSourceIds = new Set(sources.map(source => source.id));
    activeIds = activeIds.filter(id => autoSourceIds.has(id));
  }

  const nfoEnabled = config.nfoWritingEnabled ?? config.nfo_writing_enabled ?? false;
  const metadataWritingEnabled = config.metadataWritingEnabled ?? config.metadata_writing_enabled ?? false;
  const preferAudioTitle = config.preferAudioTitle ?? config.prefer_audio_title ?? true;
  const extractAudioCover = config.extractAudioCover ?? config.extract_audio_cover ?? true;
  const disableWatcher = config.disableWatcher ?? config.disable_watcher ?? false;
  const cloudMode = config.cloudMode ?? config.cloud_mode ?? false;

  const handleNfoChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      const newConfig: Record<string, unknown> = { ...config, nfoWritingEnabled: e.target.checked };
      delete newConfig.nfo_writing_enabled;
      onChange(JSON.stringify(newConfig, null, 2));
  };

  const handleMetadataWritingChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      const newConfig: Record<string, unknown> = { ...config, metadataWritingEnabled: e.target.checked };
      delete newConfig.metadata_writing_enabled;
      onChange(JSON.stringify(newConfig, null, 2));
  };

  const handlePreferAudioTitleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      const newConfig: Record<string, unknown> = { ...config, preferAudioTitle: e.target.checked };
      delete newConfig.prefer_audio_title;
      onChange(JSON.stringify(newConfig, null, 2));
  };

  const handleExtractAudioCoverChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      const newConfig: Record<string, unknown> = { ...config, extractAudioCover: e.target.checked };
      delete newConfig.extract_audio_cover;
      onChange(JSON.stringify(newConfig, null, 2));
  };

  const handleDisableWatcherChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      // e.target.checked is true when user wants to ENABLE watcher
      // so disableWatcher should be the opposite (!e.target.checked)
      const newConfig: Record<string, unknown> = { ...config, disableWatcher: !e.target.checked };
      delete newConfig.disable_watcher;
      onChange(JSON.stringify(newConfig, null, 2));
  };

  const handleCloudModeChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      const newConfig: Record<string, unknown> = { ...config, cloudMode: e.target.checked };
      delete newConfig.cloud_mode;
      onChange(JSON.stringify(newConfig, null, 2));
  };

  const handleAdd = (sourceId: string) => {
    const newConfig = { ...config, [currentKey]: [...activeIds, sourceId] };
    delete newConfig[snakeCaseMap[currentKey]];
    onChange(JSON.stringify(newConfig, null, 2));
  };

  const handleRemove = (sourceId: string) => {
    const newConfig = { ...config, [currentKey]: activeIds.filter(id => id !== sourceId) };
    delete newConfig[snakeCaseMap[currentKey]];
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
    delete newConfig[snakeCaseMap[currentKey]];
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
              启用 NFO 元数据写入
            </label>
            <HelpHint text="开启后，刮削或修改元数据时将同步写入 book.nfo 文件" />
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
              写入 metadata.json
            </label>
            <HelpHint text="开启后，生成 Audiobookshelf 兼容的 metadata.json 元数据文件" />
          </div>
        </div>

        {/* Prefer Audio Title - Show for all libraries */}
        <div className="flex items-center gap-3 p-3 bg-white dark:bg-slate-900 rounded-lg border border-slate-200 dark:border-slate-700 shadow-sm">
          <input
            type="checkbox"
            id="prefer-audio-title"
            checked={preferAudioTitle}
            onChange={handlePreferAudioTitleChange}
            className="w-4 h-4 text-primary-600 rounded focus:ring-primary-500 cursor-pointer"
          />
          <div className="flex min-w-0 items-center gap-1.5">
            <label htmlFor="prefer-audio-title" className="text-sm font-bold text-slate-700 dark:text-slate-300 cursor-pointer">
              优先使用文件/文件夹名作为标题
            </label>
            <HelpHint text="开启后，忽略优先级配置，强制使用文件夹名作为书名、文件名作为章节名" />
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
              提取音频封面
            </label>
            <HelpHint text="开启后，系统/插件将尝试从音频文件中提取并保存封面" />
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
                自动检测媒体库变化
              </label>
              <HelpHint text="开启后，将监控该媒体库目录的文件变化并自动触发扫描（修改后即时生效）" />
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
              网盘模式（减少远程音频探测）
            </label>
            <HelpHint text="WebDAV 库开启后，仅使用 book.nfo / metadata.json / 封面等刮削文件，不再从音频文件读取元数据；本地库开启后，.strm 文件将不再探测远程音频时长。" />
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
              <span>{activeTab === 'priority' ? '元数据来源优先级排序 (拖动调整)' : '已启用 (按优先级排序)'}</span>
              <HelpHint text="系统将按照列表顺序依次尝试获取信息。如果是“默认”配置，将应用于所有未单独配置的字段。" />
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
                <span>暂无启用的源</span>
                <span>请从右侧添加</span>
              </div>
            )}
          </div>
        </div>

        {/* Available List */}
        {activeTab !== 'priority' && (
          <div className="space-y-2">
            <div className="text-xs font-bold text-slate-500 uppercase tracking-wider flex justify-between">
              <span>可用插件</span>
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
                  {sources.length === 0 ? '未检测到插件' : '已全部添加'}
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
