import axios from 'axios';
import { useAuthStore } from '../stores/authStore';
import i18n from '../i18n';
import { safeStorage } from '../utils/storage';

// Initial base URL
const API_BASE_URL = safeStorage.getItem('active_url') || safeStorage.getItem('server_url') || (import.meta.env.PROD ? '' : 'http://localhost:3000');

const apiClient = axios.create({
  baseURL: API_BASE_URL,
  headers: {
    'Content-Type': 'application/json',
  },
});

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
