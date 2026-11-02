import React, { lazy, Suspense } from 'react';
import { BrowserRouter as Router, Routes, Route, Navigate } from 'react-router';
import { useAuthStore } from './core/stores/authStore';
import { getGatewayBasePath } from './core/utils/runtimeUrl';
import LoadingSpinner from './shared/ui/LoadingSpinner';

const Layout = lazy(() => import('./shared/layout/Layout'));
const LoginPage = lazy(() => import('./features/auth/LoginPage'));
const HomePage = lazy(() => import('./features/home/HomePage'));
const BookshelfPage = lazy(() => import('./features/bookshelf/BookshelfPage'));
const BookDetailPage = lazy(() => import('./features/bookshelf/BookDetailPage'));
const SeriesDetailPage = lazy(() => import('./features/bookshelf/SeriesDetailPage'));
const SearchPage = lazy(() => import('./features/bookshelf/SearchPage'));
const MyPage = lazy(() => import('./features/mine/MyPage'));
const AboutPage = lazy(() => import('./features/mine/AboutPage'));
const FavoritesPage = lazy(() => import('./features/mine/FavoritesPage'));
const HistoryPage = lazy(() => import('./features/mine/HistoryPage'));
const BookmarksPage = lazy(() => import('./features/mine/BookmarksPage'));
const PersonalizationPage = lazy(() => import('./features/mine/PersonalizationPage'));
const NotificationSettingsPage = lazy(() => import('./features/mine/NotificationSettingsPage'));
const AdminStatisticsPage = lazy(() => import('./features/mine/AdminStatisticsPage'));
const MyPlaylistsPage = lazy(() => import('./features/playlists/MyPlaylistsPage'));
const PlaylistDetailPage = lazy(() => import('./features/playlists/PlaylistDetailPage'));
const AdminLibraries = lazy(() => import('./features/admin/AdminLibraries'));
const AdminUsers = lazy(() => import('./features/admin/AdminUsers'));
const LogsPage = lazy(() => import('./features/admin/LogsPage'));
const PluginsPage = lazy(() => import('./features/admin/PluginsPage'));
const PluginLogsPage = lazy(() => import('./features/admin/PluginLogsPage'));
const CacheManagementPage = lazy(() => import('./features/mine/CacheManagementPage'));
const WidgetPage = lazy(() => import('./features/widget/WidgetPage'));
const PluginPage = lazy(() => import('./shared/pluginExtensions/PluginPage'));

const ProtectedRoute = ({ children }: { children: React.ReactNode }) => {
  const isAuthenticated = useAuthStore((state) => state.isAuthenticated);
  return isAuthenticated ? <>{children}</> : <Navigate to="/login" />;
};

const AdminRoute = ({ children }: { children: React.ReactNode }) => {
  const { isAuthenticated, user } = useAuthStore();
  if (!isAuthenticated) return <Navigate to="/login" />;
  if (user?.role !== 'admin') return <Navigate to="/" />;
  return <>{children}</>;
};

function App() {
  return (
    <Router basename={getGatewayBasePath()}>
      <Suspense fallback={<LoadingSpinner className="min-h-screen" />}>
        <Routes>
          <Route path="/login" element={<LoginPage />} />
          <Route path="/widget" element={<WidgetPage />} />
          <Route path="/widget/:id" element={<WidgetPage />} />
        
          <Route path="/" element={
            <ProtectedRoute>
              <Layout />
            </ProtectedRoute>
          }>
            <Route index element={<HomePage />} />
            <Route path="bookshelf" element={<BookshelfPage />} />
            <Route path="book/:id" element={<BookDetailPage />} />
            <Route path="series/:id" element={<SeriesDetailPage />} />
            <Route path="search" element={<SearchPage />} />
            <Route path="favorites" element={<FavoritesPage />} />
            <Route path="mine" element={<MyPage />} />
            <Route path="history" element={<HistoryPage />} />
            <Route path="bookmarks" element={<BookmarksPage />} />
            <Route path="playlists" element={<MyPlaylistsPage />} />
            <Route path="playlists/:id" element={<PlaylistDetailPage />} />
            <Route path="personalization" element={<PersonalizationPage />} />
            <Route path="plugin-pages/:pluginId/:capabilityId" element={<PluginPage />} />
            <Route path="notifications" element={
              <AdminRoute>
                <NotificationSettingsPage />
              </AdminRoute>
            } />
            <Route path="about" element={<AboutPage />} />
            <Route path="settings" element={<Navigate to="/personalization" replace />} />
            <Route path="cache" element={
              <AdminRoute>
                <CacheManagementPage />
              </AdminRoute>
            } />
            <Route path="statistics" element={
              <AdminRoute>
                <AdminStatisticsPage />
              </AdminRoute>
            } />
          
            <Route path="admin/statistics" element={
              <AdminRoute>
                <Navigate to="/statistics" replace />
              </AdminRoute>
            } />
            <Route path="admin/libraries" element={
              <AdminRoute>
                <AdminLibraries />
              </AdminRoute>
            } />
            <Route path="admin/users" element={
              <AdminRoute>
                <AdminUsers />
              </AdminRoute>
            } />
            <Route path="admin/logs" element={
              <AdminRoute>
                <LogsPage />
              </AdminRoute>
            } />
            <Route path="admin/plugins" element={
              <AdminRoute>
                <PluginsPage />
              </AdminRoute>
            } />
            <Route path="admin/plugins/:pluginId/logs" element={
              <AdminRoute>
                <PluginLogsPage />
              </AdminRoute>
            } />
            <Route path="admin/widget-config" element={
              <AdminRoute>
                <Navigate to="/personalization" replace />
              </AdminRoute>
            } />
          </Route>
        </Routes>
      </Suspense>
    </Router>
  );
}

export default App;
