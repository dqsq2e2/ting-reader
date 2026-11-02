import assert from 'node:assert/strict';
import { SavedSessionRecovery, type SessionSnapshot } from '../src/core/api/sessionRecovery.ts';
import { afterEach, test } from 'node:test';
import axios, { AxiosHeaders, type AxiosAdapter, type InternalAxiosRequestConfig } from 'axios';
import { BookOperations, readChapterGroupOrders } from '../src/core/api/books.ts';
import type { Book } from '../src/core/types/index.ts';
import { createSingleFlightAdapter } from '../src/core/api/mutations.ts';
import { getGatewayBasePath, getRuntimeAssetUrl, getRuntimePath, getRuntimeUrl } from '../src/core/utils/runtimeUrl.ts';
import { apiErrorCode, apiErrorMessage, apiErrorPayload, ApiRequestError, readApiError } from '../src/core/api/errors.ts';

function sessionHarness() {
  const user = { id: 'listener', username: 'Reader' };
  let current: SessionSnapshot | null = {
    revision: 1, token: 'expired', userId: user.id, username: user.username, server: 'https://reader.test',
  };
  let credentials = { username: user.username, password: 'saved', server: current.server };
  let requests = 0;
  let commits = 0;
  let now = 0;
  let signal: AbortSignal | undefined;
  let login = async (): Promise<{ user: typeof user; token: string } | null> => ({ user, token: 'renewed' });
  const recovery = new SavedSessionRecovery({
    current: () => current,
    credentials: () => credentials,
    login: async (_session, _credentials, requestSignal) => {
      requests++;
      signal = requestSignal;
      return login();
    },
    commit: (_user, token) => {
      commits++;
      current = current ? { ...current, token } : null;
    },
    rejected: error => error === 'rejected',
    now: () => now,
  });
  return {
    user, recovery,
    get current() { return current; },
    set current(value: SessionSnapshot | null) { current = value; },
    get credentials() { return credentials; },
    set credentials(value: typeof credentials) { credentials = value; },
    get requests() { return requests; },
    get commits() { return commits; },
    get signal() { return signal; },
    set login(value: typeof login) { login = value; },
    set now(value: number) { now = value; },
  };
}

test('expired sessions share one login and preserve their request lifetime', async () => {
  const h = sessionHarness();
  assert.deepEqual(await Promise.all([h.recovery.restore(), h.recovery.restore(), h.recovery.restore()]),
    ['restored', 'restored', 'restored']);
  assert.equal(h.requests, 1);
  assert.equal(h.commits, 1);
  assert.equal(h.current?.token, 'renewed');
  assert.equal(h.current?.revision, 1);
});

test('saved passwords are not sent to another server or another account', async () => {
  for (const patch of [{ server: 'https://other.test' }, { username: 'Other' }]) {
    const h = sessionHarness();
    h.credentials = { ...h.credentials, ...patch };
    assert.equal(await h.recovery.restore(), 'unavailable');
    assert.equal(h.requests, 0);
  }
});

test('logout and account changes discard late login responses', async () => {
  for (const logout of [true, false]) {
    const h = sessionHarness();
    let finish!: (value: { user: typeof h.user; token: string }) => void;
    h.login = () => new Promise(resolve => { finish = resolve; });
    const pending = h.recovery.restore();
    await Promise.resolve();
    h.recovery.cancel();
    assert.equal(h.signal?.aborted, true);
    h.current = logout ? null : { ...h.current!, revision: 2, userId: 'another' };
    finish({ user: h.user, token: 'late' });
    assert.equal(await pending, 'obsolete');
    assert.equal(h.commits, 0);
  }
});

test('transient login errors retain the session and throttle repeated recovery', async () => {
  const h = sessionHarness();
  h.login = async () => { throw new Error('network unavailable'); };
  assert.equal(await h.recovery.restore(), 'transient');
  assert.equal(await h.recovery.restore(), 'transient');
  assert.equal(h.requests, 1);
  assert.equal(h.current?.token, 'expired');
  h.now = 30001;
  assert.equal(await h.recovery.restore(), 'transient');
  assert.equal(h.requests, 2);
});

test('rejected passwords and mismatched login identities are not restored', async () => {
  const h = sessionHarness();
  h.login = async () => { throw 'rejected'; };
  assert.equal(await h.recovery.restore(), 'unavailable');
  h.login = async () => ({ user: { ...h.user, id: 'another' }, token: 'wrong' });
  assert.equal(await h.recovery.restore(), 'unavailable');
  assert.equal(h.commits, 0);
});

