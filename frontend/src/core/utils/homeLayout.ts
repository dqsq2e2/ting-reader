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
    ? value as Record<string, unknown>
    : {};

  return {
    showHero: typeof source.show_hero === 'boolean' ? source.show_hero : DEFAULT_HOME_LAYOUT.showHero,
    showStats: typeof source.show_stats === 'boolean' ? source.show_stats : DEFAULT_HOME_LAYOUT.showStats,
    showRecommended: typeof source.show_recommended === 'boolean' ? source.show_recommended : DEFAULT_HOME_LAYOUT.showRecommended,
    showRecent: typeof source.show_recent === 'boolean' ? source.show_recent : DEFAULT_HOME_LAYOUT.showRecent,
    showRecentlyAdded: typeof source.show_recently_added === 'boolean' ? source.show_recently_added : DEFAULT_HOME_LAYOUT.showRecentlyAdded,
    showCollections: typeof source.show_collections === 'boolean' ? source.show_collections : DEFAULT_HOME_LAYOUT.showCollections,
  };
};

export const serializeHomeLayout = (value: HomeLayoutSettings) => ({
  show_hero: value.showHero,
  show_stats: value.showStats,
  show_recommended: value.showRecommended,
  show_recent: value.showRecent,
  show_recently_added: value.showRecentlyAdded,
  show_collections: value.showCollections,
});
