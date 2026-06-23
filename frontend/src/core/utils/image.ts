import { useAuthStore } from '../stores/authStore';

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
  
  // 处理带有 referer hash 的图片 URL (例如：http://example.com/image.jpg#referer=http://example.com)
  // 如果是这种情况，我们不能直接返回原 URL，而是要通过后端的代理接口来请求，这样后端可以把 referer 带上
  if (url.startsWith('http') && url.includes('#referer=')) {
    // 如果没有 libraryId，我们可以使用一个通用的代理接口，或者在现有的 proxy cover 接口中处理外部 URL
    // 因为这里我们主要是在请求外部 URL 的代理
    let coverUrl = `${baseUrl}/api/proxy/cover?path=${encodeURIComponent(url)}`;
    if (libraryId) {
      coverUrl += `&library_id=${libraryId}`;
    }
    if (token) {
      coverUrl += `&token=${token}`;
    }
    return coverUrl;
  }

  if (url.startsWith('http')) return url;
  
  // If we have a libraryId, use the proxy endpoint
  if (libraryId) {
    let coverUrl = `${baseUrl}/api/proxy/cover?path=${encodeURIComponent(url)}&library_id=${libraryId}`;
    
    if (url === 'embedded://first-chapter' && bookId) {
      coverUrl += `&book_id=${bookId}`;
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
