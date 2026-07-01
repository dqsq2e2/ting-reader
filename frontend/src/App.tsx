import React from 'react';
import { BrowserRouter as Router, Routes, Route, Navigate } from 'react-router-dom';
import Layout from './shared/layout/Layout';
import LoginPage from './features/auth/LoginPage';
import HomePage from './features/home/HomePage';
import BookshelfPage from './features/bookshelf/BookshelfPage';
import BookDetailPage from './features/bookshelf/BookDetailPage';
import SeriesDetailPage from './features/bookshelf/SeriesDetailPage';
import SearchPage from './features/bookshelf/SearchPage';
import MyPage from './features/mine/MyPage';
import AboutPage from './features/mine/AboutPage';
import FavoritesPage from './features/mine/FavoritesPage';
import HistoryPage from './features/mine/HistoryPage';
import PersonalizationPage from './features/mine/PersonalizationPage';
import NotificationSettingsPage from './features/mine/NotificationSettingsPage';
import AdminStatisticsPage from './features/mine/AdminStatisticsPage';
import MyPlaylistsPage from './features/playlists/MyPlaylistsPage';
import PlaylistDetailPage from './features/playlists/PlaylistDetailPage';
import AdminLibraries from './features/admin/AdminLibraries';
import AdminUsers from './features/admin/AdminUsers';
import LogsPage from './features/admin/LogsPage';
import PluginsPage from './features/admin/PluginsPage';
import DownloadsPage from './features/admin/DownloadsPage';
import WidgetPage from './features/widget/WidgetPage';
import { useAuthStore } from './core/stores/authStore';

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
    <Router>
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
          <Route path="playlists" element={<MyPlaylistsPage />} />
          <Route path="playlists/:id" element={<PlaylistDetailPage />} />
          <Route path="personalization" element={<PersonalizationPage />} />
          <Route path="notifications" element={
            <AdminRoute>
              <NotificationSettingsPage />
            </AdminRoute>
          } />
          <Route path="about" element={<AboutPage />} />
          <Route path="settings" element={<Navigate to="/personalization" replace />} />
          <Route path="downloads" element={<DownloadsPage />} />
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
          <Route path="admin/widget-config" element={
            <AdminRoute>
              <Navigate to="/personalization" replace />
            </AdminRoute>
          } />
        </Route>
      </Routes>
    </Router>
  );
}

export default App;
