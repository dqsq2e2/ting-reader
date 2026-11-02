import assert from 'node:assert/strict';
import { ChapterAdvance, chapterIntroPosition, chapterResumePosition, isChapterOutro, PendingMediaSeek } from '../src/features/player/playbackSession.ts';
import { ProgressSocket } from '../src/core/api/progressSocket.ts';
import { test } from 'node:test';
import { resolvePlaybackDuration } from '../src/core/utils/duration.ts';
import { HlsPlaybackSession, PlaybackSleep } from '../src/features/player/playbackSession.ts';
import type { HlsPlaybackInfo, HlsTransport } from '../src/features/player/playbackSession.ts';
import { ChapterCatalogue } from '../src/core/api/books.ts';
import type { ChapterPage } from '../src/core/api/books.ts';
import type { Chapter } from '../src/core/types/index.ts';

test('intro and outro preserve short chapters without an arbitrary ten-second margin', () => {
  assert.equal(chapterIntroPosition(15, 10, 20), 0);
  assert.equal(isChapterOutro(18, 20, 15, 10), false);
  assert.equal(chapterIntroPosition(15, 10, 30), 15);
  assert.equal(isChapterOutro(20, 30, 15, 10), true);
  assert.equal(isChapterOutro(19, 30, 15, 10), false);
  assert.equal(isChapterOutro(29, 30, 15, 0), false);
});

test('chapter boundaries advance once and reject paused or superseded playback', () => {
  const advance = new ChapterAdvance();
  assert.equal(advance.claim(1, 1, false), false);
  assert.equal(advance.claim(1, 2, true), false);
  assert.equal(advance.claim(2, 2, true), true);
  assert.equal(advance.claim(2, 2, true), false);
  assert.equal(advance.claim(3, 3, true), true);
});

test('resume and replay share the same intro position for native and transcoded audio', () => {
  assert.equal(chapterResumePosition(5, 600, 15, 10), 15);
  assert.equal(chapterResumePosition(200, 600, 15, 10), 200);
  assert.equal(chapterResumePosition(599, 600, 15, 10), 15);
  assert.equal(chapterResumePosition(0, 20, 15, 10), 0);
  assert.equal(chapterResumePosition(NaN, 600, 15, 10), 15);
});

test('native resume retries discarded seeks and does not overwrite a newer source or manual seek', () => {
  const pending = new PendingMediaSeek();
  const media = { currentTime: 0, readyState: 0, seeking: false };
  pending.request('first', 30);
  assert.equal(pending.apply('first', media), false);
  media.readyState = 3;
  assert.equal(pending.apply('first', media), false);
  assert.equal(media.currentTime, 30);
  media.currentTime = 0; // Platform discarded the seek while loading.
  assert.equal(pending.apply('first', media), false);
  assert.equal(media.currentTime, 30);
  assert.equal(pending.apply('first', media), true);
  pending.request('first', 90);
  pending.apply('second', media);
  assert.equal(media.currentTime, 30);
  pending.request('second', 60);
  pending.cancel();
  assert.equal(pending.apply('second', media), true);
  assert.equal(media.currentTime, 30);
});

test('native resume waits for buffering without restarting a confirmed seek', () => {
  const pending = new PendingMediaSeek();
  let currentTime = 0;
  let seeks = 0;
  const media = {
    readyState: 1,
    seeking: false,
    get currentTime() { return currentTime; },
    set currentTime(value: number) { currentTime = value; seeks++; },
  };
  pending.request('chapter', 30);
  assert.equal(pending.apply('chapter', media), false);
  assert.equal(seeks, 1);
  media.readyState = 2;
  for (let i = 0; i < 5; i++) assert.equal(pending.apply('chapter', media), false);
  assert.equal(seeks, 1);
  currentTime = 0; // A later native reset still needs correction.
  assert.equal(pending.apply('chapter', media), false);
  assert.equal(seeks, 2);
  media.readyState = 3;
  assert.equal(pending.apply('chapter', media), true);
  assert.equal(seeks, 2);
  currentTime = 35;
  assert.equal(pending.apply('chapter', media), true);
  assert.equal(currentTime, 35);
});

