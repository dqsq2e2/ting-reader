import axios, { type AxiosAdapter, type InternalAxiosRequestConfig } from 'axios';
import { createSingleFlightAdapter } from './mutations';
import { useAuthStore } from '../stores/authStore';
import i18n from '../i18n';
import { safeStorage } from '../utils/storage';
import { getGatewayBasePath, getRuntimeBaseUrl } from '../utils/runtimeUrl';

// Initial base URL
const API_BASE_URL = getRuntimeBaseUrl(
  safeStorage.getItem('active_url') || safeStorage.getItem('server_url'),
);

const apiClient = axios.create({
  baseURL: API_BASE_URL,
  timeout: 45000,
  headers: {
    'Content-Type': 'application/json',
  },
});

const mutationMethods = new Set(['post', 'put', 'patch', 'delete']);
const defaultAdapter = axios.getAdapter(apiClient.defaults.adapter) as AxiosAdapter;
const singleFlightAdapter = createSingleFlightAdapter(
  defaultAdapter,
  () => useAuthStore.getState().token || '',
);

type SessionRequest = InternalAxiosRequestConfig & {
  _sessionRevision?: number;
  _sessionToken?: string | null;
  _appRequest?: boolean;
  _authRetried?: boolean;
  _retry?: boolean;
};

apiClient.interceptors.request.use((config) => {
  const { token, activeUrl, sessionRevision } = useAuthStore.getState();
  const request = config as SessionRequest;
  request._sessionRevision = sessionRevision;
  request._sessionToken = token;
  
  // Update baseURL dynamically from store
  if (activeUrl && !config.url?.startsWith('http')) {
    config.baseURL = getRuntimeBaseUrl(activeUrl);
  }
  try {
    const base = new URL(getRuntimeBaseUrl(activeUrl) || window.location.origin);
    const target = new URL(axios.getUri(config), base);
    request._appRequest = target.origin === base.origin
      && target.pathname.startsWith(`${base.pathname.replace(/\/$/, '')}/api/`);
  } catch {
    request._appRequest = false;
  }

  // fnOS reserves Authorization for unified-gateway authentication. Sending
  // Ting Reader's JWT there makes nginx reject the request before it reaches
  // the application. Gateway requests use the same-origin token cookie.
  if (token && request._appRequest && !getGatewayBasePath()) {
    config.headers.set('Authorization', `Bearer ${token}`);
  } else {
    config.headers.delete('Authorization');
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
    const originalRequest = error.config as SessionRequest | undefined;
    const requestUrl = String(originalRequest?.url || '');
    const isAuthRequest = /\/api\/auth\/(?:login|register|logout)(?:[/?#]|$)/.test(requestUrl);
    
    // Handle 401 Unauthorized
    if (error.response?.status === 401 && originalRequest?._appRequest && !isAuthRequest) {
      const current = useAuthStore.getState();
      // An old account's failed request must not sign out a newer login.
      if (current.sessionRevision !== originalRequest._sessionRevision || !current.token) {
        return Promise.reject(error);
      }
      if (!originalRequest._authRetried) {
        originalRequest._authRetried = true;
        const result = current.token !== originalRequest._sessionToken
          ? 'restored' : await current.restoreSavedSession();
        const latest = useAuthStore.getState();
        if (latest.sessionRevision !== originalRequest._sessionRevision) return Promise.reject(error);
        if (result === 'restored' && !originalRequest.signal?.aborted) {
          return apiClient(originalRequest);
        }
        if (result === 'transient' || result === 'obsolete') return Promise.reject(error);
      }
      useAuthStore.getState().logout();
      const gatewayBasePath = getGatewayBasePath();
      window.location.href = `${gatewayBasePath}/login`;
      return Promise.reject(error);
    }

    // Handle Network Error or Connection Refused (potentially redirect expired)
    // Only in Electron environment where we manage serverUrl/activeUrl
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    if (!error.response && originalRequest && !originalRequest._retry && (window as any).electronAPI) {
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
