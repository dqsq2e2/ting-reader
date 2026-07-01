import type {
  CapabilityRegistrationLike,
  ClientExtensionDescriptor,
  ClientExtensionRegistrySnapshot,
  ClientExtensionRenderMode,
  ClientExtensionSlot,
  UiExtensionCapabilityExtra,
  UiExtensionRenderConfig,
} from "./types";

const defaultSlots: ClientExtensionSlot[] = ["global.panel"];

const isClientExtensionSlot = (value: unknown): value is ClientExtensionSlot =>
  typeof value === "string" &&
  [
    "global.floating_action",
    "global.panel",
    "settings.section",
    "book.detail_action",
    "reader.toolbar_action",
    "reader.side_panel",
    "reader.document_viewer",
  ].includes(value);

const isRenderMode = (value: unknown): value is ClientExtensionRenderMode =>
  typeof value === "string" &&
  ["schema", "builtin", "web_container", "action"].includes(value);

const capabilityExtra = (
  registration: CapabilityRegistrationLike,
): UiExtensionCapabilityExtra =>
  registration.capability as UiExtensionCapabilityExtra;

const renderConfig = (
  extra: UiExtensionCapabilityExtra,
): UiExtensionRenderConfig | undefined =>
  typeof extra.render === "object" && extra.render !== null
    ? extra.render
    : undefined;

const normalizeSlots = (extra: UiExtensionCapabilityExtra) => {
  const declaredSlots = [
    ...(Array.isArray(extra.slots) ? extra.slots : []),
    extra.slot,
  ];
  const slots = declaredSlots.filter(isClientExtensionSlot);
  return slots.length > 0 ? slots : defaultSlots;
};

const normalizeContexts = (extra: UiExtensionCapabilityExtra) =>
  Array.isArray(extra.contexts || extra.context)
    ? (extra.contexts || extra.context || []).filter(
        (context): context is string => typeof context === "string",
      )
    : [];

const localizedText = (value: unknown): string | undefined => {
  if (typeof value === "string") {
    const trimmed = value.trim();
    return trimmed || undefined;
  }
  if (!value || typeof value !== "object") return undefined;

  const record = value as Record<string, unknown>;
  const candidates = [
    record["zh-CN"],
    record.zh,
    record["en-US"],
    record.en,
    ...Object.values(record),
  ];
  for (const candidate of candidates) {
    if (typeof candidate === "string" && candidate.trim()) {
      return candidate.trim();
    }
  }
  return undefined;
};

export const createClientExtensionDescriptor = (
  registration: CapabilityRegistrationLike,
  slot: ClientExtensionSlot,
): ClientExtensionDescriptor => {
  const extra = capabilityExtra(registration);
  const render = renderConfig(extra);
  const renderMode = isRenderMode(extra.render_mode)
    ? extra.render_mode
    : isRenderMode(extra.render)
      ? extra.render
      : isRenderMode(render?.mode)
        ? render.mode
        : "action";

  return {
    id: `${registration.plugin_id}:${registration.capability.id}:${slot}`,
    pluginId: registration.plugin_id,
    pluginName: registration.plugin_name,
    slot,
    renderMode,
    render,
    title: localizedText(extra.title) || localizedText(extra.label),
    icon: extra.icon,
    capability: registration.capability,
    priority: typeof extra.priority === "number" ? extra.priority : 100,
    contexts: normalizeContexts(extra),
  };
};

export const buildClientExtensionRegistry = (
  registrations: CapabilityRegistrationLike[],
): ClientExtensionRegistrySnapshot => {
  const extensions = registrations
    .filter(
      (registration) =>
        registration.capability.kind === "ui_extension" ||
        registration.capability.kind === "client_extension",
    )
    .flatMap((registration) =>
      normalizeSlots(capabilityExtra(registration)).map((slot) =>
        createClientExtensionDescriptor(registration, slot),
      ),
    )
    .sort(
      (left, right) =>
        left.priority - right.priority || left.id.localeCompare(right.id),
    );

  const bySlot: ClientExtensionRegistrySnapshot["bySlot"] = {};
  for (const extension of extensions) {
    bySlot[extension.slot] = [...(bySlot[extension.slot] || []), extension];
  }

  return { extensions, bySlot };
};
