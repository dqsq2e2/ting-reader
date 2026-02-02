const fs = require('fs');
const crypto = require('crypto');
const nodeID3 = require('node-id3');
const path = require('path');

const XM_KEY = Buffer.from("ximalayaximalayaximalayaximalaya");

let wasmInstance = null;

async function getWasmInstance() {
  if (wasmInstance) return wasmInstance;
  const wasmBuffer = fs.readFileSync(path.join(__dirname, 'xm.wasm'));
  const wasmModule = await WebAssembly.instantiate(wasmBuffer, {});
  wasmInstance = wasmModule.instance;
  return wasmInstance;
}

async function decryptXM(content) {
  if (!content || content.length === 0) {
    throw new Error('Empty content provided to decryptXM');
  }
  
  console.log(`Starting decryption, content size: ${content.length}`);
  
  const headerSize = getID3Size(content);
  const header = content.slice(0, Math.max(headerSize, 8192));
  
  let tags = null;
  try {
    tags = nodeID3.read(header);
  } catch (e) {
    console.warn('node-id3 failed to read tags, will rely on manual extraction');
  }
  
  const xmInfo = {
    tracknumber: 0,
    size: 0,
    headerSize: headerSize,
    iv: null,
    encodingTechnology: ''
  };

  // Try extracting from nodeID3 tags
  if (tags) {
    xmInfo.tracknumber = parseInt(tags.trackNumber || tags.TRCK) || 0;
    xmInfo.size = parseInt(tags.size || tags.TSIZ || tags.userDefinedText?.find(t => t.description === 'TSIZ')?.value || '0');
    xmInfo.iv = tags.isrc || tags.TSRC || tags.encodedBy || tags.TENC || tags.ISRC;
    xmInfo.encodingTechnology = tags.encodingTechnology || tags.TSSE || tags.userDefinedText?.find(t => t.description === 'TSSE')?.value || '';
  }

  // Manual extraction always as fallback/validation
  const manual = extractTagsManually(content, headerSize);
  if (!xmInfo.iv) xmInfo.iv = manual.iv;
  if (xmInfo.size === 0) xmInfo.size = manual.size;
  if (!xmInfo.encodingTechnology) xmInfo.encodingTechnology = manual.tsse;
  if (xmInfo.tracknumber === 0) xmInfo.tracknumber = manual.track;

  // Final fallback for size
  if (xmInfo.size === 0) {
    xmInfo.size = content.length - xmInfo.headerSize;
  }

  console.log('XM Info extracted:', { 
    track: xmInfo.tracknumber, 
    size: xmInfo.size, 
    iv: xmInfo.iv ? (typeof xmInfo.iv === 'string' ? xmInfo.iv : 'Buffer') : 'MISSING',
    tech: xmInfo.encodingTechnology,
    headerSize: xmInfo.headerSize
  });

  if (!xmInfo.iv) {
    throw new Error('No IV found in tags (TSRC, ISRC, TENC, or manual)');
  }

  try {
    const encryptedData = content.slice(xmInfo.headerSize, xmInfo.headerSize + xmInfo.size);
    if (encryptedData.length === 0) {
      throw new Error(`Encrypted segment is empty. HeaderSize: ${xmInfo.headerSize}, Size: ${xmInfo.size}`);
    }

    const decodedData = await decryptAesSegment(encryptedData, xmInfo);
    
    const finalData = Buffer.concat([
      decodedData,
      content.slice(xmInfo.headerSize + xmInfo.size)
    ]);

    return finalData;
  } catch (err) {
    console.error('Decryption failed:', err.message);
    throw err;
  }
}

function getID3Size(buffer) {
  if (buffer.slice(0, 3).toString() !== 'ID3') return 0;
  // Size is at bytes 6-9, each 7 bits (synchsafe)
  const size = ((buffer[6] & 0x7f) << 21) | ((buffer[7] & 0x7f) << 14) | ((buffer[8] & 0x7f) << 7) | (buffer[9] & 0x7f);
  return size + 10; // +10 for header
}

