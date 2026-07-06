/* eslint-disable react-refresh/only-export-components */
import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type {
  Plugin,
  PluginCapability,
  PluginDependency,
  StorePlugin,
} from "../../core/types";
import {
  AlertCircle,
  CheckCircle,
  Cpu,
  Download,
  FileText,
  ListChecks,
  Package,
  Puzzle,
  RefreshCw,
  Search,
  Settings,
  Shield,
  Tag,
  Trash2,
  XCircle,
} from "lucide-react";
import GithubIcon from "../../shared/ui/GithubIcon";

const PluginName = ({
  name,
  className = "",
}: {
  name: string;
  className?: string;
}) => {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  const [isOverflowing, setIsOverflowing] = useState(false);
  const ref = useRef<HTMLHeadingElement>(null);

  useEffect(() => {
    const checkOverflow = () => {
      if (ref.current && !expanded) {
        setIsOverflowing(ref.current.scrollWidth > ref.current.clientWidth);
      }
    };

    checkOverflow();

    const observer = new ResizeObserver(checkOverflow);
    if (ref.current) {
      observer.observe(ref.current);
    }

    return () => observer.disconnect();
  }, [name, expanded]);

  const handleClick = () => {
    if (expanded || isOverflowing) {
      setExpanded(!expanded);
    }
  };

  return (
    <h3
      ref={ref}
      className={`${className} ${expanded ? "break-words" : "truncate"} ${expanded || isOverflowing ? "cursor-pointer" : ""}`}
      onClick={handleClick}
      title={
        expanded || isOverflowing
          ? expanded
            ? t("adminPlugins.collapseName")
            : t("adminPlugins.expandName")
          : undefined
      }
    >
      {name}
    </h3>
  );
};

type PluginCardData = {
  id: string;
  baseId: string;
  name: string;
  description: string;
  description_i18n?: Record<string, string | undefined>;
  long_description?: string;
  version: string;
  installedVersion?: string | null;
  plugin_type: string;
  runtime?: string;
  author?: string;
  license?: string;
  repo?: string;
  min_core_version?: string;
  min_flutter_version?: string;
  admin_only?: boolean;
  dependencies?: string[];
  permissions?: string[];
  config_schema?: Record<string, unknown>;
  supported_extensions?: string[];
  capabilities?: PluginCapability[];
  state?: Plugin["state"];
  error?: string;
  isInstalled?: boolean;
  hasUpdate?: boolean;
};

type PluginCardProps = {
  data: PluginCardData;
  expanded: boolean;
  installing?: boolean;
  onToggleDescription: (id: string) => void;
  onInstall?: () => void;
  onReload?: () => void;
  onUninstall?: () => void;
  onConfigure?: () => void;
};

const cleanText = (value?: string | null) => {
  const text = value?.trim();
  return text || undefined;
};

const isEnglishLanguage = (language?: string) =>
  language?.toLowerCase().startsWith("en") ?? false;

const normalizeLocaleKey = (language?: string) => {
  if (!language) return undefined;
  const camelRegion = language.match(/^([a-z]{2,3})([A-Z][a-z0-9]{1,7})$/);
  if (camelRegion) {
    return `${camelRegion[1]}-${camelRegion[2]}`.toLowerCase();
  }
  return language.replace("_", "-").toLowerCase();
};

const localizedRecordText = (
  record?: Record<string, string | undefined>,
  language?: string,
) => {
  if (!record) return undefined;

  const normalized = normalizeLocaleKey(language);
  const base = normalized?.split("-")[0];
  const candidates = [
    normalized,
    base,
    isEnglishLanguage(language) ? "en" : "zh",
    isEnglishLanguage(language) ? "zh" : "en",
  ].filter(Boolean) as string[];

  for (const candidate of candidates) {
    const direct = cleanText(record[candidate]);
    if (direct) return direct;

    const matchedKey = Object.keys(record).find(
      (key) => normalizeLocaleKey(key) === candidate,
    );
    const matched = cleanText(matchedKey ? record[matchedKey] : undefined);
    if (matched) return matched;
  }

  return Object.values(record).find((value) => cleanText(value));
};

const getLocalizedPluginDescription = (
  data: Pick<
    PluginCardData,
    "description" | "description_i18n" | "long_description"
  >,
  language?: string,
) => {
  const declared = localizedRecordText(data.description_i18n, language);
  if (declared) return declared;

  return cleanText(data.long_description) || cleanText(data.description);
};

