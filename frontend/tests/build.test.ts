import assert from 'node:assert/strict';
import { mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import { build, loadConfigFromFile } from 'vite';

test('production outputs keep minification, legacy loading and CSP-safe scripts', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'ting-reader-build-'));
  try {
    const loaded = await loadConfigFromFile(
      { command: 'build', mode: 'production' },
      fileURLToPath(new URL('../vite.config.ts', import.meta.url)),
    );
    assert.ok(loaded);
    assert.equal(loaded.config.build?.minify, 'oxc');
    await writeFile(path.join(root, 'index.html'), '<!doctype html><html><head></head><body><div id="root"></div><script type="module" src="/entry.js"></script></body></html>');
    await writeFile(path.join(root, 'entry.js'), `
      console.log('__CONSOLE_MUST_BE_REMOVED__');
      debugger;
      window.buildReady = () => import('./lazy.js').then(module => module.value);
    `);
    await writeFile(path.join(root, 'lazy.js'), `
      console.warn('__CONSOLE_MUST_BE_REMOVED__');
      debugger;
      export const value = window.missing?.value ?? 'ready';
    `);
    await build({
      ...loaded.config,
      configFile: false,
      root,
      logLevel: 'silent',
      build: { ...loaded.config.build, outDir: path.join(root, 'dist') },
    });
    const dist = path.join(root, 'dist');
    const html = await readFile(path.join(dist, 'index.html'), 'utf8');
    assert.match(html, /id="vite-legacy-entry"/);
    assert.match(html, /id="vite-legacy-polyfill"/);
    for (const script of html.matchAll(/<script\b[^>]*>([\s\S]*?)<\/script>/g)) {
      assert.equal(script[1].trim(), '');
    }
    const files = await readdir(path.join(dist, 'assets'));
    assert.ok(files.some(file => /^index-legacy-.*\.js$/.test(file)));
    assert.ok(files.some(file => /^index-(?!legacy-).*\.js$/.test(file)));
    for (const file of files.filter(file => file.endsWith('.js'))) {
      const code = await readFile(path.join(dist, 'assets', file), 'utf8');
      assert.ok(!code.includes('__CONSOLE_MUST_BE_REMOVED__'), file);
      assert.doesNotMatch(code, /(?:^|[;{}])\s*debugger\s*(?:;|})/, file);
      assert.doesNotMatch(code, /import\s*(?:\(|['"])data:text\/javascript/, file);
      if (file.includes('-legacy-')) assert.doesNotMatch(code, /\?\.|\?\?/, file);
    }
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
