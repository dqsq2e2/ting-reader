import type { PluginCapabilityRegistration } from "../types";

export type DocumentReaderOperation =
  "probe" | "extract_metadata" | "list_sections" | "read_chunk" | "render_page";

export type DocumentResourceRef = {
  uri: string;
  extension?: string;
  mimeType?: string;
  bookId?: string;
  chapterId?: string;
};

export type DocumentProbeResult = {
  supported: boolean;
  confidence?: number;
  reason?: string;
};

export type DocumentMetadata = {
  title?: string;
  author?: string;
  language?: string;
  pageCount?: number;
  wordCount?: number;
  [key: string]: unknown;
};

export type DocumentSection = {
  id: string;
  title?: string;
  index?: number;
  pageStart?: number;
  pageEnd?: number;
  [key: string]: unknown;
};

export type DocumentChunk = {
  sectionId?: string;
  text?: string;
  html?: string;
  nextCursor?: string;
  progress?: number;
  [key: string]: unknown;
};

export type DocumentPageRender = {
  page: number;
  imageBase64?: string;
  svg?: string;
  text?: string;
  width?: number;
  height?: number;
  [key: string]: unknown;
};

export type DocumentProcessorRegistration = PluginCapabilityRegistration;

export type DocumentReaderSession = {
  resource: DocumentResourceRef;
  processor: DocumentProcessorRegistration;
  probe?: DocumentProbeResult;
};
