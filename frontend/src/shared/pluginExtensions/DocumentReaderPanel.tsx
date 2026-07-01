import { Loader2 } from "lucide-react";
import { useMemo, useState } from "react";
import {
  extractDocumentMetadata,
  listDocumentSections,
  openDocumentSession,
  readDocumentChunk,
  renderDocumentPage,
  type DocumentChunk,
  type DocumentMetadata,
  type DocumentPageRender,
  type DocumentReaderSession,
  type DocumentResourceRef,
  type DocumentSection,
} from "../../core/documentReader";

type DocumentReaderPanelProps = {
  context?: Record<string, unknown>;
};

const textValue = (value: unknown) =>
  typeof value === "string" && value.trim() ? value.trim() : undefined;

const extensionFromUri = (uri: string) =>
  uri.split("?")[0]?.match(/\.([a-z0-9]+)$/i)?.[1] || "";

const resourceFromContext = (
  context?: Record<string, unknown>,
): DocumentResourceRef => {
  const uri =
    textValue(context?.document_uri) ||
    textValue(context?.uri) ||
    textValue(context?.chapter_path) ||
    textValue(context?.book_path) ||
    "";
  const extension =
    textValue(context?.extension) ||
    textValue(context?.document_extension) ||
    extensionFromUri(uri);

  return {
    uri,
    extension,
    mimeType: textValue(context?.mime_type),
    bookId: textValue(context?.book_id),
    chapterId: textValue(context?.chapter_id),
  };
};

const formatJson = (value: unknown) => JSON.stringify(value, null, 2);