class TestSocket {
  readyState = 0;
  onopen: (() => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;
  onmessage: ((event: { data: string }) => void) | null = null;
  sent: string[] = [];
  closed = 0;
  open() { this.readyState = 1; this.onopen?.(); }
  send(data: string) { this.sent.push(data); }
  close() { this.readyState = 3; this.closed++; this.onclose?.(); }
}

function socketHarness() {
  const sockets: TestSocket[] = [];
  const connected: boolean[] = [];
  const messages: unknown[] = [];
  const connection = new ProgressSocket({
    createSocket: () => {
      const socket = new TestSocket();
      sockets.push(socket);
      return socket as unknown as WebSocket;
    },
    connected: value => connected.push(value),
    message: value => messages.push(value),
  });
  return { connection, sockets, connected, messages };
}

test('progress sockets reconnect on missing pong and dispose all timers', t => {
  t.mock.timers.enable({ apis: ['setTimeout', 'setInterval'] });
  const h = socketHarness();
  h.connection.connect();
  const socket = h.sockets[0];
  socket.open();
  t.mock.timers.tick(30000);
  assert.deepEqual(socket.sent, ['{"type":"ping"}']);
  t.mock.timers.tick(10000);
  assert.equal(socket.closed, 1);
  assert.equal(h.connected.at(-1), false);
  t.mock.timers.tick(1000);
  assert.equal(h.sockets.length, 2);
  h.connection.dispose();
  t.mock.timers.tick(120000);
  assert.equal(h.sockets.length, 2);
});

test('progress sockets discard old callbacks and retry stalled handshakes', t => {
  t.mock.timers.enable({ apis: ['setTimeout', 'setInterval'] });
  const h = socketHarness();
  h.connection.connect();
  const first = h.sockets[0];
  const oldMessage = first.onmessage!;
  const oldOpen = first.onopen!;
  t.mock.timers.tick(15000);
  t.mock.timers.tick(1000);
  h.sockets[1].open();
  oldMessage({ data: '{"type":"progress_updated"}' });
  oldOpen();
  assert.deepEqual(h.messages, []);
  assert.equal(h.connected.filter(Boolean).length, 1);
  h.connection.dispose();
});

test('foreground probes keep healthy progress sockets and reconnect dead ones', t => {
  t.mock.timers.enable({ apis: ['setTimeout', 'setInterval'] });
  const h = socketHarness();
  h.connection.connect();
  h.sockets[0].open();
  h.connection.resume();
  h.sockets[0].onmessage!({ data: '{"type":"pong"}' });
  t.mock.timers.tick(10000);
  assert.equal(h.sockets.length, 1);
  assert.equal(h.sockets[0].closed, 0);
  h.connection.send({ type: 'progress_update', position: 20 });
  assert.equal(JSON.parse(h.sockets[0].sent.at(-1)!).position, 20);
  h.connection.resume();
  t.mock.timers.tick(10000);
  h.connection.resume();
  assert.equal(h.sockets.length, 2);
  h.connection.dispose();
});
import {
  registerMediaSessionControls,
  updateMediaSessionMetadata,
  updateMediaSessionPlayback,
} from '../src/features/player/mediaSession.ts';

const deferred = <T>() => {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
};

test('sleep minutes pause, survive delayed ticks and expire exactly once', () => {
  const sleep = new PlaybackSleep();
  sleep.startSeconds(60, false, 0);
  assert.equal(sleep.snapshot(100_000).seconds, 60);
  assert.equal(sleep.expired(100_000), false);
  sleep.setPlaying(true, 100_000);
  assert.equal(sleep.snapshot(120_000).seconds, 40);
  sleep.setPlaying(false, 120_000);
  assert.equal(sleep.snapshot(200_000).seconds, 40);
  sleep.setPlaying(true, 200_000);
  assert.equal(sleep.expired(250_000), true);
  assert.equal(sleep.expired(250_001), false);
});

test('minute and episode sleep share one mutually exclusive policy', () => {
  const sleep = new PlaybackSleep();
  sleep.startSeconds(60, true, 0);
  sleep.startEpisodes(2);
  assert.deepEqual(sleep.snapshot(0), { seconds: null, episodes: 2 });
  assert.equal(sleep.chapterEnded(), false);
  assert.equal(sleep.chapterEnded(), true);
  assert.equal(sleep.chapterEnded(), false);
  sleep.startEpisodes(3);
  sleep.startSeconds(10, true, 0);
  assert.equal(sleep.snapshot(0).episodes, null);
  sleep.cancel();
  assert.deepEqual(sleep.snapshot(20_000), { seconds: null, episodes: null });
});

const hlsFixture = () => {
  const created = deferred<HlsPlaybackInfo>();
  const released: string[] = [];
  const requests: number[] = [];
  const responses: ReturnType<typeof deferred<HlsPlaybackInfo>>[] = [];
  const transport: HlsTransport = {
    create: () => created.promise,
    seek: async (_session, position) => {
      requests.push(position);
      const response = deferred<HlsPlaybackInfo>();
      responses.push(response);
      return response.promise;
    },
    touch: async () => {},
    release: async (session) => { released.push(session); },
  };
  return { created, released, requests, responses, transport };
};

test('chapter paging cancels replaced requests and suppresses late results', async () => {
  const responses: ReturnType<typeof deferred<ChapterPage>>[] = [];
  const signals: AbortSignal[] = [];
  const owner = new ChapterCatalogue(async (_params, signal) => {
    signals.push(signal);
    const response = deferred<ChapterPage>();
    responses.push(response);
    return response.promise;
  });
  const first = owner.page({ offset: 0 });
  const second = owner.page({ offset: 100 });
  assert.equal(signals[0].aborted, true);
  const data: ChapterPage = { chapters: [], total: 200, main_total: 200,
    extra_total: 0, chapter_type: 'main', offset: 100 };
  responses[1].resolve(data);
  assert.equal(await second, data);
  responses[0].resolve({ ...data, offset: 0 });
  assert.equal(await first, null);
  owner.dispose();
});

test('playback and chapter editing share the catalogue and invalidation cancels stale loads', async () => {
  const responses: ReturnType<typeof deferred<Chapter[]>>[] = [];
  const signals: AbortSignal[] = [];
  const owner = new ChapterCatalogue(async (_params, signal) => {
    signals.push(signal);
    const response = deferred<Chapter[]>();
    responses.push(response);
    return response.promise;
  });
  const playing = owner.all();
  assert.equal(owner.all(), playing);
  owner.invalidate();
  assert.equal(signals[0].aborted, true);
  const fresh = owner.all();
  responses[0].resolve([]);
  assert.equal(await playing, null);
  responses[1].resolve([]);
  assert.deepEqual(await fresh, []);
  assert.deepEqual(await owner.all(), []);
  assert.equal(responses.length, 2);
  owner.dispose();
  assert.equal(await owner.all(), null);
});

test('HLS opening is shared and a slow heartbeat cannot accumulate requests', async () => {
  const fixture = hlsFixture();
  let touches = 0;
  const touching = deferred<void>();
  fixture.transport.touch = async () => {
    touches++;
    await touching.promise;
  };
  const owner = new HlsPlaybackSession(fixture.transport, () => {}, 1);
  const opening = owner.open();
  assert.equal(owner.open(), opening);
  fixture.created.resolve({ session_id: 'heartbeat', playlist_url: '/list.m3u8' });
  await opening;
  await new Promise(resolve => setTimeout(resolve, 20));
  assert.equal(touches, 1);
  owner.dispose();
  touching.resolve();
  await new Promise(resolve => setTimeout(resolve, 5));
  assert.equal(touches, 1);
  assert.deepEqual(fixture.released, ['heartbeat']);
});

test('HLS owners release a late creation result after chapter disposal exactly once', async () => {
  const fixture = hlsFixture();
  const owner = new HlsPlaybackSession(fixture.transport, () => {});
  const opened = owner.open();
  owner.dispose();
  fixture.created.resolve({ session_id: 'late', playlist_url: '/playlist.m3u8' });
  assert.equal(await opened, null);
  owner.dispose();
  assert.deepEqual(fixture.released, ['late']);
});

test('HLS seek serializes server operations and discards superseded responses', async () => {
  const fixture = hlsFixture();
  const owner = new HlsPlaybackSession(fixture.transport, () => {});
  fixture.created.resolve({ session_id: 'current', playlist_url: '/playlist.m3u8' });
  await owner.open();
  const first = owner.seek(10);
  await Promise.resolve();
  const second = owner.seek(20);
  assert.deepEqual(fixture.requests, [10]);
  fixture.responses[0].resolve({ session_id: 'current', playlist_url: '/old.m3u8' });
  assert.equal(await first, null);
  await Promise.resolve();
  assert.deepEqual(fixture.requests, [10, 20]);
  fixture.responses[1].resolve({ session_id: 'current', playlist_url: '/new.m3u8', start_offset: 20 });
  assert.equal((await second)?.start_offset, 20);
  owner.dispose();
});

test('HLS invalid responses are released and failed seeks do not poison the queue', async () => {
  const fixture = hlsFixture();
  const owner = new HlsPlaybackSession(fixture.transport, () => {});
  fixture.created.resolve({ session_id: 'invalid', playlist_url: '', start_offset: NaN });
  await assert.rejects(owner.open());
  assert.deepEqual(fixture.released, ['invalid']);
  owner.dispose();
  const next = hlsFixture();
  const active = new HlsPlaybackSession(next.transport, () => {});
  next.created.resolve({ session_id: 'session', playlist_url: '/playlist.m3u8' });
  await active.open();
  const failed = active.seek(10);
  await Promise.resolve();
  next.responses[0].reject(new Error('network'));
  await assert.rejects(failed);
  const retry = active.seek(20);
  await Promise.resolve();
  next.responses[1].resolve({ session_id: 'session', playlist_url: '/retry.m3u8' });
  assert.equal((await retry)?.playlist_url, '/retry.m3u8');
  active.dispose();
});

const createSession = (unsupported: MediaSessionAction[] = []) => {
  const actions = new Map<MediaSessionAction, MediaSessionActionHandler | null>();
  const positions: (MediaPositionState | undefined)[] = [];
  const session = {
    metadata: null as MediaMetadata | null,
    playbackState: 'none' as MediaSessionPlaybackState,
    setActionHandler(action: MediaSessionAction, handler: MediaSessionActionHandler | null) {
      if (unsupported.includes(action)) throw new Error('Unsupported action');
      actions.set(action, handler);
    },
    setPositionState(state?: MediaPositionState) {
      positions.push(state);
    },
  } as MediaSession;
  return { session, actions, positions };
};

test('chapter duration overrides partial browser estimates and Infinity', () => {
  assert.equal(resolvePlaybackDuration(660, 120, true), 660);
  assert.equal(resolvePlaybackDuration(660, Infinity, true), 660);
  assert.equal(resolvePlaybackDuration(660, 650, false), 660);
});

test('transcodes never use or persist an estimated duration', () => {
  for (const source of [undefined, 0, -1, NaN, Infinity]) {
    for (const estimate of [120, Infinity, NaN, 0]) {
      assert.equal(resolvePlaybackDuration(source, estimate, true), 0);
    }
  }
});

test('ordinary audio can use finite browser metadata', () => {
  assert.equal(resolvePlaybackDuration(undefined, 660, false), 660);
  for (const estimate of [Infinity, NaN, 0, -1]) {
    assert.equal(resolvePlaybackDuration(undefined, estimate, false), 0);
  }
});

test('notification controls route actions and use the latest chapter position', () => {
  const { session, actions } = createSession();
  const calls: (string | number)[] = [];
  let position = 100;
  const cleanup = registerMediaSessionControls(session, {
    play: () => calls.push('play'),
    pause: () => calls.push('pause'),
    previous: () => calls.push('previous'),
    next: () => calls.push('next'),
    seek: (target) => calls.push(target),
    getPosition: () => position,
  });
  const dispatch = (action: MediaSessionAction, details = {}) =>
    actions.get(action)?.({ action, ...details });

  dispatch('play');
  dispatch('pause');
  dispatch('previoustrack');
  dispatch('nexttrack');
  dispatch('seekbackward');
  position = 200;
  dispatch('seekforward');
  dispatch('seekforward', { seekOffset: 10 });
  dispatch('seekto', { seekTime: 400 });
  dispatch('seekto');
  dispatch('seekto', { seekTime: NaN });
  assert.deepEqual(calls, ['play', 'pause', 'previous', 'next', 85, 230, 210, 400]);

  cleanup();
  assert.equal(actions.size, 7);
  assert.ok([...actions.values()].every((handler) => handler === null));
});

test('unsupported media actions do not prevent registration or cleanup', () => {
  const { session, actions } = createSession(['seekto', 'nexttrack']);
  const cleanup = registerMediaSessionControls(session, {
    play() {}, pause() {}, previous() {}, next() {}, seek() {},
    getPosition: () => 0,
  });
  assert.equal(typeof actions.get('play'), 'function');
  assert.equal(actions.has('seekto'), false);
  assert.doesNotThrow(cleanup);
});

test('system progress uses full chapter duration and playback speed', () => {
  const { session, positions } = createSession();
  updateMediaSessionPlayback(session, true, 660, 180, 1.5);
  assert.equal(session.playbackState, 'playing');
  assert.deepEqual(positions.at(-1), { duration: 660, position: 180, playbackRate: 1.5 });
  updateMediaSessionPlayback(session, false, 660, 180, 1.5);
  assert.equal(session.playbackState, 'paused');
});

test('system progress clears stale duration when the new source is unknown', () => {
  const { session, positions } = createSession();
  updateMediaSessionPlayback(session, true, 660, 180, 1);
  for (const duration of [0, NaN, Infinity, -1]) {
    updateMediaSessionPlayback(session, true, duration, 180, 1);
    assert.equal(positions.at(-1), undefined);
  }
});

test('system progress clamps positions and rejects invalid rates', () => {
  const { session, positions } = createSession();
  for (const [position, expected] of [[-10, 0], [800, 660], [NaN, 0]]) {
    updateMediaSessionPlayback(session, true, 660, position, 0);
    assert.deepEqual(positions.at(-1), { duration: 660, position: expected, playbackRate: 1 });
  }
});

test('browsers without system position support can still play and pause', () => {
  const session = { playbackState: 'none' } as MediaSession;
  assert.doesNotThrow(() => updateMediaSessionPlayback(session, true, 660, 10, 1));
  assert.equal(session.playbackState, 'playing');
});

test('metadata contains chapter, book, narrator and authenticated cover URL', () => {
  const original = Object.getOwnPropertyDescriptor(globalThis, 'MediaMetadata');
  Object.defineProperty(globalThis, 'MediaMetadata', {
    configurable: true,
    value: class {
      constructor(metadata: MediaMetadataInit) {
        Object.assign(this, metadata);
      }
    },
  });
  try {
    const { session } = createSession();
    const metadata = {
      title: 'Chapter 1',
      artist: 'Narrator',
      album: 'Audiobook',
      artwork: [{ src: 'https://reader.test/ting/api/proxy/cover?book_id=1&token=test' }],
    };
    updateMediaSessionMetadata(session, metadata);
    assert.equal(session.metadata?.title, metadata.title);
    assert.equal(session.metadata?.artist, metadata.artist);
    assert.equal(session.metadata?.album, metadata.album);
    assert.deepEqual(session.metadata?.artwork, metadata.artwork);
    updateMediaSessionMetadata(session, { ...metadata, title: 'Chapter 2' });
    assert.equal(session.metadata?.title, 'Chapter 2');
    updateMediaSessionMetadata(session, null);
    assert.equal(session.metadata, null);
  } finally {
    if (original) Object.defineProperty(globalThis, 'MediaMetadata', original);
    else Reflect.deleteProperty(globalThis, 'MediaMetadata');
  }
});

test('browsers without MediaMetadata do not throw', () => {
  const { session } = createSession();
  assert.doesNotThrow(() => updateMediaSessionMetadata(session, { title: 'Chapter' }));
});
