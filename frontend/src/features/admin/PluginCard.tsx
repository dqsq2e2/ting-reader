/* eslint-disable react-refresh/only-export-components */
import React, { useEffect, useRef, useState } from 'react';
import type { Plugin, PluginDependency, StorePlugin } from '../../core/types';
import {
  AlertCircle,
  CheckCircle,
  Cpu,
  Download,
  FileText,
  Package,
  Puzzle,
  RefreshCw,
  Settings,
  Shield,
  Tag,
  Trash2,
  XCircle,
} from 'lucide-react';
import GithubIcon from '../../shared/ui/GithubIcon';

const PluginName = ({ name, className = '' }: { name: string; className?: string }) => {
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
      className={`${className} ${expanded ? 'break-words' : 'truncate'} ${(expanded || isOverflowing) ? 'cursor-pointer' : ''}`}
      onClick={handleClick}
      title={(expanded || isOverflowing) ? (expanded ? '收起名称' : '展开名称') : undefined}
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
  longDescription?: string;
  version: string;
  installedVersion?: string | null;
  pluginType: string;
  runtime?: string;
  author?: string;
  license?: string;
  repo?: string;
  dependencies?: string[];
  permissions?: string[];
  configSchema?: Record<string, unknown>;
  supportedExtensions?: string[];
  scraper?: StorePlugin['scraper'];
  state?: Plugin['state'];
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

const typeLabels: Record<string, string> = {
  scraper: '元数据',
  format: '格式',
  utility: '工具',
};

const runtimeLabels: Record<string, string> = {
  wasm: 'WASM',
  javascript: 'JavaScript',
  native: 'Native',
};

const typeStyles: Record<string, { icon: string; chip: string }> = {
  scraper: {
    icon: 'border-blue-100 bg-blue-50 text-blue-700 dark:border-blue-900/50 dark:bg-blue-950/40 dark:text-blue-300',
    chip: 'border-blue-100 bg-blue-50 text-blue-700 dark:border-blue-900/50 dark:bg-blue-950/40 dark:text-blue-300',
  },
  format: {
    icon: 'border-cyan-100 bg-cyan-50 text-cyan-700 dark:border-cyan-900/50 dark:bg-cyan-950/40 dark:text-cyan-300',
    chip: 'border-cyan-100 bg-cyan-50 text-cyan-700 dark:border-cyan-900/50 dark:bg-cyan-950/40 dark:text-cyan-300',
  },
  utility: {
    icon: 'border-emerald-100 bg-emerald-50 text-emerald-700 dark:border-emerald-900/50 dark:bg-emerald-950/40 dark:text-emerald-300',
    chip: 'border-emerald-100 bg-emerald-50 text-emerald-700 dark:border-emerald-900/50 dark:bg-emerald-950/40 dark:text-emerald-300',
  },
};

const getBasePluginId = (id: string) => id.split('@')[0];

const formatVersion = (version?: string | null) => {
  if (!version) return '未知版本';
  return version.startsWith('v') ? version : `v${version}`;
};

const getTypeLabel = (type?: string) => typeLabels[type || ''] || type || '未知';

const getRuntimeLabel = (runtime?: string) => runtimeLabels[runtime || ''] || runtime || 'unknown';

const normalizeDependencyIds = (dependencies?: string[] | PluginDependency[]) => {
  if (!dependencies) return [];
  return dependencies.map((dependency) => (
    typeof dependency === 'string' ? dependency : dependency.pluginName
  ));
};

const getRepoUrl = (repo: string) => (
  repo.startsWith('http://') || repo.startsWith('https://')
    ? repo
    : `https://github.com/${repo}`
);

const getExternalLink = (data: PluginCardData) => {
  if (data.repo) {
    return {
      href: getRepoUrl(data.repo),
      label: '仓库',
      title: '查看仓库',
      icon: <GithubIcon size={17} />,
    };
  }

  return null;
};

const getInstalledStoreMeta = (plugin: Plugin, storePlugins: StorePlugin[]) => {
  const baseId = getBasePluginId(plugin.id);
  return storePlugins.find((storePlugin) => storePlugin.id === baseId);
};

const toInstalledCardData = (plugin: Plugin, storeMeta?: StorePlugin): PluginCardData => ({
  id: plugin.id,
  baseId: getBasePluginId(plugin.id),
  name: plugin.name,
  description: storeMeta?.description || plugin.description,
  longDescription: storeMeta?.longDescription || plugin.description,
  version: plugin.version,
  pluginType: plugin.pluginType || storeMeta?.pluginType || 'utility',
  runtime: plugin.runtime || storeMeta?.runtime,
  author: plugin.author || storeMeta?.author,
  license: plugin.license || storeMeta?.license,
  repo: plugin.repo || storeMeta?.repo,
  dependencies: normalizeDependencyIds(plugin.dependencies || storeMeta?.dependencies),
  permissions: plugin.permissions || storeMeta?.permissions,
  configSchema: plugin.configSchema || storeMeta?.configSchema,
  supportedExtensions: plugin.supportedExtensions || storeMeta?.supportedExtensions,
  scraper: plugin.scraper || storeMeta?.scraper,
  state: plugin.state,
  isInstalled: true,
});

const toStoreCardData = (
  plugin: StorePlugin,
  installedVersion: string | null,
  hasUpdate: boolean
): PluginCardData => ({
  id: plugin.id,
  baseId: plugin.id,
  name: plugin.name,
  description: plugin.description,
  longDescription: plugin.longDescription || plugin.description,
  version: plugin.version,
  installedVersion,
  pluginType: plugin.pluginType,
  runtime: plugin.runtime,
  author: plugin.author,
  license: plugin.license,
  repo: plugin.repo,
  dependencies: normalizeDependencyIds(plugin.dependencies),
  permissions: plugin.permissions,
  configSchema: plugin.configSchema,
  supportedExtensions: plugin.supportedExtensions,
  scraper: plugin.scraper,
  isInstalled: !!installedVersion,
  hasUpdate,
});

const TypeIcon = ({ type }: { type: string }) => {
  if (type === 'format') return <FileText size={19} />;
  if (type === 'utility') return <Package size={19} />;
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

const PluginStateBadge = ({ state }: { state?: Plugin['state'] }) => {
  if (state === 'active') {
    return (
      <span className="inline-flex items-center gap-1 rounded-md border border-green-200 bg-green-50 px-2 py-1 text-xs font-semibold text-green-700 dark:border-green-900/40 dark:bg-green-950/40 dark:text-green-300">
        <CheckCircle size={13} /> 活跃
      </span>
    );
  }

  if (state === 'failed') {
    return (
      <span className="inline-flex items-center gap-1 rounded-md border border-red-200 bg-red-50 px-2 py-1 text-xs font-semibold text-red-700 dark:border-red-900/40 dark:bg-red-950/40 dark:text-red-300">
        <XCircle size={13} /> 失败
      </span>
    );
  }

  return (
    <span className="inline-flex items-center gap-1 rounded-md border border-slate-200 bg-slate-50 px-2 py-1 text-xs font-semibold text-slate-600 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-300">
      <AlertCircle size={13} /> {state || '未知'}
    </span>
  );
};

const StoreStateBadge = ({ isInstalled, hasUpdate }: { isInstalled?: boolean; hasUpdate?: boolean }) => {
  if (hasUpdate) {
    return (
      <span className="inline-flex items-center gap-1 rounded-md border border-emerald-200 bg-emerald-50 px-2 py-1 text-xs font-semibold text-emerald-700 dark:border-emerald-900/40 dark:bg-emerald-950/40 dark:text-emerald-300">
        可更新
      </span>
    );
  }

  if (isInstalled) {
    return (
      <span className="inline-flex items-center gap-1 rounded-md border border-slate-200 bg-slate-50 px-2 py-1 text-xs font-semibold text-slate-600 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-300">
        已安装
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
  const description = data.longDescription || data.description || '暂无描述';
  const supports = data.supportedExtensions || [];
  const dependencies = data.dependencies || [];
  const permissions = data.permissions || [];
  const canInstall = onInstall && (!data.isInstalled || data.hasUpdate);
  const externalLink = getExternalLink(data);
  const typeStyle = typeStyles[data.pluginType] || {
    icon: 'border-slate-200 bg-slate-50 text-slate-700 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200',
    chip: 'border-slate-200 bg-slate-50 text-slate-600 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-300',
  };

  return (
    <article className="flex min-h-[18rem] flex-col rounded-lg border border-slate-200 bg-white p-5 shadow-sm transition-colors hover:border-slate-300 dark:border-slate-800 dark:bg-slate-900 dark:hover:border-slate-700">
      <header className="flex items-start gap-3">
        <div className={`flex h-10 w-10 shrink-0 items-center justify-center rounded-lg border ${typeStyle.icon}`}>
          <TypeIcon type={data.pluginType} />
        </div>

        <div className="min-w-0 flex-1">
          <PluginName
            name={data.name}
            className="text-base font-semibold leading-6 text-slate-950 dark:text-white"
          />
          <div className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-slate-500 dark:text-slate-400">
            <span>{formatVersion(data.version)}</span>
            {data.hasUpdate && data.installedVersion ? (
              <span className="line-through">{formatVersion(data.installedVersion)}</span>
            ) : null}
            {data.author ? <span>{data.author}</span> : null}
          </div>
        </div>

        <div className="shrink-0">
          {data.state ? (
            <PluginStateBadge state={data.state} />
          ) : (
            <StoreStateBadge isInstalled={data.isInstalled} hasUpdate={data.hasUpdate} />
          )}
        </div>
      </header>

      <button
        type="button"
        onClick={() => onToggleDescription(data.id)}
        className={`mt-4 text-left text-sm leading-6 text-slate-600 dark:text-slate-300 ${
          expanded ? '' : 'line-clamp-3'
        }`}
        title={expanded ? '收起描述' : '展开描述'}
      >
        {description}
      </button>

      <div className="mt-4 flex flex-wrap gap-1.5">
        <span className={`inline-flex max-w-full items-center gap-1 rounded-md border px-2 py-1 text-xs font-medium ${typeStyle.chip}`}>
          <Tag size={12} />
          <span className="truncate">{getTypeLabel(data.pluginType)}</span>
        </span>
        <InfoChip icon={<Cpu size={12} />}>{getRuntimeLabel(data.runtime)}</InfoChip>
        {supports.length > 0 ? (
          <InfoChip icon={<FileText size={12} />} title={supports.join(', ')}>
            {supports.slice(0, 4).join(', ')}
            {supports.length > 4 ? ` +${supports.length - 4}` : ''}
          </InfoChip>
        ) : null}
        {dependencies.length > 0 ? (
          <InfoChip icon={<Package size={12} />} title={dependencies.join(', ')}>
            {dependencies.length} 依赖
          </InfoChip>
        ) : null}
        {permissions.length > 0 ? (
          <InfoChip icon={<Shield size={12} />} title={permissions.join(', ')}>
            {permissions.length} 权限
          </InfoChip>
        ) : null}
        {data.license ? <InfoChip>{data.license}</InfoChip> : null}
        {data.configSchema ? <InfoChip icon={<Settings size={12} />}>可配置</InfoChip> : null}
        {data.scraper?.autoScrape ? <InfoChip>自动刮削</InfoChip> : null}
        {data.scraper?.searchFields?.length ? (
          <InfoChip>{data.scraper.searchFields.length} 搜索项</InfoChip>
        ) : null}
        {data.scraper?.resultFields?.length ? (
          <InfoChip title={data.scraper.resultFields.join(', ')}>
            {data.scraper.resultFields.length} 返回字段
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
          {onConfigure && data.configSchema ? (
            <button
              type="button"
              onClick={onConfigure}
              className="inline-flex h-9 w-9 items-center justify-center rounded-lg text-slate-500 transition-colors hover:bg-amber-50 hover:text-amber-700 dark:hover:bg-amber-950/40 dark:hover:text-amber-300"
              title="配置"
            >
              <Settings size={17} />
            </button>
          ) : null}

          {onReload ? (
            <button
              type="button"
              onClick={onReload}
              className="inline-flex h-9 w-9 items-center justify-center rounded-lg text-slate-500 transition-colors hover:bg-blue-50 hover:text-blue-700 dark:hover:bg-blue-950/40 dark:hover:text-blue-300"
              title="重新加载"
            >
              <RefreshCw size={17} />
            </button>
          ) : null}

          {onUninstall ? (
            <button
              type="button"
              onClick={onUninstall}
              className="inline-flex h-9 w-9 items-center justify-center rounded-lg text-slate-500 transition-colors hover:bg-red-50 hover:text-red-700 dark:hover:bg-red-950/40 dark:hover:text-red-300"
              title="卸载"
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
                  ? 'bg-slate-100 text-slate-400 dark:bg-slate-800'
                  : !canInstall
                    ? 'bg-slate-100 text-slate-400 dark:bg-slate-800 dark:text-slate-500'
                    : data.hasUpdate
                      ? 'bg-emerald-600 text-white hover:bg-emerald-700'
                      : 'bg-primary-600 text-white hover:bg-primary-700'
              }`}
            >
              {installing ? (
                <span className="h-4 w-4 rounded-full border-2 border-current border-t-transparent animate-spin" />
              ) : (
                <Download size={16} />
              )}
              {installing ? '处理中' : data.hasUpdate ? '更新' : data.isInstalled ? '已安装' : '安装'}
            </button>
          ) : null}
        </div>
      </footer>
    </article>
  );
};

export default PluginCard;
export { getBasePluginId, getInstalledStoreMeta, toInstalledCardData, toStoreCardData };
export type { PluginCardData };
