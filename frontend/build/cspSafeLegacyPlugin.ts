import { createHash } from 'node:crypto'
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises'
import path from 'node:path'
import type { Plugin, ResolvedConfig } from 'vite'

const createAssetName = (prefix: string, code: string) => {
  const hash = createHash('sha256').update(code).digest('hex').slice(0, 12)
  return `${prefix}-${hash}.js`
}

const ensureTrailingNewline = (code: string) => code.endsWith('\n') ? code : `${code}\n`

const executableScriptType = (attrs: string) => {
  const type = attrs.match(/\btype\s*=\s*["']?([^"'\s>]+)/i)?.[1]?.toLowerCase()
  if (!type) return true
  return ['module', 'text/javascript', 'application/javascript'].includes(type)
}

const hasScriptSrc = (attrs: string) => /(?:^|\s)src\s*=/i.test(attrs)

const publicAssetPath = (config: ResolvedConfig, assetsDir: string, filename: string) => {
  const base = config.base || '/'
  const assetPath = `${assetsDir}/${filename}`

  if (base === './' || base === '') return assetPath
  return `${base.replace(/\/$/, '')}/${assetPath}`
}

const cspUnsafeDataScriptRe = /import\s*(?:\(|['"])data:text\/javascript/

export const cspSafeLegacyPlugin = (): Plugin => {
  const emittedDataAssets = new Set<string>()
  let config: ResolvedConfig

  const externalizeDataImports = (
    code: string,
    writeDataAsset: (source: string) => string,
  ) => {
    const dataImportRe = /import(['"])data:text\/javascript,([\s\S]*?)\1/g
    let rewritten = ''
    let lastIndex = 0
    let changed = false

    for (const match of code.matchAll(dataImportRe)) {
      const [fullMatch, quote, dataModuleSource] = match
      const index = match.index ?? 0
      const assetPath = writeDataAsset(ensureTrailingNewline(dataModuleSource))
      rewritten += code.slice(lastIndex, index)
      rewritten += `import${quote}${assetPath}${quote}`
      lastIndex = index + fullMatch.length
      changed = true
    }

    if (!changed) return { code, changed: false }
    return { code: rewritten + code.slice(lastIndex), changed: true }
  }

  return {
    name: 'ting-reader:csp-safe-legacy-output',
    apply: 'build',
    enforce: 'post',
    configResolved(resolvedConfig) {
      config = resolvedConfig
    },
    renderChunk(code) {
      const assetsDir = config.build.assetsDir
      const result = externalizeDataImports(code, (source) => {
        const filename = createAssetName('vite-legacy-data', source)
        if (!emittedDataAssets.has(filename)) {
          this.emitFile({
            type: 'asset',
            fileName: `${assetsDir}/${filename}`,
            source,
          })
          emittedDataAssets.add(filename)
        }
        return `./${filename}`
      })

      return result.changed ? { code: result.code, map: null } : null
    },
    async writeBundle(options) {
      const outDir = options.dir
      if (!outDir) return

      const assetsDir = config.build.assetsDir
      const indexPath = path.join(outDir, 'index.html')
      const outputAssetsDir = path.join(outDir, assetsDir)
      let html: string

      try {
        html = await readFile(indexPath, 'utf8')
      } catch {
        return
      }

      await mkdir(outputAssetsDir, { recursive: true })

      const writeInlineAsset = async (code: string) => {
        const filename = createAssetName('vite-legacy-inline', code)
        await writeFile(path.join(outputAssetsDir, filename), code)
        return publicAssetPath(config, assetsDir, filename)
      }
      const pendingDataAssets = new Map<string, string>()

      const scriptRe = /<script\b([^>]*)>([\s\S]*?)<\/script>/gi
      let rewrittenHtml = ''
      let lastIndex = 0
      let changed = false

      for (const match of html.matchAll(scriptRe)) {
        const [fullMatch, attrs, rawCode] = match
        const index = match.index ?? 0
        const code = rawCode.trim()

        if (hasScriptSrc(attrs) || !code || !executableScriptType(attrs)) {
          continue
        }

        const externalized = externalizeDataImports(code, (source) => {
          const filename = createAssetName('vite-legacy-data', source)
          pendingDataAssets.set(filename, source)
          return `./${filename}`
        })
        const assetPath = await writeInlineAsset(ensureTrailingNewline(externalized.code))

        rewrittenHtml += html.slice(lastIndex, index)
        rewrittenHtml += `<script${attrs} src="${assetPath}"></script>`
        lastIndex = index + fullMatch.length
        changed = true
      }

      const finalHtml = changed ? rewrittenHtml + html.slice(lastIndex) : html

      if (changed) {
        await Promise.all(
          Array.from(pendingDataAssets, ([filename, source]) => (
            writeFile(path.join(outputAssetsDir, filename), source)
          )),
        )
        await writeFile(indexPath, finalHtml)
      }

      for (const match of finalHtml.matchAll(scriptRe)) {
        const [fullMatch, attrs, rawCode] = match
        if (executableScriptType(attrs) && rawCode.trim()) {
          throw new Error(`CSP-unsafe inline script remained in index.html: ${fullMatch}`)
        }
      }

      if (cspUnsafeDataScriptRe.test(finalHtml)) {
        throw new Error('CSP-unsafe data: script import remained in index.html')
      }

      const jsFiles = await readdir(outputAssetsDir)
      await Promise.all(
        jsFiles
          .filter((file) => file.endsWith('.js'))
          .map(async (file) => {
            const filePath = path.join(outputAssetsDir, file)
            const js = await readFile(filePath, 'utf8')
            if (cspUnsafeDataScriptRe.test(js)) {
              throw new Error(`CSP-unsafe data: script import remained in ${file}`)
            }
          }),
      )
    },
  }
}
