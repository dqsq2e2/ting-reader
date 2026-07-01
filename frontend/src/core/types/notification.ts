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
  body_template: string;
  created_at: string;
  updated_at: string;
}
