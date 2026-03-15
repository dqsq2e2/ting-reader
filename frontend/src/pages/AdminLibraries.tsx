import React, { useEffect, useState } from 'react';
import apiClient from '../api/client';
import type { Library } from '../types';
import { 
  Plus, 
  Database, 
  RefreshCw, 
  Trash2, 
  Globe, 
  Folder,
  Loader2,
  CheckCircle2,
  AlertCircle,
  Edit,
  ArrowUp,
  ArrowDown,
  X
} from 'lucide-react';

const ScraperConfigurator = ({ 
  configStr, 
  sources, 
  onChange
}: { 
  configStr: string, 
  sources: {id: string, name: string}[], 
  onChange: (newConfigStr: string) => void,
  libraryType: string
}) => {
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

  const nfoEnabled = config.nfoWritingEnabled ?? config.nfo_writing_enabled ?? false;
  const metadataWritingEnabled = config.metadataWritingEnabled ?? config.metadata_writing_enabled ?? false;
  const preferAudioTitle = config.preferAudioTitle ?? config.prefer_audio_title ?? false;

  const handleNfoChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      const newConfig = { ...config, nfoWritingEnabled: e.target.checked };
      delete newConfig.nfo_writing_enabled;
      onChange(JSON.stringify(newConfig, null, 2));
  };

  const handleMetadataWritingChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      const newConfig = { ...config, metadataWritingEnabled: e.target.checked };
      delete newConfig.metadata_writing_enabled;
      onChange(JSON.stringify(newConfig, null, 2));
  };

  const handlePreferAudioTitleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      const newConfig = { ...config, preferAudioTitle: e.target.checked };
      delete newConfig.prefer_audio_title;
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
          <div className="flex flex-col">
            <label htmlFor="nfo-writing" className="text-sm font-bold text-slate-700 dark:text-slate-300 cursor-pointer">
              启用 NFO 元数据写入
            </label>
            <span className="text-[10px] text-slate-400">
              开启后，刮削或修改元数据时将同步写入 book.nfo 文件
            </span>
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
          <div className="flex flex-col">
            <label htmlFor="metadata-writing" className="text-sm font-bold text-slate-700 dark:text-slate-300 cursor-pointer">
              写入 metadata.json
            </label>
            <span className="text-[10px] text-slate-400">
              开启后，生成 Audiobookshelf 兼容的 metadata.json 元数据文件
            </span>
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
          <div className="flex flex-col">
            <label htmlFor="prefer-audio-title" className="text-sm font-bold text-slate-700 dark:text-slate-300 cursor-pointer">
              优先使用文件/文件夹名作为标题
            </label>
            <span className="text-[10px] text-slate-400">
              开启后，忽略优先级配置，强制使用文件夹名作为书名、文件名作为章节名
            </span>
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
            <span>{activeTab === 'priority' ? '元数据来源优先级排序 (拖动调整)' : '已启用 (按优先级排序)'}</span>
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
      
      <p className="text-[10px] text-slate-400 mt-3">
        提示：系统将按照列表顺序依次尝试获取信息。如果是“默认”配置，将应用于所有未单独配置的字段。
      </p>
    </div>
  );
};

