import React, { useState, useEffect, useCallback } from 'react';
import apiClient from '../api/client';
import { X, Save, Loader2 } from 'lucide-react';

interface Props {
  pluginId: string;
  pluginName: string;
  configSchema: Record<string, unknown>;
  onClose: () => void;
  onSaved: () => void;
}

function fieldLabel(key: string, prop: Record<string, unknown>): string {
  return (typeof prop.title === 'string' ? prop.title : '') || key.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase());
}

const PluginConfigDialog: React.FC<Props> = ({ pluginId, pluginName, configSchema, onClose, onSaved }) => {
  const [config, setConfig] = useState<Record<string, unknown>>({});
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchConfig = useCallback(async () => {
    try {
      setLoading(true);
      const res = await apiClient.get(`/api/v1/plugins/${pluginId}/config`);
      setConfig(res.data.config || {});
    } catch {
      setConfig({});
    } finally {
      setLoading(false);
    }
  }, [pluginId]);

  useEffect(() => {
    fetchConfig();
  }, [fetchConfig]);

  const handleSave = async () => {
    try {
      setSaving(true);
      setError(null);
      await apiClient.put(`/api/v1/plugins/${pluginId}/config`, { config });
      onSaved();
      onClose();
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : '保存配置失败';
      setError(msg);
    } finally {
      setSaving(false);
    }
  };

  const setValue = (key: string, value: unknown) => {
    setConfig(prev => ({ ...prev, [key]: value }));
  };

  const properties = (configSchema.properties as Record<string, Record<string, unknown>>) || {};

  const renderField = (key: string, prop: Record<string, unknown>) => {
    const value = config[key] ?? prop.default ?? '';
    const label = fieldLabel(key, prop);
    const propType = typeof prop.type === 'string' ? prop.type : 'string';
    const propEnum = Array.isArray(prop.enum) ? (prop.enum as string[]) : [];

    if (propEnum.length > 0) {
      return (
        <div key={key} className="mb-4">
          <label className="block text-sm font-medium text-slate-700 dark:text-slate-300 mb-1.5">{label}</label>
          <select
            value={String(value)}
            onChange={e => setValue(key, e.target.value)}
            className="w-full px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 text-slate-900 dark:text-white text-sm focus:ring-2 focus:ring-primary-500 focus:border-transparent outline-none"
          >
            {propEnum.map(opt => (
              <option key={opt} value={opt}>{opt}</option>
            ))}
          </select>
          {prop.description && (
            <p className="mt-1 text-xs text-slate-400">{String(prop.description)}</p>
          )}
        </div>
      );
    }

    switch (propType) {
      case 'boolean':
        return (
          <div key={key} className="mb-4 flex items-center gap-3">
            <input
              type="checkbox"
              id={`cfg-${key}`}
              checked={!!value}
              onChange={e => setValue(key, e.target.checked)}
              className="w-4 h-4 rounded border-slate-300 text-primary-600 focus:ring-primary-500"
            />
            <label htmlFor={`cfg-${key}`} className="text-sm font-medium text-slate-700 dark:text-slate-300 cursor-pointer">
              {label}
            </label>
            {prop.description && (
              <span className="text-xs text-slate-400">{String(prop.description)}</span>
            )}
          </div>
        );
      case 'integer':
      case 'number':
        return (
          <div key={key} className="mb-4">
            <label className="block text-sm font-medium text-slate-700 dark:text-slate-300 mb-1.5">{label}</label>
            <input
              type="number"
              value={Number(value)}
              onChange={e => setValue(key, e.target.value === '' ? '' : Number(e.target.value))}
              className="w-full px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 text-slate-900 dark:text-white text-sm focus:ring-2 focus:ring-primary-500 focus:border-transparent outline-none"
            />
            {prop.description && (
              <p className="mt-1 text-xs text-slate-400">{String(prop.description)}</p>
            )}
          </div>
        );
      default:
        return (
          <div key={key} className="mb-4">
            <label className="block text-sm font-medium text-slate-700 dark:text-slate-300 mb-1.5">{label}</label>
            <input
              type="text"
              value={String(value)}
              onChange={e => setValue(key, e.target.value)}
              className="w-full px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 text-slate-900 dark:text-white text-sm focus:ring-2 focus:ring-primary-500 focus:border-transparent outline-none"
            />
            {prop.description && (
              <p className="mt-1 text-xs text-slate-400">{String(prop.description)}</p>
            )}
          </div>
        );
    }
  };

  if (loading) {
    return (
      <div className="fixed inset-0 z-[200] flex items-center justify-center p-4">
        <div className="absolute inset-0 bg-slate-900/60 backdrop-blur-sm" />
        <div className="relative bg-white dark:bg-slate-900 p-8 rounded-3xl shadow-2xl flex flex-col items-center gap-4">
          <Loader2 className="animate-spin text-primary-600" size={40} />
          <p className="font-bold text-slate-600 dark:text-slate-400">正在加载配置...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="fixed inset-0 z-[200] flex items-center justify-center p-4">
      <div className="absolute inset-0 bg-slate-900/60 backdrop-blur-sm" onClick={onClose} />
      <div className="relative w-full max-w-lg max-h-[85vh] bg-white dark:bg-slate-900 rounded-3xl shadow-2xl flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-slate-100 dark:border-slate-800">
          <div>
            <h2 className="text-lg font-bold text-slate-900 dark:text-white">插件配置</h2>
            <p className="text-sm text-slate-500 dark:text-slate-400">{pluginName}</p>
          </div>
          <button onClick={onClose} className="p-2 text-slate-400 hover:text-slate-600 dark:hover:text-slate-300 rounded-lg hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors">
            <X size={20} />
          </button>
        </div>

        {/* Form */}
        <div className="flex-1 overflow-y-auto px-6 py-4">
          {Object.keys(properties).length === 0 ? (
            <p className="text-slate-400 text-sm text-center py-8">该插件无可配置项。</p>
          ) : (
            Object.entries(properties).map(([key, prop]) => renderField(key, prop))
          )}

          {error && (
            <div className="mt-3 p-3 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-900/30 text-sm text-red-600 dark:text-red-400">
              {error}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex justify-end gap-3 px-6 py-4 border-t border-slate-100 dark:border-slate-800">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm font-medium text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-lg transition-colors"
          >
            取消
          </button>
          <button
            onClick={handleSave}
            disabled={saving}
            className="px-4 py-2 text-sm font-medium text-white bg-primary-600 hover:bg-primary-700 rounded-lg transition-colors flex items-center gap-2 disabled:opacity-50"
          >
            {saving ? (
              <Loader2 size={16} className="animate-spin" />
            ) : (
              <Save size={16} />
            )}
            保存
          </button>
        </div>
      </div>
    </div>
  );
};

export default PluginConfigDialog;
