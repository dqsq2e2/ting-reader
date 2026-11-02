import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
import { getPluginLucideIcon } from '../src/shared/pluginExtensions/pluginIconRegistry.ts';
import {
  externalUrlRequest,
  isBridgeMessageWithinLimit,
  isBridgeRequest,
  isDocumentReady,
  isDocumentUnloading,
  responseFor,
} from '../src/shared/pluginExtensions/pluginBridge.ts';

const request = {
  type: 'ting-plugin:request',
  bridge_token: 'document-token',
  id: 'request-1',
  method: 'host.invoke',
  params: { title: 'book' },
};

test('deferred plugin icon registry preserves names, prefixes and separators', () => {
  const headphones = getPluginLucideIcon('Headphones');
  assert.ok(headphones);
  for (const name of ['headphones', ' Headphones ', 'lucide:headphones']) {
    assert.equal(getPluginLucideIcon(name), headphones);
  }
  const book = getPluginLucideIcon('BookOpen');
  assert.ok(book);
  for (const name of ['book-open', 'book_open', 'book open', 'lucide:book-open']) {
    assert.equal(getPluginLucideIcon(name), book);
  }
  assert.ok(getPluginLucideIcon('CircleAlert'));
  assert.equal(getPluginLucideIcon('no-such-icon'), undefined);
  assert.equal(getPluginLucideIcon(''), undefined);
});

test('bridge accepts only the current document token and declared operations', () => {
  assert.equal(isBridgeRequest(request, 'document-token'), true);
  assert.equal(isBridgeRequest(request, 'previous-token'), false);
  assert.equal(isBridgeRequest({ ...request, method: 'other.invoke' }, 'document-token'), false);
  assert.equal(isBridgeRequest({ ...request, id: '' }, 'document-token'), false);
  assert.equal(isBridgeRequest({ ...request, id: 'x'.repeat(129) }, 'document-token'), false);
});

test('bridge rejects oversized, non-JSON, cyclic and deeply nested messages', () => {
  assert.equal(isBridgeMessageWithinLimit(request), true);
  for (const params of [NaN, Infinity, () => {}, new Date(), 'x'.repeat(256 * 1024)]) {
    assert.equal(isBridgeMessageWithinLimit({ ...request, params }), false);
  }
  const cyclic: Record<string, unknown> = {};
  cyclic.self = cyclic;
  assert.equal(isBridgeMessageWithinLimit(cyclic), false);
  let deep: unknown = {};
  for (let level = 0; level < 70; level++) deep = { nested: deep };
  assert.equal(isBridgeMessageWithinLimit(deep), false);
});

test('external navigation is restricted to HTTP(S) and the current document', () => {
  const message = { type: 'ting-plugin:external-url', bridge_token: 'document-token', url: 'https://example.com' };
  assert.equal(externalUrlRequest(message, 'document-token')?.url, 'https://example.com/');
  assert.equal(externalUrlRequest(message, 'previous-token'), undefined);
  for (const url of ['javascript:alert(1)', 'file:///etc/passwd', 'data:text/html,hello', '/relative']) {
    assert.equal(externalUrlRequest({ ...message, url }, 'document-token'), undefined);
  }
});

test('lifecycle messages and responses are bound to their document and request', () => {
  assert.equal(isDocumentReady({ type: 'ting-plugin:document-ready', bridge_token: 'current' }, 'current'), true);
  assert.equal(isDocumentUnloading({ type: 'ting-plugin:document-unloading', bridge_token: 'old' }, 'current'), false);
  const response = responseFor(request as Parameters<typeof responseFor>[0], { ok: true, result: { done: true } });
  assert.equal(response.id, request.id);
  assert.equal(response.bridge_token, request.bridge_token);
  assert.equal(response.type, 'ting-plugin:response');
});

test('plugin bridge rejection preserves structured HTTP failures in the page Error', () => {
  const source = readFileSync(new URL('../public/plugin-container-bootstrap.js', import.meta.url), 'utf8');
  const start = source.indexOf('function createRequestError(');
  const end = source.indexOf('  var postBridgeMessage', start);
  const createError = vm.runInNewContext(`(${source.slice(start, end).trim()})`);
  const payload = {
    error: 'PluginExecutionError', message: 'Provider unavailable', status: 503,
    details: { reason: 'plugin_unavailable', operation: 'search', retryable: true },
    trace_id: 'trace-bridge',
  };
  const failure = responseFor(request as Parameters<typeof responseFor>[0], { ok: false, error: payload });
  const error = createError(failure.error, 'fallback');
  assert.equal(error.message, payload.message);
  assert.equal(error.code, payload.error);
  assert.equal(error.details.reason, payload.details.reason);
  assert.equal(error.retryable, true);
  assert.equal(error.trace_id, payload.trace_id);
  assert.equal(error.status, 503);
  assert.equal(error.body.message, payload.message);
  assert.equal(createError(null, 'fallback').message, 'fallback');
});
