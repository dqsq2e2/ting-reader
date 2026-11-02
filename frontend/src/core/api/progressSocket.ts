interface ProgressSocketOptions {
  createSocket(): WebSocket;
  connected(value: boolean): void;
  message(data: unknown): void;
}

/** Owns one authenticated progress connection and all of its timers. */
export class ProgressSocket {
  private readonly options: ProgressSocketOptions;
  private socket: WebSocket | null = null;
  private disposed = false;
  private attempts = 0;
  private reconnectTimer: ReturnType<typeof setTimeout> | undefined;
  private connectTimer: ReturnType<typeof setTimeout> | undefined;
  private pingTimer: ReturnType<typeof setInterval> | undefined;
  private pongTimer: ReturnType<typeof setTimeout> | undefined;

  constructor(options: ProgressSocketOptions) {
    this.options = options;
  }

  connect() {
    if (this.disposed || this.socket?.readyState === 0 || this.socket?.readyState === 1) return;
    clearTimeout(this.reconnectTimer);
    try {
      const socket = this.options.createSocket();
      this.socket = socket;
      this.connectTimer = setTimeout(() => this.reconnect(), 15000);
      socket.onopen = () => {
        if (this.disposed || this.socket !== socket) return;
        clearTimeout(this.connectTimer);
        this.attempts = 0;
        this.options.connected(true);
        this.pingTimer = setInterval(() => this.ping(), 30000);
      };
      socket.onmessage = event => {
        if (this.disposed || this.socket !== socket) return;
        try {
          const data: unknown = JSON.parse(String(event.data));
          if (data && typeof data === 'object' && 'type' in data && data.type === 'pong') {
            clearTimeout(this.pongTimer);
            this.pongTimer = undefined;
          }
          this.options.message(data);
        } catch {
          // A malformed message must not interrupt reconnection or playback.
        }
      };
      socket.onclose = () => {
        if (this.socket === socket) this.reconnect();
      };
      socket.onerror = () => {
        if (this.socket === socket) this.reconnect();
      };
    } catch {
      this.reconnect();
    }
  }

  send(message: unknown): boolean {
    if (this.disposed || this.socket?.readyState !== 1) return false;
    try {
      this.socket.send(JSON.stringify(message));
      return true;
    } catch {
      this.reconnect();
      return false;
    }
  }

  resume() {
    if (this.disposed) return;
    clearTimeout(this.reconnectTimer);
    if (this.socket?.readyState === 1) {
      this.ping();
    } else {
      this.closeSocket();
      this.connect();
    }
  }

  dispose() {
    this.disposed = true;
    clearTimeout(this.reconnectTimer);
    this.closeSocket();
  }

  private ping() {
    // Browsers suspend timers in the background. Allow a fresh response window
    // when foregrounded instead of treating a suspended tab as disconnected.
    clearTimeout(this.pongTimer);
    if (this.send({ type: 'ping' })) {
      this.pongTimer = setTimeout(() => this.reconnect(), 10000);
    }
  }

  private reconnect() {
    this.closeSocket();
    if (this.disposed) return;
    this.options.connected(false);
    clearTimeout(this.reconnectTimer);
    const delay = Math.min(1000 * 2 ** Math.min(this.attempts++, 5), 30000);
    this.reconnectTimer = setTimeout(() => this.connect(), delay);
  }

  private closeSocket() {
    clearTimeout(this.connectTimer);
    clearInterval(this.pingTimer);
    clearTimeout(this.pongTimer);
    const socket = this.socket;
    this.socket = null;
    if (socket) {
      socket.onopen = socket.onclose = socket.onmessage = socket.onerror = null;
      socket.close();
    }
  }
}