const runtimeLabels: Record<string, string> = {
  wasm: "WASM",
  javascript: "JavaScript",
  native: "Native",
};

const typeStyles: Record<string, { icon: string; chip: string }> = {
  scraper: {
    icon: "border-blue-100 bg-blue-50 text-blue-700 dark:border-blue-900/50 dark:bg-blue-950/40 dark:text-blue-300",
    chip: "border-blue-100 bg-blue-50 text-blue-700 dark:border-blue-900/50 dark:bg-blue-950/40 dark:text-blue-300",
  },
  format: {
    icon: "border-cyan-100 bg-cyan-50 text-cyan-700 dark:border-cyan-900/50 dark:bg-cyan-950/40 dark:text-cyan-300",
    chip: "border-cyan-100 bg-cyan-50 text-cyan-700 dark:border-cyan-900/50 dark:bg-cyan-950/40 dark:text-cyan-300",
  },
  utility: {
    icon: "border-emerald-100 bg-emerald-50 text-emerald-700 dark:border-emerald-900/50 dark:bg-emerald-950/40 dark:text-emerald-300",
    chip: "border-emerald-100 bg-emerald-50 text-emerald-700 dark:border-emerald-900/50 dark:bg-emerald-950/40 dark:text-emerald-300",
  },
};

const getBasePluginId = (id: string) => id.split("@")[0];

const formatVersion = (
  version: string | null | undefined,
  t: ReturnType<typeof useTranslation>["t"],
) => {
  if (!version) return t("adminPlugins.unknownVersion");
  return version.startsWith("v") ? version : `v${version}`;
};

const getRuntimeLabel = (runtime?: string) =>
  runtimeLabels[runtime || ""] || runtime || "unknown";

const getPluginCategory = (capabilities?: PluginCapability[]) => {
  const kinds = new Set((capabilities || []).map((capability) => capability.kind));
  if (kinds.has("metadata_provider")) return "scraper";
  if (kinds.has("format_handler") || kinds.has("content_processor")) {
    return "format";
  }
  return "utility";
};

const capabilityObject = (value: unknown) =>
  value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;

const getCapabilityArray = (capability: PluginCapability, key: string) => {
  const metadata = capabilityObject(capability.metadata);
  const value = Array.isArray(capability[key])
    ? capability[key]
    : metadata?.[key];
  return Array.isArray(value) ? value : [];
};

const getMetadataSearchFieldCount = (capabilities?: PluginCapability[]) =>
  (capabilities || [])
    .filter((capability) => capability.kind === "metadata_provider")
    .reduce(
      (count, capability) =>
        count + getCapabilityArray(capability, "search_fields").length,
      0,
    );

const getMetadataResultFieldCount = (capabilities?: PluginCapability[]) =>
  (capabilities || [])
    .filter((capability) => capability.kind === "metadata_provider")
    .reduce(
      (count, capability) =>
        count + getCapabilityArray(capability, "result_fields").length,
      0,
    );

const getCapabilityExtensions = (capability: PluginCapability) => {
  const direct = capability.extensions;
  const matches = capability.matches;
  const nested =
    matches && typeof matches === "object" && !Array.isArray(matches)
      ? (matches as { extensions?: unknown }).extensions
      : undefined;
  const value = Array.isArray(direct) ? direct : nested;
  if (!Array.isArray(value)) return [];
  return value
    .filter((item): item is string => typeof item === "string")
    .map((extension) => extension.trim().replace(/^\./, "").toLowerCase())
    .filter(Boolean);
};

const getCapabilitySupportedExtensions = (
  capabilities?: PluginCapability[],
) => {
  const extensions: string[] = [];
  for (const capability of capabilities || []) {
    if (
      capability.kind === "format_handler" ||
      capability.kind === "content_processor"
    ) {
      for (const extension of getCapabilityExtensions(capability)) {
        if (!extensions.includes(extension)) extensions.push(extension);
      }
    }
  }
  return extensions;
};

const getPluginTypeLabel = (
  type: string,
  t: ReturnType<typeof useTranslation>["t"],
) => {
  if (type === "scraper") return t("adminPlugins.metadata");
  if (type === "format") return t("adminPlugins.format");
  if (type === "utility") return t("adminPlugins.utility");
  return type || t("adminPlugins.unknownType");
};

const getSupportLabel = (support: string) => {
  return support;
};

