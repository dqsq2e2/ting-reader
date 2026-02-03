const axios = require('axios');
const crypto = require('crypto');
const cheerio = require('cheerio');

// Specific headers and cookies from Abs-Ximalaya-1.1.1
const detailApiHeaders = {
    'User-Agent': 'Mozilla/5.0 (Linux; Android 9; SM-S9110 Build/PQ3A.190605.09291615; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/92.0.4515.131 Mobile Safari/537.36 iting(main)/9.3.96/android_1 xmly(main)/9.3.96/android_1 kdtUnion_iting/9.3.96',
    'Accept': 'application/json, text/plain, */*',
    'x-requested-with': 'XMLHttpRequest',
    'referer': 'https://mobile.ximalaya.com/',
    'accept-language': 'zh-CN,zh;q=0.9,en-US;q=0.8,en;q=0.7',
};

const detailApiCookies = '1&_device=android&28b5647f-40d9-3cb6-802a-54905eccc23d&9.3.96; 1&_token=575426552&C29CC6B0140C8E529835C3060AD1FE97FBF87FFBF4DB5BFB15C60DECE3899A36EDA3462173EE229Mbf90403ACAFF0C4_; channel=and-f5; impl=com.ximalaya.ting.android; osversion=28; fp=009517657x2222322v64v050210000k120211200200000001103611000040; device_model=SM-S9110; XUM=CAAn8P8v; c-oper=%E4%B8%AD%E5%9B%BD%E7%A7%BB%E5%8A%A8; net-mode=WIFI; res=1600%2C900; AID=Yjg2YWIyZTRmNzYyN2FjNA==; manufacturer=samsung; umid=ai0fc70f150ccc444005b5c665d7ee7861; xm_grade=0; specialModeStatus=0; yzChannel=and-f5; _xmLog=h5&9550461b-17b4-4dcc-ab09-8609fcda6c02&2.4.24; xm-page-viewid=album-detail-intro';

// 移除购买须知及其后的所有内容 (Synced from Abs-Ximalaya)
function removePurchaseNotes(htmlContent) {
    if (!htmlContent) return '';
    const $ = cheerio.load(htmlContent);

    let found = false;
    $('p, div, span').each(function() {
        const $el = $(this);
        const text = $el.text();
        if (!found && /(购买须知|温馨提示|版权声明|加听友群|联系方式|侵权|主播联系|本节目|本作品)/.test(text)) {
            found = true;
        }
        if (found) {
            $el.remove();
        }
    });

    return $.html();
}

// 移除结尾多余的 <br> 标签 (Synced from Abs-Ximalaya)
function removeTrailingBr(html) {
  if (!html) return '';
  const $ = cheerio.load(html);

  // 删除 body 末尾所有连续的 <br> 标签（无论是否被包裹）
  $('body').find('br').each((_, el) => {
    // 如果这个 <br> 之后没有非空内容，就删掉
    const next = $(el).nextAll().text().trim();
    if (!next && $(el).parent().nextAll().text().trim() === '') {
      $(el).remove();
    }
  });

  // 清空仅包含 <br> 的标签（例如 <span><br><br></span>）
  $('body').find('*').each((_, el) => {
    const content = $(el).html()?.trim();
    if (content && /^(\s*<br\s*\/?>\s*)+$/.test(content)) {
      $(el).empty();
    }
  });

  return $.html();
}

/**
 * Generate xm-sign for Ximalaya API
 */
async function getXmSign() {
  try {
    const serverTimeRes = await axios.get('https://www.ximalaya.com/revision/time', { timeout: 3000 });
    const serverTime = serverTimeRes.data;
    const now = Date.now();
    const randomNum = Math.floor(Math.random() * 100);
    const md5Hash = crypto.createHash('md5').update(`himalaya-${serverTime}`).digest('hex');
    return `${md5Hash}(${randomNum})${serverTime}(${randomNum})${now}`;
  } catch (err) {
    return null;
  }
}

/**
 * Ximalaya scraper implementation
 */
