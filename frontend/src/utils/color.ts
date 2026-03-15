/**
 * Utility to manipulate colors
 */

/**
 * Changes the alpha (opacity) of an rgba color string or hex color
 * @param color The color string (e.g., "rgba(0, 0, 0, 0.1)" or "#000000")
 * @param newAlpha The new alpha value (e.g., 1.0)
 * @returns The modified rgba string
 */
export const setAlpha = (color: string | undefined, newAlpha: number | string): string => {
  if (!color) return '';
  
  if (color.startsWith('#')) {
    const r = parseInt(color.slice(1, 3), 16);
    const g = parseInt(color.slice(3, 5), 16);
    const b = parseInt(color.slice(5, 7), 16);
    return `rgba(${r}, ${g}, ${b}, ${newAlpha})`;
  }
  
  if (!color.startsWith('rgba')) {
    return color;
  }
  return color.replace(/[\d.]+\)$/g, `${newAlpha})`);
};

/**
 * Ensures a color is solid (alpha 1.0)
 */
export const toSolidColor = (rgba: string | undefined): string => {
  return setAlpha(rgba, 1.0);
};

export const getLuminance = (color: string): number => {
  if (!color) return 0;
  let r = 0, g = 0, b = 0;
  if (color.startsWith('#')) {
    const hex = color.replace('#', '');
    r = parseInt(hex.substring(0, 2), 16);
    g = parseInt(hex.substring(2, 4), 16);
    b = parseInt(hex.substring(4, 6), 16);
  } else if (color.startsWith('rgb')) {
    const rgb = color.match(/\d+/g);
    if (rgb) {
      r = parseInt(rgb[0]);
      g = parseInt(rgb[1]);
      b = parseInt(rgb[2]);
    }
  }
  return (0.299 * r + 0.587 * g + 0.114 * b) / 255;
};

export const isLight = (color: string | undefined): boolean => {
  if (!color) return false;
  return getLuminance(color) > 0.65;
};