const DocumentReaderPanel = ({ context }: DocumentReaderPanelProps) => {
  const initialResource = useMemo(
    () => resourceFromContext(context),
    [context],
  );
  const [uri, setUri] = useState(initialResource.uri);
  const [extension, setExtension] = useState(initialResource.extension || "");
  const [session, setSession] = useState<DocumentReaderSession>();
  const [metadata, setMetadata] = useState<DocumentMetadata>();
  const [sections, setSections] = useState<DocumentSection[]>([]);
  const [sectionId, setSectionId] = useState("");
  const [chunk, setChunk] = useState<DocumentChunk>();
  const [pageNumber, setPageNumber] = useState(1);
  const [page, setPage] = useState<DocumentPageRender>();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string>();

  const resource = (): DocumentResourceRef => ({
    ...initialResource,
    uri: uri.trim(),
    extension: extension.trim() || extensionFromUri(uri),
  });

  const open = async () => {
    setLoading(true);
    setError(undefined);
    setSession(undefined);
    setMetadata(undefined);
    setSections([]);
    setChunk(undefined);
    setPage(undefined);
    try {
      const selected = await openDocumentSession(resource());
      if (!selected) {
        setError("No content processor supports this document.");
        return;
      }
      setSession(selected);
      const [nextMetadata, nextSections] = await Promise.all([
        extractDocumentMetadata(selected.resource, selected).catch(
          () => undefined,
        ),
        listDocumentSections(selected.resource, selected).catch(() => []),
      ]);
      setMetadata(nextMetadata);
      setSections(nextSections);
      setSectionId(nextSections[0]?.id || "");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  const readChunk = async () => {
    if (!session) return;
    setLoading(true);
    setError(undefined);
    try {
      setChunk(
        await readDocumentChunk(
          session.resource,
          { sectionId: sectionId || undefined, limit: 4000 },
          session,
        ),
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  const renderPage = async () => {
    if (!session) return;
    setLoading(true);
    setError(undefined);
    try {
      setPage(await renderDocumentPage(session.resource, pageNumber, session));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex flex-1 flex-col gap-4 p-5 text-sm">
      <div className="space-y-3">
        <label className="block">
          <span className="mb-1 block font-medium text-slate-700 dark:text-slate-200">
            URI
          </span>
          <input
            value={uri}
            onChange={(event) => setUri(event.target.value)}
            placeholder="book/chapter path or plugin resource URI"
            className="h-10 w-full rounded-md border border-slate-200 bg-white px-3 text-slate-900 outline-none focus:border-primary-500 dark:border-slate-700 dark:bg-slate-950 dark:text-white"
          />
        </label>
        <div className="flex gap-2">
          <label className="block w-28 shrink-0">
            <span className="mb-1 block font-medium text-slate-700 dark:text-slate-200">
              Ext
            </span>
            <input
              value={extension}
              onChange={(event) => setExtension(event.target.value)}
              placeholder="txt"
              className="h-10 w-full rounded-md border border-slate-200 bg-white px-3 text-slate-900 outline-none focus:border-primary-500 dark:border-slate-700 dark:bg-slate-950 dark:text-white"
            />
          </label>
          <button
            type="button"
            onClick={() => void open()}
            disabled={loading || !uri.trim()}
            className="mt-6 inline-flex h-10 flex-1 items-center justify-center gap-2 rounded-md bg-primary-600 px-4 font-semibold text-white transition-colors hover:bg-primary-700 disabled:cursor-not-allowed disabled:opacity-60"
          >
            {loading ? <Loader2 size={16} className="animate-spin" /> : null}
            Open
          </button>
        </div>
      </div>

      {error ? (
        <div className="rounded-md border border-red-200 bg-red-50 px-3 py-2 text-red-700 dark:border-red-900/40 dark:bg-red-950/30 dark:text-red-300">
          {error}
        </div>
      ) : null}

      {session ? (
        <div className="min-h-0 flex-1 space-y-4 overflow-auto pr-1">
          <pre className="rounded-md border border-slate-200 bg-slate-50 px-3 py-2 text-xs text-slate-700 dark:border-slate-800 dark:bg-slate-950 dark:text-slate-300">
            {formatJson({
              processor: session.processor.capability.id,
              probe: session.probe,
              metadata,
            })}
          </pre>

          <div className="flex gap-2">
            <select
              value={sectionId}
              onChange={(event) => setSectionId(event.target.value)}
              className="h-10 min-w-0 flex-1 rounded-md border border-slate-200 bg-white px-3 text-slate-900 outline-none focus:border-primary-500 dark:border-slate-700 dark:bg-slate-950 dark:text-white"
            >
              <option value="">Default section</option>
              {sections.map((section) => (
                <option key={section.id} value={section.id}>
                  {section.title || section.id}
                </option>
              ))}
            </select>
            <button
              type="button"
              onClick={() => void readChunk()}
              disabled={loading}
              className="inline-flex h-10 items-center justify-center rounded-md border border-slate-200 px-3 font-semibold text-slate-700 dark:border-slate-700 dark:text-slate-200"
            >
              Chunk
            </button>
          </div>

          {chunk ? (
            <pre className="max-h-72 overflow-auto rounded-md border border-slate-200 bg-white px-3 py-2 text-xs text-slate-800 dark:border-slate-800 dark:bg-slate-950 dark:text-slate-200">
              {chunk.text || chunk.html || formatJson(chunk)}
            </pre>
          ) : null}

          <div className="flex gap-2">
            <input
              type="number"
              min={1}
              value={pageNumber}
              onChange={(event) =>
                setPageNumber(Number(event.target.value) || 1)
              }
              className="h-10 w-24 rounded-md border border-slate-200 bg-white px-3 text-slate-900 outline-none focus:border-primary-500 dark:border-slate-700 dark:bg-slate-950 dark:text-white"
            />
            <button
              type="button"
              onClick={() => void renderPage()}
              disabled={loading}
              className="inline-flex h-10 flex-1 items-center justify-center rounded-md border border-slate-200 px-3 font-semibold text-slate-700 dark:border-slate-700 dark:text-slate-200"
            >
              Render page
            </button>
          </div>

          {page?.imageBase64 ? (
            <img
              alt={`Document page ${page.page}`}
              src={`data:image/png;base64,${page.imageBase64}`}
              className="max-h-96 w-full rounded-md border border-slate-200 object-contain dark:border-slate-800"
            />
          ) : page ? (
            <pre className="max-h-72 overflow-auto rounded-md border border-slate-200 bg-white px-3 py-2 text-xs text-slate-800 dark:border-slate-800 dark:bg-slate-950 dark:text-slate-200">
              {page.text || page.svg || formatJson(page)}
            </pre>
          ) : null}
        </div>
      ) : null}
    </div>
  );
};

export default DocumentReaderPanel;
