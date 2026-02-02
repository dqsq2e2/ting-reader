const fs = require('fs');
const path = require('path');
const { Readable } = require('stream');

function createLocalClient(basePath = '') {
  return {
    getDirectoryContents: async (dirPath) => {
      const absolutePath = path.join(basePath, dirPath);
      const items = await fs.promises.readdir(absolutePath, { withFileTypes: true });
      return items.map(item => ({
        filename: path.join(dirPath, item.name).replace(/\\/g, '/'),
        basename: item.name,
        type: item.isDirectory() ? 'directory' : 'file',
        size: item.isFile() ? fs.statSync(path.join(absolutePath, item.name)).size : 0
      }));
    },

    getFileContents: async (filePath, options = {}) => {
      const absolutePath = path.join(basePath, filePath);
      if (options.range) {
        const { start, end } = options.range;
        const length = end - start + 1;
        const buffer = Buffer.alloc(length);
        const fd = await fs.promises.open(absolutePath, 'r');
        await fd.read(buffer, 0, length, start);
        await fd.close();
        return buffer;
      }
      return await fs.promises.readFile(absolutePath);
    },

    stat: async (filePath) => {
      const absolutePath = path.join(basePath, filePath);
      const stats = await fs.promises.stat(absolutePath);
      return {
        size: stats.size,
        type: stats.isDirectory() ? 'directory' : 'file',
        lastmod: stats.mtime.toISOString()
      };
    },

    createReadStream: (filePath, options = {}) => {
      const absolutePath = path.join(basePath, filePath);
      const fsOptions = {};
      if (options.range) {
        fsOptions.start = options.range.start;
        fsOptions.end = options.range.end;
      }
      return fs.createReadStream(absolutePath, fsOptions);
    }
  };
}

module.exports = { createLocalClient };