async function scrapeXimalaya(keyword) {
  try {
    // Clean up keyword for better search
    let cleanKeyword = keyword
      .replace(/\.(mp3|m4a|wav|flac|m4b|xm)$/i, '')
      .replace(/第\s*\d+\s*[集|回|章|话].*/g, '')
      .replace(/[（\(\[\{【].*?[）\)\]\}】]/g, '')
      .replace(/[\-_]ZmAudio/gi, '')
      .replace(/[\-_]xm/gi, '')
      .replace(/[^\u4e00-\u9fa5a-zA-Z0-9]/g, ' ') // Keep only Chinese characters, letters, and numbers
      .replace(/\s+/g, ' ')
      .trim();
    
    if (cleanKeyword.length < 2) {
      cleanKeyword = keyword.split(/[第\s\-_]/)[0].trim();
    }
    
    // Remove common prefixes/suffixes
    cleanKeyword = cleanKeyword
      .replace(/^(有声书|有声小说|听书|小说|精品)[:：\s]*/, '')
      .replace(/(有声书|有声小说|听书|小说|精品|全集|全本|完结|播讲|演播)$/, '')
      .trim();
    
    if (!cleanKeyword) return null;

    console.log(`Scraping Ximalaya for: ${cleanKeyword}`);
    
    const xmSign = await getXmSign();
    const headers = {
      'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36',
      'Accept': 'application/json, text/plain, */*',
      'Accept-Language': 'zh-CN,zh;q=0.9,en;q=0.8',
      'xm-sign': xmSign || ''
    };

    // 1. Try primary search endpoint from Abs-Ximalaya
    // Use the revision/search endpoint which returns result.response.docs
    let searchUrl = `https://www.ximalaya.com/revision/search?core=album&kw=${encodeURIComponent(cleanKeyword)}&page=1&spellchecker=true&rows=10&condition=relation&device=web`;
    
    console.log(`Searching Ximalaya (Primary): ${searchUrl}`);

    let searchResponse = await axios.get(searchUrl, { 
      headers: { ...headers, 'Referer': `https://www.ximalaya.com/so/${encodeURIComponent(cleanKeyword)}/` }, 
      timeout: 10000 
    }).catch(() => null);
    
    let searchData = searchResponse?.data;
    let album = null;

    if (searchData?.ret === 200 && searchData.data?.result?.response?.docs?.length > 0) {
      album = searchData.data.result.response.docs[0];
      console.log(`Found album via Primary search: ${album.title} (ID: ${album.id})`);
    } else {
      // Try a different search endpoint if primary one fails or returns no results
      console.log('Primary search failed or flagged. Trying Main search API...');
      const mainSearchUrl = `https://www.ximalaya.com/revision/search/main?core=all&kw=${encodeURIComponent(cleanKeyword)}&p=1&rows=10&condition=relation&scope=all`;
      const mainSearchRes = await axios.get(mainSearchUrl, { headers, timeout: 5000 }).catch(() => null);
      
      if (mainSearchRes?.data?.data?.album?.docs?.length > 0) {
        album = mainSearchRes.data.data.album.docs[0];
        console.log(`Found album via Main search: ${album.title} (ID: ${album.id})`);
      } else if (mainSearchRes?.data?.data?.all?.docs?.length > 0) {
        // Look for album in "all" results
        album = mainSearchRes.data.data.all.docs.find(d => d.core_type === 'album');
        if (album) console.log(`Found album in "all" results: ${album.title} (ID: ${album.id})`);
      }
      
      if (!album) {
        // 2. Try Suggest API as fallback
        console.log('Main search failed. Trying Suggest API...');
        const suggestUrl = `https://www.ximalaya.com/revision/suggest?kw=${encodeURIComponent(cleanKeyword)}`;
        const suggestRes = await axios.get(suggestUrl, { headers, timeout: 5000 }).catch(() => null);
        
        if (suggestRes?.data?.data?.album?.length > 0) {
          const firstSuggest = suggestRes.data.data.album[0];
          console.log(`Found ID via Suggest API: ${firstSuggest.id} (${firstSuggest.title})`);
          album = { id: firstSuggest.id, title: firstSuggest.title };
        }
      }

      if (!album && cleanKeyword !== keyword) {
        // 3. Try original keyword as last resort
        console.log('Trying original keyword search...');
        const originalUrl = `https://www.ximalaya.com/revision/search?core=album&kw=${encodeURIComponent(keyword.substring(0, 20))}&page=1&rows=5&device=web`;
        const originalRes = await axios.get(originalUrl, { headers, timeout: 5000 }).catch(() => null);
        if (originalRes?.data?.data?.result?.response?.docs?.length > 0) {
           album = originalRes.data.data.result.response.docs[0];
        }
      }
    }

    if (!album) return null;

    // Use the detail API from Abs-Ximalaya for full info
    return processAlbum(album, headers);
  } catch (err) {
    console.error('Scraping error:', err.message);
    return null;
  }
}

