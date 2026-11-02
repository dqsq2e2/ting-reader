import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { VitePWA } from 'vite-plugin-pwa'
import legacy from 'vite-plugin-legacy-swc'
import { readFileSync } from 'node:fs'
import { cspSafeLegacyPlugin } from './build/cspSafeLegacyPlugin.ts'

const packageJson = JSON.parse(
  readFileSync(new URL('./package.json', import.meta.url), 'utf-8'),
) as { version?: string }

// A relative base works for both the fnOS gateway path and direct port access.
// A dedicated gateway-only build can still override it with VITE_APP_BASE_URL.
const appBase = process.env.VITE_APP_BASE_URL || './'

// https://vite.dev/config/
export default defineConfig(({ command }) => ({
  define: {
    __APP_VERSION__: JSON.stringify(packageJson.version ?? 'unknown'),
  },
  plugins: [
    react(),
    legacy({
      targets: ['defaults', 'not IE 11', 'chrome >= 49', 'safari >= 10'],
    }),
    cspSafeLegacyPlugin(),
    VitePWA({
      registerType: 'autoUpdate',
      useCredentials: true,
      // vite-plugin-pwa matches dev requests literally, so a relative base
      // makes /manifest.webmanifest and its virtual module fall through to HTML.
      base: command === 'serve' && appBase === './' ? '/' : appBase,
      buildBase: './',
      includeAssets: ['favicon.ico', 'pwa-64.png', 'pwa-128.png', 'pwa-192.png', 'pwa-256.png', 'pwa-512.png', 'pwa-*-maskable.png'],
      devOptions: {
        enabled: true,
        type: 'module',
      },
      manifest: {
        id: 'ting-reader',
        name: 'Ting Reader',
        short_name: 'Ting Reader',
        description: 'Your self-hosted audiobook platform',
        theme_color: '#0284c7',
        background_color: '#ffffff',
        display: 'standalone',
        start_url: appBase,
        scope: appBase,
        orientation: 'portrait-primary',
        icons: [
          {
            src: 'favicon.ico',
            sizes: '256x256 128x128 64x64 48x48 32x32 16x16',
            type: 'image/x-icon'
          },
          {
            src: 'pwa-64.png',
            sizes: '64x64',
            type: 'image/png',
            purpose: 'any'
          },
          {
            src: 'pwa-128.png',
            sizes: '128x128',
            type: 'image/png',
            purpose: 'any'
          },
          {
            src: 'pwa-192.png',
            sizes: '192x192',
            type: 'image/png',
            purpose: 'any'
          },
          {
            src: 'pwa-256.png',
            sizes: '256x256',
            type: 'image/png',
            purpose: 'any'
          },
          {
            src: 'pwa-512.png',
            sizes: '512x512',
            type: 'image/png',
            purpose: 'any'
          },
          {
            src: 'pwa-64-maskable.png',
            sizes: '64x64',
            type: 'image/png',
            purpose: 'maskable'
          },
          {
            src: 'pwa-128-maskable.png',
            sizes: '128x128',
            type: 'image/png',
            purpose: 'maskable'
          },
          {
            src: 'pwa-192-maskable.png',
            sizes: '192x192',
            type: 'image/png',
            purpose: 'maskable'
          },
          {
            src: 'pwa-256-maskable.png',
            sizes: '256x256',
            type: 'image/png',
            purpose: 'maskable'
          },
          {
            src: 'pwa-512-maskable.png',
            sizes: '512x512',
            type: 'image/png',
            purpose: 'maskable'
          }
        ]
      },
      workbox: {
        globPatterns: ['**/*.{js,css,html,ico,png,svg}'],
        // Exclude widget routes from service worker navigation fallback
        // This ensures widget requests go to the server and get fresh headers (CSP)
        navigateFallbackDenylist: [/^\/widget/],
        runtimeCaching: [
          {
            urlPattern: /^https:\/\/fonts\.googleapis\.com\/.*/i,
            handler: 'CacheFirst',
            options: {
              cacheName: 'google-fonts-cache',
              expiration: {
                maxEntries: 10,
                maxAgeSeconds: 60 * 60 * 24 * 365 // <== 365 days
              },
              cacheableResponse: {
                statuses: [0, 200]
              }
            }
          },
          {
            urlPattern: /^https:\/\/fonts\.gstatic\.com\/.*/i,
            handler: 'CacheFirst',
            options: {
              cacheName: 'gstatic-fonts-cache',
              expiration: {
                maxEntries: 10,
                maxAgeSeconds: 60 * 60 * 24 * 365 // <== 365 days
              },
              cacheableResponse: {
                statuses: [0, 200]
              }
            }
          }
        ]
      }
    })
  ],
  base: appBase,
  build: {
    outDir: 'dist',
    minify: 'oxc',
    sourcemap: false,
    // SWC reads these options for legacy chunks and their polyfills.
    terserOptions: {
      compress: {
        drop_console: true,
        drop_debugger: true,
      },
    },
    chunkSizeWarningLimit: 1000,
    rolldownOptions: {
      output: {
        minify: {
          compress: { dropConsole: true, dropDebugger: true },
        },
        codeSplitting: {
          groups: [
            {
              name: 'vendor-react',
              test: /node_modules[\\/](?:react|react-dom|react-router|scheduler|zustand)[\\/]/,
            },
          ],
        },
      }
    }
  },
  server: {
    port: 3000,
    host: true,
  },
  optimizeDeps: {
    include: ['react-window']
  }
}))
