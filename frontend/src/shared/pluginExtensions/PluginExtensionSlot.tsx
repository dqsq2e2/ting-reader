import { Loader2, MoreHorizontal, X } from "lucide-react";
import type { FormEvent, ReactNode } from "react";
import { useEffect, useState } from "react";
import {
  invokePluginCapability,
  invokePluginHost,
} from "../../core/api/pluginCapabilities";
import { useClientExtensions } from "../../core/hooks/useClientExtensions";
import type {
  ClientExtensionDescriptor,
  ClientExtensionSlot,
} from "../../core/pluginExtensions";
import DocumentReaderPanel from "./DocumentReaderPanel";
import PluginExtensionIcon from "./PluginExtensionIcon";
import PluginWebContainer from "./PluginWebContainer";

type PluginExtensionSlotProps = {
  slot: ClientExtensionSlot;
  context?: Record<string, unknown>;
  className?: string;
  buttonClassName?: string;
  showLabel?: boolean;
  menuLabel?: string;
  menuClassName?: string;
  limit?: number;
  empty?: ReactNode;
};

const extensionLabel = (extension: ClientExtensionDescriptor) =>
  extension.title || extension.pluginName || extension.capability.id;

const defaultButtonClassName =
  "inline-flex h-9 w-9 items-center justify-center rounded-md text-slate-500 transition-colors hover:bg-slate-100 hover:text-slate-900 dark:text-slate-300 dark:hover:bg-slate-800 dark:hover:text-white";

type SchemaFieldConfig = {
  name: string;
  label?: string;
  type?: "text" | "textarea" | "number" | "boolean" | "select";
  placeholder?: string;
  required?: boolean;
  default?: unknown;
  options?: Array<string | { label?: string; value?: unknown }>;
};

const schemaFieldsFor = (
  extension: ClientExtensionDescriptor,
): SchemaFieldConfig[] => {
  const schema = (extension.render?.schema || {}) as Record<string, unknown>;
  const fields = Array.isArray(schema.fields) ? schema.fields : [];
  return fields
    .filter(
      (field): field is Record<string, unknown> =>
        typeof field === "object" &&
        field !== null &&
        typeof field.name === "string",
    )
    .map((field) => ({
      name: field.name as string,
      label: typeof field.label === "string" ? field.label : undefined,
      type:
        field.type === "textarea" ||
        field.type === "number" ||
        field.type === "boolean" ||
        field.type === "select"
          ? field.type
          : "text",
      placeholder:
        typeof field.placeholder === "string" ? field.placeholder : undefined,
      required: field.required === true,
      default: field.default,
      options: Array.isArray(field.options)
        ? (field.options as SchemaFieldConfig["options"])
        : undefined,
    }));
};

const builtinConfigFor = (extension: ClientExtensionDescriptor) => ({
  component:
    extension.render?.builtin?.component ||
    extension.render?.component ||
    "capability_result",
  method: extension.render?.builtin?.method || extension.render?.method,
  params: extension.render?.builtin?.params || extension.render?.params || {},
  autoRun:
    extension.render?.builtin?.auto_run === true ||
    extension.render?.auto_run === true,
  submitLabel:
    extension.render?.builtin?.submit_label ||
    extension.render?.submit_label ||
    "Run",
});

