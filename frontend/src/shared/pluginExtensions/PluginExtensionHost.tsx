import { X } from "lucide-react";
import { useState } from "react";
import { invokePluginCapability } from "../../core/api/pluginCapabilities";
import { useClientExtensions } from "../../core/hooks/useClientExtensions";
import type { ClientExtensionDescriptor } from "../../core/pluginExtensions";
import { usePlayerStore } from "../../core/stores/playerStore";
import PluginExtensionIcon from "./PluginExtensionIcon";
import PluginWebContainer from "./PluginWebContainer";

const extensionLabel = (extension: ClientExtensionDescriptor) =>
  extension.title || extension.pluginName || extension.capability.id;

const PluginLauncherIcon = () => (
  <span
    aria-hidden="true"
    className="grid h-6 w-6 grid-cols-2 gap-0.5"
  >
    <span className="rounded-[1px] bg-[#54cde3]" />
    <span className="rounded-[1px] bg-[#48bfdd]" />
    <span className="rounded-[1px] bg-[#32b4d4]" />
    <span className="rounded-[1px] bg-[#249ec8]" />
  </span>
);

const PluginExtensionHost = () => {
  const { registry } = useClientExtensions();
  const hasCurrentChapter = usePlayerStore((state) => !!state.currentChapter);
  const floatingActions = registry.bySlot["global.floating_action"] || [];
  const panels = registry.bySlot["global.panel"] || [];
  const primaryActions = floatingActions.length > 0 ? floatingActions : panels;
  const [activeExtension, setActiveExtension] =
    useState<ClientExtensionDescriptor | null>(null);
  const [menuOpen, setMenuOpen] = useState(false);
  const [actionState, setActionState] = useState<
    "idle" | "running" | "success" | "error"
  >("idle");
  const [actionMessage, setActionMessage] = useState<string>();

  if (floatingActions.length === 0 && panels.length === 0) {
    return null;
  }

  const openExtension = (extension: ClientExtensionDescriptor) => {
    setActionState("idle");
    setActionMessage(undefined);
    setMenuOpen(false);
    setActiveExtension(extension);
  };

  const invokeActiveAction = async () => {
    if (!activeExtension) return;

    setActionState("running");
    setActionMessage(undefined);
    try {
      const result = await invokePluginCapability(
        activeExtension.pluginId,
        activeExtension.capability.id,
        {
          slot: activeExtension.slot,
          contexts: activeExtension.contexts,
        },
      );
      setActionState("success");
      setActionMessage(
        typeof result === "string"
          ? result
          : JSON.stringify(result ?? { ok: true }, null, 2),
      );
    } catch (err) {
      setActionState("error");
      setActionMessage(err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <>
      <div
        className="fixed right-4 z-[90] flex flex-col items-center gap-2"
        style={{
          bottom: hasCurrentChapter
            ? "var(--safe-bottom-with-player)"
            : "var(--safe-bottom-base)",
        }}
      >
        {menuOpen ? (
          <div className="mb-1 flex max-h-[min(60vh,24rem)] flex-col items-center gap-2 overflow-y-auto">
            {primaryActions.map((extension) => (
              <button
                key={extension.id}
                type="button"
                onClick={() => openExtension(extension)}
                className="inline-flex h-10 w-10 shrink-0 items-center justify-center rounded-lg border border-slate-200/80 bg-white/95 text-slate-700 shadow-md shadow-slate-900/10 transition-colors hover:border-primary-200 hover:bg-primary-50 hover:text-primary-700 focus:outline-none focus:ring-2 focus:ring-cyan-300 focus:ring-offset-2 focus:ring-offset-white dark:border-slate-700/80 dark:bg-slate-900/95 dark:text-slate-100 dark:hover:border-primary-700 dark:hover:bg-primary-950/40 dark:hover:text-primary-300 dark:focus:ring-offset-slate-950"
                title={extensionLabel(extension)}
                aria-label={extensionLabel(extension)}
              >
                <PluginExtensionIcon extension={extension} size={18} />
              </button>
            ))}
          </div>
        ) : null}
        <button
          type="button"
          onClick={() => setMenuOpen((open) => !open)}
          className="inline-flex h-12 w-12 items-center justify-center rounded-xl border border-slate-200/80 bg-white/95 text-primary-600 shadow-lg shadow-slate-900/10 transition-colors hover:border-primary-200 hover:bg-primary-50 focus:outline-none focus:ring-2 focus:ring-cyan-300 focus:ring-offset-2 focus:ring-offset-white dark:border-slate-700/80 dark:bg-slate-900/95 dark:text-primary-300 dark:shadow-slate-950/25 dark:hover:bg-slate-800 dark:focus:ring-offset-slate-950"
          title="Plugin entries"
          aria-label="Plugin entries"
          aria-expanded={menuOpen}
        >
          <PluginLauncherIcon />
        </button>
      </div>

      {activeExtension ? (
        <div className="fixed inset-0 z-[120] flex items-end justify-end bg-slate-950/30 p-3 backdrop-blur-sm sm:p-6">
          <section className="flex h-[min(42rem,88vh)] w-full max-w-md flex-col overflow-hidden rounded-lg border border-slate-200 bg-white shadow-2xl dark:border-slate-700 dark:bg-slate-900">
            <header className="flex h-14 shrink-0 items-center gap-3 border-b border-slate-200 px-4 dark:border-slate-800">
              <div className="flex h-8 w-8 items-center justify-center rounded-md bg-primary-50 text-primary-700 dark:bg-primary-950/40 dark:text-primary-300">
                <PluginExtensionIcon extension={activeExtension} size={17} />
              </div>
              <div className="min-w-0 flex-1">
                <h2 className="truncate text-sm font-semibold text-slate-950 dark:text-white">
                  {extensionLabel(activeExtension)}
                </h2>
                <p className="truncate text-xs text-slate-500 dark:text-slate-400">
                  {activeExtension.pluginName}
                </p>
              </div>
              <button
                type="button"
                onClick={() => setActiveExtension(null)}
                className="flex h-9 w-9 items-center justify-center rounded-md text-slate-500 transition-colors hover:bg-slate-100 hover:text-slate-900 dark:hover:bg-slate-800 dark:hover:text-white"
                title="Close"
              >
                <X size={18} />
              </button>
            </header>
            <div
              className={`flex flex-1 flex-col text-sm leading-6 text-slate-500 dark:text-slate-400 ${
                activeExtension.renderMode === "web_container"
                  ? "min-h-0"
                  : "justify-center gap-4 px-6"
              }`}
            >
              {activeExtension.renderMode === "action" ? (
                <>
                  <button
                    type="button"
                    onClick={invokeActiveAction}
                    disabled={actionState === "running"}
                    className="inline-flex h-10 items-center justify-center rounded-md bg-primary-600 px-4 text-sm font-semibold text-white transition-colors hover:bg-primary-700 disabled:cursor-not-allowed disabled:bg-slate-300 dark:disabled:bg-slate-700"
                  >
                    {actionState === "running" ? "Running..." : "Run"}
                  </button>
                  {actionMessage ? (
                    <pre
                      className={`max-h-64 overflow-auto rounded-md border px-3 py-2 text-left text-xs ${
                        actionState === "error"
                          ? "border-red-200 bg-red-50 text-red-700 dark:border-red-900/40 dark:bg-red-950/30 dark:text-red-300"
                          : "border-slate-200 bg-slate-50 text-slate-700 dark:border-slate-800 dark:bg-slate-950 dark:text-slate-300"
                      }`}
                    >
                      {actionMessage}
                    </pre>
                  ) : null}
                </>
              ) : activeExtension.renderMode === "web_container" ? (
                <PluginWebContainer extension={activeExtension} />
              ) : (
                <div className="text-center">
                  {activeExtension.capability.id}
                </div>
              )}
            </div>
          </section>
        </div>
      ) : null}
    </>
  );
};

export default PluginExtensionHost;