test('API error parsing retains classification while displaying the server message', () => {
  const error = { response: { data: {
    error: 'PluginExecutionError',
    message: '  Plugin is temporarily unavailable  ',
    details: {
      reason: 'plugin_unavailable',
      operation: 'invoke',
      resource: { kind: 'plugin', id: 'example@1.0.0' },
      context: ['search'],
      retryable: true,
      capability_id: 'scraper',
    },
    trace_id: 'trace-1',
  } } };
  assert.equal(apiErrorMessage(error, 'fallback'), 'Plugin is temporarily unavailable');
  assert.equal(apiErrorCode(error), 'PluginExecutionError');
  assert.deepEqual(readApiError(error)?.details, error.response.data.details);
  assert.equal(readApiError(error)?.trace_id, 'trace-1');
});

test('API errors tolerate malformed and non JSON responses without throwing', () => {
  for (const data of [null, 'gateway login page', [], { message: 12, error: {} }]) {
    const error = { response: { data } };
    assert.equal(readApiError(error), null);
    assert.equal(apiErrorMessage(error, 'fallback'), 'fallback');
  }
  const malformed = { response: { data: {
    message: 'Readable message',
    details: { reason: 42, retryable: 'false', context: [1, 'safe'] },
  } } };
  const details = readApiError(malformed)?.details;
  assert.equal(details?.reason, undefined);
  assert.equal(details?.retryable, undefined);
  assert.deepEqual(details?.context, ['safe']);
  assert.equal(apiErrorMessage(new Error('transport failed'), 'fallback'), 'transport failed');
  assert.equal(apiErrorMessage({ response: { data: { error: 'NotFound' } } }, 'fallback'), 'NotFound');
});

test('bridge payloads preserve error metadata without serializing request credentials', () => {
  const error = Object.assign(new Error('HTTP 503'), {
    response: {
      status: 503,
      data: {
        error: 'PluginExecutionError',
        message: 'Provider unavailable',
        details: { reason: 'plugin_unavailable', retryable: true, operation: 'search' },
        trace_id: 'trace-bridge',
      },
    },
    config: { headers: { Authorization: 'private-credential' } },
  });
  const payload = apiErrorPayload(error, 'fallback');
  assert.equal(payload.status, 503);
  assert.equal(payload.trace_id, 'trace-bridge');
  assert.equal(payload.details?.retryable, true);
  assert.equal(JSON.stringify(payload).includes('private-credential'), false);
  const local = new ApiRequestError(payload);
  assert.equal(apiErrorCode(local), 'PluginExecutionError');
  assert.equal(apiErrorMessage(local, 'fallback'), 'Provider unavailable');
  assert.deepEqual(apiErrorPayload(local, 'fallback'), payload);
});

const originalWindow = Object.getOwnPropertyDescriptor(globalThis, 'window');

afterEach(() => {
  if (originalWindow) Object.defineProperty(globalThis, 'window', originalWindow);
  else Reflect.deleteProperty(globalThis, 'window');
});

test('shared book batches deduplicate ids and bound deletion concurrency', async () => {
  const batches: string[][] = [];
  let active = 0;
  let peak = 0;
  let deletions = 0;
  const api = axios.create({ adapter: async request => {
    if (request.method === 'post') {
      batches.push(JSON.parse(request.data as string).book_ids);
    } else {
      active++;
      peak = Math.max(peak, active);
      await new Promise(resolve => setTimeout(resolve, 1));
      active--;
      deletions++;
    }
    return { data: {}, status: 200, statusText: 'OK', headers: {}, config: request };
  } });
  const operations = new BookOperations(api);
  const ids = Array.from({ length: 401 }, (_, index) => String(index));
  await operations.markRead([...ids, '0'], false);
  assert.deepEqual(batches.map(batch => batch.length), [200, 200, 1]);
  await operations.deleteBooks([...ids.slice(0, 9), '0']);
  assert.equal(peak, 4);
  assert.equal(deletions, 9);
  operations.dispose();
});

test('replacing a details query aborts every request and rejects late responses', async () => {
  const signals: AbortSignal[] = [];
  const finish: (() => void)[] = [];
  const api = axios.create({ adapter: request => {
    signals.push(request.signal as AbortSignal);
    return new Promise(resolve => finish.push(() =>
      resolve({ data: {}, status: 200, statusText: 'OK', headers: {}, config: request })));
  } });
  const owner = new BookOperations(api);
  const parent = new AbortController();
  const details = owner.details('book', parent.signal);
  const rejected = assert.rejects(details);
  parent.abort();
  assert.equal(signals.length, 3);
  assert.ok(signals.every(signal => signal.aborted));
  finish.forEach(resolve => resolve());
  await rejected;
  owner.dispose();
});