/**
 * Scrape detailed info from YPShuo API
 */
async function scrapeYPShuo(title) {
  if (!title || title === 'Unknown Book' || title === '未知书籍') return null;
  
  // Clean up title specifically for YPShuo search
  let cleanTitle = title
    .replace(/\.(mp3|m4a|wav|flac|m4b|xm)$/i, '')
    .replace(/第\s*\d+\s*[集|回|章|话].*/g, '')
    .replace(/[（\(\[\{【].*?[）\)\]\}】]/g, '')
    .split(/[丨|｜\-\/\:\：|｜\s&]/)[0] // Split by more separators and take the first part
    .replace(/^(有声书|有声小说|听书|小说|精品)[:：\s]*/, '')
    .replace(/(有声书|有声小说|听书|小说|精品|全集|全本|完结|播讲|演播)$/, '')
    .trim();

  if (cleanTitle.length < 2) return null;

  try {
    console.log(`YPShuo search for: ${cleanTitle}`);
    const ypUrl = `https://m.ypshuo.com/api/novel/search?keyword=${encodeURIComponent(cleanTitle)}&searchType=1&page=1`;
    const ypRes = await axios.get(ypUrl, {
      headers: {
        'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36',
        'Accept': 'application/json',
      },
      timeout: 10000
    }).catch(() => null);

    if (ypRes?.data?.code === '00' && ypRes.data.data?.data?.length > 0) {
      const books = ypRes.data.data.data;
      const exactMatch = books.find(b => b.novel_name === cleanTitle);
      const bestMatch = exactMatch || books[0];
      
      return {
        author: bestMatch.author_name?.trim(),
        description: bestMatch.synopsis?.trim(),
        cover_url: ensureFullUrl(bestMatch.novel_img || ''),
        tags: bestMatch.tags?.trim()
      };
    }
  } catch (err) {
    console.warn('YPShuo scraping error:', err.message);
  }
  return null;
}

