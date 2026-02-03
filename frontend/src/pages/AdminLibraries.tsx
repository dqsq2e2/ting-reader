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
  Edit
} from 'lucide-react';

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
  
  // Form state
  const [formData, setFormData] = useState({
    name: '',
    type: 'webdav' as 'webdav' | 'local',
    url: '',
    username: '',
    password: '',
    root_path: '/'
  });

  useEffect(() => {
    fetchLibraries();
  }, []);

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
    setFormData({
      name: lib.name,
      type: lib.type || 'webdav',
      url: lib.url,
      username: lib.username || '',
      password: lib.password || '',
      root_path: lib.root_path
    });
    setIsModalOpen(true);
  };

  const handleSaveLibrary = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      if (editingId) {
        await apiClient.patch(`/api/libraries/${editingId}`, formData);
      } else {
        await apiClient.post('/api/libraries', formData);
      }
      setIsModalOpen(false);
      setEditingId(null);
      setFormData({ name: '', type: 'webdav', url: '', username: '', password: '', root_path: '/' });
      fetchLibraries();
    } catch (err) {
      alert(editingId ? '修改失败，请检查配置' : '添加失败，请检查配置');
    }
  };

  const handleScan = async (id: string) => {
    setScanning(id);
    try {
      await apiClient.post(`/api/libraries/${id}/scan`);
      alert('扫描任务已启动');
    } catch (err) {
      alert('扫描启动失败');
    } finally {
      setScanning(null);
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await apiClient.delete(`/api/libraries/${id}`);
      setDeleteConfirmId(null);
      fetchLibraries();
    } catch (err) {
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
              setFormData({ name: '', type: 'webdav', url: '', username: '', password: '', root_path: '/' });
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
            <div className="flex items-center gap-4">
              <div className="w-14 h-14 rounded-xl bg-primary-50 dark:bg-primary-900/20 text-primary-600 flex items-center justify-center">
                <Database size={28} />
              </div>
              <div>
                <div className="flex items-center gap-2">
                  <h3 className="text-xl font-bold dark:text-white">{lib.name}</h3>
                  <span className={`text-[10px] font-bold px-2 py-0.5 rounded-full uppercase tracking-wider ${
                    lib.type === 'local' 
                      ? 'bg-amber-100 text-amber-600 dark:bg-amber-900/20 dark:text-amber-400' 
                      : 'bg-blue-100 text-blue-600 dark:bg-blue-900/20 dark:text-blue-400'
                  }`}>
                    {lib.type === 'local' ? '本地存储' : 'WebDAV'}
                  </span>
                </div>
                <div className="flex flex-wrap gap-4 mt-1">
                  <div className="flex items-center gap-1.5 text-sm text-slate-500 max-w-full">
                    <Globe size={14} className="shrink-0" />
                    <span className="truncate" title={lib.url}>{lib.url}</span>
                  </div>
                  <div className="flex items-center gap-1.5 text-sm text-slate-500 max-w-full">
                    <Folder size={14} className="shrink-0" />
                    <span className="truncate max-w-[300px] md:max-w-[400px]" title={lib.root_path}>{lib.root_path}</span>
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
                      onClick={() => setFormData({...formData, type: 'webdav'})}
                      className={`py-2.5 rounded-xl font-bold transition-all border ${
                        formData.type === 'webdav' 
                          ? 'bg-primary-50 border-primary-200 text-primary-600' 
                          : 'bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700 text-slate-400'
                      }`}
                    >
                      WebDAV
                    </button>
                    <button
                      type="button"
                      onClick={() => setFormData({...formData, type: 'local'})}
                      className={`py-2.5 rounded-xl font-bold transition-all border ${
                        formData.type === 'local' 
                          ? 'bg-primary-50 border-primary-200 text-primary-600' 
                          : 'bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700 text-slate-400'
                      }`}
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
                          required
                          value={formData.password}
                          onChange={e => setFormData({...formData, password: e.target.value})}
                          className="w-full px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                        />
                      </div>
                    </div>
                    <div className="space-y-2">
                      <label className="text-sm font-bold text-slate-600 dark:text-slate-400">根目录</label>
                      <input 
                        type="text" 
                        value={formData.root_path}
                        onChange={e => setFormData({...formData, root_path: e.target.value})}
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
                                  setFormData({...formData, url: currentBrowsePath, root_path: '/'});
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