const capabilityRenderMode = (capability: PluginCapability) => {
  const render = capability.render;
  if (typeof capability.render_mode === "string") return capability.render_mode;
  if (typeof render === "string") return render;
  if (render && typeof render === "object" && "mode" in render) {
    const mode = (render as { mode?: unknown }).mode;
    return typeof mode === "string" ? mode : undefined;
  }
  return undefined;
};

const capabilityRouteAuth = (capability: PluginCapability) => {
  const route = capability.route;
  if (route && typeof route === "object" && "auth" in route) {
    const auth = (route as { auth?: unknown }).auth;
    return typeof auth === "string" ? auth : undefined;
  }
  return typeof capability.auth === "string" ? capability.auth : undefined;
};

const normalizePermission = (permission: string) =>
  permission
    .replace(/\(.+\)$/, "")
    .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
    .toLowerCase();

const getPluginSignals = (
  data: Pick<PluginCardData, "runtime" | "permissions" | "capabilities">,
) => {
  const permissions = (data.permissions || []).map(normalizePermission);
  const capabilities = data.capabilities || [];
  const signals = new Set<string>();

  if (
    capabilities.some(
      (capability) =>
        capability.kind === "ui_extension" &&
        capabilityRenderMode(capability) === "web_container",
    )
  ) {
    signals.add("Web UI");
  }
  if (
    capabilities.some(
      (capability) =>
        capability.kind === "http_route" &&
        ["public", "signed", "public_or_signed"].includes(
          capabilityRouteAuth(capability) || "",
        ),
    )
  ) {
    signals.add("Public HTTP");
  }
  if (
    permissions.some((permission) =>
      ["books_read", "chapters_read", "media_read", "media_read_url"].includes(
        permission,
      ),
    )
  ) {
    signals.add("Library read");
  }
  if (permissions.includes("progress_read")) {
    signals.add("Playback progress");
  }
  if (permissions.includes("cache_read") || permissions.includes("cache_write")) {
    signals.add("Cache access");
  }
  if (permissions.some((permission) => permission.endsWith("_write"))) {
    signals.add("Write access");
  }
  if (permissions.includes("task_create")) {
    signals.add("Task create");
  }
  if (permissions.some((permission) => permission.startsWith("network"))) {
    signals.add("Network");
  }

  return Array.from(signals);
};

const normalizeDependencyIds = (
  dependencies?: string[] | PluginDependency[],
) => {
  if (!dependencies) return [];
  return dependencies.map((dependency) =>
    typeof dependency === "string" ? dependency : dependency.plugin_name,
  );
};

const getRepoUrl = (repo: string) =>
  repo.startsWith("http://") || repo.startsWith("https://")
    ? repo
    : `https://github.com/${repo}`;

const getExternalLink = (
  data: PluginCardData,
  t: ReturnType<typeof useTranslation>["t"],
) => {
  if (data.repo) {
    return {
      href: getRepoUrl(data.repo),
      label: t("adminPlugins.repo"),
      title: t("adminPlugins.viewRepo"),
      icon: <GithubIcon size={17} />,
    };
  }

  return null;
};

const getInstalledStoreMeta = (plugin: Plugin, storePlugins: StorePlugin[]) => {
  const baseId = getBasePluginId(plugin.id);
  return storePlugins.find((storePlugin) => storePlugin.id === baseId);
};

const toInstalledCardData = (
  plugin: Plugin,
  storeMeta?: StorePlugin,
) => {
  const capabilities = plugin.capabilities?.length
    ? plugin.capabilities
    : storeMeta?.capabilities || [];
  return {
    id: plugin.id,
    baseId: getBasePluginId(plugin.id),
    name: plugin.name,
    description: storeMeta?.description || plugin.description,
    description_i18n: storeMeta?.description_i18n || plugin.description_i18n,
    long_description: storeMeta?.long_description || plugin.description,
    version: plugin.version,
    plugin_type: getPluginCategory(capabilities),
    runtime: plugin.runtime || storeMeta?.runtime,
    author: plugin.author || storeMeta?.author,
    license: plugin.license || storeMeta?.license,
    repo: plugin.repo || storeMeta?.repo,
    min_core_version: plugin.min_core_version || storeMeta?.min_core_version,
    min_flutter_version: plugin.min_flutter_version || storeMeta?.min_flutter_version,
    admin_only: plugin.admin_only || storeMeta?.admin_only,
    dependencies: normalizeDependencyIds(
      plugin.dependencies || storeMeta?.dependencies,
    ),
    permissions: plugin.permissions || storeMeta?.permissions,
    config_schema: plugin.config_schema || storeMeta?.config_schema,
    supported_extensions: getCapabilitySupportedExtensions(capabilities),
    capabilities,
    state: plugin.state,
    error: plugin.error,
    isInstalled: true,
  };
};

