import React, { useEffect, useState, useRef } from 'react';
import apiClient from '../api/client';
import type { Plugin } from '../types';
import { 
  Puzzle, 
  Upload, 
  RefreshCw, 
  Trash2, 
  CheckCircle, 
  XCircle, 
  AlertCircle,
  // MoreVertical
} from 'lucide-react';

const PluginsPage: React.FC = () => {
  const [plugins, setPlugins] = useState<Plugin[]>([]);
  const [loading, setLoading] = useState(true);
  const [uploading, setUploading] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const fetchPlugins = async () => {
    try {
      const response = await apiClient.get('/api/v1/plugins');
      setPlugins(response.data);
    } catch (err) {
      console.error('Failed to fetch plugins', err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchPlugins();
  }, []);

  const handleUpload = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;

    const formData = new FormData();
    formData.append('file', file);

    setUploading(true);
    try {
      await apiClient.post('/api/v1/plugins/install', formData, {
        headers: {
          'Content-Type': 'multipart/form-data',
        },
      });
      fetchPlugins();
      alert('Plugin installed successfully!');
    } catch (err: unknown) {
      console.error('Failed to install plugin', err);
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const msg = (err as any)?.response?.data?.error || (err as Error)?.message || 'Unknown error';
      alert(`Failed to install plugin: ${msg}`);
    } finally {
      setUploading(false);
      if (fileInputRef.current) {
        fileInputRef.current.value = '';
      }
    }
  };

  const handleReload = async (id: string) => {
    try {
      await apiClient.post(`/api/v1/plugins/${id}/reload`);
      fetchPlugins();
      alert('Plugin reloaded successfully!');
    } catch (err: unknown) {
      console.error('Failed to reload plugin', err);
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const msg = (err as any)?.response?.data?.error || (err as Error)?.message || 'Unknown error';
      alert(`Failed to reload plugin: ${msg}`);
    }
  };

  const handleUninstall = async (id: string) => {
    if (!confirm('Are you sure you want to uninstall this plugin?')) return;

    try {
      await apiClient.delete(`/api/v1/plugins/${id}`);
      fetchPlugins();
      alert('Plugin uninstalled successfully!');
    } catch (err: unknown) {
      console.error('Failed to uninstall plugin', err);
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const msg = (err as any)?.response?.data?.error || (err as Error)?.message || 'Unknown error';
      alert(`Failed to uninstall plugin: ${msg}`);
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-primary-600"></div>
      </div>
    );
  }

  return (
    <div className="flex-1 min-h-full flex flex-col p-4 sm:p-6 md:p-8 animate-in fade-in duration-500">
      <div className="flex items-center justify-between mb-8">
        <div>
          <h1 className="text-2xl md:text-3xl font-bold text-slate-900 dark:text-white flex items-center gap-3">
            <Puzzle size={28} className="text-primary-600 md:w-8 md:h-8" />
            插件管理
          </h1>
          <p className="text-sm md:text-base text-slate-500 dark:text-slate-400 mt-1">管理系统的扩展功能插件</p>
        </div>
        <div className="flex gap-3">
          <button 
            onClick={() => fetchPlugins()} 
            className="p-2 text-slate-500 hover:text-primary-600 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-xl transition-colors"
            title="Refresh"
          >
            <RefreshCw size={20} />
          </button>
          <button 
            onClick={() => fileInputRef.current?.click()}
            disabled={uploading}
            className="flex items-center gap-2 bg-primary-600 hover:bg-primary-700 text-white px-4 py-2 rounded-xl transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {uploading ? (
              <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-white"></div>
            ) : (
              <Upload size={18} />
            )}
            <span>安装插件</span>
          </button>
          <input 
            type="file" 
            ref={fileInputRef} 
            onChange={handleUpload} 
            accept=".zip" 
            className="hidden" 
          />
        </div>
      </div>

      {plugins.length === 0 ? (
        <div className="flex-1 flex flex-col items-center justify-center text-slate-400">
          <Puzzle size={64} className="mb-4 opacity-50" />
          <p className="text-lg font-medium">暂无已安装的插件</p>
          <p className="text-sm mt-2">点击右上角的"安装插件"按钮添加新功能</p>
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
          {plugins.map((plugin) => (
            <div key={plugin.id} className="bg-white dark:bg-slate-900 rounded-2xl p-6 shadow-sm border border-slate-100 dark:border-slate-800 flex flex-col">
              <div className="flex items-start justify-between mb-4">
                <div className="flex items-center gap-3">
                  <div className={`w-10 h-10 rounded-xl flex items-center justify-center text-white ${
                    plugin.pluginType === 'scraper' ? 'bg-blue-500' : 
                    plugin.pluginType === 'format' ? 'bg-purple-500' : 'bg-green-500'
                  }`}>
                    <Puzzle size={20} />
                  </div>
                  <div>
                    <h3 className="font-bold text-slate-900 dark:text-white truncate max-w-[150px]">{plugin.name}</h3>
                    <p className="text-xs text-slate-500 dark:text-slate-400">v{plugin.version}</p>
                  </div>
                </div>
                <div className="flex items-center gap-1">
                  {plugin.state === 'active' ? (
                    <span className="flex items-center gap-1 text-[10px] uppercase font-bold text-green-600 bg-green-50 dark:bg-green-900/20 px-2 py-1 rounded-full border border-green-100 dark:border-green-900/30">
                      <CheckCircle size={12} /> Active
                    </span>
                  ) : plugin.state === 'failed' ? (
                    <span className="flex items-center gap-1 text-[10px] uppercase font-bold text-red-600 bg-red-50 dark:bg-red-900/20 px-2 py-1 rounded-full border border-red-100 dark:border-red-900/30">
                      <XCircle size={12} /> Failed
                    </span>
                  ) : (
                    <span className="flex items-center gap-1 text-[10px] uppercase font-bold text-slate-600 bg-slate-50 dark:bg-slate-800 px-2 py-1 rounded-full border border-slate-100 dark:border-slate-700">
                      <AlertCircle size={12} /> {plugin.state}
                    </span>
                  )}
                </div>
              </div>
              
              <div className="flex-1 mb-4">
                <p className="text-sm text-slate-600 dark:text-slate-300 line-clamp-2">{plugin.description}</p>
                <div className="mt-3 flex flex-wrap gap-2">
                  <span className="text-xs text-slate-500 bg-slate-50 dark:bg-slate-800 px-2 py-1 rounded-md border border-slate-100 dark:border-slate-700">
                    Type: {plugin.pluginType}
                  </span>
                  <span className="text-xs text-slate-500 bg-slate-50 dark:bg-slate-800 px-2 py-1 rounded-md border border-slate-100 dark:border-slate-700">
                    Author: {plugin.author}
                  </span>
                </div>
              </div>

              <div className="pt-4 border-t border-slate-100 dark:border-slate-800 flex justify-end gap-2">
                <button 
                  onClick={() => handleReload(plugin.id)}
                  className="p-2 text-slate-500 hover:text-blue-600 hover:bg-blue-50 dark:hover:bg-blue-900/20 rounded-lg transition-colors"
                  title="Reload"
                >
                  <RefreshCw size={18} />
                </button>
                <button 
                  onClick={() => handleUninstall(plugin.id)}
                  className="p-2 text-slate-500 hover:text-red-600 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-lg transition-colors"
                  title="Uninstall"
                >
                  <Trash2 size={18} />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

export default PluginsPage;
