import assert from 'node:assert/strict';
import { test } from 'node:test';
import { bookCredits, bookPrimaryCredit } from '../src/core/utils/book.ts';
import { LibraryViewCache, libraryCacheIdentity } from '../src/core/api/libraryCache.ts';

test('view snapshots persist without crossing accounts or servers and evict oversized data', () => {
  const values = new Map<string, string>();
  const storage = {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => { values.set(key, value); },
    removeItem: (key: string) => { values.delete(key); },
  };
  const alice = libraryCacheIdentity('https://server/', 'alice');
  const bob = libraryCacheIdentity('https://server/', 'bob');
  const cache = new LibraryViewCache(storage, 1024);
  cache.set(alice, 'shelf', { title: 'Cached book' });
  cache.flush();
  const restored = new LibraryViewCache(storage, 1024);
  assert.deepEqual(restored.get(alice, 'shelf'), { title: 'Cached book' });
  assert.equal(restored.get(bob, 'shelf'), null);
  assert.equal(restored.get(libraryCacheIdentity('https://other', 'alice'), 'shelf'), null);
  cache.set(alice, 'shelf', { title: 'x'.repeat(2000) });
  assert.equal(cache.get(alice, 'shelf'), null);
  for (let i = 0; i < 30; i++) cache.set(alice, `book:${i}`, { title: 'Book' });
  assert.equal(cache.get(alice, 'book:0'), null);
  assert.deepEqual(cache.get(alice, 'book:29'), { title: 'Book' });
  cache.dispose();
  restored.dispose();
});

test('corrupt or disabled browser cache never prevents viewing books', () => {
  const storage = {
    getItem: () => 'not json',
    setItem: () => { throw new Error('disabled'); },
    removeItem: () => { throw new Error('disabled'); },
  };
  const cache = new LibraryViewCache(storage);
  assert.equal(cache.get('user', 'shelf'), null);
  cache.set('user', 'shelf', { books: [] });
  assert.doesNotThrow(() => cache.dispose());
});

test('book credits omit missing and legacy unknown placeholders in either field', () => {
  for (const unknown of [undefined, null, '', '  ', 'Unknown', ' unknow ', 'UNKNOWN AUTHOR',
    'Unknown Narrator', 'Unknown Artist', '未知', '未知作者', '未知演播', '未知主播', '未知演播者', '未知演播人']) {
    assert.equal(bookCredits(unknown, unknown), '');
    assert.equal(bookCredits(unknown, ' 演播者 '), '演播者');
    assert.equal(bookCredits(' 作者 ', unknown), '作者');
    assert.equal(bookPrimaryCredit(unknown, unknown), '');
    assert.equal(bookPrimaryCredit(unknown, ' 演播者 '), '演播者');
    assert.equal(bookPrimaryCredit(' 作者 ', unknown), '作者');
  }
  assert.equal(bookCredits('作者', '演播者'), '作者 · 演播者');
  assert.equal(bookCredits('未知君', 'Unknown Stories'), '未知君 · Unknown Stories');
  assert.equal(bookPrimaryCredit(' 作者 ', ' 演播者 '), '作者');
  assert.equal(bookPrimaryCredit('未知君', 'Unknown Stories'), '未知君');
});
