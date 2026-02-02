const ColorThief = require('colorthief');
const axios = require('axios');

/**
 * Calculates a theme color from an image buffer or URL
 * Returns an rgba string with 0.1 alpha for UI background use
 */
async function calculateThemeColor(imageUrl, client = null) {
  if (!imageUrl || imageUrl === '') return null;
  
  try {
    let buffer;
    if (imageUrl.startsWith('http') || imageUrl.startsWith('//')) {
      const fullUrl = imageUrl.startsWith('//') ? `https:${imageUrl}` : imageUrl;
      const response = await axios.get(fullUrl, { responseType: 'arraybuffer', timeout: 5000 });
      buffer = Buffer.from(response.data);
    } else if (imageUrl.startsWith('embedded://')) {
      // Embedded covers are handled by the proxy, hard to extract here
      return null;
    } else if (client) {
      // Local path from WebDAV/Local
      buffer = await client.getFileContents(imageUrl, { format: 'binary' });
    }
    
    if (buffer) {
      const color = await ColorThief.getColor(buffer);
      if (color) {
        // Return the rgba string that the frontend expects
        return `rgba(${color[0]}, ${color[1]}, ${color[2]}, 0.1)`;
      }
    }
  } catch (e) {
    // console.error('Error calculating theme color:', e.message);
  }
  return null;
}

module.exports = { calculateThemeColor };
