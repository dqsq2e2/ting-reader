import type {
  PluginCapabilityRegistration,
  ToolProviderRegistration,
} from "../types";
import apiClient from "./client";

export type PluginCapabilityInvokeResult<T = unknown> = {
  result: T;
};

export type SignPluginRouteRequest = {
  method: string;
  path: string;
  expires_in_seconds?: number;
  bind_current_user?: boolean;
};

export type SignPluginRouteResponse = {
  path: string;
  expires: number;
  signature: string;
  user_id?: string | null;
  signed_url: string;
};

export type InvokePluginHostRequest = {
  plugin_id: string;
  method: string;
  params?: unknown;
};

export type InvokePluginHostResponse<T = unknown> = {
  result: T;
};

export const listPluginCapabilities = async (kind?: string) => {
  const response = await apiClient.get<PluginCapabilityRegistration[]>(
    "/api/v1/plugin-capabilities",
    {
      params: kind ? { kind } : undefined,
    },
  );
  return response.data;
};

export const findContentProcessors = async (
  extension: string,
  operation?: string,
) => {
  const response = await apiClient.get<PluginCapabilityRegistration[]>(
    "/api/v1/plugin-capabilities/content-processors",
    {
      params: { extension, operation },
    },
  );
  return response.data;
};

export const findToolProviders = async (name?: string) => {
  const response = await apiClient.get<ToolProviderRegistration[]>(
    "/api/v1/plugin-capabilities/tools",
    {
      params: name ? { name } : undefined,
    },
  );
  return response.data;
};

export const findTaskHandlers = async (taskType?: string) => {
  const response = await apiClient.get<PluginCapabilityRegistration[]>(
    "/api/v1/plugin-capabilities/task-handlers",
    {
      params: taskType ? { task_type: taskType } : undefined,
    },
  );
  return response.data;
};

export const findEventHandlers = async (event?: string) => {
  const response = await apiClient.get<PluginCapabilityRegistration[]>(
    "/api/v1/plugin-capabilities/event-handlers",
    {
      params: event ? { event } : undefined,
    },
  );
  return response.data;
};

export const invokePluginCapability = async <T = unknown>(
  pluginId: string,
  capabilityId: string,
  params: unknown = {},
) => {
  const response = await apiClient.post<PluginCapabilityInvokeResult<T>>(
    `/api/v1/plugins/${encodeURIComponent(pluginId)}/capabilities/${encodeURIComponent(capabilityId)}/invoke`,
    { params },
  );
  return response.data.result;
};

export const signPluginRoute = async (request: SignPluginRouteRequest) => {
  const response = await apiClient.post<SignPluginRouteResponse>(
    "/api/v1/plugin-route-signatures",
    request,
  );
  return response.data;
};

export const invokePluginHost = async <T = unknown>(
  request: InvokePluginHostRequest,
) => {
  const response = await apiClient.post<InvokePluginHostResponse<T>>(
    "/api/v1/plugin-host/invoke",
    request,
  );
  return response.data.result;
};
