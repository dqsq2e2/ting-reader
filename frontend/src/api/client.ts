import axios from 'axios';
import { useAuthStore } from '../store/authStore';

// Initial base URL
const API_BASE_URL = localStorage.getItem('active_url') || localStorage.getItem('server_url') || (import.meta.env.PROD ? '' : 'http://localhost:3000');

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
  return config;
});

apiClient.interceptors.response.use(
  (response) => {
    // Check if we were redirected and update activeUrl if needed
    // Note: This relies on the browser/XHR exposing the final URL
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
    if (!error.response && !originalRequest._retry && (window as any).electronAPI) {
      const { serverUrl, activeUrl, setActiveUrl } = useAuthStore.getState();

      // If we have a serverUrl and it's different or we want to re-verify
      if (serverUrl) {
        console.log('Network error, attempting to re-resolve server URL from:', serverUrl);
        originalRequest._retry = true;
        
        try {
           // Call Electron IPC to resolve again
           const result = await (window as any).electronAPI.resolveRedirect(serverUrl);
           const newActiveUrl = (result && result.finalUrl) ? result.finalUrl : serverUrl;
           
           if (newActiveUrl) {
             console.log('Resolved new active URL:', newActiveUrl);
             setActiveUrl(newActiveUrl);
             
             // Update request baseURL and retry
             originalRequest.baseURL = newActiveUrl;
             // Ensure the url is relative to the new baseURL if it was absolute
             // Actually axios merges baseURL + url. If url is absolute, baseURL is ignored.
             // We need to be careful. Usually 'url' in config is relative (e.g. /api/auth/login).
             
             return apiClient(originalRequest);
           }
        } catch (resolveErr) {
          console.error('Failed to re-resolve server URL', resolveErr);
        }
      }
    }
    
    return Promise.reject(error);
  }
);

export default apiClient;
