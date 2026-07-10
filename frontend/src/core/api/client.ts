import axios, { type AxiosAdapter, type AxiosRequestConfig, type AxiosResponse } from 'axios';
import { useAuthStore } from '../stores/authStore';
import i18n from '../i18n';
import { safeStorage } from '../utils/storage';

// Initial base URL
const API_BASE_URL = safeStorage.getItem('active_url') || safeStorage.getItem('server_url') || (import.meta.env.PROD ? '' : 'http://localhost:3000');

const apiClient = axios.create({
  baseURL: API_BASE_URL,
  timeout: 45000,
  headers: {
    'Content-Type': 'application/json',
  },
});

const mutationMethods = new Set(['post', 'put', 'patch', 'delete']);
const inFlightMutations = new Map<string, Promise<AxiosResponse>>();
const defaultAdapter = axios.getAdapter(apiClient.defaults.adapter) as AxiosAdapter;

const stableSerialize = (value: unknown): string => {
  if (value === null || value === undefined) return '';
  if (typeof FormData !== 'undefined' && value instanceof FormData) {
    return JSON.stringify(
      Array.from(value.entries()).map(([key, entry]) => [
        key,
        typeof entry === 'string' ? entry : `${entry.name}:${entry.size}:${entry.type}`,
      ]),
    );
  }
  if (Array.isArray(value)) return `[${value.map(stableSerialize).join(',')}]`;
  if (typeof value === 'object') {
    return `{${Object.entries(value as Record<string, unknown>)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, entry]) => `${JSON.stringify(key)}:${stableSerialize(entry)}`)
      .join(',')}}`;
  }
  return JSON.stringify(value);
};

const mutationFingerprint = (config: AxiosRequestConfig) => [
  config.method?.toLowerCase(),
  config.baseURL,
  config.url,
  stableSerialize(config.params),
  stableSerialize(config.data),
].join('|');

const createIdempotencyKey = () => {
  const now = Date.now();
  return typeof crypto !== 'undefined' && crypto.randomUUID
    ? crypto.randomUUID()
    : `${now}-${Math.random().toString(36).slice(2)}`;
};

const singleFlightAdapter: AxiosAdapter = (config) => {
  const fingerprint = mutationFingerprint(config);
  const existing = inFlightMutations.get(fingerprint);
  if (existing) return existing;

  if (!config.headers.has('Idempotency-Key')) {
    config.headers.set('Idempotency-Key', createIdempotencyKey());
  }
  const request = defaultAdapter(config).finally(() => {
    inFlightMutations.delete(fingerprint);
  });
  inFlightMutations.set(fingerprint, request);
  return request;
};

apiClient.interceptors.request.use((config) => {
  const { token, activeUrl } = useAuthStore.getState();
  
  // Update baseURL dynamically from store
  if (activeUrl && !config.url?.startsWith('http')) {
    config.baseURL = activeUrl;
  }

  if (token) {
    config.headers.Authorization = `Bearer ${token}`;
  }
  config.headers['Accept-Language'] = i18n.resolvedLanguage || i18n.language || 'zh-CN';

  if (mutationMethods.has(config.method?.toLowerCase() || '')) {
    config.adapter = singleFlightAdapter;
  }

  return config;
});

apiClient.interceptors.response.use(
  (response) => {
    // Check if we were redirected and update activeUrl if needed
    if (response.request && response.request.responseURL) {
      // ... (existing logic if needed)
    }

    return response;
  },
  async (error) => {
    const originalRequest = error.config;
    
    // Handle 401 Unauthorized
    if (error.response?.status === 401 && !originalRequest._retry) {
      useAuthStore.getState().logout();
      window.location.href = '/login';
      return Promise.reject(error);
    }

    // Handle Network Error or Connection Refused (potentially redirect expired)
    // Only in Electron environment where we manage serverUrl/activeUrl
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    if (!error.response && !originalRequest._retry && (window as any).electronAPI) {
      const { serverUrl, setActiveUrl } = useAuthStore.getState();

      // If we have a serverUrl and it's different or we want to re-verify
      if (serverUrl) {
        console.log('网络错误，尝试重新解析服务器 URL:', serverUrl);
        originalRequest._retry = true;
        
        try {
           // Call Electron IPC to resolve again
           // eslint-disable-next-line @typescript-eslint/no-explicit-any
           const result = await (window as any).electronAPI.resolveRedirect(serverUrl);
           const newActiveUrl = (result && result.finalUrl) ? result.finalUrl : serverUrl;
           
           if (newActiveUrl) {
             console.log('解析了新的活动 URL:', newActiveUrl);
             setActiveUrl(newActiveUrl);
             
             // Update request baseURL and retry
             originalRequest.baseURL = newActiveUrl;
             
             return apiClient(originalRequest);
           }
        } catch (resolveErr) {
          console.error('重新解析服务器 URL 失败', resolveErr);
        }
      }
    }
    
    return Promise.reject(error);
  }
);

export default apiClient;
