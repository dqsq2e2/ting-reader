import React, { useState } from 'react';
import apiClient from '../../core/api/client';
import type { NotificationEventOption } from '../../core/types';
import {
  Braces,
  Check,
  FlaskConical,
  Link2,
  Loader2,
  Plus,
  Search,
  Trash2,
  X,
} from 'lucide-react';
import {
  WEBHOOK_PRESETS,
  type WebhookDraft,
  createHeader,
  draftHeadersToRecord,
  headersToDraft,
} from './webhookData';

interface Props {
  draft: WebhookDraft;
  saving: boolean;
  eventFilter: string;
  eventOptions: NotificationEventOption[];
  filteredEventOptions: NotificationEventOption[];
  onChangeDraft: (draft: WebhookDraft) => void;
  onChangeEventFilter: (value: string) => void;
  onToggleEvent: (eventId: string) => void;
  onClose: () => void;
  onSave: (event: React.FormEvent) => void;
}

const WebhookModal: React.FC<Props> = ({
  draft,
  saving,
  eventFilter,
  eventOptions,
  filteredEventOptions,
  onChangeDraft,
  onChangeEventFilter,
  onToggleEvent,
  onClose,
  onSave,
}) => {
  const [testing, setTesting] = useState(false);
  const [selectedPresetId, setSelectedPresetId] = useState('');
  const [testResult, setTestResult] = useState<{
    success: boolean;
    status: number;
    responseBody: string;
    renderedBody: string;
    error?: string;
  } | null>(null);

  const selectCommonEvents = () => {
    const common = ['user.login', 'playback.play', 'library.scan_completed']
      .filter((eventId) => eventOptions.some((event) => event.id === eventId));
    onChangeDraft({ ...draft, events: common });
  };

  const applyPreset = (presetId: string) => {
    const preset = WEBHOOK_PRESETS.find((item) => item.id === presetId);
    if (!preset) return;
    setSelectedPresetId(presetId);
    onChangeDraft({
      ...draft,
      headers: headersToDraft(preset.headers),
      bodyTemplate: preset.bodyTemplate,
    });
    setTestResult(null);
  };

  const selectedPreset = WEBHOOK_PRESETS.find((preset) => preset.id === selectedPresetId);

  const updateHeader = (id: string, field: 'key' | 'value', value: string) => {
    onChangeDraft({
      ...draft,
      headers: draft.headers.map((header) =>
        header.id === id ? { ...header, [field]: value } : header
      ),
    });
    setTestResult(null);
  };

  const testWebhook = async () => {
    if (!draft.name.trim() || !draft.url.trim() || draft.events.length === 0 || testing) {
      alert('请先填写名称、URL 并选择事件');
      return;
    }

    setTesting(true);
    setTestResult(null);
    try {
      const response = await apiClient.post('/api/system/notifications/test', {
        name: draft.name.trim(),
        url: draft.url.trim(),
        enabled: true,
        events: draft.events,
        secret: draft.secret?.trim() || undefined,
        headers: draftHeadersToRecord(draft.headers),
        bodyTemplate: draft.bodyTemplate,
      });
      setTestResult(response.data);
    } catch (error) {
      console.error('测试 Webhook 失败', error);
      setTestResult({
        success: false,
        status: 0,
        responseBody: '',
        renderedBody: '',
        error: '请求未完成，请检查 URL、请求头和模板',
      });
    } finally {
      setTesting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-[200] flex items-center justify-center p-4">
      <div className="absolute inset-0 bg-slate-900/60 backdrop-blur-sm" onClick={onClose} />
      <div className="relative w-full max-w-2xl bg-white dark:bg-slate-900 rounded-3xl shadow-2xl overflow-hidden animate-in zoom-in-95 duration-200 max-h-[90vh] flex flex-col">
        <div className="px-6 md:px-8 py-5 border-b border-slate-100 dark:border-slate-800 flex items-center justify-between gap-4">
          <div>
            <h2 className="text-2xl font-bold dark:text-white">{draft.id ? '编辑 Webhook' : '添加 Webhook'}</h2>
            <p className="text-xs text-slate-500 mt-1">{draft.events.length} 个事件</p>
          </div>
          <button
            onClick={onClose}
            className="p-2 rounded-xl text-slate-400 hover:text-slate-700 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors"
            title="关闭"
          >
            <X size={20} />
          </button>
        </div>

        <form onSubmit={onSave} className="p-6 md:p-8 overflow-y-auto space-y-5">
          <label className="space-y-2 block">
            <span className="text-sm font-bold text-slate-600 dark:text-slate-400">名称</span>
            <input
              value={draft.name}
              onChange={(event) => onChangeDraft({ ...draft, name: event.target.value })}
              placeholder="企业微信通知"
              className="w-full px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
              autoFocus
            />
          </label>

          <label className="space-y-2 block">
            <span className="text-sm font-bold text-slate-600 dark:text-slate-400">Webhook URL</span>
            <div className="relative">
              <Link2 size={17} className="absolute left-4 top-1/2 -translate-y-1/2 text-slate-400" />
              <input
                value={draft.url}
                onChange={(event) => onChangeDraft({ ...draft, url: event.target.value })}
                placeholder={selectedPreset?.urlPlaceholder || 'https://example.com/webhook'}
                className="w-full pl-11 pr-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
              />
            </div>
          </label>

          <div className="grid grid-cols-1 md:grid-cols-[minmax(0,1fr)_auto] gap-3 items-end">
            <label className="space-y-2">
              <span className="text-sm font-bold text-slate-600 dark:text-slate-400">常见模板</span>
              <select
                value={selectedPresetId}
                onChange={(event) => applyPreset(event.target.value)}
                className="w-full px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
              >
                <option value="">选择模板</option>
                {WEBHOOK_PRESETS.map((preset) => (
                  <option key={preset.id} value={preset.id}>
                    {preset.name}
                  </option>
                ))}
              </select>
            </label>
            <button
              type="button"
              onClick={testWebhook}
              disabled={testing}
              className="h-12 px-4 inline-flex items-center justify-center gap-2 rounded-xl border border-primary-200 text-primary-700 dark:border-primary-900 dark:text-primary-300 font-bold hover:bg-primary-50 dark:hover:bg-primary-900/20 disabled:opacity-60"
            >
              {testing ? <Loader2 size={17} className="animate-spin" /> : <FlaskConical size={17} />}
              测试发送
            </button>
          </div>

          <div className="rounded-2xl border border-slate-100 dark:border-slate-800 p-4 space-y-3">
            <div className="flex items-center justify-between gap-3">
              <p className="text-sm font-bold text-slate-900 dark:text-white">请求头</p>
              <button
                type="button"
                onClick={() => onChangeDraft({ ...draft, headers: [...draft.headers, createHeader()] })}
                className="p-2 rounded-lg text-primary-600 hover:bg-primary-50 dark:hover:bg-primary-900/20"
                title="添加请求头"
              >
                <Plus size={17} />
              </button>
            </div>
            {draft.headers.length === 0 ? (
              <p className="text-xs text-slate-400">未设置请求头</p>
            ) : (
              <div className="space-y-2">
                {draft.headers.map((header) => (
                  <div
                    key={header.id}
                    className="grid grid-cols-[minmax(0,1fr)_auto] sm:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)_auto] gap-2"
                  >
                    <input
                      value={header.key}
                      onChange={(event) => updateHeader(header.id, 'key', event.target.value)}
                      placeholder="Header"
                      className="col-start-1 row-start-1 sm:col-auto sm:row-auto min-w-0 px-3 py-2.5 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 text-sm dark:text-white"
                    />
                    <input
                      value={header.value}
                      onChange={(event) => updateHeader(header.id, 'value', event.target.value)}
                      placeholder="Value"
                      className="col-start-1 row-start-2 sm:col-auto sm:row-auto min-w-0 px-3 py-2.5 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 text-sm dark:text-white"
                    />
                    <button
                      type="button"
                      onClick={() =>
                        onChangeDraft({
                          ...draft,
                          headers: draft.headers.filter((item) => item.id !== header.id),
                        })
                      }
                      className="col-start-2 row-start-1 row-span-2 sm:col-auto sm:row-auto sm:row-span-1 p-2.5 rounded-xl text-slate-400 hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20"
                      title="删除请求头"
                    >
                      <Trash2 size={17} />
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>

          <label className="space-y-2 block">
            <span className="text-sm font-bold text-slate-600 dark:text-slate-400 flex items-center gap-2">
              <Braces size={16} />
              Body 模板
            </span>
            <textarea
              value={draft.bodyTemplate}
              onChange={(event) => {
                onChangeDraft({ ...draft, bodyTemplate: event.target.value });
                setTestResult(null);
              }}
              rows={9}
              spellCheck={false}
              className="w-full px-4 py-3 bg-slate-950 text-slate-100 border border-slate-800 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 font-mono text-sm resize-y"
            />
          </label>

          {testResult && (
            <div
              className={`rounded-2xl border p-4 space-y-2 ${
                testResult.success
                  ? 'border-emerald-200 bg-emerald-50 dark:border-emerald-900 dark:bg-emerald-900/20'
                  : 'border-red-200 bg-red-50 dark:border-red-900 dark:bg-red-900/20'
              }`}
            >
              <p
                className={`text-sm font-bold ${
                  testResult.success ? 'text-emerald-700 dark:text-emerald-300' : 'text-red-700 dark:text-red-300'
                }`}
              >
                {testResult.success
                  ? `发送成功 · HTTP ${testResult.status}`
                  : testResult.error || `发送失败 · HTTP ${testResult.status}`}
              </p>
              {testResult.responseBody && (
                <pre className="max-h-32 overflow-auto whitespace-pre-wrap break-all text-xs text-slate-600 dark:text-slate-300">
                  {testResult.responseBody}
                </pre>
              )}
              {testResult.renderedBody && (
                <details className="text-xs text-slate-500 dark:text-slate-400">
                  <summary className="cursor-pointer font-bold">实际请求体</summary>
                  <pre className="mt-2 max-h-40 overflow-auto whitespace-pre-wrap break-all text-slate-600 dark:text-slate-300">
                    {testResult.renderedBody}
                  </pre>
                </details>
              )}
            </div>
          )}

          <div className="flex items-center justify-between gap-4 p-4 rounded-2xl bg-slate-50 dark:bg-slate-800/70">
            <div>
              <p className="font-bold text-slate-900 dark:text-white">启用</p>
              <p className="text-xs text-slate-500">{draft.enabled ? '开启' : '关闭'}</p>
            </div>
            <button
              type="button"
              onClick={() => onChangeDraft({ ...draft, enabled: !draft.enabled })}
              className={`w-14 h-8 rounded-full transition-all relative ${
                draft.enabled ? 'bg-primary-600' : 'bg-slate-300 dark:bg-slate-700'
              }`}
            >
              <span
                className={`absolute top-1 w-6 h-6 bg-white rounded-full transition-all shadow-sm ${
                  draft.enabled ? 'left-7' : 'left-1'
                }`}
              />
            </button>
          </div>

          <div className="rounded-2xl border border-slate-100 dark:border-slate-800 p-4 space-y-4">
            <div className="flex flex-col md:flex-row md:items-center justify-between gap-3">
              <div>
                <p className="text-sm font-bold text-slate-900 dark:text-white">监听事件</p>
                <p className="text-xs text-slate-500 mt-1">已选 {draft.events.length}</p>
              </div>
              <div className="flex flex-wrap gap-2">
                <button
                  type="button"
                  onClick={selectCommonEvents}
                  className="px-3 py-1.5 rounded-lg bg-slate-100 dark:bg-slate-800 text-xs font-bold text-slate-600 dark:text-slate-300 hover:text-primary-600 transition-colors"
                >
                  常用
                </button>
                <button
                  type="button"
                  onClick={() => onChangeDraft({ ...draft, events: eventOptions.map((event) => event.id) })}
                  className="px-3 py-1.5 rounded-lg bg-slate-100 dark:bg-slate-800 text-xs font-bold text-slate-600 dark:text-slate-300 hover:text-primary-600 transition-colors"
                >
                  全选
                </button>
                <button
                  type="button"
                  onClick={() => onChangeDraft({ ...draft, events: [] })}
                  className="px-3 py-1.5 rounded-lg bg-slate-100 dark:bg-slate-800 text-xs font-bold text-slate-600 dark:text-slate-300 hover:text-red-500 transition-colors"
                >
                  清空
                </button>
              </div>
            </div>

            <div className="relative">
              <Search size={17} className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" />
              <input
                value={eventFilter}
                onChange={(event) => onChangeEventFilter(event.target.value)}
                placeholder="搜索事件"
                className="w-full pl-10 pr-4 py-2.5 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 text-sm dark:text-white"
              />
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
              {filteredEventOptions.map((event) => {
                const checked = draft.events.includes(event.id);
                return (
                  <button
                    key={event.id}
                    type="button"
                    onClick={() => onToggleEvent(event.id)}
                    className={`w-full text-left p-3 rounded-xl border transition-all ${
                      checked
                        ? 'border-primary-200 bg-primary-50 text-primary-700 dark:border-primary-900/50 dark:bg-primary-900/20 dark:text-primary-300'
                        : 'border-slate-100 bg-slate-50 text-slate-600 hover:bg-slate-100 dark:border-slate-800 dark:bg-slate-800/50 dark:text-slate-300'
                    }`}
                  >
                    <div className="flex items-start justify-between gap-3">
                      <div className="min-w-0">
                        <p className="font-bold text-sm">{event.label}</p>
                        <p className="text-[11px] opacity-75 mt-0.5 truncate">{event.id}</p>
                      </div>
                      <span
                        className={`mt-0.5 w-5 h-5 rounded-lg border-2 shrink-0 flex items-center justify-center ${
                          checked ? 'border-primary-600 bg-primary-600' : 'border-slate-300'
                        }`}
                      >
                        {checked && <Check size={13} className="text-white" />}
                      </span>
                    </div>
                  </button>
                );
              })}
            </div>

            {filteredEventOptions.length === 0 && (
              <div className="py-8 text-center text-sm text-slate-400">没有匹配的事件</div>
            )}
          </div>

          <div className="flex gap-4 pt-2">
            <button
              type="button"
              onClick={onClose}
              className="flex-1 py-3 font-bold text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-xl transition-all"
            >
              取消
            </button>
            <button
              type="submit"
              disabled={saving}
              className="flex-1 py-3 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-xl shadow-lg shadow-primary-500/30 transition-all disabled:opacity-60"
            >
              {saving ? <Loader2 size={18} className="animate-spin mx-auto" /> : '保存'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};

export default WebhookModal;
