import { useEffect, useRef, useCallback, useState } from 'react';
import { useAuthStore } from '../stores/authStore';
import { usePlayerStore } from '../stores/playerStore';

interface ProgressUpdate {
  type: 'progress_updated';
  book_id: string;
  chapter_id: string | null;
  position: number;
  updated_at: string;
}

interface ServerError {
  type: 'error';
  message: string;
}

type ServerMessage = ProgressUpdate | { type: 'pong' } | ServerError;

/** Manages a persistent WebSocket connection for real-time progress sync */
export function useWebSocket() {
  const { token, activeUrl } = useAuthStore();
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const reconnectAttemptsRef = useRef(0);
  const [isConnected, setIsConnected] = useState(false);
  const pingTimerRef = useRef<ReturnType<typeof setInterval> | undefined>(undefined);

  const connectRef = useRef<() => void>(() => {});
  const maxReconnectAttempts = 20;

  const getWsUrl = useCallback(() => {
    if (!activeUrl) return null;
    try {
      const url = new URL(activeUrl);
      url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
      url.pathname = url.pathname.replace(/\/$/, '') + '/api/ws';
      return url.toString();
    } catch {
      // Fallback: construct from string
      const base = activeUrl.replace(/\/$/, '');
      const wsProtocol = base.startsWith('https') ? 'wss' : 'ws';
      return `${wsProtocol}://${new URL(base).host}/api/ws`;
    }
  }, [activeUrl]);

  const connect = useCallback(() => {
    if (!token) return;

    const wsUrl = getWsUrl();
    if (!wsUrl) return;

    // Don't reconnect if already connected
    if (wsRef.current?.readyState === WebSocket.OPEN) return;
    // Don't reconnect if connecting
    if (wsRef.current?.readyState === WebSocket.CONNECTING) return;

    try {
      const urlWithToken = `${wsUrl}?token=${encodeURIComponent(token)}`;
      const ws = new WebSocket(urlWithToken);

      ws.onopen = () => {
        setIsConnected(true);
        reconnectAttemptsRef.current = 0;

        // Start keep-alive ping every 30 seconds
        pingTimerRef.current = setInterval(() => {
          if (ws.readyState === WebSocket.OPEN) {
            ws.send(JSON.stringify({ type: 'ping' }));
          }
        }, 30000);
      };

      ws.onmessage = (event) => {
        try {
          const msg: ServerMessage = JSON.parse(event.data);
          handleServerMessage(msg);
        } catch {
          // Ignore malformed messages
        }
      };

      ws.onclose = () => {
        setIsConnected(false);
        if (pingTimerRef.current) {
          clearInterval(pingTimerRef.current);
        }

        // Attempt reconnection with exponential backoff
        if (reconnectAttemptsRef.current < maxReconnectAttempts) {
          const delay = Math.min(
            1000 * Math.pow(2, reconnectAttemptsRef.current),
            30000
          );
          reconnectAttemptsRef.current += 1;
          reconnectTimerRef.current = setTimeout(() => connectRef.current(), delay);
        }
      };

      ws.onerror = () => {
        // onclose will fire after onerror, reconnect logic is in onclose
      };

      wsRef.current = ws;
    } catch {
      // Connection failed, retry later
      reconnectTimerRef.current = setTimeout(() => connectRef.current(), 5000);
    }
  }, [token, getWsUrl]);

  useEffect(() => { connectRef.current = connect; }, [connect]);

  const disconnect = useCallback(() => {
    if (reconnectTimerRef.current) {
      clearTimeout(reconnectTimerRef.current);
    }
    if (pingTimerRef.current) {
      clearInterval(pingTimerRef.current);
    }
    reconnectAttemptsRef.current = maxReconnectAttempts; // Prevent reconnection
    wsRef.current?.close();
    wsRef.current = null;
    setIsConnected(false);
  }, []);

  // Connect on mount, disconnect on unmount
  useEffect(() => {
    connect();
    return () => disconnect();
  }, [connect, disconnect]);

  /** Send a progress update via WebSocket */
  const sendProgress = useCallback(
    (bookId: string, chapterId: string, position: number, playbackStart?: number) => {
      if (wsRef.current?.readyState === WebSocket.OPEN) {
        wsRef.current.send(
          JSON.stringify({
            type: 'progress_update',
            book_id: bookId,
            chapter_id: chapterId,
            position: Math.floor(position),
            ...(playbackStart !== undefined
              ? { playback_start: Math.floor(playbackStart) }
              : {}),
          })
        );
      }
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
