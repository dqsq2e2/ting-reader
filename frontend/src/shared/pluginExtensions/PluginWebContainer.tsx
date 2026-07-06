import { Loader2 } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import {
  invokePluginCapability,
  invokePluginHost,
} from "../../core/api/pluginCapabilities";
import apiClient from "../../core/api/client";
import { useAuthStore } from "../../core/stores/authStore";
import type { ClientExtensionDescriptor } from "../../core/pluginExtensions";

type PluginBridgeRequest = {
  type: "ting-plugin:request";
  id: string;
  method: "capability.invoke" | "host.invoke";
  params?: unknown;
};

type PluginBridgeResponse = {
  type: "ting-plugin:response";
  id: string;
  ok: boolean;
  result?: unknown;
  error?: string;
};

type PluginWebContainerProps = {
  extension: ClientExtensionDescriptor;
  context?: Record<string, unknown>;
};

const isBridgeRequest = (value: unknown): value is PluginBridgeRequest => {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<PluginBridgeRequest>;
  return (
    candidate.type === "ting-plugin:request" &&
    typeof candidate.id === "string" &&
    (candidate.method === "capability.invoke" ||
      candidate.method === "host.invoke")
  );
};

const pluginAssetPath = (extension: ClientExtensionDescriptor) => {
  const entry = extension.render?.entry?.replace(/^\/+/, "");
  if (!entry) return undefined;
  const encodedEntry = entry
    .split("/")
    .map((segment) => encodeURIComponent(segment))
    .join("/");
  return `/api/v1/plugin-assets/${encodeURIComponent(extension.pluginId)}/${encodedEntry}`;
};

const absoluteAssetUrl = (path: string, activeUrl?: string) => {
  try {
    return new URL(path, activeUrl || window.location.origin).toString();
  } catch {
    return path;
  }
};

const escapeAttribute = (value: string) =>
  value
    .replace(/&/g, "&amp;")
    .replace(/"/g, "&quot;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");

const withBaseHref = (html: string, href: string) => {
  if (/<base\b/i.test(html)) return html;
  const base = `<base href="${escapeAttribute(href)}">`;
  if (/<head[^>]*>/i.test(html)) {
    return html.replace(/<head([^>]*)>/i, `<head$1>${base}`);
  }
  return `${base}${html}`;
};

const responseFor = (
  request: PluginBridgeRequest,
  payload: Omit<PluginBridgeResponse, "type" | "id">,
): PluginBridgeResponse => ({
  type: "ting-plugin:response",
  id: request.id,
  ...payload,
});

const PluginWebContainer = ({
  extension,
  context,
}: PluginWebContainerProps) => {
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const activeUrl = useAuthStore((state) => state.activeUrl);
  const [loadError, setLoadError] = useState<string>();
  const [srcDoc, setSrcDoc] = useState<string>();
  const src = useMemo(() => pluginAssetPath(extension), [extension]);
  const srcBaseUrl = useMemo(
    () => (src ? absoluteAssetUrl(src, activeUrl) : undefined),
    [activeUrl, src],
  );

  const postToFrame = (message: unknown) => {
    iframeRef.current?.contentWindow?.postMessage(message, "*");
  };

  const handleLoad = () => {
    setLoadError(undefined);
    postToFrame({
      type: "ting-plugin:init",
      pluginId: extension.pluginId,
      pluginName: extension.pluginName,
      capabilityId: extension.capability.id,
      slot: extension.slot,
      contexts: extension.contexts,
      context: context || {},
    });
  };

  useEffect(() => {
    if (!src || !srcBaseUrl) {
      setSrcDoc(undefined);
      return;
    }

    let cancelled = false;
    setLoadError(undefined);
    setSrcDoc(undefined);

    void apiClient
      .get<string>(src, { responseType: "text" })
      .then((response) => {
        if (cancelled) return;
        const html =
          typeof response.data === "string"
            ? response.data
            : String(response.data ?? "");
        setSrcDoc(withBaseHref(html, srcBaseUrl));
      })
      .catch((error) => {
        if (cancelled) return;
        setLoadError(
          error instanceof Error ? error.message : "Plugin UI failed to load.",
        );
      });

    return () => {
      cancelled = true;
    };
  }, [src, srcBaseUrl]);

  const handleMessage = async (event: MessageEvent) => {
    if (event.source !== iframeRef.current?.contentWindow) return;
    if (!isBridgeRequest(event.data)) return;

    const request = event.data;
    try {
      if (request.method === "capability.invoke") {
        const params =
          request.params && typeof request.params === "object"
            ? (request.params as {
                capabilityId?: string;
                params?: unknown;
              })
            : {};
        const result = await invokePluginCapability(
          extension.pluginId,
          params.capabilityId || extension.capability.id,
          params.params ?? {},
        );
        postToFrame(responseFor(request, { ok: true, result }));
        return;
      }

      const params =
        request.params && typeof request.params === "object"
          ? (request.params as { method?: string; params?: unknown })
          : {};
      if (!params.method) {
        throw new Error("Missing host method");
      }

      const result = await invokePluginHost({
        plugin_id: extension.pluginId,
        method: params.method,
        params: params.params ?? {},
      });
      postToFrame(responseFor(request, { ok: true, result }));
    } catch (err) {
      postToFrame(
        responseFor(request, {
          ok: false,
          error: err instanceof Error ? err.message : String(err),
        }),
      );
    }
  };

  useEffect(() => {
    window.addEventListener("message", handleMessage);
    return () => window.removeEventListener("message", handleMessage);
  });

  if (!src) {
    return (
      <div className="flex h-full items-center justify-center px-6 text-center text-sm text-slate-500 dark:text-slate-400">
        Missing plugin UI entry.
      </div>
    );
  }

  return (
    <div className="relative h-full w-full">
      {loadError ? (
        <div className="absolute inset-x-4 top-4 z-10 rounded-md border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700 dark:border-red-900/40 dark:bg-red-950/40 dark:text-red-300">
          {loadError}
        </div>
      ) : null}
      {!srcDoc && !loadError ? (
        <div className="flex h-full items-center justify-center gap-2 text-sm text-slate-500 dark:text-slate-400">
          <Loader2 size={16} className="animate-spin" />
          <span>Loading plugin UI...</span>
        </div>
      ) : null}
      {srcDoc ? (
        <iframe
          ref={iframeRef}
          srcDoc={srcDoc}
          title={extension.title || extension.capability.id}
          sandbox="allow-scripts allow-forms allow-popups allow-popups-to-escape-sandbox"
          className="h-full w-full border-0 bg-white dark:bg-slate-950"
          onLoad={handleLoad}
        />
      ) : null}
    </div>
  );
};

export default PluginWebContainer;
