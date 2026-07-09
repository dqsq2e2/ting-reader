import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import apiClient from '../../core/api/client';
import type { Library, ScraperSource } from '../../core/types';
import {
  Plus,
  Database,
  RefreshCw,
  Trash2,
  Folder,
  Loader2,
  CheckCircle2,
  AlertCircle,
  Edit,
  Wifi,
  ChevronDown,
  RotateCcw,
  Rss,
  Globe
} from 'lucide-react';
import HelpHint from '../../shared/ui/HelpHint';
import ScraperConfigurator from './ScraperConfigurator';

const DEFAULT_SCRAPER_CONFIG = JSON.stringify({
  extract_audio_cover: true,
  use_filename_as_title: true,
  nfo_writing_enabled: false,
  metadata_writing_enabled: false,
  disable_watcher: false,
  cloud_mode: false,
}, null, 2);

interface StorageRoot {
  path: string;
  source: string;
  readable: boolean;
  writable: boolean;
}

const normalizeComparePath = (value: string) => (
  value.replace(/\\/g, '/').replace(/\/+$/g, '').toLowerCase()
);

const joinRootAndSubPath = (root: string, subPath: string) => {
  const cleanRoot = root.replace(/[\\/]+$/g, '');
  const cleanSubPath = subPath.replace(/^[\\/]+|[\\/]+$/g, '');
  if (!cleanRoot) return cleanSubPath;
  return cleanSubPath ? `${cleanRoot}/${cleanSubPath}` : cleanRoot;
};

const relativePathFromRoot = (path: string, root: string) => {
  const normalizedPath = normalizeComparePath(path);
  const normalizedRoot = normalizeComparePath(root);
  if (!normalizedPath || !normalizedRoot) return '';
  if (normalizedPath === normalizedRoot) return '';
  if (!normalizedPath.startsWith(`${normalizedRoot}/`)) return '';
  return path.replace(/\\/g, '/').slice(root.replace(/\\/g, '/').replace(/\/+$/g, '').length + 1);
};

const findRootForPath = (path: string, roots: StorageRoot[]) => {
  const normalizedPath = normalizeComparePath(path);
  if (!normalizedPath) return null;
  return roots
    .filter((root) => {
      const normalizedRoot = normalizeComparePath(root.path);
      return normalizedPath === normalizedRoot || normalizedPath.startsWith(`${normalizedRoot}/`);
    })
    .sort((a, b) => b.path.length - a.path.length)[0] || null;
};

const libraryTypeLabel = (type: Library['library_type'], t: (key: string) => string) => {
  if (type === 'local') return t('adminLibraries.localStorage');
  if (type === 'rss') return t('adminLibraries.rssSubscription');
  return 'WebDAV';
};

const libraryTypeBadgeClass = (type: Library['library_type']) => {
  if (type === 'local') {
    return 'bg-amber-100 text-amber-600 dark:bg-amber-900/20 dark:text-amber-400';
  }
  if (type === 'rss') {
    return 'bg-emerald-100 text-emerald-600 dark:bg-emerald-900/20 dark:text-emerald-400';
  }
  return 'bg-blue-100 text-blue-600 dark:bg-blue-900/20 dark:text-blue-400';
};

