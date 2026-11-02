import { create } from 'zustand';
import axios from 'axios';
import { SavedSessionRecovery, type SessionRecoveryResult } from '../api/sessionRecovery';
import type { User } from '../types';
import { safeStorage } from '../utils/storage';
import {
  clearAuthCookie,
  clearSessionRestoreMarkers,
  persistAuthCookie,
} from '../utils/sessionRestore';
import { getRuntimeBaseUrl } from '../utils/runtimeUrl';

const isStoredUser = (value: unknown): value is User => {
  if (!value || typeof value !== 'object') return false;

  const candidate = value as Record<string, unknown>;
  return (
    typeof candidate.id === 'string' &&
    typeof candidate.username === 'string' &&
    (candidate.role === 'admin' || candidate.role === 'user')
  );
};

const readStoredUser = (): User | null => {
  const rawUser = safeStorage.getItem('user');
  if (!rawUser || rawUser === 'null') return null;

  try {
    const parsedUser: unknown = JSON.parse(rawUser);
    if (parsedUser === null) return null;
    if (isStoredUser(parsedUser)) return parsedUser;
  } catch {
    // A previous failed login/session restore could have persisted the
    // string "undefined". Treat the cache as stale instead of preventing the
    // entire React application from mounting.
  }

  // Remove malformed or incompatible cached data so the next reload is clean.
  safeStorage.removeItem('user');
  return null;
};

const storedToken = safeStorage.getItem('auth_token');
if (storedToken) {
  persistAuthCookie(storedToken);
}

interface AuthState {
  user: User | null;
  token: string | null;
  serverUrl: string; // The original URL input by user
  activeUrl: string; // The resolved URL (after redirect)
  isAuthenticated: boolean;
  sessionRevision: number;
  setAuth: (user: User, token: string) => void;
  setUser: (user: User) => void;
  setToken: (token: string) => void;
  setServerUrl: (url: string) => void;
  setActiveUrl: (url: string) => void;
  restoreSavedSession: () => Promise<SessionRecoveryResult>;
  logout: () => void;
}

const savedCredentials = () => {
  if (safeStorage.getItem('login_remember_password') === 'false') return null;
  const username = safeStorage.getItem('login_username')?.trim();
  const password = safeStorage.getItem('login_password');
  const server = safeStorage.getItem('login_server_url');
  return username && password && server ? { username, password, server } : null;
};

const persistSession = (user: User | null, token: string | null) => {
  if (token) {
    safeStorage.setItem('auth_token', token);
    persistAuthCookie(token);
  } else {
    safeStorage.removeItem('auth_token');
    clearAuthCookie();
  }
  if (user) safeStorage.setItem('user', JSON.stringify(user));
  else safeStorage.removeItem('user');
};

export const useAuthStore = create<AuthState>((set, get) => ({
  user: readStoredUser(),
  token: storedToken,
  serverUrl: safeStorage.getItem('server_url') || getRuntimeBaseUrl(),
  activeUrl: safeStorage.getItem('active_url') || safeStorage.getItem('server_url') || getRuntimeBaseUrl(),
  isAuthenticated: !!storedToken,
  sessionRevision: 0,
  setAuth: (user, token) => {
    sessionRecovery.cancel();
    const validUser = isStoredUser(user) ? user : null;
    const validToken = typeof token === 'string' && token.length > 0 ? token : null;

    persistSession(validUser, validToken);
    set({ user: validUser, token: validToken, isAuthenticated: !!validToken, sessionRevision: get().sessionRevision + 1 });
  },
  setUser: (user) => {
    const validUser = isStoredUser(user) ? user : null;
    const changedIdentity = validUser?.id !== get().user?.id || validUser?.username !== get().user?.username;
    if (changedIdentity) sessionRecovery.cancel();
    if (validUser) {
      safeStorage.setItem('user', JSON.stringify(validUser));
    } else {
      safeStorage.removeItem('user');
    }
    set({ user: validUser, sessionRevision: get().sessionRevision + (changedIdentity ? 1 : 0) });
  },
  setToken: (token) => {
    sessionRecovery.cancel();
    const validToken = typeof token === 'string' && token.length > 0 ? token : null;
    if (validToken) {
      safeStorage.setItem('auth_token', validToken);
      persistAuthCookie(validToken);
    } else {
      safeStorage.removeItem('auth_token');
    }
    set({ token: validToken, isAuthenticated: !!validToken, sessionRevision: get().sessionRevision + 1 });
  },
  setServerUrl: (url) => {
    sessionRecovery.cancel();
    safeStorage.setItem('server_url', url);
    set({ serverUrl: url, sessionRevision: get().sessionRevision + 1 });
  },
  setActiveUrl: (url) => {
    sessionRecovery.cancel();
    safeStorage.setItem('active_url', url);
    set({ activeUrl: url, sessionRevision: get().sessionRevision + 1 });
  },
  restoreSavedSession: (): Promise<SessionRecoveryResult> => sessionRecovery.restore(),
  logout: () => {
    sessionRecovery.cancel();
    safeStorage.removeItem('auth_token');
    safeStorage.removeItem('user');
    clearAuthCookie();
    clearSessionRestoreMarkers();
    set({ user: null, token: null, isAuthenticated: false, sessionRevision: get().sessionRevision + 1 });
  },
}));

const sessionRecovery = new SavedSessionRecovery<User>({
  current: () => {
    const state = useAuthStore.getState();
    return state.token && state.user ? {
      revision: state.sessionRevision,
      token: state.token,
      userId: state.user.id,
      username: state.user.username,
      server: state.serverUrl,
    } : null;
  },
  credentials: savedCredentials,
  login: async (_session, { username, password }, signal) => {
    const response = await axios.post('/api/auth/login', { username, password }, {
      baseURL: getRuntimeBaseUrl(useAuthStore.getState().activeUrl),
      timeout: 15000,
      signal,
    });
    const payload = response.data;
    return payload && typeof payload.token === 'string' && isStoredUser(payload.user)
      ? { token: payload.token, user: payload.user } : null;
  },
  commit: (user, token) => {
    persistSession(user, token);
    // A recovered token belongs to the same login lifetime as pending requests.
    useAuthStore.setState({ user, token, isAuthenticated: true });
  },
  rejected: error => axios.isAxiosError(error)
    && [400, 401, 403, 404, 422].includes(error.response?.status ?? 0),
});

