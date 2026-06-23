/* eslint-disable react-refresh/only-export-components */
import React from 'react';
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
    name: '原始事件 JSON',
    urlPlaceholder: 'https://example.com/webhook',
    headers: { 'Content-Type': 'application/json' },
    bodyTemplate: '{{json:payload}}',
  },
  {
    id: 'wecom-markdown',
    name: '企业微信 Markdown',
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
    name: '企业微信文本',
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
    name: '纯文本',
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
  { id: 'user.login', label: '用户登录', description: '用户成功登录系统' },
  { id: 'playback.play', label: '播放', description: '用户开始播放作品或章节' },
  { id: 'library.created', label: '新增媒体库', description: '管理员创建媒体库' },
  { id: 'library.deleted', label: '删除媒体库', description: '管理员删除媒体库' },
  { id: 'book.created', label: '作品入库', description: '作品被创建或入库' },
  { id: 'book.deleted', label: '删除作品', description: '作品被删除' },
  { id: 'library.scan_completed', label: '扫描完成', description: '媒体库扫描任务完成' },
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
