export type SessionRecoveryResult = 'restored' | 'unavailable' | 'transient' | 'obsolete';

export interface SessionSnapshot {
  revision: number;
  token: string;
  userId: string;
  username: string;
  server: string;
}

export interface SavedLogin {
  username: string;
  password: string;
  server: string;
}

interface RecoveryOptions<User extends { id: string; username: string }> {
  current(): SessionSnapshot | null;
  credentials(): SavedLogin | null;
  login(session: SessionSnapshot, credentials: SavedLogin, signal: AbortSignal): Promise<{ user: User; token: string } | null>;
  commit(user: User, token: string): void;
  rejected(error: unknown): boolean;
  now?: () => number;
}

/** Single-flight recovery scoped to the current account, server and login lifetime. */
export class SavedSessionRecovery<User extends { id: string; username: string }> {
  private readonly options: RecoveryOptions<User>;
  private pending: { revision: number; promise: Promise<SessionRecoveryResult>; controller: AbortController } | null = null;
  private retryAfter = 0;
  private retryRevision = -1;

  constructor(options: RecoveryOptions<User>) {
    this.options = options;
  }

  cancel() {
    this.pending?.controller.abort();
    this.pending = null;
    this.retryAfter = 0;
    this.retryRevision = -1;
  }

  restore(): Promise<SessionRecoveryResult> {
    const session = this.options.current();
    if (!session) return Promise.resolve('obsolete');
    if (this.pending?.revision === session.revision) return this.pending.promise;
    const credentials = this.options.credentials();
    if (!credentials || credentials.server !== session.server || credentials.username !== session.username) {
      return Promise.resolve('unavailable');
    }
    const now = this.options.now ?? Date.now;
    if (session.revision === this.retryRevision && now() < this.retryAfter) {
      return Promise.resolve('transient');
    }
    const controller = new AbortController();
    const isCurrent = () => !controller.signal.aborted
      && this.options.current()?.revision === session.revision;
    const promise = Promise.resolve().then(async (): Promise<SessionRecoveryResult> => {
      try {
        if (!isCurrent()) return 'obsolete';
        const login = await this.options.login(session, credentials, controller.signal);
        if (!isCurrent()) return 'obsolete';
        if (!login || !login.token || login.user.id !== session.userId || login.user.username !== session.username) {
          return 'unavailable';
        }
        this.options.commit(login.user, login.token);
        this.retryAfter = 0;
        return 'restored';
      } catch (error) {
        if (!isCurrent()) return 'obsolete';
        if (this.options.rejected(error)) return 'unavailable';
        this.retryRevision = session.revision;
        this.retryAfter = now() + 30000;
        return 'transient';
      } finally {
        if (this.pending?.controller === controller) this.pending = null;
      }
    });
    this.pending = { revision: session.revision, promise, controller };
    return promise;
  }
}
