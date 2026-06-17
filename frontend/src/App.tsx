import React from 'react';
import { BrowserRouter as Router, Routes, Route, Navigate } from 'react-router-dom';
import Layout from './components/Layout';
import LoginPage from './pages/LoginPage';
import HomePage from './pages/HomePage';
import BookshelfPage from './pages/BookshelfPage';
import BookDetailPage from './pages/BookDetailPage';
import SeriesDetailPage from './pages/SeriesDetailPage';
import SearchPage from './pages/SearchPage';
import FavoritesPage from './pages/FavoritesPage';
import MyPage from './pages/MyPage';
import MyPlaylistsPage from './pages/MyPlaylistsPage';
import PlaylistDetailPage from './pages/PlaylistDetailPage';
import HistoryPage from './pages/HistoryPage';
import PersonalizationPage from './pages/PersonalizationPage';
import NotificationSettingsPage from './pages/NotificationSettingsPage';
import AdminLibraries from './pages/AdminLibraries';
import AdminUsers from './pages/AdminUsers';
import AdminStatisticsPage from './pages/AdminStatisticsPage';
import LogsPage from './pages/LogsPage';
import PluginsPage from './pages/PluginsPage';
import DownloadsPage from './pages/DownloadsPage';
import WidgetPage from './pages/WidgetPage';
import { useAuthStore } from './store/authStore';

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
          <Route path="about" element={<Navigate to="/mine" replace />} />
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
