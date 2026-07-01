import React, { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import apiClient from "../../core/api/client";
import BackButton from "../../shared/widgets/BackButton";
import { usePlayerStore } from "../../core/stores/playerStore";
import type {
  NotificationEventOption,
  NotificationWebhook,
} from "../../core/types";
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
} from "lucide-react";

import {
  StatCard,
  type WebhookDraft,
  createEmptyDraft,
  draftHeadersToRecord,
  fallbackEvents,
  headersToDraft,
  translateEventOption,
} from "./webhookData";
import WebhookModal from "./WebhookModal";

const NotificationSettingsPage: React.FC = () => {
  const { t } = useTranslation();
  const currentChapter = usePlayerStore((state) => state.currentChapter);
  const [webhooks, setWebhooks] = useState<NotificationWebhook[]>([]);
  const [eventOptions, setEventOptions] =
    useState<NotificationEventOption[]>(fallbackEvents);
  const [draft, setDraft] = useState<WebhookDraft | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<NotificationWebhook | null>(
    null,
  );
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [eventFilter, setEventFilter] = useState("");

  const fetchData = async () => {
    setLoading(true);
    try {
      const [hooksRes, eventsRes] = await Promise.all([
        apiClient.get("/api/system/notifications"),
        apiClient.get("/api/system/notifications/events"),
      ]);
      setWebhooks(hooksRes.data || []);
      setEventOptions(eventsRes.data?.length ? eventsRes.data : fallbackEvents);
    } catch (error) {
      console.error("Failed to load notification settings", error);
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
    () =>
      new Set(webhooks.flatMap((item) => (item.enabled ? item.events : [])))
        .size,
    [webhooks],
  );

  const filteredEventOptions = useMemo(() => {
    const keyword = eventFilter.trim().toLowerCase();
    if (!keyword) return eventOptions;
    return eventOptions.filter((event) =>
      `${translateEventOption(event, t).label} ${translateEventOption(event, t).description} ${event.label} ${event.id} ${event.description}`
        .toLowerCase()
        .includes(keyword),
    );
  }, [eventFilter, eventOptions, t]);

  const eventLabel = (eventId: string) =>
    eventOptions.find((event) => event.id === eventId)
      ? translateEventOption(
          eventOptions.find((event) => event.id === eventId)!,
          t,
        ).label
      : eventId;

  const startCreate = () => {
    setEventFilter("");
    setDraft(createEmptyDraft(eventOptions[0]?.id || "user.login"));
  };

  const startEdit = (webhook: NotificationWebhook) => {
    setEventFilter("");
    setDraft({
      id: webhook.id,
      name: webhook.name,
      url: webhook.url,
      enabled: webhook.enabled,
      events: webhook.events,
      secret: webhook.secret || "",
      headers: headersToDraft(webhook.headers),
      bodyTemplate: webhook.body_template || "{{json:payload}}",
    });
  };

  const closeModal = () => {
    setDraft(null);
    setEventFilter("");
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
        body_template: webhook.body_template,
      });
      setWebhooks((items) =>
        items.map((item) =>
          item.id === webhook.id ? { ...item, enabled: nextEnabled } : item,
        ),
      );
      if (draft?.id === webhook.id) {
        setDraft({ ...draft, enabled: nextEnabled });
      }
    } catch (error) {
      console.error("Failed to update notification status", error);
      alert(t("notifications.updateStatusRetry"));
    }
  };

  const saveDraft = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!draft || saving) return;
    if (!draft.name.trim()) {
      alert(t("notifications.nameRequired"));
      return;
    }
    if (!draft.url.trim()) {
      alert(t("notifications.urlRequired"));
      return;
    }
    if (draft.events.length === 0) {
      alert(t("notifications.eventRequired"));
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
        body_template: draft.bodyTemplate,
      };
      if (draft.id) {
        await apiClient.put(`/api/system/notifications/${draft.id}`, payload);
      } else {
        await apiClient.post("/api/system/notifications", payload);
      }
      closeModal();
      setSaved(true);
      setTimeout(() => setSaved(false), 1600);
      await fetchData();
    } catch (error) {
      console.error("Failed to save notification config", error);
      alert(t("notifications.saveRetry"));
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
      console.error("Failed to delete notification config", error);
      alert(t("notifications.deleteRetry"));
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
              {t("notifications.title")}
            </h1>
            <p className="text-sm md:text-base text-slate-500 dark:text-slate-400 mt-1">
              {t("notifications.subtitle")}
            </p>
          </div>
          {saved && (
            <span className="text-sm text-green-600 font-bold flex items-center gap-1">
              <CheckCircle2 size={15} />
              {t("common.saved")}
            </span>
          )}
        </div>

        <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
          <StatCard
            label="Webhook"
            value={webhooks.length}
            icon={<Webhook size={20} />}
          />
          <StatCard
            label={t("notifications.enabled")}
            value={enabledCount}
            icon={<Power size={20} />}
          />
          <StatCard
            label={t("notifications.disabled")}
            value={disabledCount}
            icon={<Radio size={20} />}
          />
          <StatCard
            label={t("notifications.listenedEvents")}
            value={listenedEvents}
            icon={<BellRing size={20} />}
          />
        </div>

        <section className="bg-white dark:bg-slate-900 rounded-3xl border border-slate-100 dark:border-slate-800 shadow-sm overflow-hidden">
          <div className="px-5 md:px-6 py-5 border-b border-slate-100 dark:border-slate-800 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
            <div>
              <h2 className="text-lg font-bold text-slate-900 dark:text-white">
                {t("notifications.webhookList")}
              </h2>
              <p className="text-xs text-slate-500 mt-1">
                {t("notifications.configCount", { count: webhooks.length })}
              </p>
            </div>
            {!loading && (
              <button
                onClick={startCreate}
                className="inline-flex items-center justify-center gap-2 px-4 py-2.5 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-xl shadow-lg shadow-primary-500/25 transition-all"
              >
                <Plus size={18} />
                {t("notifications.addWebhook")}
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
              <p className="font-bold text-slate-900 dark:text-white">
                {t("notifications.noWebhook")}
              </p>
              <p className="text-sm text-slate-500 mt-2">
                {t("notifications.noWebhookHint")}
              </p>
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
                          ? "bg-emerald-50 text-emerald-600 dark:bg-emerald-900/20"
                          : "bg-slate-100 text-slate-400 dark:bg-slate-800"
                      }`}
                    >
                      <Webhook size={22} />
                    </div>
                    <div className="min-w-0">
                      <div className="flex flex-wrap items-center gap-2">
                        <h3 className="font-bold text-slate-900 dark:text-white truncate">
                          {webhook.name}
                        </h3>
                        <span
                          className={`px-2 py-0.5 rounded-lg text-[11px] font-black ${
                            webhook.enabled
                              ? "bg-emerald-50 text-emerald-600 dark:bg-emerald-900/20"
                              : "bg-slate-100 text-slate-400 dark:bg-slate-800"
                          }`}
                        >
                          {webhook.enabled
                            ? t("notifications.enabled")
                            : t("notifications.disabled")}
                        </span>
                      </div>
                      <p className="text-sm text-slate-500 mt-1 truncate max-w-3xl">
                        {webhook.url}
                      </p>
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
                          ? "bg-emerald-50 text-emerald-600 hover:bg-emerald-100 dark:bg-emerald-900/20"
                          : "bg-slate-100 text-slate-500 hover:bg-slate-200 dark:bg-slate-800 dark:text-slate-300"
                      }`}
                    >
                      <Power size={16} />
                      {webhook.enabled
                        ? t("notifications.turnOff")
                        : t("notifications.turnOn")}
                    </button>
                    <button
                      onClick={() => startEdit(webhook)}
                      className="p-2.5 rounded-xl text-slate-400 hover:text-primary-600 hover:bg-primary-50 dark:hover:bg-primary-900/20 transition-all"
                      title={t("notifications.edit")}
                    >
                      <Edit size={20} />
                    </button>
                    <button
                      onClick={() => setDeleteTarget(webhook)}
                      className="p-2.5 rounded-xl text-slate-400 hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 transition-all"
                      title={t("notifications.delete")}
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
          <div
            className="absolute inset-0 bg-slate-900/60 backdrop-blur-sm"
            onClick={() => setDeleteTarget(null)}
          />
          <div className="relative w-full max-w-sm bg-white dark:bg-slate-900 rounded-3xl shadow-2xl p-8 animate-in zoom-in-95 duration-200 text-center">
            <div className="w-16 h-16 bg-red-50 dark:bg-red-900/20 text-red-500 rounded-full flex items-center justify-center mx-auto mb-4">
              <Trash2 size={30} />
            </div>
            <h3 className="text-xl font-bold dark:text-white mb-2">
              {t("notifications.deleteTitle")}
            </h3>
            <p className="text-slate-500 text-sm mb-8">
              {t("notifications.deleteMessage", { name: deleteTarget.name })}
            </p>
            <div className="flex gap-3">
              <button
                onClick={() => setDeleteTarget(null)}
                className="flex-1 py-3 font-bold text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-xl transition-all"
              >
                {t("common.cancel")}
              </button>
              <button
                onClick={deleteWebhook}
                className="flex-1 py-3 bg-red-500 hover:bg-red-600 text-white font-bold rounded-xl shadow-lg shadow-red-500/30 transition-all"
              >
                {t("notifications.delete")}
              </button>
            </div>
          </div>
        </div>
      )}

      <div
        className="shrink-0 transition-all duration-300"
        style={{
          height: currentChapter
            ? "var(--safe-bottom-with-player)"
            : "var(--safe-bottom-base)",
        }}
      />
    </div>
  );
};

export default NotificationSettingsPage;
