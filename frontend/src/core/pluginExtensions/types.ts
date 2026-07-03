import type { PluginCapability, PluginCapabilityRegistration } from "../types";

export type ClientExtensionSlot =
  | "global.floating_action"
  | "global.panel"
  | "settings.section"
  | "book.detail_action"
  | "reader.toolbar_action"
  | "reader.side_panel"
  | "reader.document_viewer";

export type ClientExtensionRenderMode =
  "schema" | "builtin" | "web_container" | "action";

export type ClientExtensionIcon =
  | string
  | {
      type?: "lucide" | "emoji" | "image" | "url";
      name?: string;
      value?: string;
      src?: string;
      alt?: string;
    };

export type ClientExtensionDescriptor = {
  id: string;
  pluginId: string;
  pluginName: string;
  slot: ClientExtensionSlot;
  renderMode: ClientExtensionRenderMode;
  render?: UiExtensionRenderConfig;
  title?: string;
  icon?: ClientExtensionIcon;
  capability: PluginCapability;
  priority: number;
  contexts: string[];
};

export type ClientExtensionRegistrySnapshot = {
  extensions: ClientExtensionDescriptor[];
  bySlot: Partial<Record<ClientExtensionSlot, ClientExtensionDescriptor[]>>;
};

export type UiExtensionCapabilityExtra = {
  slot?: ClientExtensionSlot;
  slots?: ClientExtensionSlot[];
  render?: ClientExtensionRenderMode | UiExtensionRenderConfig;
  render_mode?: ClientExtensionRenderMode;
  title?: string;
  label?: string;
  icon?: ClientExtensionIcon;
  priority?: number;
  contexts?: string[];
  context?: string[];
};

export type UiExtensionRenderConfig = {
  mode?: ClientExtensionRenderMode;
  entry?: string;
  invoke?: string;
  schema?: {
    fields?: Array<{
      name: string;
      label?: string;
      type?: "text" | "textarea" | "number" | "boolean" | "select";
      placeholder?: string;
      required?: boolean;
      default?: unknown;
      options?: Array<string | { label?: string; value?: unknown }>;
    }>;
  };
  builtin?: {
    component?: "host_method" | "capability_result" | "document_reader";
    method?: string;
    params?: Record<string, unknown>;
    auto_run?: boolean;
    submit_label?: string;
  };
  component?: "host_method" | "capability_result" | "document_reader";
  method?: string;
  params?: Record<string, unknown>;
  auto_run?: boolean;
  submit_label?: string;
  panel?: {
    type?: string;
    width?: number;
    mobile_type?: string;
  };
};

export type CapabilityRegistrationLike = Pick<
  PluginCapabilityRegistration,
  "plugin_id" | "plugin_name" | "capability"
>;