/**
  * Fallback: search for web novel author by title using Baidu Baike and other sources
  */
 async function scrapeWebNovelAuthor(title) {
   const ypData = await scrapeYPShuo(title);
   if (ypData && ypData.author) return ypData.author;
   
   if (!title || title === 'Unknown Book' || title === '未知书籍') return null;
   
   try {
    // 1. Thoroughly clean the title by splitting with more separators
    // Covers: | (pipe), 丨 (full-width pipe), ｜ (full-width pipe), -, /, :, ：
    const baseTitle = title.split(/[丨|｜\-\/\:\：]/)[0].trim();
    
    // 2. Remove any remaining bracketed info
    const cleanTitle = baseTitle
      .replace(/[（\(\[\{【].*?[）\)\]\}】]/g, '')
      .trim();

    if (cleanTitle.length < 2) return null;

     console.log(`Fallback: Searching Baidu Baike for author of: ${cleanTitle}`);
     
     // 2. Try Baidu Baike (very stable for books)
     const baikeUrl = `https://baike.baidu.com/item/${encodeURIComponent(cleanTitle)}`;
     const baikeRes = await axios.get(baikeUrl, {
       headers: {
         'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36',
         'Accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8',
         'Accept-Language': 'zh-CN,zh;q=0.9',
       },
       timeout: 10000
     }).catch(() => null);
 
     if (baikeRes?.data) {
       const html = baikeRes.data;
       
       // Pattern 1: Basic info items (modern structure)
       const modernAuthorMatch = html.match(/<dt[^>]*>作(?:&nbsp;|\s)*者<\/dt>\s*<dd[^>]*>\s*(?:<a[^>]*>)?([^<]+)(?:<\/a>)?\s*<\/dd>/);
       if (modernAuthorMatch && modernAuthorMatch[1]) {
         return modernAuthorMatch[1].trim();
       }

       // Pattern 2: Title contains author: "诡秘之主（爱潜水的乌贼创作的长篇小说）"
       const titleMatch = html.match(/<title>.*?（(.*?)创作/);
       if (titleMatch && titleMatch[1]) {
         return titleMatch[1].trim();
       }
 
       // Pattern 3: Basic info items (legacy structure)
       const basicInfoMatch = html.match(/<dt class="basicInfo-item name">作(?:&nbsp;|\s)*者<\/dt>\s*<dd class="basicInfo-item value">\s*(?:<a[^>]*>)?([^<]+)(?:<\/a>)?\s*<\/dd>/);
       if (basicInfoMatch && basicInfoMatch[1]) {
         return basicInfoMatch[1].trim();
       }
 
       // Pattern 4: Meta description
       const descMatch = html.match(/meta name="description" content=".*?是.*?作家\s*([^创作]+?)\s*创作/);
       if (descMatch && descMatch[1]) {
         return descMatch[1].trim();
       }
     }
 
     console.log(`Fallback: Searching Qidian for author of: ${cleanTitle}`);
     
     // 2. Try Qidian Search (Legacy fallback)
     const qidianUrl = `https://www.qidian.com/search?kw=${encodeURIComponent(cleanTitle)}`;
     const qidianRes = await axios.get(qidianUrl, {
       headers: {
         'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36',
         'Referer': 'https://www.qidian.com/',
         'Accept-Language': 'zh-CN,zh;q=0.9',
       },
       timeout: 10000
     }).catch(() => null);
 
     if (qidianRes?.data) {
       const html = qidianRes.data;
       // Qidian modern structure
       const authorMatch = html.match(/data-eid="qd_S04"[^>]*>([^<]+)<\/a>/) || 
                           html.match(/class="author"[^>]*>.*?<a[^>]*>([^<]+)<\/a>/) ||
                           html.match(/<p class="author">.*?<a[^>]*>([^<]+)<\/a>/);
       
       if (authorMatch && authorMatch[1]) {
        return authorMatch[1].trim();
      }
    }
  } catch (err) {
    console.warn('Fallback scraping error:', err.message);
  }
  return null;
}

async function enrichWithYPShuo(result) {
  if (!result || !result.title) return result;
  
  const ypData = await scrapeYPShuo(result.title);
  if (ypData) {
    if (ypData.description) result.description = ypData.description;
    if (!result.cover_url && ypData.cover_url) result.cover_url = ypData.cover_url;
    if (ypData.tags) result.tags = ypData.tags;
    if ((!result.author || result.author === '未知作者') && ypData.author) result.author = ypData.author;
  }
  return result;
}

async function processAlbum(album, headers = {}) {
  const albumId = album.id || album.albumId;
  console.log(`Getting full details for album ID: ${albumId}`);

  try {
    // Try the mobile detail API used in Abs-Ximalaya
    const detailUrl = 'https://mobile.ximalaya.com/mobile-album/album/plant/detail';
    const detailResponse = await axios.get(detailUrl, {
      params: {
        albumId: albumId,
        identity: 'podcast',
        supportWebp: 'true',
      },
      headers: {
        ...detailApiHeaders,
        'Cookie': detailApiCookies
      },
      timeout: 10000
    }).catch(() => null);

    const detailData = detailResponse?.data?.data;
    
    if (detailData) {
      console.log(`Successfully got details for ${albumId} from mobile API`);
      const mainInfo = detailData.albumInfo || detailData.mainInfo || detailData.album || {};
      const richIntro = detailData.intro?.richIntro || detailData.intro?.intro || detailData.albumInfo?.intro || '';
      
      // Clean up rich intro using synced logic from Abs-Ximalaya
      let description = removePurchaseNotes(richIntro);
      description = removeTrailingBr(description);
      
      // Final text-only conversion and cleanup
      description = description
        .replace(/<br\s*\/?>/gi, '\n')
        .replace(/<p[^>]*>/gi, '\n')
        .replace(/<[^>]+>/g, ' ')
        .split('\n')
        .map(line => line.trim())
        .filter(line => {
          // Keep the line if it's not empty and doesn't look like an ad
          return line.length > 0 && 
                 !/^([【\[\(]|[】\]\)])$/.test(line) &&
                 !line.includes('xmcdn.com') &&
                 !line.includes('http');
        })
        .join('\n')
        .trim();

      // Remove redundant leading headers
      description = description.replace(/^(内容简介|简介|作品简介|本书简介|故事简介|内容提要|内容摘要)[:：\s]*/, '').trim();

      // Try to extract author and narrator from description
      let author = '';
      let narrator = mainInfo.nickname || mainInfo.anchorName || album.nickname || '';
      
      // More comprehensive regex for author and narrator
      const authorMatch = description.match(/(?:作者|原著|编剧|作者\s*[:：\s]*|原著\s*[:：\s]*)\s*([^\n\s|【〔(（]+)/) ||
                          description.match(/([^\n\s|【〔(（]+)\s*(?:著|编著)/);
      if (authorMatch) {
        author = authorMatch[1].trim();
      }
      
      const narratorMatch = description.match(/(?:演播|播音|主播|播讲|后期|制作)[:：\s]*([^\n\s|【〔(（]+)/);
      if (narratorMatch) {
        const foundNarrator = narratorMatch[1].trim();
        if (foundNarrator.length > 1 && !foundNarrator.includes('xmcdn') && !foundNarrator.includes('http')) {
          narrator = foundNarrator;
        }
      }

      // If we couldn't find an author in description, try title or tags
      if (!author) {

        const titleAuthorMatch = (mainInfo.title || album.title || '').match(/[（\(\[\{【](.*?)著[）\)\]\}】]/);
        if (titleAuthorMatch) {
          author = titleAuthorMatch[1].trim();
        } else {
          // If still unknown, try external scraping
          console.log(`Still unknown author, trying external scraping for: ${mainInfo.title || album.title}`);
          const scrapedAuthor = await scrapeWebNovelAuthor(mainInfo.title || album.title);
          if (scrapedAuthor) {
            author = scrapedAuthor;
          } else {
            author = 'Unknown Author';
          }
        }
      }

      // If author is still Unknown or just the narrator's name, try one last check in description for common author markers
      if (!author || author === 'Unknown Author' || author === narrator) {
        const fallbackAuthorMatch = description.match(/(?:作者|原著|编剧)[:：\s]*([^\s\n\r\t,，。！!|]+)/);
        if (fallbackAuthorMatch && fallbackAuthorMatch[1]) {
           author = fallbackAuthorMatch[1].trim();
        }
      }

      // Clean up title: remove narrator and tags from the book name
      let finalTitle = (mainInfo.title || album.title || '')
        .replace(/^(内容简介|简介|作品简介|本书简介|故事简介)[:：\s]*/, '')
        .split(/[丨|｜]/)[0] // Take the first part before separator
        .replace(/[（\(\[\{【].*?[）\)\]\}】]/g, '') // Remove parenthetical content
        .trim();

      // Fallback for author if still unknown
      if (!author || author === '未知作者') {
        const fallbackAuthor = await scrapeWebNovelAuthor(finalTitle);
        if (fallbackAuthor) author = fallbackAuthor;
      }

      return enrichWithYPShuo({
        title: finalTitle,
        author: author || '未知作者',
        narrator: narrator || '未知演播',
        description: description,
        cover_url: ensureFullUrl(mainInfo.coverPath || mainInfo.cover || mainInfo.albumCoverPath || album.cover_path || album.coverPath || '')
      });
    }

    // Fallback to the web detail API if mobile one fails
    console.log('Mobile detail API failed, trying web detail API...');
    const webDetailUrl = `https://www.ximalaya.com/revision/album/v1/getDetail?albumId=${albumId}`;
    const webDetailRes = await axios.get(webDetailUrl, {
      headers: {
        ...headers,
        'Referer': `https://www.ximalaya.com/album/${albumId}`
      },
      timeout: 10000
    }).catch(() => null);

    const webData = webDetailRes?.data?.data;
    const mainInfo = webData?.mainInfo || webData?.albumInfo;
    
    if (mainInfo) {
      let description = mainInfo.detailRichIntro || mainInfo.shortIntro || mainInfo.intro || '';
      description = description
        .replace(/<[^>]+>/g, ' ')
        .split(/[\s\n\r\t]+/)
        .join(' ')
        .trim();

      let narrator = mainInfo.nickname || mainInfo.anchorName || album.nickname || '';
      let author = '';

      const authorMatch = description.match(/(?:作者|原著|编剧|作者\s*[:：\s]*|原著\s*[:：\s]*)\s*([^\n\s|【〔(（]+)/) ||
                          description.match(/([^\n\s|【〔(（]+)\s*(?:著|编著)/);
      if (authorMatch) author = authorMatch[1].trim();
      
      const narratorMatch = description.match(/(?:演播|播音|主播|播讲|后期|制作)[:：\s]*([^\n\s|【〔(（]+)/);
      if (narratorMatch) {
        const foundNarrator = narratorMatch[1].trim();
        if (foundNarrator.length > 1 && !foundNarrator.includes('xmcdn') && !foundNarrator.includes('http')) {
          narrator = foundNarrator;
        }
      }

      const finalTitle = mainInfo.albumTitle || mainInfo.title || album.title;
      
      // Fallback for author if still unknown
      if (!author || author === '未知作者') {
        const cleanTitle = finalTitle.replace(/[（\(\[\{【].*?[）\)\]\}】]/g, '').trim();
        const fallbackAuthor = await scrapeWebNovelAuthor(cleanTitle);
        if (fallbackAuthor) author = fallbackAuthor;
      }

      return enrichWithYPShuo({
        title: finalTitle,
        author: author || '未知作者',
        narrator: narrator || '未知演播',
        description: description,
        cover_url: ensureFullUrl(mainInfo.cover || mainInfo.albumCoverPath || mainInfo.coverPath || '').replace(/!.*$/, '')
      });
    }

    // Last resort: use what we have from search
    const lastNarrator = album.nickname || '';
    const lastTitle = (album.title || '').replace(/<[^>]+>/g, '');
    let lastAuthor = '未知作者';
    
    const cleanTitle = lastTitle.replace(/[（\(\[\{【].*?[）\)\]\}】]/g, '').trim();
    const fallbackAuthor = await scrapeWebNovelAuthor(cleanTitle);
    if (fallbackAuthor) lastAuthor = fallbackAuthor;

    return enrichWithYPShuo({
      title: lastTitle,
      author: lastAuthor,
      narrator: lastNarrator || '未知演播',
      description: (album.intro || '').replace(/<[^>]+>/g, ' ').trim(),
      cover_url: ensureFullUrl(album.cover_path || album.coverPath || '').replace(/!.*$/, '')
    });
  } catch (e) {
    console.error('Error in processAlbum:', e.message);
    return null;
  }
}

function ensureFullUrl(path) {
  if (!path) return '';
  if (path.startsWith('http')) return path;
  if (path.startsWith('//')) return 'https:' + path;
  return 'https://' + path.replace(/^\/+/, '');
}

module.exports = { scrapeXimalaya };