const toStoreCardData = (
  plugin: StorePlugin,
  installedVersion: string | null,
  hasUpdate: boolean,
) => {
  const capabilities = plugin.capabilities || [];
  return {
    id: plugin.id,
    baseId: plugin.id,
    name: plugin.name,
    description: plugin.description,
    description_i18n: plugin.description_i18n,
    long_description: plugin.long_description || plugin.description,
    version: plugin.version,
    installedVersion,
    plugin_type: getPluginCategory(capabilities),
    runtime: plugin.runtime,
    author: plugin.author,
    license: plugin.license,
    repo: plugin.repo,
    min_core_version: plugin.min_core_version,
    min_flutter_version: plugin.min_flutter_version,
    admin_only: plugin.admin_only,
    dependencies: normalizeDependencyIds(plugin.dependencies),
    permissions: plugin.permissions,
    config_schema: plugin.config_schema,
    supported_extensions: getCapabilitySupportedExtensions(capabilities),
    capabilities,
    isInstalled: !!installedVersion,
    hasUpdate,
  };
};

const TypeIcon = ({ type }: { type: string }) => {
  if (type === "format") return <FileText size={19} />;
  if (type === "utility") return <Package size={19} />;
  return <Puzzle size={19} />;
};

const InfoChip = ({
  icon,
  children,
  title,
}: {
  icon?: React.ReactNode;
  children: React.ReactNode;
  title?: string;
}) => (
  <span
    title={title}
    className="inline-flex max-w-full items-center gap-1 rounded-md border border-slate-200 bg-slate-50 px-2 py-1 text-xs font-medium text-slate-600 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-300"
  >
    {icon}
    <span className="truncate">{children}</span>
  </span>
);

const PluginStateBadge = ({ state }: { state?: Plugin["state"] }) => {
  const { t } = useTranslation();
  if (state === "active") {
    return (
      <span className="inline-flex items-center gap-1 rounded-md border border-green-200 bg-green-50 px-2 py-1 text-xs font-semibold text-green-700 dark:border-green-900/40 dark:bg-green-950/40 dark:text-green-300">
        <CheckCircle size={13} /> {t("adminPlugins.active")}
      </span>
    );
  }

  if (state === "failed") {
    return (
      <span className="inline-flex items-center gap-1 rounded-md border border-red-200 bg-red-50 px-2 py-1 text-xs font-semibold text-red-700 dark:border-red-900/40 dark:bg-red-950/40 dark:text-red-300">
        <XCircle size={13} /> {t("adminPlugins.failed")}
      </span>
    );
  }

  return (
    <span className="inline-flex items-center gap-1 rounded-md border border-slate-200 bg-slate-50 px-2 py-1 text-xs font-semibold text-slate-600 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-300">
      <AlertCircle size={13} /> {state || t("adminPlugins.unknownType")}
    </span>
  );
};

const StoreStateBadge = ({
  isInstalled,
  hasUpdate,
}: {
  isInstalled?: boolean;
  hasUpdate?: boolean;
}) => {
  const { t } = useTranslation();
  if (hasUpdate) {
    return (
      <span className="inline-flex items-center gap-1 rounded-md border border-emerald-200 bg-emerald-50 px-2 py-1 text-xs font-semibold text-emerald-700 dark:border-emerald-900/40 dark:bg-emerald-950/40 dark:text-emerald-300">
        {t("adminPlugins.updateAvailable")}
      </span>
    );
  }

  if (isInstalled) {
    return (
      <span className="inline-flex items-center gap-1 rounded-md border border-slate-200 bg-slate-50 px-2 py-1 text-xs font-semibold text-slate-600 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-300">
        {t("adminPlugins.installed")}
      </span>
    );
  }

  return null;
};