function extractTagsManually(buffer, headerSize) {
  const tags = { iv: null, size: 0, tsse: '', track: 0 };
  if (headerSize <= 10) return tags;

  let pos = 10;
  const version = buffer[3]; // ID3v2.x

  while (pos < headerSize - 10) {
    let frameId = '';
    let frameSize = 0;
    
    if (version === 2) {
      frameId = buffer.slice(pos, pos + 3).toString();
      frameSize = (buffer[pos + 3] << 16) | (buffer[pos + 4] << 8) | buffer[pos + 5];
      if (frameId[0] === '\0') break;
      if (frameSize <= 0 || pos + 6 + frameSize > headerSize) break;
      
      const content = buffer.slice(pos + 6, pos + 6 + frameSize).toString('latin1').replace(/\0.*$/, '');
      if (frameId === 'TSI') tags.size = parseInt(content) || 0;
      else if (frameId === 'TRK') tags.track = parseInt(content) || 0;
      else if (frameId === 'TSS') tags.tsse = content.trim();
      else if (frameId === 'IRC' || frameId === 'TEN') {
        if (!tags.iv) tags.iv = content.trim();
      }
      pos += 6 + frameSize;
    } else {
      frameId = buffer.slice(pos, pos + 4).toString();
      if (frameId[0] === '\0') break;
      
      if (version === 4) {
        frameSize = ((buffer[pos + 4] & 0x7f) << 21) | ((buffer[pos + 5] & 0x7f) << 14) | ((buffer[pos + 6] & 0x7f) << 7) | (buffer[pos + 7] & 0x7f);
      } else {
        frameSize = buffer.readUInt32BE(pos + 4);
      }

      if (frameSize <= 0 || pos + 10 + frameSize > headerSize) break;

      const encoding = buffer[pos + 10];
      const frameContent = buffer.slice(pos + 11, pos + 10 + frameSize);
      let content = '';
      try {
        if (encoding === 0) content = frameContent.toString('latin1');
        else if (encoding === 1 || encoding === 2) content = frameContent.toString('utf16le');
        else if (encoding === 3) content = frameContent.toString('utf8');
        content = content.replace(/\0.*$/, '').trim();
      } catch (e) {}

      if (frameId === 'TSIZ') tags.size = parseInt(content) || 0;
      else if (frameId === 'TRCK') tags.track = parseInt(content) || 0;
      else if (frameId === 'TSSE') tags.tsse = content;
      else if (frameId === 'TSRC' || frameId === 'TENC' || frameId === 'ISRC') {
        if (!tags.iv && content.length >= 16) tags.iv = content;
      } else if (frameId === 'TLEN') {
        tags.length = content;
      }
      pos += 10 + frameSize;
    }
  }
  return tags;
}

async function decryptAesSegment(encryptedData, info) {
  try {
    const ivBuffer = getIvBuffer(info.iv);
    const decipher = crypto.createDecipheriv('aes-256-cbc', XM_KEY, ivBuffer);
    
    // Determine if this is the full encrypted segment or just a chunk
    const isFullSegment = encryptedData.length === info.size;
    
    if (isFullSegment) {
      decipher.setAutoPadding(true);
      try {
        const decryptedBuffer = Buffer.concat([decipher.update(encryptedData), decipher.final()]);
        return await processWasmDecryption(decryptedBuffer, info);
      } catch (e) {
        const decipher2 = crypto.createDecipheriv('aes-256-cbc', XM_KEY, ivBuffer);
        decipher2.setAutoPadding(false);
        const decryptedBuffer = Buffer.concat([decipher2.update(encryptedData), decipher2.final()]);
        return await processWasmDecryption(decryptedBuffer, info);
      }
    } else {
      decipher.setAutoPadding(false);
      let dataToDecrypt = encryptedData;
      if (encryptedData.length % 16 !== 0) {
        dataToDecrypt = encryptedData.slice(0, Math.floor(encryptedData.length / 16) * 16);
      }
      
      if (dataToDecrypt.length === 0) return Buffer.alloc(0);
      
      try {
        const decryptedBuffer = Buffer.concat([decipher.update(dataToDecrypt), decipher.final()]);
        return await processWasmDecryption(decryptedBuffer, info);
      } catch (e) {
        const decipher2 = crypto.createDecipheriv('aes-256-cbc', XM_KEY, ivBuffer);
        decipher2.setAutoPadding(false);
        const decryptedBuffer = decipher2.update(dataToDecrypt);
        return await processWasmDecryption(decryptedBuffer, info);
      }
    }
  } catch (err) {
    console.error('AES decryption or WASM processing failed:', err.message);
    throw err;
  }
}