const AdminLibraries: React.FC = () => {
  const [libraries, setLibraries] = useState<Library[]>([]);
  const [loading, setLoading] = useState(true);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [scanning, setScanning] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [deleteConfirmId, setDeleteConfirmId] = useState<string | null>(null);
  const [availableFolders, setAvailableFolders] = useState<{name: string, path: string}[]>([]);
  const [currentBrowsePath, setCurrentBrowsePath] = useState('');
  const [isFolderMenuOpen, setIsFolderMenuOpen] = useState(false);
  const [scraperSources, setScraperSources] = useState<{id: string, name: string}[]>([]);
  const [showJsonEditor, setShowJsonEditor] = useState(false);
  
  // Form state
  const [formData, setFormData] = useState({
    name: '',
    type: 'webdav' as 'webdav' | 'local',
    url: '',
    username: '',
    password: '',
    rootPath: '/',
    scraperConfig: ''
  });

  useEffect(() => {
    fetchLibraries();
    fetchScraperSources();
  }, []);

  const fetchScraperSources = async () => {
    try {
      const response = await apiClient.get('/api/scraper/sources');
      if (response.data && response.data.sources) {
        setScraperSources(response.data.sources);
      }
    } catch (err) {
      console.error('Failed to fetch scraper sources', err);
    }
  };

  useEffect(() => {
    if (isModalOpen && formData.type === 'local') {
      fetchFolders(currentBrowsePath);
    }
  }, [isModalOpen, formData.type, currentBrowsePath]);

  const fetchFolders = async (subPath: string) => {
    try {
      const response = await apiClient.get(`/api/storage/folders?subPath=${encodeURIComponent(subPath)}`);
      setAvailableFolders(response.data);
    } catch (err) {
      console.error('Failed to fetch folders', err);
    }
  };

  const fetchLibraries = async () => {
    try {
      const response = await apiClient.get('/api/libraries');
      setLibraries(response.data);
    } catch (err) {
      console.error('Failed to fetch libraries', err);
    } finally {
      setLoading(false);
    }
  };

  const openEditModal = (lib: Library) => {
    setEditingId(lib.id);
    
    // Determine the type safely
    const libType = lib.libraryType === 'local' ? 'local' : 'webdav';
    
    // Handle scraper config - check if it's already a string or an object
    let scraperConfigStr = '';
    const configData = lib.scraperConfig;
    if (configData) {
      if (typeof configData === 'string') {
        scraperConfigStr = configData;
      } else {
        scraperConfigStr = JSON.stringify(configData, null, 2);
      }
    }

    setFormData({
      name: lib.name,
      type: libType,
      url: lib.url,
      username: lib.username || '',
      password: '', // Don't populate password for security, let user enter new one if needed
      rootPath: lib.rootPath || '/',
      scraperConfig: scraperConfigStr
    });
    setIsModalOpen(true);
  };

  const handleSaveLibrary = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const payload: Record<string, any> = {
        name: formData.name,
        libraryType: formData.type,
      };

      if (formData.type === 'local') {
        payload.path = formData.url;
      } else {
        payload.webdavUrl = formData.url;
        payload.webdavUsername = formData.username;
        if (formData.password || !editingId) {
             payload.webdavPassword = formData.password;
        }
        payload.rootPath = formData.rootPath;
      }

      if (formData.scraperConfig) {
        try {
          payload.scraperConfig = JSON.parse(formData.scraperConfig);
        } catch {
          alert('刮削源配置 JSON 格式错误');
          return;
        }
      }

      // let savedLibId = editingId;
      if (editingId) {
        await apiClient.patch(`/api/libraries/${editingId}`, payload);
      } else {
        await apiClient.post('/api/libraries', payload);
        /*
        const res = await apiClient.post('/api/libraries', payload);
        if (res.data && res.data.id) {
            savedLibId = res.data.id;
        }
        */
      }
      setIsModalOpen(false);
      setEditingId(null);
      setFormData({ name: '', type: 'webdav', url: '', username: '', password: '', rootPath: '/', scraperConfig: '' });
      await fetchLibraries();
      
      // Note: Scanning is now automatically triggered by the backend upon creation.
      // We only manually trigger it here if it's an edit operation or if we want to force it,
      // but for creation, the backend handles it to avoid duplicate tasks.
    } catch (err) {
      console.error(err);
      alert(editingId ? '修改失败，请检查配置' : '添加失败，请检查配置');
    }
  };

  const handleScan = async (id: string, silent: boolean = false) => {
    setScanning(id);
    try {
      await apiClient.post(`/api/libraries/${id}/scan`);
      if (!silent) {
        alert('扫描任务已启动');
      }
    } catch {
      if (!silent) {
        alert('扫描启动失败');
      }
    } finally {
      setScanning(null);
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await apiClient.delete(`/api/libraries/${id}`);
      setDeleteConfirmId(null);
      fetchLibraries();
    } catch {
      alert('删除失败');
    }
  };

  return (
    <div className="w-full max-w-screen-2xl mx-auto p-4 sm:p-6 md:p-8 lg:p-10 space-y-8">
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-6">
        <div className="text-center md:text-left">
          <h1 className="text-2xl md:text-3xl font-bold dark:text-white flex items-center justify-center md:justify-start gap-3">
            <Database size={28} className="text-primary-600 md:w-8 md:h-8" />
            存储库管理
          </h1>
          <p className="text-sm md:text-base text-slate-500 mt-1">配置您的 WebDAV 或本地存储源并同步资源</p>
        </div>
        <div className="flex items-center gap-3 w-full md:w-auto">
          <button 
            onClick={() => {
              setEditingId(null);
              setFormData({ name: '', type: 'webdav', url: '', username: '', password: '', rootPath: '/', scraperConfig: '' });
              setIsModalOpen(true);
            }}
            className="flex-1 md:flex-none flex items-center justify-center gap-2 px-4 md:px-6 py-3 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-xl shadow-lg shadow-primary-500/30 transition-all text-sm md:text-base"
          >
            <Plus size={18} className="md:w-5 md:h-5" />
            添加库
          </button>
        </div>
      </div>

      <div className="grid gap-6">
        {libraries.map((lib) => (
          <div key={lib.id} className="bg-white dark:bg-slate-900 rounded-2xl p-6 border border-slate-100 dark:border-slate-800 shadow-sm flex flex-col md:flex-row md:items-center justify-between gap-6">
            <div className="flex items-center gap-4 min-w-0 w-full md:w-auto">
              <div className="w-14 h-14 rounded-xl bg-primary-50 dark:bg-primary-900/20 text-primary-600 flex items-center justify-center shrink-0">
                <Database size={28} />
              </div>
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2 flex-wrap">
                  <h3 className="text-xl font-bold dark:text-white truncate">{lib.name}</h3>
                  <span className={`text-[10px] font-bold px-2 py-0.5 rounded-full uppercase tracking-wider shrink-0 ${
                    lib.libraryType === 'local' 
                      ? 'bg-amber-100 text-amber-600 dark:bg-amber-900/20 dark:text-amber-400' 
                      : 'bg-blue-100 text-blue-600 dark:bg-blue-900/20 dark:text-blue-400'
                  }`}>
                    {lib.libraryType === 'local' ? '本地存储' : 'WebDAV'}
                  </span>
                </div>
                <div className="flex flex-col sm:flex-row sm:items-center gap-1 sm:gap-4 mt-1">
                  {lib.libraryType !== 'local' && (
                    <div className="flex items-center gap-1.5 text-sm text-slate-500 min-w-0">
                      <Globe size={14} className="shrink-0" />
                      <span className="truncate max-w-[180px] sm:max-w-[240px] md:max-w-[300px]" title={lib.url}>{lib.url}</span>
                    </div>
                  )}
                  <div className="flex items-center gap-1.5 text-sm text-slate-500 min-w-0">
                    <Folder size={14} className="shrink-0" />
                    <span className="truncate max-w-[180px] sm:max-w-[240px] md:max-w-[300px]" title={lib.libraryType === 'local' ? lib.url : lib.rootPath}>
                      {lib.libraryType === 'local' ? lib.url : lib.rootPath}
                    </span>
                  </div>
                </div>
              </div>
            </div>

            <div className="flex items-center gap-3">
              <button 
                onClick={() => handleScan(lib.id)}
                disabled={scanning === lib.id}
                className="flex-1 md:flex-none flex items-center justify-center gap-2 px-4 py-2.5 bg-slate-100 dark:bg-slate-800 hover:bg-primary-50 dark:hover:bg-primary-900/20 text-slate-600 dark:text-slate-400 hover:text-primary-600 rounded-xl font-bold transition-all disabled:opacity-50"
              >
                {scanning === lib.id ? (
                  <Loader2 size={18} className="animate-spin" />
                ) : (
                  <RefreshCw size={18} />
                )}
                同步
              </button>
              <button 
                onClick={() => openEditModal(lib)}
                className="p-2.5 text-slate-400 hover:text-primary-600 hover:bg-primary-50 dark:hover:bg-primary-900/20 rounded-xl transition-all"
              >
                <Edit size={20} />
              </button>
              <button 
                onClick={() => setDeleteConfirmId(lib.id)}
                className="p-2.5 text-slate-400 hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-xl transition-all"
              >
                <Trash2 size={20} />
              </button>
            </div>
          </div>
        ))}

        {libraries.length === 0 && !loading && (
          <div className="py-20 text-center bg-slate-50 dark:bg-slate-900/50 rounded-3xl border-2 border-dashed border-slate-200 dark:border-slate-800">
            <Database size={48} className="mx-auto text-slate-300 mb-4" />
            <p className="text-slate-500">暂无存储库，点击右上角添加</p>
          </div>
        )}
      </div>

      {/* Delete Confirmation Modal */}
      {deleteConfirmId && (
        <div className="fixed inset-0 z-[250] flex items-center justify-center p-4">
          <div className="absolute inset-0 bg-slate-900/60 backdrop-blur-sm" onClick={() => setDeleteConfirmId(null)}></div>
          <div className="relative w-full max-w-sm bg-white dark:bg-slate-900 rounded-3xl shadow-2xl p-8 animate-in zoom-in-95 duration-200 text-center">
            <div className="w-16 h-16 bg-red-50 dark:bg-red-900/20 text-red-500 rounded-full flex items-center justify-center mx-auto mb-4">
              <AlertCircle size={32} />
            </div>
            <h3 className="text-xl font-bold dark:text-white mb-2">确认删除？</h3>
            <p className="text-slate-500 text-sm mb-8">此操作将永久删除该存储库及其所有关联的书籍、章节和播放进度，且不可恢复。</p>
            <div className="flex gap-3">
              <button 
                onClick={() => setDeleteConfirmId(null)}
                className="flex-1 py-3 font-bold text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-xl transition-all"
              >
                取消
              </button>
              <button 
                onClick={() => handleDelete(deleteConfirmId)}
                className="flex-1 py-3 bg-red-500 hover:bg-red-600 text-white font-bold rounded-xl shadow-lg shadow-red-500/30 transition-all"
              >
                确认删除
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Add/Edit Library Modal */}
      {isModalOpen && (
        <div className="fixed inset-0 z-[200] flex items-center justify-center p-4">
          <div className="absolute inset-0 bg-slate-900/60 backdrop-blur-sm" onClick={() => setIsModalOpen(false)}></div>
          <div className="relative w-full max-w-lg bg-white dark:bg-slate-900 rounded-3xl shadow-2xl overflow-hidden animate-in zoom-in-95 duration-200 max-h-[90vh] flex flex-col">
            <div className="p-8 overflow-y-auto">
              <h2 className="text-2xl font-bold dark:text-white mb-6">{editingId ? '编辑存储库' : '添加存储库'}</h2>
              <form onSubmit={handleSaveLibrary} className="space-y-4">
                <div className="space-y-2">
                  <label className="text-sm font-bold text-slate-600 dark:text-slate-400">库类型</label>
                  <div className="grid grid-cols-2 gap-3">
                    <button
                      type="button"
                      disabled={!!editingId}
                      onClick={() => setFormData({...formData, type: 'webdav', url: '', rootPath: '/'})}
                      className={`py-2.5 rounded-xl font-bold transition-all border ${
                        formData.type === 'webdav' 
                          ? 'bg-primary-50 border-primary-200 text-primary-600' 
                          : 'bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700 text-slate-400'
                      } ${editingId ? 'opacity-50 cursor-not-allowed' : ''}`}
                    >
                      WebDAV
                    </button>
                    <button
                      type="button"
                      disabled={!!editingId}
                      onClick={() => setFormData({...formData, type: 'local', url: '', rootPath: '/'})}
                      className={`py-2.5 rounded-xl font-bold transition-all border ${
                        formData.type === 'local' 
                          ? 'bg-primary-50 border-primary-200 text-primary-600' 
                          : 'bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700 text-slate-400'
                      } ${editingId ? 'opacity-50 cursor-not-allowed' : ''}`}
                    >
                      本地存储
                    </button>
                  </div>
                </div>

                <div className="space-y-2">
                  <label className="text-sm font-bold text-slate-600 dark:text-slate-400">库名称</label>
                  <input 
                    type="text" 
                    required
                    value={formData.name}
                    onChange={e => setFormData({...formData, name: e.target.value})}
                    placeholder="例如：我的 NAS"
                    className="w-full px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                  />
                </div>

                {formData.type === 'webdav' ? (
                  <>
                    <div className="space-y-2">
                      <label className="text-sm font-bold text-slate-600 dark:text-slate-400">WebDAV 地址</label>
                      <input 
                        type="url" 
                        required
                        value={formData.url}
                        onChange={e => setFormData({...formData, url: e.target.value})}
                        placeholder="https://nas.local:5006"
                        className="w-full px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                      />
                    </div>
                    <div className="grid grid-cols-2 gap-4">
                      <div className="space-y-2">
                        <label className="text-sm font-bold text-slate-600 dark:text-slate-400">用户名</label>
                        <input 
                          type="text" 
                          required
                          value={formData.username}
                          onChange={e => setFormData({...formData, username: e.target.value})}
                          className="w-full px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                        />
                      </div>
                      <div className="space-y-2">
                        <label className="text-sm font-bold text-slate-600 dark:text-slate-400">密码</label>
                        <input 
                          type="password" 
                          required={!editingId}
                          value={formData.password}
                          onChange={e => setFormData({...formData, password: e.target.value})}
                          placeholder={editingId ? "不修改请留空" : ""}
                          className="w-full px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                        />
                      </div>
                    </div>
                    <div className="space-y-2">
                      <label className="text-sm font-bold text-slate-600 dark:text-slate-400">根目录</label>
                      <input 
                        type="text" 
                        value={formData.rootPath}
                        onChange={e => setFormData({...formData, rootPath: e.target.value})}
                        placeholder="/"
                        className="w-full px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                      />
                    </div>
                  </>
                ) : (
                  <div className="space-y-4">
                    <div className="space-y-2">
                      <label className="text-sm font-bold text-slate-600 dark:text-slate-400">选择本地路径 (相对项目 storage/ 目录)</label>
                      <div className="relative">
                        {/* Selector Trigger */}
                        <button
                          type="button"
                          onClick={() => setIsFolderMenuOpen(!isFolderMenuOpen)}
                          className="w-full px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl flex items-center justify-between group hover:border-primary-400 transition-all"
                        >
                          <div className="flex items-center gap-3 overflow-hidden">
                            <Folder size={18} className="text-primary-500 shrink-0" />
                            <div className="flex flex-col items-start overflow-hidden">
                              <span className="text-[10px] text-slate-400 font-bold uppercase tracking-wider">当前已选</span>
                              <span className="text-sm dark:text-white truncate font-medium">
                                {formData.url || '(根目录 storage/)'}
                              </span>
                            </div>
                          </div>
                          <div className="flex items-center gap-2">
                            <div className="w-px h-6 bg-slate-200 dark:bg-slate-700 mx-1" />
                            <Plus size={18} className={`text-slate-400 transition-transform duration-300 ${isFolderMenuOpen ? 'rotate-45 text-primary-500' : ''}`} />
                          </div>
                        </button>

                        {/* Dropdown Menu */}
                        {isFolderMenuOpen && (
                          <div className="absolute top-full left-0 right-0 mt-2 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-2xl shadow-2xl z-[100] overflow-hidden animate-in fade-in zoom-in-95 duration-200">
                            {/* Breadcrumbs */}
                            <div className="px-4 py-3 bg-slate-50/50 dark:bg-slate-800/50 border-b border-slate-100 dark:border-slate-800 flex items-center gap-2 overflow-x-auto no-scrollbar">
                              <button 
                                type="button"
                                onClick={() => setCurrentBrowsePath('')}
                                className={`p-1.5 rounded-lg transition-colors ${currentBrowsePath === '' ? 'bg-primary-100 text-primary-600' : 'text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800'}`}
                              >
                                <Globe size={16} />
                              </button>
                              {currentBrowsePath.split('/').filter(Boolean).map((part, i, arr) => (
                                <React.Fragment key={i}>
                                  <span className="text-slate-300 dark:text-slate-600">/</span>
                                  <button
                                    type="button"
                                    onClick={() => setCurrentBrowsePath(arr.slice(0, i + 1).join('/'))}
                                    className="px-2 py-1 text-xs font-bold text-slate-500 hover:text-primary-500 hover:bg-primary-50 dark:hover:bg-primary-900/20 rounded-md whitespace-nowrap transition-all"
                                  >
                                    {part}
                                  </button>
                                </React.Fragment>
                              ))}
                            </div>

                            {/* Action Bar */}
                            <div className="p-2 border-b border-slate-100 dark:border-slate-800 flex gap-2">
                              <button
                                type="button"
                                onClick={() => {
                                  setFormData({...formData, url: currentBrowsePath, rootPath: '/'});
                                  setIsFolderMenuOpen(false);
                                }}
                                className="flex-1 py-2 bg-primary-600 text-white text-xs font-bold rounded-xl hover:bg-primary-700 shadow-lg shadow-primary-500/20 transition-all flex items-center justify-center gap-2"
                              >
                                <CheckCircle2 size={14} />
                                选择此目录: {currentBrowsePath || '根目录'}
                              </button>
                            </div>

                            {/* Folder List */}
                            <div className="max-h-60 overflow-y-auto py-1">
                              {currentBrowsePath && (
                                <button
                                  type="button"
                                  onClick={() => setCurrentBrowsePath(currentBrowsePath.split('/').slice(0, -1).join('/'))}
                                  className="w-full px-4 py-2.5 flex items-center gap-3 hover:bg-slate-50 dark:hover:bg-slate-800 text-slate-400 transition-colors"
                                >
                                  <RefreshCw size={14} />
                                  <span className="text-xs font-medium">返回上一级...</span>
                                </button>
                              )}
                              {availableFolders.length > 0 ? (
                                availableFolders.map((folder) => (
                                  <button
                                    key={folder.path}
                                    type="button"
                                    onClick={() => setCurrentBrowsePath(folder.path)}
                                    className="w-full px-4 py-3 flex items-center gap-3 hover:bg-primary-50 dark:hover:bg-primary-900/10 text-left group transition-all"
                                  >
                                    <Folder size={16} className="text-primary-400 group-hover:scale-110 transition-transform" />
                                    <span className="flex-1 text-sm dark:text-slate-300 group-hover:text-primary-600 font-medium truncate">
                                      {folder.name}
                                    </span>
                                    <div className="opacity-0 group-hover:opacity-100 transition-opacity">
                                      <Plus size={14} className="text-primary-300" />
                                    </div>
                                  </button>
                                ))
                              ) : (
                                <div className="px-4 py-10 text-center">
                                  <Folder size={32} className="mx-auto text-slate-200 dark:text-slate-800 mb-2" />
                                  <p className="text-slate-400 text-xs italic">当前目录下没有子文件夹</p>
                                </div>
                              )}
                            </div>
                          </div>
                        )}
                      </div>
                      <p className="text-[11px] text-slate-400 pl-1">
                        提示：音频文件必须放置在后端 <strong>backend/storage/</strong> 目录下
                      </p>
                    </div>
                  </div>
                )}

                <div className="space-y-4 pt-2 border-t border-slate-100 dark:border-slate-800">
                  <div className="flex items-center justify-between">
                    <label className="text-sm font-bold text-slate-600 dark:text-slate-400">刮削源配置</label>
                    <button
                      type="button"
                      onClick={() => setShowJsonEditor(!showJsonEditor)}
                      className="text-xs text-primary-600 font-bold hover:underline"
                    >
                      {showJsonEditor ? '切换至简易模式' : '切换至高级模式 (JSON)'}
                    </button>
                  </div>

                  {!showJsonEditor ? (
                    <ScraperConfigurator 
                      configStr={formData.scraperConfig}
                      sources={scraperSources}
                      onChange={(newConfigStr) => setFormData({...formData, scraperConfig: newConfigStr})}
                      libraryType={formData.type}
                    />
                  ) : (
                    <div className="space-y-2">
                      <textarea 
                        value={formData.scraperConfig}
                        onChange={e => setFormData({...formData, scraperConfig: e.target.value})}
                        placeholder='{"defaultSources": ["xiimalaya-scraper-js"]}'
                        className="w-full px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white font-mono text-sm h-32"
                      />
                      <p className="text-xs text-slate-400">
                        可选配置：defaultSources, titleSources, coverSources, introSources 等。
                      </p>
                    </div>
                  )}
                </div>

                <div className="flex gap-4 pt-6">
                  <button 
                    type="button"
                    onClick={() => setIsModalOpen(false)}
                    className="flex-1 py-3 font-bold text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-xl transition-all"
                  >
                    取消
                  </button>
                  <button 
                    type="submit"
                    className="flex-1 py-3 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-xl shadow-lg shadow-primary-500/30 transition-all"
                  >
                    保存配置
                  </button>
                </div>
              </form>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default AdminLibraries;
