import React, { useEffect, useMemo, useState } from 'react';
import apiClient from '../api/client';
import BackButton from '../components/BackButton';
import { usePlayerStore } from '../store/playerStore';
import type { NotificationEventOption, NotificationWebhook } from '../types';
import {
  BellRing,
  Check,
  CheckCircle2,
  Edit,
  KeyRound,
  Link2,
  Loader2,
  Plus,
  Power,
  Radio,
  Search,
  Trash2,
  Webhook,
  X,
} from 'lucide-react';

type WebhookDraft = {
  id?: string;
  name: string;
  url: string;
  enabled: boolean;
  events: string[];
  secret?: string;
};

const fallbackEvents: NotificationEventOption[] = [
  { id: 'user.login', label: '用户登录', description: '用户成功登录系统' },
  { id: 'playback.play', label: '播放', description: '用户开始播放作品或章节' },
  { id: 'library.created', label: '新增媒体库', description: '管理员创建媒体库' },
  { id: 'library.deleted', label: '删除媒体库', description: '管理员删除媒体库' },
  { id: 'book.created', label: '作品入库', description: '作品被创建或入库' },
  { id: 'book.deleted', label: '删除作品', description: '作品被删除' },
  { id: 'library.scan_completed', label: '扫描完成', description: '媒体库扫描任务完成' },
];

const createEmptyDraft = (defaultEvent = 'user.login'): WebhookDraft => ({
  name: '',
  url: '',
  enabled: true,
  events: [defaultEvent],
  secret: '',
});