const PluginExtensionSlot = ({
  slot,
  context,
  className = "flex items-center gap-1",
  buttonClassName = defaultButtonClassName,
  showLabel = false,
  menuLabel,
  menuClassName = "absolute right-0 top-full z-30 mt-2 min-w-44 overflow-hidden rounded-lg border border-slate-200 bg-white p-1 shadow-xl shadow-slate-900/10 dark:border-slate-700 dark:bg-slate-900 dark:shadow-slate-950/30",
  limit,
  empty = null,
}: PluginExtensionSlotProps) => {
  const { registry } = useClientExtensions();
  const extensions = registry.bySlot[slot] || [];
  const visibleExtensions =
    typeof limit === "number" ? extensions.slice(0, limit) : extensions;
  const [activeExtension, setActiveExtension] =
    useState<ClientExtensionDescriptor | null>(null);
  const [actionState, setActionState] = useState<
    "idle" | "running" | "success" | "error"
  >("idle");
  const [actionMessage, setActionMessage] = useState<string>();
  const [menuOpen, setMenuOpen] = useState(false);

  if (visibleExtensions.length === 0) {
    return <>{empty}</>;
  }

  const closePanel = () => {
    setActiveExtension(null);
    setActionState("idle");
    setActionMessage(undefined);
  };

  const invokeAction = async (extension: ClientExtensionDescriptor) => {
    setActiveExtension(extension);
    setActionState("running");
    setActionMessage(undefined);
    try {
      const result = await invokePluginCapability(
        extension.pluginId,
        extension.capability.id,
        {
          slot: extension.slot,
          contexts: extension.contexts,
          context: context || {},
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

  const openExtension = (extension: ClientExtensionDescriptor) => {
    setMenuOpen(false);
    if (
      extension.renderMode === "web_container" ||
      extension.renderMode === "schema" ||
      extension.renderMode === "builtin"
    ) {
      setActiveExtension(extension);
      setActionState("idle");
      setActionMessage(undefined);
      return;
    }

    void invokeAction(extension);
  };

  return (
    <>
      <div className={className}>
        {menuLabel ? (
          <>
            <button
              type="button"
              onClick={() => setMenuOpen((open) => !open)}
              className={buttonClassName}
              title={menuLabel}
              aria-haspopup="menu"
              aria-expanded={menuOpen}
            >
              <MoreHorizontal size={18} />
              <span className="truncate">{menuLabel}</span>
            </button>
            {menuOpen ? (
              <div className={menuClassName} role="menu">
                {visibleExtensions.map((extension) => (
                  <button
                    key={extension.id}
                    type="button"
                    onClick={() => openExtension(extension)}
                    className="flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm font-semibold text-slate-600 transition-colors hover:bg-slate-100 hover:text-primary-700 dark:text-slate-200 dark:hover:bg-slate-800 dark:hover:text-primary-300"
                    title={extensionLabel(extension)}
                    role="menuitem"
                  >
                    <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-slate-100 text-slate-500 dark:bg-slate-800 dark:text-slate-300">
                      <PluginExtensionIcon extension={extension} size={16} />
                    </span>
                    <span className="min-w-0 flex-1 truncate">
                      {extensionLabel(extension)}
                    </span>
                  </button>
                ))}
              </div>
            ) : null}
          </>
        ) : visibleExtensions.map((extension) => (
          <button
            key={extension.id}
            type="button"
            onClick={() => openExtension(extension)}
            className={buttonClassName}
            title={extensionLabel(extension)}
          >
            <PluginExtensionIcon extension={extension} size={17} />
            {showLabel ? (
              <span className="truncate">{extensionLabel(extension)}</span>
            ) : null}
          </button>
        ))}
      </div>

      {activeExtension ? (
        <div className="fixed inset-0 z-[125] flex items-end justify-end bg-slate-950/30 p-3 backdrop-blur-sm sm:p-6">
          <section className="flex h-[min(38rem,86vh)] w-full max-w-md flex-col overflow-hidden rounded-lg border border-slate-200 bg-white shadow-2xl dark:border-slate-700 dark:bg-slate-900">
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
                onClick={closePanel}
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
              {activeExtension.renderMode === "web_container" ? (
                <PluginWebContainer
                  extension={activeExtension}
                  context={context}
                />
              ) : activeExtension.renderMode === "schema" ? (
                <PluginSchemaForm
                  extension={activeExtension}
                  context={context}
                />
              ) : activeExtension.renderMode === "builtin" ? (
                <PluginBuiltinView
                  extension={activeExtension}
                  context={context}
                />
              ) : actionState === "running" ? (
                <div className="flex items-center justify-center gap-2 text-slate-500 dark:text-slate-400">
                  <Loader2 size={16} className="animate-spin" />
                  <span>Running...</span>
                </div>
              ) : actionMessage ? (
                <pre
                  className={`mx-6 max-h-72 overflow-auto rounded-md border px-3 py-2 text-left text-xs ${
                    actionState === "error"
                      ? "border-red-200 bg-red-50 text-red-700 dark:border-red-900/40 dark:bg-red-950/30 dark:text-red-300"
                      : "border-slate-200 bg-slate-50 text-slate-700 dark:border-slate-800 dark:bg-slate-950 dark:text-slate-300"
                  }`}
                >
                  {actionMessage}
                </pre>
              ) : null}
            </div>
          </section>
        </div>
      ) : null}
    </>
  );
};

const PluginBuiltinView = ({
  extension,
  context,
}: {
  extension: ClientExtensionDescriptor;
  context?: Record<string, unknown>;
}) => {
  const config = builtinConfigFor(extension);
  const [running, setRunning] = useState(false);
  const [message, setMessage] = useState<string>();
  const [failed, setFailed] = useState(false);

  const run = async () => {
    setRunning(true);
    setMessage(undefined);
    setFailed(false);
    try {
      const result =
        config.component === "host_method"
          ? await invokePluginHost({
              plugin_id: extension.pluginId,
              method: config.method || "",
              params: config.params,
            })
          : await invokePluginCapability(
              extension.pluginId,
              extension.capability.id,
              {
                slot: extension.slot,
                contexts: extension.contexts,
                context: context || {},
                params: config.params,
              },
            );
      setMessage(
        typeof result === "string"
          ? result
          : JSON.stringify(result ?? { ok: true }, null, 2),
      );
    } catch (error) {
      setFailed(true);
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setRunning(false);
    }
  };

  useEffect(() => {
    if (config.autoRun) {
      void run();
    }
    // Run once per opened extension; config is derived from the extension.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [extension.id]);

  if (config.component === "host_method" && !config.method) {
    return (
      <div className="flex flex-1 items-center justify-center px-6 text-center text-sm text-slate-500 dark:text-slate-400">
        Missing builtin host method.
      </div>
    );
  }

  if (config.component === "document_reader") {
    return <DocumentReaderPanel context={context} />;
  }

  return (
    <div className="flex flex-1 flex-col gap-4 p-5">
      <button
        type="button"
        onClick={() => void run()}
        disabled={running}
        className="inline-flex h-10 items-center justify-center gap-2 rounded-md bg-primary-600 px-4 text-sm font-semibold text-white transition-colors hover:bg-primary-700 disabled:cursor-not-allowed disabled:opacity-60"
      >
        {running ? <Loader2 size={16} className="animate-spin" /> : null}
        {config.submitLabel}
      </button>
      <pre
        className={`min-h-0 flex-1 overflow-auto rounded-md border px-3 py-2 text-xs ${
          failed
            ? "border-red-200 bg-red-50 text-red-700 dark:border-red-900/40 dark:bg-red-950/30 dark:text-red-300"
            : "border-slate-200 bg-slate-50 text-slate-700 dark:border-slate-800 dark:bg-slate-950 dark:text-slate-300"
        }`}
      >
        {message || "Ready."}
      </pre>
    </div>
  );
};

const PluginSchemaForm = ({
  extension,
  context,
}: {
  extension: ClientExtensionDescriptor;
  context?: Record<string, unknown>;
}) => {
  const fields = schemaFieldsFor(extension);
  const [values, setValues] = useState<Record<string, unknown>>(() =>
    Object.fromEntries(
      fields.map((field) => [field.name, field.default ?? ""]),
    ),
  );
  const [running, setRunning] = useState(false);
  const [message, setMessage] = useState<string>();
  const [failed, setFailed] = useState(false);
  const submitLabel =
    typeof extension.render?.submit_label === "string"
      ? extension.render.submit_label
      : "Run";

  const update = (name: string, value: unknown) => {
    setValues((current) => ({ ...current, [name]: value }));
  };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setRunning(true);
    setMessage(undefined);
    setFailed(false);
    try {
      const result = await invokePluginCapability(
        extension.pluginId,
        extension.capability.id,
        {
          slot: extension.slot,
          contexts: extension.contexts,
          context: context || {},
          values,
        },
      );
      setMessage(
        typeof result === "string"
          ? result
          : JSON.stringify(result ?? { ok: true }, null, 2),
      );
    } catch (error) {
      setFailed(true);
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setRunning(false);
    }
  };

  if (fields.length === 0) {
    return (
      <div className="flex flex-1 items-center justify-center px-6 text-center text-sm text-slate-500 dark:text-slate-400">
        Missing schema fields.
      </div>
    );
  }

  return (
    <form onSubmit={submit} className="flex flex-1 flex-col gap-4 p-5">
      <div className="min-h-0 flex-1 space-y-4 overflow-auto pr-1">
        {fields.map((field) => (
          <label key={field.name} className="block text-sm">
            <span className="mb-1.5 block font-medium text-slate-700 dark:text-slate-200">
              {field.label || field.name}
            </span>
            {field.type === "textarea" ? (
              <textarea
                required={field.required}
                value={String(values[field.name] ?? "")}
                placeholder={field.placeholder}
                onChange={(event) => update(field.name, event.target.value)}
                className="min-h-24 w-full rounded-md border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900 outline-none focus:border-primary-500 dark:border-slate-700 dark:bg-slate-950 dark:text-white"
              />
            ) : field.type === "boolean" ? (
              <input
                type="checkbox"
                checked={values[field.name] === true}
                onChange={(event) => update(field.name, event.target.checked)}
                className="h-4 w-4 rounded border-slate-300 text-primary-600"
              />
            ) : field.type === "select" ? (
              <select
                required={field.required}
                value={String(values[field.name] ?? "")}
                onChange={(event) => update(field.name, event.target.value)}
                className="h-10 w-full rounded-md border border-slate-200 bg-white px-3 text-sm text-slate-900 outline-none focus:border-primary-500 dark:border-slate-700 dark:bg-slate-950 dark:text-white"
              >
                {(field.options || []).map((option) => {
                  const value =
                    typeof option === "string"
                      ? option
                      : String(option.value ?? "");
                  const label =
                    typeof option === "string" ? option : option.label || value;
                  return (
                    <option key={value} value={value}>
                      {label}
                    </option>
                  );
                })}
              </select>
            ) : (
              <input
                type={field.type === "number" ? "number" : "text"}
                required={field.required}
                value={String(values[field.name] ?? "")}
                placeholder={field.placeholder}
                onChange={(event) =>
                  update(
                    field.name,
                    field.type === "number"
                      ? Number(event.target.value)
                      : event.target.value,
                  )
                }
                className="h-10 w-full rounded-md border border-slate-200 bg-white px-3 text-sm text-slate-900 outline-none focus:border-primary-500 dark:border-slate-700 dark:bg-slate-950 dark:text-white"
              />
            )}
          </label>
        ))}
        {message ? (
          <pre
            className={`max-h-48 overflow-auto rounded-md border px-3 py-2 text-xs ${
              failed
                ? "border-red-200 bg-red-50 text-red-700 dark:border-red-900/40 dark:bg-red-950/30 dark:text-red-300"
                : "border-slate-200 bg-slate-50 text-slate-700 dark:border-slate-800 dark:bg-slate-950 dark:text-slate-300"
            }`}
          >
            {message}
          </pre>
        ) : null}
      </div>
      <button
        type="submit"
        disabled={running}
        className="inline-flex h-10 items-center justify-center gap-2 rounded-md bg-primary-600 px-4 text-sm font-semibold text-white transition-colors hover:bg-primary-700 disabled:cursor-not-allowed disabled:opacity-60"
      >
        {running ? <Loader2 size={16} className="animate-spin" /> : null}
        {submitLabel}
      </button>
    </form>
  );
};

export default PluginExtensionSlot;
