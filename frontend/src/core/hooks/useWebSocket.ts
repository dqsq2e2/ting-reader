import { useEffect, useRef, useCallback, useState } from 'react';
import { useAuthStore } from '../stores/authStore';
import { usePlayerStore } from '../stores/playerStore';
import { getRuntimeBaseUrl } from '../utils/runtimeUrl';
import type { ApiErrorBody } from '../api/errors';
import { ProgressSocket } from '../api/progressSocket';
import apiClient from '../api/client';

interface ProgressUpdate {
  type: 'progress_updated';
  book_id: string;
  chapter_id: string | null;
  position: number;
  updated_at: string;
}

interface ServerError extends ApiErrorBody {
  type: 'error';
  message: string;
}

type ServerMessage = ProgressUpdate | { type: 'pong' } | ServerError;

/** Manages a persistent WebSocket connection for real-time progress sync */
export function useWebSocket() {
  const { token, activeUrl } = useAuthStore();
  const connectionRef = useRef<ProgressSocket | null>(null);
  const [isConnected, setIsConnected] = useState(false);

  const getWsUrl = useCallback(() => {
    if (typeof window === 'undefined') return '';

    const base = getRuntimeBaseUrl(activeUrl) || window.location.origin;
    let url: URL;
    try {
      url = new URL(base, window.location.origin);
    } catch {
      url = new URL(window.location.origin);
    }

    url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
    url.pathname = url.pathname.replace(/\/$/, '') + '/api/ws';
    return url.toString();
  }, [activeUrl]);

  useEffect(() => {
    if (!token) return;

    const wsUrl = getWsUrl();
    if (!wsUrl) return;

    const controller = new AbortController();
    let checking = false;
    let lastCheck = 0;
    const checkSession = () => {
      if (checking || controller.signal.aborted || Date.now() - lastCheck < 30000) return;
      checking = true;
      lastCheck = Date.now();
      void apiClient.get('/api/me', { signal: controller.signal, timeout: 5000 })
        .catch(() => {})
        .finally(() => { checking = false; });
    };

    const connection = new ProgressSocket({
      createSocket: () => new WebSocket(`${wsUrl}?token=${encodeURIComponent(token)}`),
      connected: value => {
        setIsConnected(value);
        if (!value) checkSession();
      },
      message: (data) => {
        if (data && typeof data === 'object' && 'type' in data) {
          handleServerMessage(data as ServerMessage);
        }
      },
    });
    connectionRef.current = connection;
    connection.connect();
    const onVisibilityChange = () => {
      if (document.visibilityState === 'visible') {
        checkSession();
        connection.resume();
      }
    };
    const onOnline = () => { checkSession(); connection.resume(); };
    const sessionTimer = setInterval(() => {
      if (document.visibilityState === 'visible') checkSession();
    }, 60000);
    document.addEventListener('visibilitychange', onVisibilityChange);
    window.addEventListener('online', onOnline);
    return () => {
      controller.abort();
      clearInterval(sessionTimer);
      document.removeEventListener('visibilitychange', onVisibilityChange);
      window.removeEventListener('online', onOnline);
      connection.dispose();
      if (connectionRef.current === connection) connectionRef.current = null;
      setIsConnected(false);
    };
  }, [token, getWsUrl]);

  /** Send a progress update via WebSocket */
  const sendProgress = useCallback(
    (bookId: string, chapterId: string, position: number, playbackStart?: number) => {
      connectionRef.current?.send({
        type: 'progress_update',
        book_id: bookId,
        chapter_id: chapterId,
        position: Math.floor(position),
        ...(playbackStart !== undefined
          ? { playback_start: Math.floor(playbackStart) }
          : {}),
      });
    },
    []
  );

  return { isConnected, sendProgress };
}

/** Handle incoming messages from the server */
function handleServerMessage(msg: ServerMessage) {
  switch (msg.type) {
    case 'progress_updated': {
      // Update the current chapter's progress in the store
      // This enables cross-device/tab sync
      const store = usePlayerStore.getState();
      if (
        store.currentBook?.id === msg.book_id &&
        store.currentChapter?.id === msg.chapter_id
      ) {
        // If we're on the same book/chapter, update the chapter's progress
        // in our local chapters array
        const updatedChapters = store.chapters.map((c) =>
          c.id === msg.chapter_id
            ? { ...c, progress_position: msg.position, progress_updated_at: msg.updated_at }
            : c
        );
        usePlayerStore.setState({ chapters: updatedChapters });
      }
      break;
    }
    case 'pong':
    case 'error':
      // Silently ignore pong and error messages
      break;
  }
}
