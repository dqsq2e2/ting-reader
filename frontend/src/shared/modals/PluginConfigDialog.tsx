import React, { useState, useEffect, useCallback } from 'react';
import apiClient from '../../core/api/client';
import { X, Save, Loader2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';

const SECRET_UNCHANGED_PLACEHOLDER = '__TING_READER_SECRET_UNCHANGED__';

interface Props {
  pluginId: string;
  pluginName: string;
  configSchema: Record<string, unknown>;
  onClose: () => void;
  onSaved: () => void;
}

function localizedSchemaText(value: unknown, language: string): string {
  if (typeof value === 'string') return value.trim();
  if (!value || typeof value !== 'object') return '';

  const record = value as Record<string, unknown>;
  const preferred = language.toLowerCase().startsWith('en')
    ? ['en', 'enUS', 'en-US']
    : ['zh', 'zhCN', 'zh-CN', 'zhHans', 'zh-Hans'];
  const fallbackKeys = language.toLowerCase().startsWith('en')
    ? ['zh', 'zhCN', 'zh-CN', 'zhHans', 'zh-Hans']
    : ['en', 'enUS', 'en-US'];

  for (const key of [...preferred, ...fallbackKeys]) {
    const text = typeof record[key] === 'string' ? record[key].trim() : '';
    if (text) return text;
  }

  const fallbackText = Object.values(record).find(
    (text): text is string => typeof text === 'string' && text.trim().length > 0
  );
  return fallbackText?.trim() || '';
}

function localizedSchemaPair(value: unknown, language: string): string {
  if (typeof value === 'string') return value.trim();
  if (!value || typeof value !== 'object') return '';

  const record = value as Record<string, unknown>;
  const zh = (
    localizedSchemaText({ zh: record.zh, zhCN: record.zhCN, 'zh-CN': record['zh-CN'], zhHans: record.zhHans, 'zh-Hans': record['zh-Hans'] }, 'zh-CN') ||
    localizedSchemaText(value, 'zh-CN')
  );
  const en = (
    localizedSchemaText({ en: record.en, enUS: record.enUS, 'en-US': record['en-US'] }, 'en-US') ||
    localizedSchemaText(value, 'en-US')
  );

  if (zh && en && zh !== en) {
    return language.toLowerCase().startsWith('en') ? `${en} / ${zh}` : `${zh} / ${en}`;
  }
  return zh || en;
}

function enumOptionLabel(
  option: string,
  index: number,
  prop: Record<string, unknown>,
  language: string
): string {
  const rawLabels =
    prop.enum_labels ||
    prop.enumLabels ||
    prop['x-enum-labels'] ||
    prop.enumNames;

  if (Array.isArray(rawLabels)) {
    const label = localizedSchemaPair(rawLabels[index], language);
    if (label) return label;
  }

  if (rawLabels && typeof rawLabels === 'object') {
    const label = localizedSchemaPair((rawLabels as Record<string, unknown>)[option], language);
    if (label) return label;
  }

  return option;
}

function fieldLabel(key: string, prop: Record<string, unknown>, language: string): string {
  return (
    localizedSchemaText(prop.title_i18n, language) ||
    localizedSchemaText(prop.label_i18n, language) ||
    localizedSchemaText(prop.title, language) ||
    localizedSchemaText(prop.label, language) ||
    key.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase())
  );
}

function fieldDescription(prop: Record<string, unknown>, language: string): string {
  return (
    localizedSchemaText(prop.description_i18n, language) ||
    localizedSchemaText(prop.description, language)
  );
}

function fieldPlaceholder(prop: Record<string, unknown>, language: string): string {
  return (
    localizedSchemaText(prop.placeholder_i18n, language) ||
    localizedSchemaText(prop.placeholder, language)
  );
}

function isEncryptedField(prop: Record<string, unknown>): boolean {
  return prop['x-encrypted'] === true ||
    prop.encrypted === true ||
    prop.format === 'password' ||
    prop.format === 'secret';
}

const PluginConfigDialog: React.FC<Props> = ({ pluginId, pluginName, configSchema, onClose, onSaved }) => {
  const { t, i18n } = useTranslation();
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
      const msg = err instanceof Error ? err.message : t('pluginConfig.saveFailed');
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
    const language = i18n.resolvedLanguage || i18n.language || 'zh-CN';
    const label = fieldLabel(key, prop, language);
    const description = fieldDescription(prop, language);
    const encrypted = isEncryptedField(prop);
    const propType = typeof prop.type === 'string' ? prop.type : 'string';
    const propEnum = Array.isArray(prop.enum) ? (prop.enum as string[]) : [];
    const displayValue = encrypted && value === SECRET_UNCHANGED_PLACEHOLDER ? '' : value;

    if (propEnum.length > 0) {
      return (
        <div key={key} className="mb-4">
          <label className="block text-sm font-medium text-slate-700 dark:text-slate-300 mb-1.5">{label}</label>
          <select
            value={String(value)}
            onChange={e => setValue(key, e.target.value)}
            className="w-full px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 text-slate-900 dark:text-white text-sm focus:ring-2 focus:ring-primary-500 focus:border-transparent outline-none"
          >
            {propEnum.map((opt, index) => (
              <option key={opt} value={opt}>{enumOptionLabel(opt, index, prop, language)}</option>
            ))}
          </select>
          {!!description && (
            <p className="mt-1 text-xs text-slate-400">{description}</p>
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
            {!!description && (
              <span className="text-xs text-slate-400">{description}</span>
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
            {!!description && (
              <p className="mt-1 text-xs text-slate-400">{description}</p>
            )}
          </div>
        );
      default:
        return (
          <div key={key} className="mb-4">
            <label className="block text-sm font-medium text-slate-700 dark:text-slate-300 mb-1.5">{label}</label>
            <input
              type={encrypted ? 'password' : 'text'}
              value={String(displayValue)}
              onChange={e => setValue(key, e.target.value)}
              placeholder={encrypted ? t('pluginConfig.secretPlaceholder') : fieldPlaceholder(prop, language)}
              className="w-full px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 text-slate-900 dark:text-white text-sm focus:ring-2 focus:ring-primary-500 focus:border-transparent outline-none"
            />
            {encrypted && (
              <p className="mt-1 text-xs text-slate-400">{t('pluginConfig.encryptedHint')}</p>
            )}
            {!!description && (
              <p className="mt-1 text-xs text-slate-400">{description}</p>
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
          <p className="font-bold text-slate-600 dark:text-slate-400">{t('pluginConfig.loading')}</p>
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
            <h2 className="text-lg font-bold text-slate-900 dark:text-white">{t('pluginConfig.title')}</h2>
            <p className="text-sm text-slate-500 dark:text-slate-400">{pluginName}</p>
          </div>
          <button onClick={onClose} className="p-2 text-slate-400 hover:text-slate-600 dark:hover:text-slate-300 rounded-lg hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors">
            <X size={20} />
          </button>
        </div>

        {/* Form */}
        <div className="flex-1 overflow-y-auto px-6 py-4">
          {Object.keys(properties).length === 0 ? (
            <p className="text-slate-400 text-sm text-center py-8">{t('pluginConfig.empty')}</p>
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
            {t('common.cancel')}
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
            {t('common.save')}
          </button>
        </div>
      </div>
    </div>
  );
};

export default PluginConfigDialog;
