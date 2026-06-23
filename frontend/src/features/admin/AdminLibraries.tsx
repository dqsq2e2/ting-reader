import React, { useEffect, useState } from 'react';
import apiClient from '../../core/api/client';
import type { Library, ScraperSource } from '../../core/types';
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
  Wifi,
  ChevronDown,
  RotateCcw
} from 'lucide-react';
import HelpHint from '../../shared/ui/HelpHint';
import ScraperConfigurator from './ScraperConfigurator';

const DEFAULT_SCRAPER_CONFIG = JSON.stringify({
  extractAudioCover: true,
  preferAudioTitle: true,
  nfoWritingEnabled: false,
  metadataWritingEnabled: false,
  disableWatcher: false,
  cloudMode: false,
}, null, 2);

const AdminLibraries: React.FC = () => {
  const [libraries, setLibraries] = useState<Library[]>([]);
  const [loading, setLoading] = useState(true);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [scanning, setScanning] = useState<string | null>(null);
  const [syncMenuOpenId, setSyncMenuOpenId] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [deleteConfirmId, setDeleteConfirmId] = useState<string | null>(null);
  const [availableFolders, setAvailableFolders] = useState<{name: string, path: string}[]>([]);
  const [currentBrowsePath, setCurrentBrowsePath] = useState('');
  const [isFolderMenuOpen, setIsFolderMenuOpen] = useState(false);
  const [scraperSources, setScraperSources] = useState<Pick<ScraperSource, 'id' | 'name' | 'autoScrape'>[]>([]);
  const [showJsonEditor, setShowJsonEditor] = useState(false);
  const [testingConnection, setTestingConnection] = useState(false);

  // Form state
  const [formData, setFormData] = useState({
    name: '',
    type: 'webdav' as 'webdav' | 'local',
    url: '',
    username: '',
    password: '',
    rootPath: '/',
    scraperConfig: DEFAULT_SCRAPER_CONFIG
  });

  useEffect(() => {
    fetchLibraries();
    fetchScraperSources();
  }, []);

  const fetchScraperSources = async () => {
    try {
      const response = await apiClient.get('/api/scraper/sources');
      if (response.data && response.data.sources) {
        setScraperSources((response.data.sources as ScraperSource[]).filter(source => source.autoScrape));
      }
    } catch (err) {
      console.error('获取刮削源失败', err);
    }
  };

  useEffect(() => {
    if (isModalOpen && formData.type === 'local') {
      fetchFolders(currentBrowsePath);
    }
  }, [isModalOpen, formData.type, currentBrowsePath]);

  const fetchFolders = async (subPath: string) => {
    try {
      const response = await apiClient.get('/api/storage/folders', { params: { sub_path: subPath } });
      setAvailableFolders(response.data);
    } catch (err) {
      console.error('获取文件夹失败', err);
    }
  };

  const fetchLibraries = async () => {
    try {
      const response = await apiClient.get('/api/libraries');
      setLibraries(response.data);
    } catch (err) {
      console.error('获取库失败', err);
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

  const handleTestConnection = async () => {
    if (!formData.url) {
      alert('请输入 WebDAV 地址');
      return;
    }

    setTestingConnection(true);
    try {
      const payload = {
        url: formData.url,
        username: formData.username || null,
        password: formData.password || null,
        root_path: formData.rootPath || null
      };

      const response = await apiClient.post('/api/libraries/test-connection', payload);

      if (response.data.success) {
        alert('连接成功！');
      } else {
        alert(`${response.data.message}`);
      }
    } catch (err) {
      console.error(err);
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const msg = (err as any).response?.data?.message || (err as any).message || '未知错误';
      alert(`请求失败: ${msg}`);
    } finally {
      setTestingConnection(false);
    }
  };

  const handleSaveLibrary = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      const payload: Record<string, unknown> = {
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
      setFormData({ name: '', type: 'webdav', url: '', username: '', password: '', rootPath: '/', scraperConfig: DEFAULT_SCRAPER_CONFIG });
      await fetchLibraries();

      // Note: Scanning is now automatically triggered by the backend upon creation.
      // We only manually trigger it here if it's an edit operation or if we want to force it,
      // but for creation, the backend handles it to avoid duplicate tasks.
    } catch (err) {
      console.error(err);
      alert(editingId ? '修改失败，请检查配置' : '添加失败，请检查配置');
    }
  };

  const handleScan = async (id: string, mode: 'incremental' | 'full' = 'incremental', silent: boolean = false) => {
    setScanning(id);
    setSyncMenuOpenId(null);
    try {
      await apiClient.post(`/api/libraries/${id}/scan`, { mode });
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
              setFormData({ name: '', type: 'webdav', url: '', username: '', password: '', rootPath: '/', scraperConfig: DEFAULT_SCRAPER_CONFIG });
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

            <div className="relative flex items-center gap-3">
              <button
                type="button"
                onClick={() => setSyncMenuOpenId(syncMenuOpenId === lib.id ? null : lib.id)}
                disabled={scanning === lib.id}
                className="flex-1 md:flex-none flex items-center justify-center gap-2 px-4 py-2.5 bg-slate-100 dark:bg-slate-800 hover:bg-primary-50 dark:hover:bg-primary-900/20 text-slate-600 dark:text-slate-400 hover:text-primary-600 rounded-xl font-bold transition-all disabled:opacity-50"
              >
                {scanning === lib.id ? (
                  <Loader2 size={18} className="animate-spin" />
                ) : (
                  <RefreshCw size={18} />
                )}
                同步
                <ChevronDown size={16} className={`transition-transform ${syncMenuOpenId === lib.id ? 'rotate-180' : ''}`} />
              </button>
              {syncMenuOpenId === lib.id && (
                <div className="absolute right-0 top-full mt-2 w-44 rounded-2xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-900 shadow-xl z-20 overflow-hidden">
                  <button
                    type="button"
                    onClick={() => handleScan(lib.id, 'incremental')}
                    className="w-full px-4 py-3 text-left text-sm font-medium text-slate-700 dark:text-slate-300 hover:bg-slate-50 dark:hover:bg-slate-800 flex items-center gap-2"
                  >
                    <RefreshCw size={15} />
                    增量同步
                  </button>
                  <button
                    type="button"
                    onClick={() => handleScan(lib.id, 'full')}
                    className="w-full px-4 py-3 text-left text-sm font-medium text-slate-700 dark:text-slate-300 hover:bg-slate-50 dark:hover:bg-slate-800 flex items-center gap-2"
                  >
                    <RotateCcw size={15} />
                    全量同步
                  </button>
                </div>
              )}
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
                    <div className="flex justify-end">
                      <button
                        type="button"
                        onClick={handleTestConnection}
                        disabled={testingConnection || !formData.url}
                        className="px-4 py-2 bg-blue-50 dark:bg-blue-900/20 text-blue-600 dark:text-blue-400 font-bold rounded-xl hover:bg-blue-100 dark:hover:bg-blue-900/30 transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2 text-sm"
                      >
                        {testingConnection ? <Loader2 size={16} className="animate-spin" /> : <Wifi size={16} />}
                        测试连接
                      </button>
                    </div>
                  </>
                ) : (
                  <div className="space-y-4">
                    <div className="space-y-2">
                      <label className="text-sm font-bold text-slate-600 dark:text-slate-400">选择本地路径 (相对 storage/ 目录)</label>
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
                    </div>
                  </div>
                )}

                <div className="space-y-4 pt-2 border-t border-slate-100 dark:border-slate-800">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-1.5">
                      <label className="text-sm font-bold text-slate-600 dark:text-slate-400">刮削源配置</label>
                      <HelpHint text="高级模式可直接编辑 metadataPriority, defaultSources, coverSources, introSources, authorSources, narratorSources, tagsSources 等配置。" />
                    </div>
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
