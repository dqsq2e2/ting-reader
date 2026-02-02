/**
 * Utility to manipulate colors
 */

/**
 * Changes the alpha (opacity) of an rgba color string
 * @param rgba The rgba string (e.g., "rgba(0, 0, 0, 0.1)")
 * @param newAlpha The new alpha value (e.g., 1.0)
 * @returns The modified rgba string
 */
export const setAlpha = (rgba: string | undefined, newAlpha: number | string): string => {
  if (!rgba) return '';
  if (!rgba.startsWith('rgba')) {
    // If it's hex, we might need a different approach, but the project seems to use rgba for dynamic colors
    return rgba;
  }
  return rgba.replace(/[\d.]+\)$/g, `${newAlpha})`);
};

/**
 * Ensures a color is solid (alpha 1.0)
 */
export const toSolidColor = (rgba: string | undefined): string => {
  return setAlpha(rgba, 1.0);
};
