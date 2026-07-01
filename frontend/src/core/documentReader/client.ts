import {
  findContentProcessors,
  invokePluginCapability,
} from "../api/pluginCapabilities";
import type {
  DocumentChunk,
  DocumentMetadata,
  DocumentPageRender,
  DocumentProcessorRegistration,
  DocumentProbeResult,
  DocumentReaderOperation,
  DocumentReaderSession,
  DocumentResourceRef,
  DocumentSection,
} from "./types";

type DocumentOperationParams = {
  resource: DocumentResourceRef;
  operation: DocumentReaderOperation;
  [key: string]: unknown;
};

type DocumentProcessorOrSession =
  DocumentProcessorRegistration | DocumentReaderSession;

const getResourceExtension = (resource: DocumentResourceRef) => {
  if (resource.extension) {
    return resource.extension.replace(/^\./, "");
  }

  const path = resource.uri.split("?")[0] || "";
  const match = path.match(/\.([a-z0-9]+)$/i);
  return match?.[1] || "";
};

export const findDocumentProcessors = async (
  resource: DocumentResourceRef,
  operation?: DocumentReaderOperation,
) => findContentProcessors(getResourceExtension(resource), operation);

export const invokeDocumentProcessor = async <T>(
  processor: DocumentProcessorRegistration,
  params: DocumentOperationParams,
) =>
  invokePluginCapability<T>(
    processor.plugin_id,
    processor.capability.id,
    params,
  );

const processorFrom = (
  processorOrSession?: DocumentProcessorOrSession,
): DocumentProcessorRegistration | undefined => {
  if (!processorOrSession) return undefined;
  return "processor" in processorOrSession
    ? processorOrSession.processor
    : processorOrSession;
};

export const openDocumentSession = async (
  resource: DocumentResourceRef,
): Promise<DocumentReaderSession | undefined> => {
  const processors = await findDocumentProcessors(resource, "probe");
  let best: DocumentReaderSession | undefined;

  for (const processor of processors) {
    try {
      const probe = await invokeDocumentProcessor<DocumentProbeResult>(
        processor,
        {
          resource,
          operation: "probe",
        },
      );
      if (!probe?.supported) continue;

      const confidence = probe.confidence ?? 0;
      const bestConfidence = best?.probe?.confidence ?? 0;
      if (!best || confidence > bestConfidence) {
        best = { resource, processor, probe };
      }
    } catch {
      // A broken processor should not prevent another plugin from handling the document.
    }
  }

  return best;
};

export const probeDocument = async (
  resource: DocumentResourceRef,
): Promise<DocumentProbeResult | undefined> => {
  const session = await openDocumentSession(resource);
  return session?.probe;
};

export const extractDocumentMetadata = async (
  resource: DocumentResourceRef,
  processor?: DocumentProcessorOrSession,
) => {
  const selected =
    processorFrom(processor) ||
    (await findDocumentProcessors(resource, "extract_metadata"))[0];
  return selected
    ? invokeDocumentProcessor<DocumentMetadata>(selected, {
        resource,
        operation: "extract_metadata",
      })
    : undefined;
};

export const listDocumentSections = async (
  resource: DocumentResourceRef,
  processor?: DocumentProcessorOrSession,
) => {
  const selected =
    processorFrom(processor) ||
    (await findDocumentProcessors(resource, "list_sections"))[0];
  return selected
    ? invokeDocumentProcessor<DocumentSection[]>(selected, {
        resource,
        operation: "list_sections",
      })
    : [];
};

export const readDocumentChunk = async (
  resource: DocumentResourceRef,
  params: {
    sectionId?: string;
    cursor?: string;
    limit?: number;
  } = {},
  processor?: DocumentProcessorOrSession,
) => {
  const selected =
    processorFrom(processor) ||
    (await findDocumentProcessors(resource, "read_chunk"))[0];
  return selected
    ? invokeDocumentProcessor<DocumentChunk>(selected, {
        resource,
        operation: "read_chunk",
        ...params,
      })
    : undefined;
};

export const renderDocumentPage = async (
  resource: DocumentResourceRef,
  page: number,
  processor?: DocumentProcessorOrSession,
) => {
  const selected =
    processorFrom(processor) ||
    (await findDocumentProcessors(resource, "render_page"))[0];
  return selected
    ? invokeDocumentProcessor<DocumentPageRender>(selected, {
        resource,
        operation: "render_page",
        page,
      })
    : undefined;
};
