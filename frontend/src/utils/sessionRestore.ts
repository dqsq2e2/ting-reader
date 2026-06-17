const SESSION_ID_KEY = 'ting_reader_browser_session_id';
const RESTORE_LOG_PREFIX = 'ting_reader_session_restore_logged:';
const TOKEN_COOKIE = 'ting_reader_token';

const canUseBrowserStorage = () => typeof window !== 'undefined' && !!window.sessionStorage;

export function getBrowserSessionId(): string {
  if (!canUseBrowserStorage()) {
    return '';
  }

  let sessionId = window.sessionStorage.getItem(SESSION_ID_KEY);
  if (!sessionId) {
    sessionId = typeof crypto !== 'undefined' && 'randomUUID' in crypto
      ? crypto.randomUUID()
      : `${Date.now()}-${Math.random().toString(36).slice(2)}`;
    window.sessionStorage.setItem(SESSION_ID_KEY, sessionId);
  }
  return sessionId;
}

function restoreLogKey(token: string): string {
  return `${RESTORE_LOG_PREFIX}${token.slice(-24)}`;
}

export function hasSessionRestoreLogged(token: string | null): boolean {
  if (!token || !canUseBrowserStorage()) {
    return false;
  }
  return window.sessionStorage.getItem(restoreLogKey(token)) === getBrowserSessionId();
}

export function markSessionRestoreLogged(token: string | null): void {
  if (!token || !canUseBrowserStorage()) {
    return;
  }
  window.sessionStorage.setItem(restoreLogKey(token), getBrowserSessionId());
}

export function clearSessionRestoreMarkers(): void {
  if (!canUseBrowserStorage()) {
    return;
  }

  for (let i = window.sessionStorage.length - 1; i >= 0; i -= 1) {
    const key = window.sessionStorage.key(i);
    if (key?.startsWith(RESTORE_LOG_PREFIX)) {
      window.sessionStorage.removeItem(key);
    }
  }
}

export function persistAuthCookie(token: string): void {
  if (typeof document === 'undefined') {
    return;
  }
  document.cookie = `${TOKEN_COOKIE}=${encodeURIComponent(token)}; path=/; max-age=2592000; SameSite=Lax`;
}

export function clearAuthCookie(): void {
  if (typeof document === 'undefined') {
    return;
  }
  document.cookie = `${TOKEN_COOKIE}=; path=/; max-age=0; SameSite=Lax`;
}
