import React, { useEffect, useState } from 'react';
import { useParams, useSearchParams } from 'react-router-dom';
import apiClient from '../api/client';
import Player from '../components/Player';
import { usePlayerStore } from '../store/playerStore';
import { useAuthStore } from '../store/authStore';
import { Search, ArrowLeft, User, Lock, LogIn } from 'lucide-react';
import { getCoverUrl } from '../utils/image';

const WidgetPage: React.FC = () => {
  const { id } = useParams();
  const [searchParams] = useSearchParams();
  const token = searchParams.get('token');
  const { setToken, setAuth, isAuthenticated } = useAuthStore();
  const { playChapter } = usePlayerStore();
  
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const [books, setBooks] = useState<any[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [loading, setLoading] = useState(true);
  const [showBookList, setShowBookList] = useState(!id);
  
  // Login State
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [loginError, setLoginError] = useState('');
  const [isLoggingIn, setIsLoggingIn] = useState(false);

  // Set token if provided in URL
  useEffect(() => {
    if (token) {
      // Decode user info from token (simple jwt decode)
      try {
        const payload = JSON.parse(atob(token.split('.')[1]));
        const user = { 
          id: payload.userId, 
          username: payload.username, 
          role: payload.role 
        };
        setAuth(user, token);
      } catch (e) {
        console.error('Invalid token in widget', e);
        setToken(token); // Fallback
      }
    }
  }, [token, setToken, setAuth]);

  // Load widget CSS from settings
  useEffect(() => {
    if (isAuthenticated) {
      apiClient.get('/api/settings').then(res => {
        // API returns camelCase
        const css = res.data.widgetCss ?? res.data.widget_css;
        if (css) {
          const styleId = 'widget-custom-css';
          let style = document.getElementById(styleId) as HTMLStyleElement;
          if (!style) {
            style = document.createElement('style');
            style.id = styleId;
            document.head.appendChild(style);
          }
          style.innerHTML = css;
        }
      }).catch(err => console.error('Failed to load widget settings', err));
    }
  }, [isAuthenticated]);

  // Fetch books for selection
  useEffect(() => {
    if (!isAuthenticated) return;
    
    const fetchBooks = async () => {
      try {
        const res = await apiClient.get('/api/books', { params: { search: searchQuery } });
        setBooks(res.data);
      } catch (err) {
        console.error('Failed to fetch books', err);
      } finally {
        setLoading(false);
      }
    };
    fetchBooks();
  }, [searchQuery, isAuthenticated]);

  // If ID is provided, auto-load that book
  useEffect(() => {
    if (id && isAuthenticated) {
      const loadBook = async () => {
        try {
          const res = await apiClient.get(`/api/books/${id}`);
          const book = res.data;
          const chaptersRes = await apiClient.get(`/api/books/${id}/chapters`);
          const chapters = chaptersRes.data;
          
          let progress = { chapter_id: '' };
          try {
             const progressRes = await apiClient.get(`/api/progress/${id}`);
             progress = progressRes.data;
          } catch {
            // Ignore progress fetch error, maybe new book
          }
          
          let targetChapter = chapters[0];
          if (progress?.chapter_id) {
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
            targetChapter = chapters.find((c: any) => c.id === progress.chapter_id) || chapters[0];
          }
          
          playChapter(book, chapters, targetChapter);
          setShowBookList(false);
        } catch (err) {
          console.error('Failed to load book for widget', err);
        }
      };
      loadBook();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id, isAuthenticated]);

  const handleLogin = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoginError('');
    setIsLoggingIn(true);

    try {
      const response = await apiClient.post('/api/auth/login', { username, password });
      const { token, user } = response.data;
      setAuth(user, token);
      // Login successful, state updates will trigger re-renders
    } catch (err: unknown) {
      console.error('Login failed', err);
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const msg = (err as any)?.response?.data?.error || (err as any)?.response?.data?.message || '登录失败';
      setLoginError(msg);
    } finally {
      setIsLoggingIn(false);
    }
  };

  if (!isAuthenticated) {
    return (
      <div className="h-screen flex flex-col items-center justify-center bg-slate-50 dark:bg-slate-950 p-4 widget-mode overflow-hidden">
        <div className="w-full max-w-xs bg-white dark:bg-slate-900 rounded-2xl shadow-sm p-4 border border-slate-100 dark:border-slate-800">
          <form onSubmit={handleLogin} className="space-y-2">
            <div className="flex gap-2">
              <div className="relative flex-1">
                <User size={12} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-slate-400" />
                <input
                  type="text"
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  placeholder="用户名"
                  className="w-full pl-8 pr-2 py-1.5 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-lg text-xs outline-none focus:ring-1 focus:ring-primary-500 dark:text-white"
                  required
                />
              </div>
              <div className="relative flex-1">
                <Lock size={12} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-slate-400" />
                <input
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  placeholder="密码"
                  className="w-full pl-8 pr-2 py-1.5 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-lg text-xs outline-none focus:ring-1 focus:ring-primary-500 dark:text-white"
                  required
                />
              </div>
            </div>

            {loginError && (
              <div className="text-red-500 text-[10px] text-center">
                {loginError}
              </div>
            )}

            <button
              type="submit"
              disabled={isLoggingIn}
              className="w-full py-1.5 bg-primary-600 hover:bg-primary-700 text-white text-xs font-bold rounded-lg shadow-sm transition-all flex items-center justify-center gap-1"
            >
              {isLoggingIn ? (
                <div className="w-3 h-3 border-2 border-white/30 border-t-white rounded-full animate-spin" />
              ) : (
                <>
                  <LogIn size={12} />
                  登录
                </>
              )}
            </button>
          </form>
        </div>
      </div>
    );
  }

  if (showBookList) {
    return (
      <div className="h-screen bg-white dark:bg-slate-950 flex flex-col p-4 overflow-hidden widget-mode">
        <div className="relative mb-4">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" size={16} />
          <input 
            type="text" 
            placeholder="搜索专辑..."
            value={searchQuery}
            onChange={e => setSearchQuery(e.target.value)}
            className="w-full pl-10 pr-4 py-2 bg-slate-100 dark:bg-slate-900 border-none rounded-xl text-xs outline-none dark:text-white"
          />
        </div>
        <div className="flex-1 overflow-y-auto space-y-2 scrollbar-hide">
          {loading ? (
            <div className="flex justify-center py-8">
              <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-primary-600"></div>
            </div>
          ) : books.length > 0 ? (
            books.map(book => (
              <div 
                key={book.id}
                onClick={() => {
                  setShowBookList(false);
                  window.history.pushState({}, '', `/widget/${book.id}${token ? `?token=${token}` : ''}`);
                  // Reload book logic
                  apiClient.get(`/api/books/${book.id}`).then(async (res) => {
                    const chaptersRes = await apiClient.get(`/api/books/${book.id}/chapters`);
                    playChapter(res.data, chaptersRes.data, chaptersRes.data[0]);
                  });
                }}
                className="flex items-center gap-3 p-2 hover:bg-slate-50 dark:hover:bg-slate-900 rounded-xl cursor-pointer transition-colors"
              >
                <img 
                  src={getCoverUrl(book.coverUrl || book.cover_url, book.libraryId || book.library_id, book.id)} 
                  className="w-10 h-10 rounded-lg object-cover" 
                  alt={book.title}
                />
                <div className="min-w-0">
                  <p className="font-bold text-xs truncate dark:text-white">{book.title}</p>
                  <p className="text-[10px] text-slate-500 truncate">{book.author}</p>
                </div>
              </div>
            ))
          ) : (
            <p className="text-center text-xs text-slate-400 py-8">未找到相关书籍</p>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="h-screen bg-transparent flex flex-col relative overflow-hidden widget-mode">
      <button 
        onClick={() => setShowBookList(true)}
        className="absolute top-2 left-2 z-[200] p-1.5 bg-white/80 dark:bg-slate-900/80 rounded-full shadow-sm text-slate-500 hover:text-primary-600 transition-colors"
        title="更换专辑"
      >
        <ArrowLeft size={14} />
      </button>
      
      <div className="flex-1">
        <Player />
      </div>

      <style>{`
        /* Widget Mode Adjustments */
        .widget-mode .player-container { bottom: 0 !important; }
        .widget-mode .mini-player-offset { --mini-player-offset: 0px !important; }
        
        /* Force player into specific state if needed */
        #root { height: 100vh; margin: 0; padding: 0; }
        body { margin: 0; overflow: hidden; background: transparent !important; }

        /* Hide some UI elements that don't make sense in a small widget */
        .widget-mode [title="展开播放器"] { display: none !important; }
      `}</style>
    </div>
  );
};

export default WidgetPage;