const PluginCard = ({
  data,
  expanded,
  installing,
  onToggleDescription,
  onInstall,
  onReload,
  onUninstall,
  onConfigure,
}: PluginCardProps) => {
  const { t, i18n } = useTranslation();
  const description =
    getLocalizedPluginDescription(
      data,
      i18n.resolvedLanguage || i18n.language,
    ) || t("adminPlugins.noDescription");
  const supports = data.supported_extensions || [];
  const supportLabels = supports.map((support) => getSupportLabel(support));
  const dependencies = data.dependencies || [];
  const permissions = data.permissions || [];
  const searchFieldCount = getMetadataSearchFieldCount(data.capabilities);
  const resultFieldCount = getMetadataResultFieldCount(data.capabilities);
  const pluginSignals = getPluginSignals(data);
  const canInstall = onInstall && (!data.isInstalled || data.hasUpdate);
  const externalLink = getExternalLink(data, t);
  const typeStyle = typeStyles[data.plugin_type] || {
    icon: "border-slate-200 bg-slate-50 text-slate-700 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200",
    chip: "border-slate-200 bg-slate-50 text-slate-600 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-300",
  };

  return (
    <article className="flex min-h-[18rem] flex-col rounded-lg border border-slate-200 bg-white p-5 shadow-sm transition-colors hover:border-slate-300 dark:border-slate-800 dark:bg-slate-900 dark:hover:border-slate-700">
      <header className="flex items-start gap-3">
        <div
          className={`flex h-10 w-10 shrink-0 items-center justify-center rounded-lg border ${typeStyle.icon}`}
        >
          <TypeIcon type={data.plugin_type} />
        </div>

        <div className="min-w-0 flex-1">
          <PluginName
            name={data.name}
            className="text-base font-semibold leading-6 text-slate-950 dark:text-white"
          />
          <div className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-slate-500 dark:text-slate-400">
            <span>{formatVersion(data.version, t)}</span>
            {data.hasUpdate && data.installedVersion ? (
              <span className="line-through">
                {formatVersion(data.installedVersion, t)}
              </span>
            ) : null}
            {data.author ? <span>{data.author}</span> : null}
          </div>
        </div>

        <div className="shrink-0">
          {data.state ? (
            <PluginStateBadge state={data.state} />
          ) : (
            <StoreStateBadge
              isInstalled={data.isInstalled}
              hasUpdate={data.hasUpdate}
            />
          )}
        </div>
      </header>

      <button
        type="button"
        onClick={() => onToggleDescription(data.id)}
        className={`mt-4 text-left text-sm leading-6 text-slate-600 dark:text-slate-300 ${
          expanded ? "" : "line-clamp-3"
        }`}
        title={
          expanded
            ? t("adminPlugins.collapseDescription")
            : t("adminPlugins.expandDescription")
        }
      >
        {description}
      </button>

      {data.state === "failed" && data.error ? (
        <div
          className="mt-3 flex items-start gap-2 rounded-lg border border-red-100 bg-red-50 px-3 py-2 text-xs leading-5 text-red-700 dark:border-red-900/40 dark:bg-red-950/30 dark:text-red-300"
          title={data.error}
        >
          <AlertCircle className="mt-0.5 shrink-0" size={14} />
          <span className="line-clamp-2">
            {t("adminPlugins.loadError", { message: data.error })}
          </span>
        </div>
      ) : null}

      <div className="mt-4 flex flex-wrap gap-1.5">
        <InfoChip icon={<Tag size={12} />}>
          {getPluginTypeLabel(data.plugin_type, t)}
        </InfoChip>
        <InfoChip icon={<Cpu size={12} />}>
          {getRuntimeLabel(data.runtime)}
        </InfoChip>
        {supportLabels.length > 0 ? (
          <InfoChip
            icon={<FileText size={12} />}
            title={supportLabels.join(", ")}
          >
            {supportLabels.slice(0, 4).join(", ")}
            {supportLabels.length > 4 ? ` +${supportLabels.length - 4}` : ""}
          </InfoChip>
        ) : null}
        {permissions.length > 0 ? (
          <InfoChip icon={<Shield size={12} />}>
            {t("adminPlugins.permissionCount", {
              count: permissions.length,
            })}
          </InfoChip>
        ) : null}
        {data.admin_only ? (
          <InfoChip icon={<Shield size={12} />}>Admin</InfoChip>
        ) : null}
        {searchFieldCount > 0 ? (
          <InfoChip icon={<Search size={12} />}>
            {t("adminPlugins.searchFieldCount", {
              count: searchFieldCount,
            })}
          </InfoChip>
        ) : null}
        {resultFieldCount > 0 ? (
          <InfoChip icon={<ListChecks size={12} />}>
            {t("adminPlugins.resultFieldCount", {
              count: resultFieldCount,
            })}
          </InfoChip>
        ) : null}
        {dependencies.length > 0 ? (
          <InfoChip
            icon={<Package size={12} />}
            title={dependencies.join(", ")}
          >
            {t("adminPlugins.dependencyCount", { count: dependencies.length })}
          </InfoChip>
        ) : null}
        {pluginSignals.length > 0 ? (
          <InfoChip icon={<Shield size={12} />} title={pluginSignals.join(", ")}>
            {pluginSignals.slice(0, 3).join(", ")}
            {pluginSignals.length > 3 ? ` +${pluginSignals.length - 3}` : ""}
          </InfoChip>
        ) : null}
        {data.license ? <InfoChip>{data.license}</InfoChip> : null}
        {data.config_schema ? (
          <InfoChip icon={<Settings size={12} />}>
            {t("adminPlugins.configurable")}
          </InfoChip>
        ) : null}
      </div>

      <footer className="mt-auto flex items-center gap-2 border-t border-slate-100 pt-4 dark:border-slate-800">
        {externalLink ? (
          <a
            href={externalLink.href}
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex h-9 items-center justify-center gap-1.5 rounded-lg px-2 text-slate-500 transition-colors hover:bg-blue-50 hover:text-blue-700 dark:hover:bg-blue-950/40 dark:hover:text-blue-300"
            title={externalLink.title}
          >
            {externalLink.icon}
            <span className="text-xs font-medium">{externalLink.label}</span>
          </a>
        ) : null}

        <div className="ml-auto flex items-center gap-2">
          {onConfigure && data.config_schema ? (
            <button
              type="button"
              onClick={onConfigure}
              className="inline-flex h-9 w-9 items-center justify-center rounded-lg text-slate-500 transition-colors hover:bg-amber-50 hover:text-amber-700 dark:hover:bg-amber-950/40 dark:hover:text-amber-300"
              title={t("adminPlugins.configure")}
            >
              <Settings size={17} />
            </button>
          ) : null}

          {onReload ? (
            <button
              type="button"
              onClick={onReload}
              className="inline-flex h-9 w-9 items-center justify-center rounded-lg text-slate-500 transition-colors hover:bg-blue-50 hover:text-blue-700 dark:hover:bg-blue-950/40 dark:hover:text-blue-300"
              title={t("adminPlugins.reload")}
            >
              <RefreshCw size={17} />
            </button>
          ) : null}

          {onUninstall ? (
            <button
              type="button"
              onClick={onUninstall}
              className="inline-flex h-9 w-9 items-center justify-center rounded-lg text-slate-500 transition-colors hover:bg-red-50 hover:text-red-700 dark:hover:bg-red-950/40 dark:hover:text-red-300"
              title={t("adminPlugins.uninstall")}
            >
              <Trash2 size={17} />
            </button>
          ) : null}

          {onInstall ? (
            <button
              type="button"
              onClick={onInstall}
              disabled={installing || !canInstall}
              className={`inline-flex h-9 items-center justify-center gap-2 rounded-lg px-3 text-sm font-semibold transition-colors ${
                installing
                  ? "bg-slate-100 text-slate-400 dark:bg-slate-800"
                  : !canInstall
                    ? "bg-slate-100 text-slate-400 dark:bg-slate-800 dark:text-slate-500"
                    : data.hasUpdate
                      ? "bg-emerald-600 text-white hover:bg-emerald-700"
                      : "bg-primary-600 text-white hover:bg-primary-700"
              }`}
            >
              {installing ? (
                <span className="h-4 w-4 rounded-full border-2 border-current border-t-transparent animate-spin" />
              ) : (
                <Download size={16} />
              )}
              {installing
                ? t("adminPlugins.processing")
                : data.hasUpdate
                  ? t("adminPlugins.update")
                  : data.isInstalled
                    ? t("adminPlugins.installed")
                    : t("adminPlugins.install")}
            </button>
          ) : null}
        </div>
      </footer>
    </article>
  );
};

export default PluginCard;
export {
  getBasePluginId,
  getPluginCategory,
  getInstalledStoreMeta,
  getLocalizedPluginDescription,
  toInstalledCardData,
  toStoreCardData,
};
export type { PluginCardData };
