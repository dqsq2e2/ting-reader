import React, { useEffect, useMemo, useState } from 'react';
import apiClient from '../../core/api/client';
import BackButton from '../../shared/widgets/BackButton';
import { usePlayerStore } from '../../core/stores/playerStore';
import type { NotificationEventOption, NotificationWebhook } from '../../core/types';
import {
  BellRing,
  CheckCircle2,
  Edit,
  Loader2,
  Plus,
  Power,
  Radio,
  Trash2,
  Webhook,
} from 'lucide-react';

import {
  StatCard,
  type WebhookDraft,
  createEmptyDraft,
  draftHeadersToRecord,
  fallbackEvents,
  headersToDraft,
} from './webhookData';
import WebhookModal from './WebhookModal';

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
      headers: headersToDraft(webhook.headers),
      bodyTemplate: webhook.bodyTemplate || '{{json:payload}}',
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
        headers: webhook.headers,
        bodyTemplate: webhook.bodyTemplate,
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
        headers: draftHeadersToRecord(draft.headers),
        bodyTemplate: draft.bodyTemplate,
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


export default NotificationSettingsPage;