const AdminLibraries: React.FC = () => {
  const { t } = useTranslation();
  const [libraries, setLibraries] = useState<Library[]>([]);
  const [loading, setLoading] = useState(true);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [scanning, setScanning] = useState<string | null>(null);
  const [syncMenuOpenId, setSyncMenuOpenId] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [deleteConfirmId, setDeleteConfirmId] = useState<string | null>(null);
  const [availableFolders, setAvailableFolders] = useState<{name: string, path: string}[]>([]);
  const [storageRoots, setStorageRoots] = useState<StorageRoot[]>([]);
  const [selectedStorageRoot, setSelectedStorageRoot] = useState('');
  const [currentBrowsePath, setCurrentBrowsePath] = useState('');
  const [isFolderMenuOpen, setIsFolderMenuOpen] = useState(false);
  const [scraperSources, setScraperSources] = useState<Pick<ScraperSource, 'id' | 'name' | 'auto_scrape'>[]>([]);
  const [showJsonEditor, setShowJsonEditor] = useState(false);
  const [testingConnection, setTestingConnection] = useState(false);

  // Form state
  const [formData, setFormData] = useState({
    name: '',
    type: 'webdav' as 'webdav' | 'local' | 'rss',
    url: '',
    username: '',
    password: '',
    root_path: '/',
    scraper_config: DEFAULT_SCRAPER_CONFIG
  });

  useEffect(() => {
    fetchLibraries();
    fetchScraperSources();
  }, []);

  const fetchScraperSources = async () => {
    try {
      const response = await apiClient.get('/api/scraper/sources');
      if (response.data && response.data.sources) {
        setScraperSources((response.data.sources as ScraperSource[]).filter(source => source.auto_scrape));
      }
    } catch (err) {
      console.error('Failed to fetch scraper sources', err);
    }
  };

  useEffect(() => {
    if (isModalOpen && formData.type === 'local') {
      fetchStorageRoots();
    }
  }, [isModalOpen, formData.type]);

  useEffect(() => {
    if (isModalOpen && formData.type === 'local') {
      fetchFolders(currentBrowsePath);
    }
  }, [isModalOpen, formData.type, currentBrowsePath, selectedStorageRoot]);

  const fetchFolders = async (subPath: string) => {
    try {
      const params: Record<string, string> = { sub_path: subPath };
      if (selectedStorageRoot) {
        params.root = selectedStorageRoot;
      }
      const response = await apiClient.get('/api/storage/folders', { params });
      setAvailableFolders(response.data);
    } catch (err) {
      console.error('Failed to fetch folders', err);
      setAvailableFolders([]);
    }
  };

  const fetchStorageRoots = async () => {
    try {
      const response = await apiClient.get('/api/storage/roots');
      const roots = response.data as StorageRoot[];
      setStorageRoots(roots);

      const matchedRoot = findRootForPath(formData.url, roots);
      const nextRoot = matchedRoot?.path || selectedStorageRoot || roots[0]?.path || '';
      setSelectedStorageRoot(nextRoot);
      setCurrentBrowsePath(nextRoot ? relativePathFromRoot(formData.url, nextRoot) : '');
    } catch (err) {
      console.error('Failed to fetch storage roots', err);
      setStorageRoots([]);
      setSelectedStorageRoot('');
      setCurrentBrowsePath('');
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
    setCurrentBrowsePath('');
    setSelectedStorageRoot('');

    // Determine the type safely
    const libType = lib.library_type === 'local' || lib.library_type === 'rss' ? lib.library_type : 'webdav';

    // Handle scraper config - check if it's already a string or an object
    let scraperConfigStr = '';
    const configData = lib.scraper_config;
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
      root_path: lib.root_path || '/',
      scraper_config: scraperConfigStr
    });
    setIsModalOpen(true);
  };

  const handleTestConnection = async () => {
    if (!formData.url) {
      alert(t('adminLibraries.requireWebdavUrl'));
      return;
    }

    setTestingConnection(true);
    try {
      const payload = {
        url: formData.url,
        username: formData.username || null,
        password: formData.password || null,
        root_path: formData.root_path || null
      };

      const response = await apiClient.post('/api/libraries/test-connection', payload);

      if (response.data.success) {
        alert(t('adminLibraries.connectionSuccess'));
      } else {
        alert(`${response.data.message}`);
      }
    } catch (err) {
      console.error(err);
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const msg = (err as any).response?.data?.message || (err as any).message || t('adminLibraries.unknownError');
      alert(t('adminLibraries.requestFailed', { message: msg }));
    } finally {
      setTestingConnection(false);
    }
  };

  const handleSaveLibrary = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      const payload: Record<string, unknown> = {
        name: formData.name,
        library_type: formData.type,
      };

      if (formData.type === 'local') {
        payload.path = formData.url;
      } else if (formData.type === 'rss') {
        payload.rss_feed_url = formData.url;
        payload.root_path = '/';
      } else {
        payload.webdav_url = formData.url;
        payload.webdav_username = formData.username;
        if (formData.password || !editingId) {
             payload.webdav_password = formData.password;
        }
        payload.root_path = formData.root_path;
      }

      if (formData.type !== 'rss' && formData.scraper_config) {
        try {
          payload.scraper_config = JSON.parse(formData.scraper_config);
        } catch {
          alert(t('adminLibraries.jsonInvalid'));
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
      setFormData({ name: '', type: 'webdav', url: '', username: '', password: '', root_path: '/', scraper_config: DEFAULT_SCRAPER_CONFIG });
      await fetchLibraries();

      // Note: Scanning is now automatically triggered by the backend upon creation.
      // We only manually trigger it here if it's an edit operation or if we want to force it,
      // but for creation, the backend handles it to avoid duplicate tasks.
    } catch (err) {
      console.error(err);
      alert(editingId ? t('adminLibraries.updateFailed') : t('adminLibraries.addFailed'));
    }
  };

  const handleScan = async (id: string, mode: 'incremental' | 'full' = 'incremental', silent: boolean = false) => {
    setScanning(id);
    setSyncMenuOpenId(null);
    try {
      await apiClient.post(`/api/libraries/${id}/scan`, { mode });
      if (!silent) {
        alert(t('adminLibraries.scanStarted'));
      }
    } catch {
      if (!silent) {
        alert(t('adminLibraries.scanStartFailed'));
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
      alert(t('adminLibraries.deleteFailed'));
    }
  };

  const selectedBrowsePath = selectedStorageRoot
    ? joinRootAndSubPath(selectedStorageRoot, currentBrowsePath)
    : currentBrowsePath;

  return (
    <div className="w-full max-w-screen-2xl mx-auto p-4 sm:p-6 md:p-8 lg:p-10 space-y-8">
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-6">
        <div className="text-center md:text-left">
          <h1 className="text-2xl md:text-3xl font-bold dark:text-white flex items-center justify-center md:justify-start gap-3">
            <Database size={28} className="text-primary-600 md:w-8 md:h-8" />
            {t('adminLibraries.title')}
          </h1>
          <p className="text-sm md:text-base text-slate-500 mt-1">{t('adminLibraries.subtitle')}</p>
        </div>
        <div className="flex items-center gap-3 w-full md:w-auto">
          <button
            onClick={() => {
              setEditingId(null);
              setCurrentBrowsePath('');
              setSelectedStorageRoot('');
              setFormData({ name: '', type: 'webdav', url: '', username: '', password: '', root_path: '/', scraper_config: DEFAULT_SCRAPER_CONFIG });
              setIsModalOpen(true);
            }}
            className="flex-1 md:flex-none flex items-center justify-center gap-2 px-4 md:px-6 py-3 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-xl shadow-lg shadow-primary-500/30 transition-all text-sm md:text-base"
          >
            <Plus size={18} className="md:w-5 md:h-5" />
            {t('adminLibraries.addLibrary')}
          </button>
        </div>
      </div>

      <div className="grid gap-6">
        {libraries.map((lib) => (
          <div key={lib.id} className="bg-white dark:bg-slate-900 rounded-2xl p-6 border border-slate-100 dark:border-slate-800 shadow-sm flex flex-col md:flex-row md:items-center justify-between gap-6">
            <div className="flex items-center gap-4 min-w-0 w-full md:w-auto">
              <div className="w-14 h-14 rounded-xl bg-primary-50 dark:bg-primary-900/20 text-primary-600 flex items-center justify-center shrink-0">
                {lib.library_type === 'rss' ? <Rss size={28} /> : <Database size={28} />}
              </div>
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2 flex-wrap">
                  <h3 className="text-xl font-bold dark:text-white truncate">{lib.name}</h3>
                  <span className={`text-[10px] font-bold px-2 py-0.5 rounded-full uppercase tracking-wider shrink-0 ${libraryTypeBadgeClass(lib.library_type)}`}>
                    {libraryTypeLabel(lib.library_type, t)}
                  </span>
                </div>
                <div className="flex flex-col sm:flex-row sm:items-center gap-1 sm:gap-4 mt-1">
                  {lib.library_type !== 'local' && (
                    <div className="flex items-center gap-1.5 text-sm text-slate-500 min-w-0">
                      <Globe size={14} className="shrink-0" />
                      <span className="truncate max-w-[180px] sm:max-w-[240px] md:max-w-[300px]" title={lib.url}>{lib.url}</span>
                    </div>
                  )}
                  {lib.library_type !== 'rss' && (
                    <div className="flex items-center gap-1.5 text-sm text-slate-500 min-w-0">
                      <Folder size={14} className="shrink-0" />
                      <span className="truncate max-w-[180px] sm:max-w-[240px] md:max-w-[300px]" title={lib.library_type === 'local' ? lib.url : lib.root_path}>
                        {lib.library_type === 'local' ? lib.url : lib.root_path}
                      </span>
                    </div>
                  )}
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
                {t('adminLibraries.sync')}
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
                    {t('adminLibraries.incrementalSync')}
                  </button>
                  <button
                    type="button"
                    onClick={() => handleScan(lib.id, 'full')}
                    className="w-full px-4 py-3 text-left text-sm font-medium text-slate-700 dark:text-slate-300 hover:bg-slate-50 dark:hover:bg-slate-800 flex items-center gap-2"
                  >
                    <RotateCcw size={15} />
                    {t('adminLibraries.fullSync')}
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
            <p className="text-slate-500">{t('adminLibraries.empty')}</p>
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
            <h3 className="text-xl font-bold dark:text-white mb-2">{t('adminLibraries.deleteTitle')}</h3>
            <p className="text-slate-500 text-sm mb-8">{t('adminLibraries.deleteMessage')}</p>
            <div className="flex gap-3">
              <button
                onClick={() => setDeleteConfirmId(null)}
                className="flex-1 py-3 font-bold text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-xl transition-all"
              >
                {t('common.cancel')}
              </button>
              <button
                onClick={() => handleDelete(deleteConfirmId)}
                className="flex-1 py-3 bg-red-500 hover:bg-red-600 text-white font-bold rounded-xl shadow-lg shadow-red-500/30 transition-all"
              >
                {t('adminLibraries.confirmDelete')}
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
              <h2 className="text-2xl font-bold dark:text-white mb-6">
                {editingId ? t('adminLibraries.editLibrary') : t('adminLibraries.addStorageLibrary')}
              </h2>
              <form onSubmit={handleSaveLibrary} className="space-y-4">
                <div className="space-y-2">
                  <label className="text-sm font-bold text-slate-600 dark:text-slate-400">{t('adminLibraries.libraryType')}</label>
                  <div className="grid grid-cols-3 gap-3">
                    <button
                      type="button"
                      disabled={!!editingId}
                      onClick={() => setFormData({...formData, type: 'rss', url: '', root_path: '/'})}
                      className={`py-2.5 rounded-xl font-bold transition-all border ${
                        formData.type === 'rss'
                          ? 'bg-primary-50 border-primary-200 text-primary-600'
                          : 'bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700 text-slate-400'
                      } ${editingId ? 'opacity-50 cursor-not-allowed' : ''}`}
                    >
                      {t('adminLibraries.rssSubscription')}
                    </button>
                    <button
                      type="button"
                      disabled={!!editingId}
                      onClick={() => setFormData({...formData, type: 'webdav', url: '', root_path: '/'})}
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
                      onClick={() => {
                        setCurrentBrowsePath('');
                        setSelectedStorageRoot('');
                        setFormData({...formData, type: 'local', url: '', root_path: '/'});
                      }}
                      className={`py-2.5 rounded-xl font-bold transition-all border ${
                        formData.type === 'local'
                          ? 'bg-primary-50 border-primary-200 text-primary-600'
                          : 'bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700 text-slate-400'
                      } ${editingId ? 'opacity-50 cursor-not-allowed' : ''}`}
                    >
                      {t('adminLibraries.localStorage')}
                    </button>
                  </div>
                </div>

                <div className="space-y-2">
                  <label className="text-sm font-bold text-slate-600 dark:text-slate-400">{t('adminLibraries.libraryName')}</label>
                  <input
                    type="text"
                    required
                    value={formData.name}
                    onChange={e => setFormData({...formData, name: e.target.value})}
                    placeholder={t('adminLibraries.libraryNamePlaceholder')}
                    className="w-full px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                  />
                </div>

                {formData.type === 'webdav' ? (
                  <>
                    <div className="space-y-2">
                      <label className="text-sm font-bold text-slate-600 dark:text-slate-400">{t('adminLibraries.webdavAddress')}</label>
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
                        <label className="text-sm font-bold text-slate-600 dark:text-slate-400">{t('adminLibraries.username')}</label>
                        <input
                          type="text"
                          required
                          value={formData.username}
                          onChange={e => setFormData({...formData, username: e.target.value})}
                          className="w-full px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                        />
                      </div>
                      <div className="space-y-2">
                        <label className="text-sm font-bold text-slate-600 dark:text-slate-400">{t('adminLibraries.password')}</label>
                        <input
                          type="password"
                          required={!editingId}
                          value={formData.password}
                          onChange={e => setFormData({...formData, password: e.target.value})}
                          placeholder={editingId ? t('adminLibraries.passwordPlaceholder') : ''}
                          className="w-full px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                        />
                      </div>
                    </div>
                    <div className="space-y-2">
                      <label className="text-sm font-bold text-slate-600 dark:text-slate-400">{t('adminLibraries.rootPath')}</label>
                      <input
                        type="text"
                        value={formData.root_path}
                        onChange={e => setFormData({...formData, root_path: e.target.value})}
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
                        {t('adminLibraries.testConnection')}
                      </button>
                    </div>
                  </>
                ) : formData.type === 'rss' ? (
                  <div className="space-y-2">
                    <label className="text-sm font-bold text-slate-600 dark:text-slate-400">{t('adminLibraries.rssFeedUrl')}</label>
                    <input
                      type="url"
                      required
                      value={formData.url}
                      onChange={e => setFormData({...formData, url: e.target.value})}
                      placeholder="https://example.com/podcast/feed.xml"
                      className="w-full px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                    />
                  </div>
                ) : (
                  <div className="space-y-4">
                    <div className="space-y-2">
                      <label className="text-sm font-bold text-slate-600 dark:text-slate-400">{t('adminLibraries.localPath')}</label>
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
                              <span className="text-[10px] text-slate-400 font-bold uppercase tracking-wider">{t('adminLibraries.currentSelected')}</span>
                              <span className="text-sm dark:text-white truncate font-medium" title={formData.url || selectedBrowsePath}>
                                {formData.url || selectedBrowsePath || t('adminLibraries.storageRoot')}
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
                            {storageRoots.length > 0 && (
                              <div className="p-3 border-b border-slate-100 dark:border-slate-800">
                                <label className="mb-1.5 block text-[10px] font-bold uppercase tracking-wider text-slate-400">
                                  {t('adminLibraries.authorizedRoot')}
                                </label>
                                <select
                                  value={selectedStorageRoot}
                                  onChange={(event) => {
                                    setSelectedStorageRoot(event.target.value);
                                    setCurrentBrowsePath('');
                                  }}
                                  className="w-full rounded-xl border border-slate-200 bg-slate-50 px-3 py-2 text-sm font-medium text-slate-700 outline-none focus:border-primary-400 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200"
                                >
                                  {storageRoots.map((root) => (
                                    <option key={root.path} value={root.path}>
                                      {root.path}
                                    </option>
                                  ))}
                                </select>
                              </div>
                            )}
                            {/* Breadcrumbs */}
                            <div className="px-4 py-3 bg-slate-50/50 dark:bg-slate-800/50 border-b border-slate-100 dark:border-slate-800 flex items-center gap-2 overflow-x-auto no-scrollbar">
                              <button
                                type="button"
                                onClick={() => setCurrentBrowsePath('')}
                                className={`px-2 py-1 text-xs font-bold rounded-lg transition-colors shrink-0 ${
                                  currentBrowsePath === ''
                                    ? 'bg-primary-50 text-primary-600 dark:bg-primary-950/30'
                                    : 'text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800'
                                }`}
                              >
                                {t('adminLibraries.rootDirectory')}
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
                                  setFormData({...formData, url: selectedBrowsePath, root_path: '/'});
                                  setIsFolderMenuOpen(false);
                                }}
                                className="flex-1 py-2 bg-primary-600 text-white text-xs font-bold rounded-xl hover:bg-primary-700 shadow-lg shadow-primary-500/20 transition-all flex items-center justify-center gap-2"
                              >
                                <CheckCircle2 size={14} />
                                {t('adminLibraries.chooseThisDirectory', { path: selectedBrowsePath || t('adminLibraries.rootDirectory') })}
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
                                  <span className="text-xs font-medium">{t('adminLibraries.goParent')}</span>
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
                                  <p className="text-slate-400 text-xs italic">{t('adminLibraries.noSubfolders')}</p>
                                </div>
                              )}
                            </div>
                          </div>
                        )}
                      </div>
                    </div>
                  </div>
                )}

                {formData.type !== 'rss' && (
                <div className="space-y-4 pt-2 border-t border-slate-100 dark:border-slate-800">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-1.5">
                      <label className="text-sm font-bold text-slate-600 dark:text-slate-400">{t('adminLibraries.scraperConfig')}</label>
                      <HelpHint text={t('adminLibraries.scraperHelp')} />
                    </div>
                    <button
                      type="button"
                      onClick={() => setShowJsonEditor(!showJsonEditor)}
                      className="text-xs text-primary-600 font-bold hover:underline"
                    >
                      {showJsonEditor ? t('adminLibraries.simpleMode') : t('adminLibraries.advancedMode')}
                    </button>
                  </div>

                  {!showJsonEditor ? (
                    <ScraperConfigurator
                      configStr={formData.scraper_config}
                      sources={scraperSources}
                      onChange={(newConfigStr) => setFormData({...formData, scraper_config: newConfigStr})}
                      libraryType={formData.type}
                    />
                  ) : (
                    <div className="space-y-2">
                      <textarea
                        value={formData.scraper_config}
                        onChange={e => setFormData({...formData, scraper_config: e.target.value})}
                        placeholder='{"default_sources": ["ximalaya-scraper-wasm"]}'
                        className="w-full px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white font-mono text-sm h-32"
                      />
                    </div>
                  )}
                </div>
                )}

                <div className="flex gap-4 pt-6">
                  <button
                    type="button"
                    onClick={() => setIsModalOpen(false)}
                    className="flex-1 py-3 font-bold text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-xl transition-all"
                  >
                    {t('common.cancel')}
                  </button>
                  <button
                    type="submit"
                    className="flex-1 py-3 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-xl shadow-lg shadow-primary-500/30 transition-all"
                  >
                    {t('adminLibraries.saveConfig')}
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
