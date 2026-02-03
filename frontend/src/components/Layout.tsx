import React, { useState } from 'react';
import { Outlet, Link, useLocation, useNavigate } from 'react-router-dom';
import { 
  Home, 
  Library, 
  Search, 
  Heart, 
  Settings, 
  User, 
  LogOut, 
  Menu, 
  X,
  Database,
  Users,
  Terminal,
  Headphones
} from 'lucide-react';
import { useAuthStore } from '../store/authStore';
import { useTheme } from '../hooks/useTheme';
import { usePlayerStore } from '../store/playerStore';
import apiClient from '../api/client';

import Player from './Player';

const Layout: React.FC = () => {
  const { refreshTheme } = useTheme(); // Initialize theme application
  const [isSidebarOpen, setIsSidebarOpen] = useState(false);
  const location = useLocation();
  const navigate = useNavigate();
  
  // Use selectors to prevent unnecessary re-renders when currentTime updates
  const user = useAuthStore(state => state.user);
  const logout = useAuthStore(state => state.logout);
  const hasCurrentChapter = usePlayerStore(state => !!state.currentChapter);
  const setPlaybackSpeed = usePlayerStore(state => state.setPlaybackSpeed);

  // Fetch and apply user settings
  React.useEffect(() => {
    if (user) {
      apiClient.get('/api/settings').then(res => {
        const settings = res.data;
        if (settings.playback_speed) {
          setPlaybackSpeed(settings.playback_speed);
        }
      }).catch(err => console.error('Failed to sync user settings', err));
    }
  }, [user?.id, setPlaybackSpeed]);

  React.useEffect(() => {
    refreshTheme();
  }, []);

  const menuItems = [
    { icon: <Home size={20} />, label: '首页', path: '/' },
    { icon: <Library size={20} />, label: '书架', path: '/bookshelf' },
    { icon: <Search size={20} />, label: '搜索', path: '/search' },
    { icon: <Heart size={20} />, label: '收藏', path: '/favorites' },
  ];

  const adminItems = [
    { icon: <Database size={20} />, label: '库管理', path: '/admin/libraries' },
    { icon: <Terminal size={20} />, label: '任务日志', path: '/admin/tasks' },
    { icon: <Users size={20} />, label: '用户管理', path: '/admin/users' },
  ];

  const handleLogout = () => {
    logout();
    navigate('/login');
  };

  const NavLink = ({ item, mobile = false }: { item: typeof menuItems[0], mobile?: boolean }) => {
    const isActive = location.pathname === item.path;
    
    if (mobile) {
      return (
        <Link
          to={item.path}
          className={`flex flex-col items-center justify-center flex-1 py-1 transition-all ${
            isActive ? 'text-primary-600' : 'text-slate-500 dark:text-slate-400'
          }`}
        >
          <div className={`p-1.5 rounded-xl transition-all ${isActive ? 'bg-primary-50 dark:bg-primary-900/20' : ''}`}>
            {React.cloneElement(item.icon as React.ReactElement<any>, { size: 22 })}
          </div>
          <span className="text-[10px] font-bold mt-0.5">{item.label}</span>
        </Link>
      );
    }

    return (
      <Link
        to={item.path}
        onClick={() => setIsSidebarOpen(false)}
        className={`flex items-center gap-3 px-4 py-3 rounded-xl transition-all ${
          isActive 
            ? 'bg-primary-600 text-white shadow-lg shadow-primary-500/30' 
            : 'text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800'
        }`}
      >
        {item.icon}
        <span className="font-medium">{item.label}</span>
      </Link>
    );
  };

  return (
    <div className="flex h-screen bg-slate-50 dark:bg-slate-950 overflow-hidden">
      {/* Sidebar Overlay */}
      {isSidebarOpen && (
        <div 
          className="xl:hidden fixed inset-0 bg-slate-900/60 z-40 backdrop-blur-sm animate-in fade-in duration-300"
          onClick={() => setIsSidebarOpen(false)}
        />
      )}

      {/* Sidebar */}
      <aside className={`
        fixed xl:sticky top-0 inset-y-0 left-0 w-72 bg-white dark:bg-slate-900 border-r border-slate-200 dark:border-slate-800 z-[100] transform transition-transform duration-300 ease-out xl:translate-x-0
        ${isSidebarOpen ? 'translate-x-0' : '-translate-x-full'}
      `}>
        <div className="flex flex-col h-full p-4">
          <div className="hidden xl:flex items-center gap-3 px-4 py-6 mb-4">
            <img src="/logo.png" alt="Logo" className="w-10 h-10 shadow-lg shadow-primary-500/10 object-contain" />
            <span className="font-bold text-xl dark:text-white tracking-tight">Ting Reader</span>
          </div>

          <nav className="flex-1 space-y-1 overflow-y-auto custom-scrollbar">
            <div className="xl:block hidden">
              <div className="text-xs font-bold text-slate-400 uppercase tracking-widest px-4 mb-2 mt-4">主菜单</div>
              {menuItems.map((item) => <NavLink key={item.path} item={item} />)}
            </div>

            <div className="xl:mt-8">
              <div className="text-xs font-bold text-slate-400 uppercase tracking-widest px-4 mb-2 mt-4 xl:mt-0">管理后台</div>
              {user?.role === 'admin' && adminItems.map((item) => <NavLink key={item.path} item={item} />)}
              <Link
                to="/settings"
                onClick={() => setIsSidebarOpen(false)}
                className={`flex items-center gap-3 px-4 py-3 rounded-xl transition-all ${
                  location.pathname === '/settings'
                    ? 'bg-primary-600 text-white shadow-lg shadow-primary-500/30'
                    : 'text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800'
                }`}
              >
                <Settings size={20} />
                <span className="font-medium">系统设置</span>
              </Link>
            </div>
          </nav>

          <div className="mt-auto pt-4 border-t border-slate-100 dark:border-slate-800">
            <div className="flex items-center justify-between bg-slate-50 dark:bg-slate-800/50 p-3 rounded-2xl">
              <div className="flex items-center gap-3 overflow-hidden">
                <div className="w-10 h-10 rounded-full bg-primary-100 dark:bg-primary-900/30 flex items-center justify-center text-primary-600 shrink-0 font-bold text-sm">
                  {user?.username.charAt(0).toUpperCase()}
                </div>
                <div className="truncate">
                  <p className="text-sm font-bold dark:text-white truncate">{user?.username}</p>
                  <p className="text-[10px] font-bold text-slate-400 uppercase tracking-tight">{user?.role === 'admin' ? 'Administrator' : 'User'}</p>
                </div>
              </div>
              <button 
                onClick={handleLogout}
                className="p-2 text-slate-400 hover:text-red-500 transition-colors"
                title="退出登录"
              >
                <LogOut size={20} />
              </button>
            </div>
          </div>
        </div>
      </aside>

      {/* Main Content Wrapper */}
      <div className="flex-1 flex flex-col min-w-0 overflow-hidden relative">
        {/* Mobile Header */}
        <div className="xl:hidden h-16 shrink-0 bg-white/80 dark:bg-slate-900/80 backdrop-blur-md border-b border-slate-200 dark:border-slate-800 flex items-center justify-between px-4 z-40 pt-[env(safe-area-inset-top)]">
          <div className="flex items-center gap-2">
            <img src="/logo.png" alt="Logo" className="w-9 h-9 shadow-lg shadow-primary-500/10 object-contain" />
            <span className="font-bold text-lg dark:text-white tracking-tight">Ting Reader</span>
          </div>
          <div className="flex items-center gap-2">
            <button 
              onClick={() => setIsSidebarOpen(!isSidebarOpen)}
              className="p-2 text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-full transition-colors"
            >
              {isSidebarOpen ? <X size={24} /> : <Menu size={24} />}
            </button>
          </div>
        </div>

        {/* Main Content Area */}
        <main 
          id="main-content" 
          className="flex-1 overflow-y-auto relative flex flex-col min-h-0 scroll-smooth transition-colors duration-1000"
          style={{ backgroundColor: 'var(--page-background, transparent)' }}
        >
          <Outlet />
        </main>

        {/* Mobile Bottom Nav */}
        <div 
          className="xl:hidden shrink-0 bg-white/90 dark:bg-slate-900/90 backdrop-blur-lg border-t border-slate-200 dark:border-slate-800 px-2 flex items-center justify-around z-40 shadow-[0_-4px_12px_rgba(0,0,0,0.05)]"
          style={{ 
            paddingBottom: 'env(safe-area-inset-bottom, 0px)',
            height: 'calc(var(--bottom-nav-h) + env(safe-area-inset-bottom, 0px))'
          }}
        >
          {menuItems.map((item) => <NavLink key={item.path} item={item} mobile />)}
        </div>

        {/* Player - Moved inside the right-side container to prevent sidebar overlap */}
        {hasCurrentChapter && <Player />}
      </div>
    </div>
  );
};

export default Layout;
