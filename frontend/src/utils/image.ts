import { useAuthStore } from '../store/authStore';

export const getCoverUrl = (url?: string, libraryId?: string, bookId?: string) => {
  // Prioritize the store's activeUrl, which is dynamically updated and authoritative
  let baseUrl = useAuthStore.getState().activeUrl;

  // Fallback to localStorage directly if store is empty (e.g. early init)
  if (!baseUrl) {
    baseUrl = localStorage.getItem('active_url') || localStorage.getItem('server_url') || '';
  }

  // Final fallback for Web Dev environment
  if (!baseUrl) {
     baseUrl = import.meta.env.VITE_API_BASE_URL || (import.meta.env.PROD ? '' : 'http://localhost:3000');
  }
  
  // Remove trailing slash from baseUrl if present to avoid double slashes
  if (baseUrl.endsWith('/')) {
    baseUrl = baseUrl.slice(0, -1);
  }

  const token = useAuthStore.getState().token || localStorage.getItem('auth_token');
  
  if (!url) return '/placeholder-cover.png';
  if (url.startsWith('http')) return url;
  
  // If we have a libraryId, use the proxy endpoint
  if (libraryId) {
    let coverUrl = `${baseUrl}/api/proxy/cover?path=${encodeURIComponent(url)}&libraryId=${libraryId}`;
    
    if (url === 'embedded://first-chapter' && bookId) {
      coverUrl += `&bookId=${bookId}`;
    }
    
    if (token) {
      coverUrl += `&token=${token}`;
    }
    return coverUrl;
  }

  // If no libraryId but it's a relative path, prepend baseUrl (Crucial for App/Electron)
  // This handles cases where url is already a direct API path or static asset
  if (url.startsWith('/')) {
      return `${baseUrl}${url}`;
  }
  
  return url;
};
