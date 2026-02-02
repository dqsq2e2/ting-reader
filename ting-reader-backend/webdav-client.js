const { createClient } = require('webdav');
const iconv = require('iconv-lite');

// Hack to handle WebDAV servers that return malformed (unencoded) URIs
// Some servers like Alist may return filenames with % that aren't properly escaped,
// or use GBK encoding instead of UTF-8.
const originalDecodeURIComponent = global.decodeURIComponent;
global.decodeURIComponent = function(str) {
  try {
    return originalDecodeURIComponent(str);
  } catch (e) {
    try {
      // Try decoding percent-encoded sequences as GBK if UTF-8 fails
      // This handles strings like "%C2%D2%CA%C0"
      return str.replace(/(%[0-9A-Fa-f]{2})+/g, (match) => {
        const bytes = match.split('%').filter(Boolean).map(hex => parseInt(hex, 16));
        return iconv.decode(Buffer.from(bytes), 'gbk');
      });
    } catch (gbkError) {
      // Ignore GBK errors
    }
    // If decoding fails, return the original string instead of crashing
    return str;
  }
};

function getWebDAVClient(library) {
  const options = {
    // Enable support for HTTP 302 redirects
    maxRedirects: 5
  };
  if (library.username) {
    options.username = library.username;
    options.password = library.password;
  }
  return createClient(library.url, options);
}

module.exports = { getWebDAVClient };
