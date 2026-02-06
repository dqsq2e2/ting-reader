import { create } from 'zustand';

interface User {
  id: string;
  username: string;
  role: 'admin' | 'user';
}

interface AuthState {
  user: User | null;
  token: string | null;
  serverUrl: string; // The original URL input by user
  activeUrl: string; // The resolved URL (after redirect)
  isAuthenticated: boolean;
  setAuth: (user: User, token: string) => void;
  setUser: (user: User) => void;
  setToken: (token: string) => void;
  setServerUrl: (url: string) => void;
  setActiveUrl: (url: string) => void;
  logout: () => void;
}

export const useAuthStore = create<AuthState>((set) => ({
  user: JSON.parse(localStorage.getItem('user') || 'null'),
  token: localStorage.getItem('auth_token'),
  serverUrl: localStorage.getItem('server_url') || (import.meta.env.PROD ? window.location.origin : 'http://localhost:3000'),
  activeUrl: localStorage.getItem('active_url') || localStorage.getItem('server_url') || (import.meta.env.PROD ? window.location.origin : 'http://localhost:3000'),
  isAuthenticated: !!localStorage.getItem('auth_token'),
  setAuth: (user, token) => {
    localStorage.setItem('auth_token', token);
    localStorage.setItem('user', JSON.stringify(user));
    set({ user, token, isAuthenticated: true });
  },
  setUser: (user) => {
    localStorage.setItem('user', JSON.stringify(user));
    set({ user });
  },
  setToken: (token) => {
    localStorage.setItem('auth_token', token);
    set({ token, isAuthenticated: true });
  },
  setServerUrl: (url) => {
    localStorage.setItem('server_url', url);
    set({ serverUrl: url });
  },
  setActiveUrl: (url) => {
    localStorage.setItem('active_url', url);
    set({ activeUrl: url });
  },
  logout: () => {
    localStorage.removeItem('auth_token');
    localStorage.removeItem('user');
    set({ user: null, token: null, isAuthenticated: false });
  },
}));
