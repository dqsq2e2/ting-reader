export type HomeLayoutSettings = {
  showHero: boolean;
  showStats: boolean;
  showRecommended: boolean;
  showRecent: boolean;
  showRecentlyAdded: boolean;
  showCollections: boolean;
};

export const DEFAULT_HOME_LAYOUT: HomeLayoutSettings = {
  showHero: true,
  showStats: true,
  showRecommended: true,
  showRecent: true,
  showRecentlyAdded: true,
  showCollections: true,
};

export const normalizeHomeLayout = (value: unknown): HomeLayoutSettings => {
  const source = typeof value === 'object' && value !== null
    ? value as Partial<Record<keyof HomeLayoutSettings, unknown>>
    : {};

  return {
    showHero: typeof source.showHero === 'boolean' ? source.showHero : DEFAULT_HOME_LAYOUT.showHero,
    showStats: typeof source.showStats === 'boolean' ? source.showStats : DEFAULT_HOME_LAYOUT.showStats,
    showRecommended: typeof source.showRecommended === 'boolean' ? source.showRecommended : DEFAULT_HOME_LAYOUT.showRecommended,
    showRecent: typeof source.showRecent === 'boolean' ? source.showRecent : DEFAULT_HOME_LAYOUT.showRecent,
    showRecentlyAdded: typeof source.showRecentlyAdded === 'boolean' ? source.showRecentlyAdded : DEFAULT_HOME_LAYOUT.showRecentlyAdded,
    showCollections: typeof source.showCollections === 'boolean' ? source.showCollections : DEFAULT_HOME_LAYOUT.showCollections,
  };
};
