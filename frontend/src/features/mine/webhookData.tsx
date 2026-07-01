/* eslint-disable react-refresh/only-export-components */
import React from 'react';
import type { TFunction } from 'i18next';
import type { NotificationEventOption } from '../../core/types';

interface StatCardProps {
  label: string;
  value: number;
  icon: React.ReactNode;
}

export const StatCard: React.FC<StatCardProps> = ({ label, value, icon }) => (
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

export type WebhookHeader = {
  id: string;
  key: string;
  value: string;
};

export type WebhookDraft = {
  id?: string;
  name: string;
  nameKey?: string;
  url: string;
  enabled: boolean;
  events: string[];
  secret?: string;
  headers: WebhookHeader[];
  bodyTemplate: string;
};

export type WebhookPreset = {
  id: string;
  name: string;
  nameKey?: string;
  urlPlaceholder: string;
  headers: Record<string, string>;
  bodyTemplate: string;
};

export const createHeader = (key = '', value = ''): WebhookHeader => ({
  id: `${Date.now()}-${Math.random().toString(36).slice(2)}`,
  key,
  value,
});

export const WEBHOOK_PRESETS: WebhookPreset[] = [
  {
    id: 'ting-json',
    name: 'Raw Event JSON',
    nameKey: 'notifications.presets.rawJson',
    urlPlaceholder: 'https://example.com/webhook',
    headers: { 'Content-Type': 'application/json' },
    bodyTemplate: '{{json:payload}}',
  },
  {
    id: 'wecom-markdown',
    name: 'WeCom Markdown',
    nameKey: 'notifications.presets.wecomMarkdown',
    urlPlaceholder: 'https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=...',
    headers: { 'Content-Type': 'application/json' },
    bodyTemplate: `{
  "msgtype": "markdown",
  "markdown": {
    "content": {{json:notification}}
  }
}`,
  },
  {
    id: 'wecom-text',
    name: 'WeCom Text',
    nameKey: 'notifications.presets.wecomText',
    urlPlaceholder: 'https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=...',
    headers: { 'Content-Type': 'application/json' },
    bodyTemplate: `{
  "msgtype": "text",
  "text": {
    "content": {{json:notification}}
  }
}`,
  },
  {
    id: 'ntfy-json',
    name: 'ntfy JSON',
    urlPlaceholder: 'https://ntfy.example.com',
    headers: { 'Content-Type': 'application/json' },
    bodyTemplate: `{
  "topic": "ting-reader",
  "title": {{json:title}},
  "message": {{json:message}},
  "priority": 3,
  "tags": ["headphones"]
}`,
  },
  {
    id: 'gotify-json',
    name: 'Gotify JSON',
    urlPlaceholder: 'https://gotify.example.com/message?token=...',
    headers: { 'Content-Type': 'application/json' },
    bodyTemplate: `{
  "title": {{json:title}},
  "message": {{json:message}},
  "priority": 5
}`,
  },
  {
    id: 'plain-text',
    name: 'Plain Text',
    nameKey: 'notifications.presets.plainText',
    urlPlaceholder: 'https://example.com/webhook',
    headers: { 'Content-Type': 'text/plain; charset=utf-8' },
    bodyTemplate: '{{notification}}',
  },
];

export const headersToDraft = (headers: Record<string, string> = {}): WebhookHeader[] =>
  Object.entries(headers).map(([key, value]) => createHeader(key, value));

export const draftHeadersToRecord = (headers: WebhookHeader[]): Record<string, string> =>
  Object.fromEntries(
    headers
      .map((header) => [header.key.trim(), header.value.trim()])
      .filter(([key]) => Boolean(key))
  );

export const fallbackEvents: NotificationEventOption[] = [
  { id: 'user.login', label: 'User Login', description: 'A user successfully logged in' },
  { id: 'playback.play', label: 'Playback', description: 'A user started playing a work or chapter' },
  { id: 'library.created', label: 'Library Created', description: 'An admin created a library' },
  { id: 'library.deleted', label: 'Library Deleted', description: 'An admin deleted a library' },
  { id: 'book.created', label: 'Book Imported', description: 'A work was created or imported' },
  { id: 'book.deleted', label: 'Book Deleted', description: 'A work was deleted' },
  { id: 'library.scan_completed', label: 'Scan Completed', description: 'A library scan completed' },
];

export const createEmptyDraft = (defaultEvent = 'user.login'): WebhookDraft => ({
  name: '',
  url: '',
  enabled: true,
  events: [defaultEvent],
  secret: '',
  headers: headersToDraft({ 'Content-Type': 'application/json' }),
  bodyTemplate: '{{json:payload}}',
});

export const notificationEventKeyById: Record<string, string> = {
  'user.login': 'userLogin',
  'playback.play': 'playbackPlay',
  'library.created': 'libraryCreated',
  'library.deleted': 'libraryDeleted',
  'book.created': 'bookCreated',
  'book.deleted': 'bookDeleted',
  'library.scan_completed': 'libraryScanCompleted',
};

export const translateEventOption = (event: NotificationEventOption, t: TFunction): NotificationEventOption => {
  const key = notificationEventKeyById[event.id];
  if (!key) return event;
  return {
    ...event,
    label: t(`notifications.events.${key}.label`),
    description: t(`notifications.events.${key}.description`),
  };
};

export const translatePresetName = (preset: WebhookPreset, t: TFunction) => (
  preset.nameKey ? t(preset.nameKey) : preset.name
);