test('saving metadata preserves library and favorite context and accepts both order formats', async () => {
  const api = axios.create({ adapter: async request =>
    ({ data: { title: 'New title' }, status: 200, statusText: 'OK', headers: {}, config: request }) });
  const owner = new BookOperations(api);
  const book = { id: 'one', title: 'Old', library_id: 'local',
    library_type: 'webdav', is_favorite: true } as Book;
  const saved = await owner.save(book, { title: 'New title' });
  assert.equal(saved.title, 'New title');
  assert.equal(saved.library_type, 'webdav');
  assert.equal(saved.is_favorite, true);
  const orders = [{ book_id: 'one', order: 'desc' }];
  assert.deepEqual(readChapterGroupOrders({ chapter_group_orders: { one: 'desc', bad: 'other' } }), orders);
  assert.deepEqual(readChapterGroupOrders({ chapter_group_orders: [...orders, null] }), orders);
  owner.dispose();
});

const config = (data: unknown = { book: 'one' }): InternalAxiosRequestConfig => ({
  baseURL: 'https://reader.example',
  url: '/api/books',
  method: 'post',
  headers: new AxiosHeaders(),
  data,
});

test('concurrent mutations share one request and release the entry on completion', async () => {
  let calls = 0;
  const adapter = createSingleFlightAdapter(async (request) => {
    calls++;
    assert.ok(request.headers.get('Idempotency-Key'));
    return { data: {}, status: 200, statusText: 'OK', headers: {}, config: request };
  }, () => 'user-a');
  const first = adapter(config({ a: 1, b: 2 }));
  const second = adapter(config({ b: 2, a: 1 }));
  assert.equal(first, second);
  await first;
  assert.equal(calls, 1);
  await adapter(config({ a: 1, b: 2 }));
  assert.equal(calls, 2);
});

test('different users, request credentials and payloads do not share mutations', async () => {
  let calls = 0;
  let scope = 'user-a';
  const adapter = createSingleFlightAdapter(async (request) => {
    calls++;
    return { data: {}, status: 200, statusText: 'OK', headers: {}, config: request };
  }, () => scope);
  const first = adapter(config());
  scope = 'user-b';
  const second = adapter(config());
  const third = adapter(config({ book: 'two' }));
  const authenticated = config();
  authenticated.headers.set('Authorization', 'Bearer other-token');
  const fourth = adapter(authenticated);
  await Promise.all([first, second, third, fourth]);
  assert.equal(calls, 4);
});

test('adapter failures release the request and preserve caller idempotency keys', async () => {
  let calls = 0;
  const failing: AxiosAdapter = (request) => {
    calls++;
    assert.equal(request.headers.get('Idempotency-Key'), 'caller-key');
    throw new Error('transport failed');
  };
  const adapter = createSingleFlightAdapter(failing, () => '');
  for (let attempt = 0; attempt < 2; attempt++) {
    const request = config();
    request.headers.set('Idempotency-Key', 'caller-key');
    await assert.rejects(adapter(request), /transport failed/);
  }
  assert.equal(calls, 2);
});

test('cancellable mutations and different binary uploads retain separate owners', async () => {
  let calls = 0;
  const adapter = createSingleFlightAdapter(async (request) => {
    calls++;
    return { data: {}, status: 200, statusText: 'OK', headers: {}, config: request };
  }, () => 'user');
  const a = config();
  const b = config();
  a.signal = new AbortController().signal;
  b.signal = new AbortController().signal;
  const first = adapter(a);
  const second = adapter(b);
  assert.notEqual(first, second);
  const uploads = ['one', 'two'].map((bytes) => {
    const form = new FormData();
    form.append('file', new Blob([bytes]), 'same-name.txt');
    return adapter(config(form));
  });
  assert.notEqual(uploads[0], uploads[1]);
  await Promise.all([first, second, ...uploads]);
  assert.equal(calls, 4);
});

test('native and Docker gateway URLs keep their prefix for API and assets', () => {
  for (const name of ['ting-reader', 'ting-reader-docker']) {
    for (const suffix of ['', '/', '/books/one']) {
      Object.defineProperty(globalThis, 'window', {
        configurable: true,
        value: { location: { origin: 'http://nas:5666', pathname: `/app/${name}${suffix}` } },
      });
      assert.equal(getGatewayBasePath(), `/app/${name}`);
      assert.equal(getRuntimeUrl('/api/books', 'http://other:3000'), `http://nas:5666/app/${name}/api/books`);
      assert.equal(getRuntimeAssetUrl('assets/player.js'), `/app/${name}/assets/player.js`);
      assert.equal(getRuntimePath('/login'), `/app/${name}/login`);
    }
  }
});

test('third-party STRM and cover URLs keep their original origin', () => {
  const url = 'https://media.example/chapter?signature=value';
  assert.equal(getRuntimeUrl(url, 'https://reader.example'), url);
  assert.equal(getRuntimeAssetUrl(url), url);
  assert.equal(getRuntimeUrl('/api/books', 'http://localhost:3000/'), 'http://localhost:3000/api/books');
});
