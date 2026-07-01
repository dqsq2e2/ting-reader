
import i18n from '../i18n';
import { normalizeLanguage } from '../i18n/locales';

const currentLocale = () => normalizeLanguage(i18n.resolvedLanguage || i18n.language);

export const formatDate = (dateString: string | null | undefined): string => {
  if (!dateString) return i18n.t('common.unknownTime');
  try {
    // 1. Basic cleanup
    let cleanDate = dateString.trim();
    
    // 2. Fix the ".3f" suffix issue from previous bad backend version
    // "2026-02-23T09:50:42.3fZ" -> "2026-02-23T09:50:42.300Z" (approximation)
    // Actually we just want to remove the 'f' and ensure Z is there
    if (cleanDate.includes('.3fZ')) {
       cleanDate = cleanDate.replace('.3fZ', '.000Z');
    }

    // 3. Handle SQL format "YYYY-MM-DD HH:MM:SS" -> "YYYY-MM-DDTHH:MM:SS"
    // Only replace the space between date and time (index 10)
    if (cleanDate.charAt(10) === ' ') {
      cleanDate = cleanDate.substring(0, 10) + 'T' + cleanDate.substring(11);
    }
    
    // 4. Truncate nanoseconds/microseconds to milliseconds
    // Example: .123456 -> .123
    // We look for a dot followed by more than 3 digits
    cleanDate = cleanDate.replace(/(\.\d{3})\d+/, '$1');

    let date = new Date(cleanDate);
    
    // 5. Fallback: If invalid, try appending Z (assuming UTC)
    if (isNaN(date.getTime())) {
       // If it doesn't have timezone info (Z or +HH:MM or -HH:MM)
       if (!cleanDate.endsWith('Z') && !cleanDate.includes('+')) {
          const dateWithZ = new Date(cleanDate + 'Z');
          if (!isNaN(dateWithZ.getTime())) {
            date = dateWithZ;
          }
       }
    }

    if (isNaN(date.getTime())) {
       console.warn(`解析日期失败: ${dateString} (cleaned: ${cleanDate})`);
       return i18n.t('common.unknownTime');
    }
    
    return new Intl.DateTimeFormat(currentLocale(), {
        year: 'numeric',
        month: 'long',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
        hour12: false
    }).format(date);
  } catch {
    return i18n.t('common.unknownTime');
  }
};
