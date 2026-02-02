export const getCoverUrl = (url?: string, libraryId?: string, bookId?: string) => {
  const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || (import.meta.env.PROD ? '' : 'http://localhost:3000');
  const token = localStorage.getItem('auth_token');
  
  if (!url) return '/placeholder-cover.png';
  if (url.startsWith('http')) return url;
  if (!libraryId) return url;
  
  let coverUrl = `${API_BASE_URL}/api/proxy/cover?path=${encodeURIComponent(url)}&libraryId=${libraryId}`;
  
  if (url === 'embedded://first-chapter' && bookId) {
    coverUrl += `&bookId=${bookId}`;
  }
  
  if (token) {
    coverUrl += `&token=${token}`;
  }
  return coverUrl;
};