function getIvBuffer(iv) {
  if (!iv) throw new Error('IV is missing');
  
  if (Buffer.isBuffer(iv)) {
    return iv.length >= 16 ? iv.slice(0, 16) : Buffer.concat([iv, Buffer.alloc(16 - iv.length)]);
  }
  
  if (typeof iv !== 'string') {
    throw new Error(`Invalid IV type: ${typeof iv}`);
  }

  const hexIv = iv.trim().replace(/[^0-9a-fA-F]/g, '');
  if (hexIv.length >= 32) {
    return Buffer.from(hexIv.slice(0, 32), 'hex');
  }
  
  const buf = Buffer.alloc(16);
  Buffer.from(iv, 'utf8').copy(buf);
  return buf;
}

async function processWasmDecryption(decryptedBuffer, info) {
  if (!decryptedBuffer || decryptedBuffer.length === 0) {
    return Buffer.alloc(0);
  }

  let decryptedStr;
  try {
    const decoder = new TextDecoder('utf-8', { fatal: true });
    decryptedStr = decoder.decode(decryptedBuffer);
  } catch (e) {
    console.error('AES decrypted data is not valid UTF-8. First 16 bytes hex:', decryptedBuffer.slice(0, 16).toString('hex'));
    throw new Error('Invalid AES decrypted data (not UTF-8)');
  }

  const instance = await getWasmInstance();
  const { a: func_a, c: func_c, g: func_g, i: memory } = instance.exports;

  const stackPointer = func_a(-16);
  const trackId = (info.tracknumber !== undefined && info.tracknumber !== null) ? info.tracknumber.toString() : "0";
  
  const decryptedStrBuffer = Buffer.from(decryptedStr, 'utf8');
  const trackIdBuffer = Buffer.from(trackId, 'utf8');

  const deDataOffset = func_c(decryptedStrBuffer.length);
  const trackIdOffset = func_c(trackIdBuffer.length);

  if (deDataOffset === 0 || trackIdOffset === 0) {
    throw new Error('WASM memory allocation failed');
  }

  try {
    let memView = new Uint8Array(memory.buffer);
    memView.set(decryptedStrBuffer, deDataOffset);
    memView.set(trackIdBuffer, trackIdOffset);

    func_g(stackPointer, deDataOffset, decryptedStrBuffer.length, trackIdOffset, trackIdBuffer.length);

    const resultView = new DataView(memory.buffer);
    const resultPointer = resultView.getInt32(stackPointer, true);
    const resultLength = resultView.getInt32(stackPointer + 4, true);

    if (resultPointer === 0 || resultLength < 0) {
      throw new Error('WASM decryption returned invalid pointer or length');
    }

    const resultData = Buffer.from(memory.buffer, resultPointer, resultLength).toString('utf8');
    const fullBase64 = (info.encodingTechnology || '') + resultData;
    return Buffer.from(fullBase64, 'base64');
  } catch (err) {
    console.error(`WASM Decryption Core Error: ${err.message}`);
    throw err;
  }
}

module.exports = { decryptXM };
