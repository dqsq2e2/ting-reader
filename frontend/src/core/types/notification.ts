export interface NotificationEventOption {
  id: string;
  label: string;
  description: string;
}

export interface NotificationWebhook {
  id: string;
  name: string;
  url: string;
  enabled: boolean;
  events: string[];
  secret?: string;
  headers: Record<string, string>;
  bodyTemplate: string;
  createdAt: string;
  updatedAt: string;
}