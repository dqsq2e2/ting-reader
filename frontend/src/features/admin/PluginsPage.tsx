import React, { useEffect, useRef, useState } from 'react';
import apiClient from '../../core/api/client';
import type { Plugin, StorePlugin } from '../../core/types';
import PluginConfigDialog from '../../shared/modals/PluginConfigDialog';
import {
  Puzzle,
  RefreshCw,
  Search,
  ShoppingBag,
  Upload,
} from 'lucide-react';

import PluginCard, {
  getBasePluginId,
  getInstalledStoreMeta,
  toInstalledCardData,
  toStoreCardData,
} from './PluginCard';

const PluginsPage: React.FC = () => {
  const [activeTab, setActiveTab] = useState<'installed' | 'store' | 'updates'>('store');
  const [plugins, setPlugins] = useState<Plugin[]>([]);
  const [storePlugins, setStorePlugins] = useState<StorePlugin[]>([]);
  const [loading, setLoading] = useState(true);
  const [storeLoading, setStoreLoading] = useState(false);
  const [uploading, setUploading] = useState(false);
  const [installingId, setInstallingId] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [category, setCategory] = useState<string>('all');
  const [expandedDescriptions, setExpandedDescriptions] = useState<Set<string>>(new Set());
  const [configPlugin, setConfigPlugin] = useState<Plugin | null>(null);

  const fileInputRef = useRef<HTMLInputElement>(null);

  const toggleDescription = (id: string) => {
    setExpandedDescriptions((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const fetchPlugins = async () => {
    try {
      const response = await apiClient.get('/api/v1/plugins');
      setPlugins(response.data);
    } catch (err) {
      console.error('获取插件失败', err);
    } finally {
      setLoading(false);
    }
  };

  const fetchStorePlugins = async (clearCache = false) => {
    setStoreLoading(true);
    try {
      if (clearCache) {
        try {
          await apiClient.post('/api/v1/store/cache/clear');
        } catch (err) {
          console.error('清除缓存失败', err);
        }
      }

      const response = await apiClient.get('/api/v1/store/plugins');
      setStorePlugins(response.data);
    } catch (err) {
      console.error('获取商店插件失败', err);
    } finally {
      setStoreLoading(false);
    }
  };

  useEffect(() => {
    fetchPlugins();
  }, []);

  useEffect(() => {
    if ((activeTab === 'store' || activeTab === 'updates') && storePlugins.length === 0) {
      fetchStorePlugins();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeTab]);

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
      console.error('安装插件失败', err);
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const msg = (err as any)?.response?.data?.error || (err as Error)?.message || 'Unknown error';
      alert(`安装插件失败: ${msg}`);
    } finally {
      setUploading(false);
      if (fileInputRef.current) {
        fileInputRef.current.value = '';
      }
    }
  };

  const getInstalledVersion = (pluginId: string) => {
    const exactMatch = plugins.find((plugin) => plugin.id === pluginId);
    if (exactMatch) return exactMatch.version;

    const versionMatch = plugins.find((plugin) => getBasePluginId(plugin.id) === pluginId);
    return versionMatch ? versionMatch.version : null;
  };

  const isUpdateAvailable = (storePlugin: StorePlugin) => {
    const installedVersion = getInstalledVersion(storePlugin.id);
    if (!installedVersion) return false;
    return installedVersion.replace('v', '') < storePlugin.version.replace('v', '');
  };

  const handleInstallFromStore = async (pluginId: string) => {
    const plugin = storePlugins.find((item) => item.id === pluginId);
    if (plugin?.dependencies) {
      const missingDeps = plugin.dependencies.filter((depId) => !getInstalledVersion(depId));

      if (missingDeps.length > 0) {
        const missingDepNames = missingDeps.map((depId) => {
          const dep = storePlugins.find((item) => item.id === depId);
          return dep ? dep.name : depId;
        });

        if (confirm(`安装 ${plugin.name} 需要以下依赖插件：\n${missingDepNames.join('\n')}\n\n是否立即安装这些依赖？`)) {
          for (const depId of missingDeps) {
            setInstallingId(depId);
            try {
              await apiClient.post('/api/v1/store/install', { pluginId: depId });
            } catch (err: unknown) {
              console.error(`安装依赖失败 ${depId}`, err);
              // eslint-disable-next-line @typescript-eslint/no-explicit-any
              const msg = (err as any)?.response?.data?.error || (err as Error)?.message || 'Unknown error';
              alert(`无法安装依赖插件 ${depId}: ${msg}`);
              setInstallingId(null);
              return;
            }
          }
          await fetchPlugins();
        } else {
          return;
        }
      }
    }

    setInstallingId(pluginId);
    try {
      await apiClient.post('/api/v1/store/install', { pluginId });
      fetchPlugins();
      alert('Plugin installed successfully!');
    } catch (err: unknown) {
      console.error('从商店安装插件失败', err);
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const msg = (err as any)?.response?.data?.error || (err as Error)?.message || 'Unknown error';
      alert(`安装插件失败: ${msg}`);
    } finally {
      setInstallingId(null);
    }
  };

  const handleReload = async (id: string) => {
    try {
      await apiClient.post(`/api/v1/plugins/${id}/reload`);
      fetchPlugins();
      alert('Plugin reloaded successfully!');
    } catch (err: unknown) {
      console.error('重新加载插件失败', err);
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const msg = (err as any)?.response?.data?.error || (err as Error)?.message || 'Unknown error';
      alert(`重新加载插件失败: ${msg}`);
    }
  };

  const handleUninstall = async (id: string) => {
    if (!confirm('Are you sure you want to uninstall this plugin?')) return;

    try {
      await apiClient.delete(`/api/v1/plugins/${id}`);
      fetchPlugins();
      alert('Plugin uninstalled successfully!');
    } catch (err: unknown) {
      console.error('卸载插件失败', err);
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const msg = (err as any)?.response?.data?.error || (err as Error)?.message || 'Unknown error';
      alert(`卸载插件失败: ${msg}`);
    }
  };

  const matchesSearch = (name: string, description: string) => {
    if (!searchQuery) return true;
    const keyword = searchQuery.toLowerCase();
    return name.toLowerCase().includes(keyword) || description.toLowerCase().includes(keyword);
  };

  const filteredStorePlugins = storePlugins.filter((plugin) => {
    if (activeTab === 'store' && getInstalledVersion(plugin.id)) {
      return false;
    }

    if (activeTab === 'updates' && !isUpdateAvailable(plugin)) {
      return false;
    }

    if (!matchesSearch(plugin.name, plugin.longDescription || plugin.description)) {
      return false;
    }

    if (category !== 'all' && plugin.pluginType !== category) {
      return false;
    }

    return true;
  });

  const filteredInstalledPlugins = plugins.filter((plugin) => {
    const storeMeta = getInstalledStoreMeta(plugin, storePlugins);
    const description = storeMeta?.longDescription || storeMeta?.description || plugin.description;

    if (!matchesSearch(plugin.name, description)) {
      return false;
    }

    if (category !== 'all' && plugin.pluginType !== category) {
      return false;
    }

    return true;
  });

  const updateCount = storePlugins.filter((plugin) => isUpdateAvailable(plugin)).length;

  const categoryItems = [
    { id: 'all', label: '全部' },
    { id: 'scraper', label: '元数据' },
    { id: 'format', label: '格式' },
    { id: 'utility', label: '工具' },
  ];

  return (
    <div className="flex min-h-full flex-1 flex-col p-4 animate-in fade-in duration-500 sm:p-6 md:p-8">
      <div className="mb-6 flex flex-col gap-4">
        <div className="flex flex-col justify-between gap-4 md:flex-row md:items-center">
          <div className="flex w-fit items-center gap-1 rounded-lg bg-slate-100 p-1 dark:bg-slate-800">
            <button
              onClick={() => setActiveTab('store')}
              className={`rounded-md px-4 py-2 text-sm font-medium transition-colors ${
                activeTab === 'store'
                  ? 'bg-white text-primary-600 shadow-sm dark:bg-slate-700 dark:text-primary-400'
                  : 'text-slate-500 hover:text-slate-700 dark:hover:text-slate-300'
              }`}
            >
              全部
            </button>
            <button
              onClick={() => setActiveTab('installed')}
              className={`flex items-center gap-2 rounded-md px-4 py-2 text-sm font-medium transition-colors ${
                activeTab === 'installed'
                  ? 'bg-white text-primary-600 shadow-sm dark:bg-slate-700 dark:text-primary-400'
                  : 'text-slate-500 hover:text-slate-700 dark:hover:text-slate-300'
              }`}
            >
              已安装
              {plugins.length > 0 ? (
                <span className={`rounded-full px-1.5 py-0.5 text-xs ${
                  activeTab === 'installed' ? 'bg-primary-50 text-primary-600' : 'bg-slate-200 text-slate-600'
                }`}
                >
                  {plugins.length}
                </span>
              ) : null}
            </button>
            <button
              onClick={() => setActiveTab('updates')}
              className={`relative flex items-center gap-2 rounded-md px-4 py-2 text-sm font-medium transition-colors ${
                activeTab === 'updates'
                  ? 'bg-white text-primary-600 shadow-sm dark:bg-slate-700 dark:text-primary-400'
                  : 'text-slate-500 hover:text-slate-700 dark:hover:text-slate-300'
              }`}
            >
              可升级
              {updateCount > 0 ? (
                <span className="rounded-full bg-red-500 px-1.5 py-0.5 text-xs font-semibold text-white">
                  {updateCount}
                </span>
              ) : null}
            </button>
          </div>

          <div className="flex items-center gap-3">
            {activeTab === 'installed' ? (
              <>
                <button
                  onClick={() => fileInputRef.current?.click()}
                  disabled={uploading}
                  className="flex items-center gap-2 rounded-lg px-3 py-2 text-sm font-medium text-slate-600 transition-colors hover:bg-slate-100 hover:text-primary-600 disabled:opacity-60 dark:hover:bg-slate-800"
                >
                  {uploading ? (
                    <span className="h-4 w-4 rounded-full border-2 border-current border-t-transparent animate-spin" />
                  ) : (
                    <Upload size={16} />
                  )}
                  手动安装
                </button>
                <input type="file" ref={fileInputRef} onChange={handleUpload} accept=".zip" className="hidden" />
              </>
            ) : null}
            <button
              onClick={() => (activeTab === 'installed' ? fetchPlugins() : fetchStorePlugins(true))}
              className="flex items-center gap-2 rounded-lg border border-slate-200 bg-white px-4 py-2 text-sm font-medium text-slate-700 shadow-sm transition-colors hover:bg-slate-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
            >
              <RefreshCw size={16} />
              {activeTab === 'installed' ? '刷新列表' : '更新插件列表'}
            </button>
          </div>
        </div>

        <div className="flex flex-col items-start justify-between gap-4 rounded-lg border border-slate-200 bg-white p-3 dark:border-slate-800 dark:bg-slate-900 md:flex-row md:items-center">
          <div className="flex flex-wrap items-center gap-2">
            {categoryItems.map((item) => (
              <button
                key={item.id}
                onClick={() => setCategory(item.id)}
                className={`rounded-md px-3 py-1.5 text-sm transition-colors ${
                  category === item.id
                    ? 'bg-primary-50 font-medium text-primary-600 dark:bg-primary-950/40 dark:text-primary-300'
                    : 'text-slate-500 hover:bg-slate-50 hover:text-slate-700 dark:hover:bg-slate-800 dark:hover:text-slate-300'
                }`}
              >
                {item.label}
              </button>
            ))}
          </div>

          <div className="relative w-full md:w-72">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" size={16} />
            <input
              type="text"
              placeholder="搜索插件"
              value={searchQuery}
              onChange={(event) => setSearchQuery(event.target.value)}
              className="w-full rounded-lg border border-slate-200 bg-slate-50 py-2 pl-9 pr-4 text-sm transition-all focus:outline-none focus:ring-2 focus:ring-primary-500 dark:border-slate-700 dark:bg-slate-800"
            />
          </div>
        </div>
      </div>

      {activeTab === 'installed' ? (
        loading ? (
          <div className="flex flex-1 items-center justify-center py-12">
            <div className="h-12 w-12 animate-spin rounded-full border-b-2 border-primary-600" />
          </div>
        ) : filteredInstalledPlugins.length === 0 ? (
          <div className="flex flex-1 flex-col items-center justify-center py-12 text-slate-400">
            <Puzzle size={56} className="mb-4 opacity-50" />
            <p className="text-lg font-medium">暂无已安装的插件</p>
            <p className="mt-2 text-sm">点击“全部”查看可安装插件</p>
          </div>
        ) : (
          <div className="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-3">
            {filteredInstalledPlugins.map((plugin) => {
              const storeMeta = getInstalledStoreMeta(plugin, storePlugins);
              const data = toInstalledCardData(plugin, storeMeta);

              return (
                <PluginCard
                  key={plugin.id}
                  data={data}
                  expanded={expandedDescriptions.has(plugin.id)}
                  onToggleDescription={toggleDescription}
                  onConfigure={data.configSchema ? () => setConfigPlugin(plugin) : undefined}
                  onReload={() => handleReload(plugin.id)}
                  onUninstall={() => handleUninstall(plugin.id)}
                />
              );
            })}
          </div>
        )
      ) : (
        storeLoading ? (
          <div className="flex flex-1 items-center justify-center py-12">
            <div className="h-12 w-12 animate-spin rounded-full border-b-2 border-primary-600" />
          </div>
        ) : filteredStorePlugins.length === 0 ? (
          <div className="flex flex-1 flex-col items-center justify-center py-12 text-slate-400">
            <ShoppingBag size={56} className="mb-4 opacity-50" />
            <p className="text-lg font-medium">
              {activeTab === 'updates' ? '暂无可用更新' : '未找到符合条件的插件'}
            </p>
          </div>
        ) : (
          <div className="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-3">
            {filteredStorePlugins.map((plugin) => {
              const installedVersion = getInstalledVersion(plugin.id);
              const hasUpdate = isUpdateAvailable(plugin);
              const data = toStoreCardData(plugin, installedVersion, hasUpdate);

              return (
                <PluginCard
                  key={plugin.id}
                  data={data}
                  expanded={expandedDescriptions.has(plugin.id)}
                  installing={installingId === plugin.id}
                  onToggleDescription={toggleDescription}
                  onInstall={() => handleInstallFromStore(plugin.id)}
                />
              );
            })}
          </div>
        )
      )}

      {configPlugin && configPlugin.configSchema ? (
        <PluginConfigDialog
          pluginId={configPlugin.id}
          pluginName={configPlugin.name}
          configSchema={configPlugin.configSchema as Record<string, unknown>}
          onClose={() => setConfigPlugin(null)}
          onSaved={() => fetchPlugins()}
        />
      ) : null}
    </div>
  );
};

export default PluginsPage;
