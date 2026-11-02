import React, { useState } from 'react';
import { Outlet, Link, useLocation, useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { 
  Home, 
  Library, 
  User,
  LogOut, 
  Menu, 
  X,
  Database,
  Users,
  Terminal,
  ListMusic,
  Puzzle,
  ChevronLeft,
  ChevronRight,
} from 'lucide-react';
import { useAuthStore } from '../../core/stores/authStore';
import { getRuntimeAssetUrl } from '../../core/utils/runtimeUrl';
import { useTheme } from '../../core/hooks/useTheme';
import { usePlayerStore } from '../../core/stores/playerStore';
import { normalizeLanguage } from '../../core/i18n/locales';
import { useAppLanguage } from '../../core/i18n/useAppLanguage';
import {
  getBrowserSessionId,
  hasSessionRestoreLogged,
  markSessionRestoreLogged,
} from '../../core/utils/sessionRestore';
import apiClient from '../../core/api/client';
import { setApplicationTimeZone } from '../../core/utils/timeZone';
import { useClientExtensions } from '../../core/hooks/useClientExtensions';
import { useUiPreferencesStore } from '../../core/stores/uiPreferencesStore';

import Player from '../../features/player/Player';
import DefaultAdminCredentialsDialog from '../../features/auth/DefaultAdminCredentialsDialog';
import { isMiniPlayerHiddenPath } from '../../features/player/platform';
import PluginExtensionHost from '../pluginExtensions/PluginExtensionHost';
import PluginExtensionIcon from '../pluginExtensions/PluginExtensionIcon';

type NavItem = {
  icon: React.ReactNode;
  label: string;
  path: string;
  matches?: string[];
};

const Layout: React.FC = () => {
  const { t } = useTranslation();
  const { setLanguage } = useAppLanguage();
  const { refreshTheme } = useTheme(); // Initialize theme application
  const [isSidebarOpen, setIsSidebarOpen] = useState(false);
  const [isConnecting, setIsConnecting] = useState(true);
  const [connectionError, setConnectionError] = useState<string | null>(null);
  const [dismissedDefaultCredentials, setDismissedDefaultCredentials] = useState<string | null>(null);
  const location = useLocation();
  const navigate = useNavigate();
  const isMiniPlayerHidden = isMiniPlayerHiddenPath(location.pathname);
  
  // Use selectors to prevent unnecessary re-renders when currentTime updates
  const user = useAuthStore(state => state.user);
  const sessionRevision = useAuthStore(state => state.sessionRevision);
  const setUser = useAuthStore(state => state.setUser);
  const logout = useAuthStore(state => state.logout);
  const serverUrl = useAuthStore(state => state.serverUrl);
  const credentialReminderIdentity = `${serverUrl}:${user?.id}`;
  const hasCurrentChapter = usePlayerStore(state => !!state.currentChapter);
  const setPlaybackSpeed = usePlayerStore(state => state.setPlaybackSpeed);
  const setIsExpanded = usePlayerStore(state => state.setIsExpanded);
  const sidebarCollapsed = useUiPreferencesStore(state => state.sidebarCollapsed);
  const toggleSidebarCollapsed = useUiPreferencesStore(state => state.toggleSidebarCollapsed);
  const setPluginToolMenuEnabled = useUiPreferencesStore(state => state.setPluginToolMenuEnabled);
  const setBookshelfProgressEnabled = useUiPreferencesStore(state => state.setBookshelfProgressEnabled);
  const { registry: pluginExtensionRegistry } = useClientExtensions();

  // Validate Token on Mount
  React.useEffect(() => {
    const token = useAuthStore.getState().token;
    const controller = new AbortController();
    const active = () => !controller.signal.aborted
      && useAuthStore.getState().sessionRevision === sessionRevision;
    const validateConnection = async () => {
      setIsConnecting(true);
      setConnectionError(null);
      try {
        if (hasSessionRestoreLogged(token)) {
          const response = await apiClient.get('/api/me', { signal: controller.signal });
          if (!active()) return;
          setUser(response.data);
        } else {
          const sessionId = getBrowserSessionId();
          const response = await apiClient.post(
            '/api/auth/session-restore',
            { session_id: sessionId },
            { headers: { 'X-Ting-Session-Id': sessionId }, signal: controller.signal },
          );
          if (!active()) return;
          setUser(response.data.user);
          markSessionRestoreLogged(useAuthStore.getState().token);
        }
        setIsConnecting(false);
      } catch (err: unknown) {
        if (!active()) return;
        console.error('Connection validation failed', err);
        // Don't auto-logout immediately, give user a chance to see error or retry
        setConnectionError('connection.failedMessage');
        setIsConnecting(false);
      }
    };

    if (token) {
      void validateConnection();
      return () => controller.abort();
    } else {
      const timer = window.setTimeout(() => setIsConnecting(false), 0);
      return () => {
        controller.abort();
        window.clearTimeout(timer);
      };
    }
  }, [sessionRevision, setUser]);

  // Fetch and apply user settings
  React.useEffect(() => {
    if (user && !isConnecting && !connectionError) {
      setPluginToolMenuEnabled(true);
      setBookshelfProgressEnabled(true);
      apiClient.get('/api/settings').then(res => {
        const settings = res.data;
        const speed = settings.playback_speed;
        if (speed) {
          setPlaybackSpeed(speed);
        }
        const language = settings.language || settings.settings_json?.language;
        if (language) {
          void setLanguage(normalizeLanguage(language), false);
        }
        setPluginToolMenuEnabled(
          settings.plugin_tool_menu_enabled
            ?? settings.settings_json?.plugin_tool_menu_enabled
            ?? true,
        );
        setBookshelfProgressEnabled(settings.bookshelf_progress_enabled ?? settings.settings_json?.bookshelf_progress_enabled ?? true);
      }).catch(err => console.error('Failed to sync user settings', err));
    }
  }, [user, setPlaybackSpeed, isConnecting, connectionError, setLanguage, setPluginToolMenuEnabled, setBookshelfProgressEnabled]);

  React.useEffect(() => {
    if (!user || isConnecting || connectionError) return;
    apiClient.get('/api/system/time-zone')
      .then((response) => {
        if (typeof response.data?.time_zone === 'string') {
          setApplicationTimeZone(response.data.time_zone);
        }
      })
      .catch((error) => console.error('Failed to sync application time zone', error));
  }, [user, isConnecting, connectionError]);

  React.useEffect(() => {
    refreshTheme();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const menuItems = [
    { icon: <Home size={20} />, label: t('nav.home'), path: '/' },
    { icon: <Library size={20} />, label: t('nav.bookshelf'), path: '/bookshelf', matches: ['/bookshelf', '/book/', '/series/', '/search'] },
    { icon: <ListMusic size={20} />, label: t('nav.playlists'), path: '/playlists', matches: ['/playlists'] },
    { icon: <User size={20} />, label: t('nav.mine'), path: '/mine', matches: ['/mine', '/history', '/bookmarks', '/favorites', '/personalization', '/notifications', '/statistics', '/admin/statistics', '/cache'] },
  ] satisfies NavItem[];

  const adminItems = [
    { icon: <Database size={20} />, label: t('nav.libraries'), path: '/admin/libraries' },
    { icon: <Puzzle size={20} />, label: t('nav.plugins'), path: '/admin/plugins' },
    { icon: <Terminal size={20} />, label: t('nav.logs'), path: '/admin/logs' },
    { icon: <Users size={20} />, label: t('nav.users'), path: '/admin/users' },
  ] satisfies NavItem[];

  const pluginPageItems = (pluginExtensionRegistry.bySlot['app.sidebar_page'] || []).map((extension) => ({
    icon: <PluginExtensionIcon extension={extension} size={20} />,
    label: extension.title || extension.pluginName || extension.capability.id,
    path: `/plugin-pages/${encodeURIComponent(extension.pluginId)}/${encodeURIComponent(extension.capability.id)}`,
  })) satisfies NavItem[];

  const handleLogout = () => {
    logout();
    navigate('/login');
  };

  // Connection Check / Loading Screen
  if (isConnecting || connectionError) {
    return (
      <div className="flex flex-col items-center justify-center h-screen bg-slate-50 dark:bg-slate-950 p-4">
        <div className="w-full max-w-sm bg-white dark:bg-slate-900 rounded-2xl shadow-xl p-8 text-center space-y-6 border border-slate-200 dark:border-slate-800">
          <div className="inline-flex items-center justify-center w-16 h-16 rounded-full bg-primary-50 dark:bg-primary-900/20 mb-2">
            <img src={getRuntimeAssetUrl('/logo.png')} alt={t('common.logoAlt')} className="w-10 h-10 object-contain" />
          </div>
          
          {isConnecting ? (
            <>
              <h2 className="text-xl font-bold dark:text-white">{t('connection.connectingTitle')}</h2>
              <div className="flex justify-center">
                <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600"></div>
              </div>
              <p className="text-sm text-slate-500">{t('connection.connectingSubtitle')}</p>
            </>
          ) : (
            <>
              <h2 className="text-xl font-bold text-slate-900 dark:text-white">{t('connection.failedTitle')}</h2>
              <p className="text-sm text-red-500 bg-red-50 dark:bg-red-900/10 p-3 rounded-lg border border-red-100 dark:border-red-900/20">
                {connectionError ? t(connectionError) : null}
              </p>
              <div className="space-y-3 pt-2">
                <button
                  onClick={() => window.location.reload()}
                  className="w-full py-2.5 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-xl transition-colors"
                >
                  {t('common.retry')}
                </button>
                <button
                  onClick={handleLogout}
                  className="w-full py-2.5 text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800 font-bold rounded-xl transition-colors"
                >
                  {t('nav.logout')}
                </button>
              </div>
            </>
          )}

          {isConnecting && (
            <button
              onClick={handleLogout}
              className="mt-4 text-sm text-slate-400 hover:text-slate-600 dark:hover:text-slate-300 font-medium transition-colors"
            >
              {t('connection.cancelAndLogout')}
            </button>
          )}
        </div>
      </div>
    );
  }

  const NavLink = ({ item, mobile = false }: { item: NavItem, mobile?: boolean }) => {
    const isActive = item.path === '/'
      ? location.pathname === '/'
      : location.pathname === item.path || item.matches?.some(match => location.pathname.startsWith(match));
    
    if (mobile) {
      return (
        <Link
          to={item.path}
          className={`flex flex-col items-center justify-center flex-1 py-1 transition-all ${
            isActive ? 'text-primary-600' : 'text-slate-500 dark:text-slate-400'
          }`}
        >
          <div className={`p-1.5 rounded-xl transition-all ${isActive ? 'bg-primary-50 dark:bg-primary-900/20' : ''}`}>
            {/* eslint-disable-next-line @typescript-eslint/no-explicit-any */}
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
        title={sidebarCollapsed ? item.label : undefined}
        className={`flex h-12 items-center rounded-xl transition-all ${
          sidebarCollapsed ? 'xl:justify-center xl:px-0' : 'gap-3 px-4'
        } ${
          isActive
            ? 'bg-primary-600 text-white shadow-lg shadow-primary-500/30'
            : 'text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800'
        }`}
      >
        <span className="flex h-6 w-6 shrink-0 items-center justify-center">
          {item.icon}
        </span>
        <span className={`truncate font-medium ${sidebarCollapsed ? 'xl:hidden' : ''}`}>
          {item.label}
        </span>
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
        fixed xl:sticky top-0 inset-y-0 left-0 w-72 ${sidebarCollapsed ? 'xl:w-20' : 'xl:w-72'} bg-white dark:bg-slate-900 border-r border-slate-200 dark:border-slate-800 z-[100] transform transition-[transform,width] duration-300 ease-out xl:translate-x-0
        ${isSidebarOpen ? 'translate-x-0' : '-translate-x-full'}
      `}>
        <div className={`flex h-full flex-col ${sidebarCollapsed ? 'p-2' : 'p-4'}`}>
          <div className={`relative hidden h-14 items-center py-2 xl:flex ${sidebarCollapsed ? 'justify-center px-0' : 'gap-3 px-2'}`}>
            <img src={getRuntimeAssetUrl('/logo.png')} alt={t('common.logoAlt')} className="h-9 w-9 shrink-0 object-contain shadow-lg shadow-primary-500/10" />
            {!sidebarCollapsed && (
              <span className="min-w-0 flex-1 truncate whitespace-nowrap text-xl font-bold tracking-tight dark:text-white">Ting Reader</span>
            )}
            <button
              type="button"
              onClick={toggleSidebarCollapsed}
              className={`inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-full border border-slate-200 bg-white text-slate-500 shadow-sm transition-colors hover:border-primary-300 hover:text-primary-600 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-400 dark:hover:border-primary-700 dark:hover:text-primary-300 ${sidebarCollapsed ? 'absolute -right-5 top-1/2 z-10 -translate-y-1/2' : 'ml-auto'}`}
              title={sidebarCollapsed ? t('nav.expandSidebar') : t('nav.collapseSidebar')}
              aria-label={sidebarCollapsed ? t('nav.expandSidebar') : t('nav.collapseSidebar')}
            >
              {sidebarCollapsed ? <ChevronRight size={17} /> : <ChevronLeft size={17} />}
            </button>
          </div>

          <nav className="flex flex-1 flex-col overflow-y-auto custom-scrollbar">
            <div className="xl:block hidden">
              {!sidebarCollapsed && <div className="mt-1 mb-1 px-4 text-xs font-bold uppercase tracking-widest text-slate-400">{t('nav.mainMenu')}</div>}
              {menuItems.map((item) => <NavLink key={item.path} item={item} />)}
            </div>

            {user?.role === 'admin' && (
              <div className="xl:mt-8">
                <div className={`text-xs font-bold text-slate-400 uppercase tracking-widest px-4 mb-2 mt-4 xl:mt-0 ${sidebarCollapsed ? 'xl:hidden' : ''}`}>{t('nav.admin')}</div>
                {adminItems.map((item) => <NavLink key={item.path} item={item} />)}
              </div>
            )}

            {pluginPageItems.length > 0 && (
              <div className="mt-8">
                <div className={`text-xs font-bold text-slate-400 uppercase tracking-widest px-4 mb-2 ${sidebarCollapsed ? 'xl:hidden' : ''}`}>{t('nav.pluginPages')}</div>
                {pluginPageItems.map((item) => <NavLink key={item.path} item={item} />)}
              </div>
            )}
          </nav>

          <div className="mt-auto pt-4 border-t border-slate-100 dark:border-slate-800">
            <div className={`flex items-center bg-slate-50 dark:bg-slate-800/50 p-3 rounded-lg ${sidebarCollapsed ? 'xl:flex-col xl:gap-2' : 'justify-between'}`}>
              <div className="flex items-center gap-3 overflow-hidden">
                <div className="w-10 h-10 rounded-full bg-primary-100 dark:bg-primary-900/30 flex items-center justify-center text-primary-600 shrink-0 font-bold text-sm">
                  {user?.username.charAt(0).toUpperCase()}
                </div>
                <div className={`truncate ${sidebarCollapsed ? 'xl:hidden' : ''}`}>
                  <p className="text-sm font-bold dark:text-white truncate">{user?.username}</p>
                  <p className="text-[10px] font-bold text-slate-400 uppercase tracking-tight">{user?.role === 'admin' ? 'Administrator' : 'User'}</p>
                </div>
              </div>
              <button 
                onClick={handleLogout}
                className="p-2 text-slate-400 hover:text-red-500 transition-colors"
                title={t('nav.logout')}
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
            <img src={getRuntimeAssetUrl('/logo.png')} alt={t('common.logoAlt')} className="w-9 h-9 shadow-lg shadow-primary-500/10 object-contain" />
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
          style={{
            backgroundColor: 'var(--page-background, transparent)',
            ...(isMiniPlayerHidden
              ? {
                  '--safe-bottom-with-player': '0px',
                  '--safe-bottom-base': '0px',
                }
              : {}),
          } as React.CSSProperties}
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
        {!location.pathname.startsWith('/plugin-pages') && (
          <PluginExtensionHost />
        )}
        {user?.role === 'admin' && user.uses_default_admin_credentials &&
          dismissedDefaultCredentials !== credentialReminderIdentity && (
            <DefaultAdminCredentialsDialog
              onLater={() => setDismissedDefaultCredentials(credentialReminderIdentity)}
              onChangeCredentials={() => {
                setDismissedDefaultCredentials(credentialReminderIdentity);
                setIsExpanded(false);
                navigate('/mine');
              }}
            />
          )}
      </div>
    </div>
  );
};

export default Layout;