const NotificationSettingsPage: React.FC = () => {
  const currentChapter = usePlayerStore((state) => state.currentChapter);
  const [webhooks, setWebhooks] = useState<NotificationWebhook[]>([]);
  const [eventOptions, setEventOptions] = useState<NotificationEventOption[]>(fallbackEvents);
  const [draft, setDraft] = useState<WebhookDraft | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<NotificationWebhook | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [eventFilter, setEventFilter] = useState('');

  const fetchData = async () => {
    setLoading(true);
    try {
      const [hooksRes, eventsRes] = await Promise.all([
        apiClient.get('/api/system/notifications'),
        apiClient.get('/api/system/notifications/events'),
      ]);
      setWebhooks(hooksRes.data || []);
      setEventOptions(eventsRes.data?.length ? eventsRes.data : fallbackEvents);
    } catch (error) {
      console.error('获取通知配置失败', error);
      setEventOptions(fallbackEvents);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchData();
  }, []);

  const enabledCount = webhooks.filter((item) => item.enabled).length;
  const disabledCount = Math.max(webhooks.length - enabledCount, 0);
  const listenedEvents = useMemo(
    () => new Set(webhooks.flatMap((item) => (item.enabled ? item.events : []))).size,
    [webhooks]
  );

  const filteredEventOptions = useMemo(() => {
    const keyword = eventFilter.trim().toLowerCase();
    if (!keyword) return eventOptions;
    return eventOptions.filter((event) =>
      `${event.label} ${event.id} ${event.description}`.toLowerCase().includes(keyword)
    );
  }, [eventFilter, eventOptions]);

  const eventLabel = (eventId: string) =>
    eventOptions.find((event) => event.id === eventId)?.label || eventId;

  const startCreate = () => {
    setEventFilter('');
    setDraft(createEmptyDraft(eventOptions[0]?.id || 'user.login'));
  };

  const startEdit = (webhook: NotificationWebhook) => {
    setEventFilter('');
    setDraft({
      id: webhook.id,
      name: webhook.name,
      url: webhook.url,
      enabled: webhook.enabled,
      events: webhook.events,
      secret: webhook.secret || '',
    });
  };

  const closeModal = () => {
    setDraft(null);
    setEventFilter('');
  };

  const toggleDraftEvent = (eventId: string) => {
    setDraft((current) => {
      if (!current) return current;
      const exists = current.events.includes(eventId);
      return {
        ...current,
        events: exists
          ? current.events.filter((item) => item !== eventId)
          : [...current.events, eventId],
      };
    });
  };

  const toggleWebhookEnabled = async (webhook: NotificationWebhook) => {
    try {
      const nextEnabled = !webhook.enabled;
      await apiClient.put(`/api/system/notifications/${webhook.id}`, {
        name: webhook.name,
        url: webhook.url,
        enabled: nextEnabled,
        events: webhook.events,
        secret: webhook.secret || undefined,
      });
      setWebhooks((items) =>
        items.map((item) => (item.id === webhook.id ? { ...item, enabled: nextEnabled } : item))
      );
      if (draft?.id === webhook.id) {
        setDraft({ ...draft, enabled: nextEnabled });
      }
    } catch (error) {
      console.error('更新通知状态失败', error);
      alert('更新状态失败，请稍后重试');
    }
  };

  const saveDraft = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!draft || saving) return;
    if (!draft.name.trim()) {
      alert('请填写配置名称');
      return;
    }
    if (!draft.url.trim()) {
      alert('请填写 Webhook URL');
      return;
    }
    if (draft.events.length === 0) {
      alert('请至少选择一个监听事件');
      return;
    }

    setSaving(true);
    try {
      const payload = {
        name: draft.name.trim(),
        url: draft.url.trim(),
        enabled: draft.enabled,
        events: draft.events,
        secret: draft.secret?.trim() || undefined,
      };
      if (draft.id) {
        await apiClient.put(`/api/system/notifications/${draft.id}`, payload);
      } else {
        await apiClient.post('/api/system/notifications', payload);
      }
      closeModal();
      setSaved(true);
      setTimeout(() => setSaved(false), 1600);
      await fetchData();
    } catch (error) {
      console.error('保存通知配置失败', error);
      alert('保存失败，请检查 URL 和事件配置');
    } finally {
      setSaving(false);
    }
  };

  const deleteWebhook = async () => {
    if (!deleteTarget) return;
    try {
      await apiClient.delete(`/api/system/notifications/${deleteTarget.id}`);
      if (draft?.id === deleteTarget.id) closeModal();
      setDeleteTarget(null);
      await fetchData();
    } catch (error) {
      console.error('删除通知配置失败', error);
      alert('删除失败，请稍后重试');
    }
  };

  return (
    <div className="flex-1 min-h-full flex flex-col p-4 sm:p-6 md:p-8 animate-in fade-in duration-500">
      <div className="flex-1 space-y-6 max-w-6xl w-full mx-auto">
        <BackButton fallback="/mine" />

        <div className="flex flex-col lg:flex-row lg:items-center justify-between gap-4">
          <div>
            <h1 className="text-2xl md:text-3xl font-bold text-slate-900 dark:text-white flex items-center gap-3">
              <BellRing className="text-primary-600" />
              通知与事件
            </h1>
            <p className="text-sm md:text-base text-slate-500 dark:text-slate-400 mt-1">
              Webhook 监听与事件推送
            </p>
          </div>
          {saved && (
            <span className="text-sm text-green-600 font-bold flex items-center gap-1">
              <CheckCircle2 size={15} />
              已保存
            </span>
          )}
        </div>

        <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
          <StatCard label="Webhook" value={webhooks.length} icon={<Webhook size={20} />} />
          <StatCard label="已开启" value={enabledCount} icon={<Power size={20} />} />
          <StatCard label="已关闭" value={disabledCount} icon={<Radio size={20} />} />
          <StatCard label="监听事件" value={listenedEvents} icon={<BellRing size={20} />} />
        </div>

        <section className="bg-white dark:bg-slate-900 rounded-3xl border border-slate-100 dark:border-slate-800 shadow-sm overflow-hidden">
          <div className="px-5 md:px-6 py-5 border-b border-slate-100 dark:border-slate-800 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
            <div>
              <h2 className="text-lg font-bold text-slate-900 dark:text-white">Webhook 列表</h2>
              <p className="text-xs text-slate-500 mt-1">{webhooks.length} 个配置</p>
            </div>
            {!loading && (
              <button
                onClick={startCreate}
                className="inline-flex items-center justify-center gap-2 px-4 py-2.5 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-xl shadow-lg shadow-primary-500/25 transition-all"
              >
                <Plus size={18} />
                添加 Webhook
              </button>
            )}
          </div>

          {loading ? (
            <div className="py-24 flex justify-center">
              <Loader2 size={32} className="text-primary-600 animate-spin" />
            </div>
          ) : webhooks.length === 0 ? (
            <div className="py-20 text-center bg-slate-50/70 dark:bg-slate-950/20">
              <Radio size={48} className="mx-auto text-slate-300 mb-4" />
              <p className="font-bold text-slate-900 dark:text-white">暂无 Webhook</p>
              <p className="text-sm text-slate-500 mt-2">点击列表右上角添加一个监听配置</p>
            </div>
          ) : (
            <div className="divide-y divide-slate-100 dark:divide-slate-800">
              {webhooks.map((webhook) => (
                <div
                  key={webhook.id}
                  className="px-5 md:px-6 py-5 flex flex-col xl:flex-row xl:items-center justify-between gap-4 hover:bg-slate-50/70 dark:hover:bg-slate-800/30 transition-colors"
                >
                  <div className="min-w-0 flex items-start gap-4">
                    <div
                      className={`w-12 h-12 rounded-2xl flex items-center justify-center shrink-0 ${
                        webhook.enabled
                          ? 'bg-emerald-50 text-emerald-600 dark:bg-emerald-900/20'
                          : 'bg-slate-100 text-slate-400 dark:bg-slate-800'
                      }`}
                    >
                      <Webhook size={22} />
                    </div>
                    <div className="min-w-0">
                      <div className="flex flex-wrap items-center gap-2">
                        <h3 className="font-bold text-slate-900 dark:text-white truncate">{webhook.name}</h3>
                        <span
                          className={`px-2 py-0.5 rounded-lg text-[11px] font-black ${
                            webhook.enabled
                              ? 'bg-emerald-50 text-emerald-600 dark:bg-emerald-900/20'
                              : 'bg-slate-100 text-slate-400 dark:bg-slate-800'
                          }`}
                        >
                          {webhook.enabled ? '已开启' : '已关闭'}
                        </span>
                      </div>
                      <p className="text-sm text-slate-500 mt-1 truncate max-w-3xl">{webhook.url}</p>
                      <div className="flex flex-wrap gap-1.5 mt-3">
                        {webhook.events.slice(0, 6).map((eventId) => (
                          <span
                            key={eventId}
                            className="px-2 py-1 rounded-lg bg-slate-100 dark:bg-slate-800 text-[11px] font-bold text-slate-600 dark:text-slate-300"
                          >
                            {eventLabel(eventId)}
                          </span>
                        ))}
                        {webhook.events.length > 6 && (
                          <span className="px-2 py-1 rounded-lg bg-slate-100 dark:bg-slate-800 text-[11px] font-bold text-slate-400">
                            +{webhook.events.length - 6}
                          </span>
                        )}
                      </div>
                    </div>
                  </div>

                  <div className="flex items-center gap-2 shrink-0 xl:justify-end">
                    <button
                      onClick={() => toggleWebhookEnabled(webhook)}
                      className={`inline-flex items-center gap-2 px-3 py-2 rounded-xl text-sm font-bold transition-colors ${
                        webhook.enabled
                          ? 'bg-emerald-50 text-emerald-600 hover:bg-emerald-100 dark:bg-emerald-900/20'
                          : 'bg-slate-100 text-slate-500 hover:bg-slate-200 dark:bg-slate-800 dark:text-slate-300'
                      }`}
                    >
                      <Power size={16} />
                      {webhook.enabled ? '关闭' : '开启'}
                    </button>
                    <button
                      onClick={() => startEdit(webhook)}
                      className="p-2.5 rounded-xl text-slate-400 hover:text-primary-600 hover:bg-primary-50 dark:hover:bg-primary-900/20 transition-all"
                      title="编辑"
                    >
                      <Edit size={20} />
                    </button>
                    <button
                      onClick={() => setDeleteTarget(webhook)}
                      className="p-2.5 rounded-xl text-slate-400 hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 transition-all"
                      title="删除"
                    >
                      <Trash2 size={20} />
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </section>
      </div>

      {draft && (
        <WebhookModal
          draft={draft}
          saving={saving}
          eventFilter={eventFilter}
          eventOptions={eventOptions}
          filteredEventOptions={filteredEventOptions}
          onChangeDraft={setDraft}
          onChangeEventFilter={setEventFilter}
          onToggleEvent={toggleDraftEvent}
          onClose={closeModal}
          onSave={saveDraft}
        />
      )}

      {deleteTarget && (
        <div className="fixed inset-0 z-[250] flex items-center justify-center p-4">
          <div className="absolute inset-0 bg-slate-900/60 backdrop-blur-sm" onClick={() => setDeleteTarget(null)} />
          <div className="relative w-full max-w-sm bg-white dark:bg-slate-900 rounded-3xl shadow-2xl p-8 animate-in zoom-in-95 duration-200 text-center">
            <div className="w-16 h-16 bg-red-50 dark:bg-red-900/20 text-red-500 rounded-full flex items-center justify-center mx-auto mb-4">
              <Trash2 size={30} />
            </div>
            <h3 className="text-xl font-bold dark:text-white mb-2">确认删除？</h3>
            <p className="text-slate-500 text-sm mb-8">将删除「{deleteTarget.name}」。</p>
            <div className="flex gap-3">
              <button
                onClick={() => setDeleteTarget(null)}
                className="flex-1 py-3 font-bold text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-xl transition-all"
              >
                取消
              </button>
              <button
                onClick={deleteWebhook}
                className="flex-1 py-3 bg-red-500 hover:bg-red-600 text-white font-bold rounded-xl shadow-lg shadow-red-500/30 transition-all"
              >
                删除
              </button>
            </div>
          </div>
        </div>
      )}

      <div
        className="shrink-0 transition-all duration-300"
        style={{ height: currentChapter ? 'var(--safe-bottom-with-player)' : 'var(--safe-bottom-base)' }}
      />
    </div>
  );
};

const StatCard = ({
  label,
  value,
  icon,
}: {
  label: string;
  value: number;
  icon: React.ReactNode;
}) => (
  <div className="rounded-3xl bg-white dark:bg-slate-900 border border-slate-100 dark:border-slate-800 p-4 shadow-sm">
    <div className="flex items-center justify-between gap-3">
      <div>
        <p className="text-2xl font-black text-slate-900 dark:text-white">{value}</p>
        <p className="text-xs font-bold text-slate-500 mt-1">{label}</p>
      </div>
      <div className="w-11 h-11 rounded-2xl bg-primary-50 dark:bg-primary-900/20 text-primary-600 flex items-center justify-center">
        {icon}
      </div>
    </div>
  </div>
);

const WebhookModal = ({
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
}: {
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
}) => {
  const selectCommonEvents = () => {
    const common = ['user.login', 'playback.play', 'library.scan_completed']
      .filter((eventId) => eventOptions.some((event) => event.id === eventId));
    onChangeDraft({ ...draft, events: common });
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
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <label className="space-y-2">
              <span className="text-sm font-bold text-slate-600 dark:text-slate-400">名称</span>
              <input
                value={draft.name}
                onChange={(event) => onChangeDraft({ ...draft, name: event.target.value })}
                placeholder="企业微信通知"
                className="w-full px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                autoFocus
              />
            </label>

            <label className="space-y-2">
              <span className="text-sm font-bold text-slate-600 dark:text-slate-400">密钥</span>
              <div className="relative">
                <KeyRound size={17} className="absolute left-4 top-1/2 -translate-y-1/2 text-slate-400" />
                <input
                  value={draft.secret || ''}
                  onChange={(event) => onChangeDraft({ ...draft, secret: event.target.value })}
                  placeholder="可选"
                  className="w-full pl-11 pr-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                />
              </div>
            </label>
          </div>

          <label className="space-y-2 block">
            <span className="text-sm font-bold text-slate-600 dark:text-slate-400">Webhook URL</span>
            <div className="relative">
              <Link2 size={17} className="absolute left-4 top-1/2 -translate-y-1/2 text-slate-400" />
              <input
                value={draft.url}
                onChange={(event) => onChangeDraft({ ...draft, url: event.target.value })}
                placeholder="https://example.com/webhook"
                className="w-full pl-11 pr-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
              />
            </div>
          </label>

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

export default NotificationSettingsPage;
