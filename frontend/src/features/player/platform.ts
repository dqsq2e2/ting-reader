// Player-specific platform helpers (browser / path detection)

export const isAppleMobileBrowser = (): boolean => {
  if (typeof navigator === 'undefined') return false;
  const ua = navigator.userAgent || '';
  const isiPhoneOrIPad = /iPad|iPhone|iPod/.test(ua);
  const isModernIPad = navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1;
  return isiPhoneOrIPad || isModernIPad;
};

export const isStrmPath = (path?: string): boolean =>
  path?.toLowerCase().split('?')[0].endsWith('.strm') ?? false;
