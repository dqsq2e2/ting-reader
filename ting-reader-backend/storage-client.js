const { getWebDAVClient } = require('./webdav-client');
const { createLocalClient } = require('./local-client');
const path = require('path');

const STORAGE_ROOT = path.join(__dirname, 'storage');

function getStorageClient(library) {
  if (library.type === 'local') {
    // For local libraries, url is the relative path within STORAGE_ROOT
    const libraryRoot = path.join(STORAGE_ROOT, library.url || '');
    return createLocalClient(libraryRoot);
  }
  return getWebDAVClient(library);
}

module.exports = { getStorageClient };
