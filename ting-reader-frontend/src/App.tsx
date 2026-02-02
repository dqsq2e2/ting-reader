import React from 'react';
import { BrowserRouter as Router, Routes, Route, Navigate } from 'react-router-dom';
import Layout from './components/Layout';
import LoginPage from './pages/LoginPage';
import HomePage from './pages/HomePage';
import BookshelfPage from './pages/BookshelfPage';
import BookDetailPage from './pages/BookDetailPage';
import SearchPage from './pages/SearchPage';
import FavoritesPage from './pages/FavoritesPage';
import AdminLibraries from './pages/AdminLibraries';
import AdminUsers from './pages/AdminUsers';
import TaskLogsPage from './pages/TaskLogsPage';
import SettingsPage from './pages/SettingsPage';
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
        
        <Route path="/" element={
          <ProtectedRoute>
            <Layout />
          </ProtectedRoute>
        }>
          <Route index element={<HomePage />} />
          <Route path="bookshelf" element={<BookshelfPage />} />
          <Route path="book/:id" element={<BookDetailPage />} />
          <Route path="search" element={<SearchPage />} />
          <Route path="favorites" element={<FavoritesPage />} />
          <Route path="settings" element={<SettingsPage />} />
          
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
          <Route path="admin/tasks" element={
            <AdminRoute>
              <TaskLogsPage />
            </AdminRoute>
          } />
        </Route>
      </Routes>
    </Router>
  );
}

export default App;
