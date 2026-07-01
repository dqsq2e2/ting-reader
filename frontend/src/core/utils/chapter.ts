import type { Chapter } from '../types';

export function sortChaptersForPlayback(chapters: Chapter[]): Chapter[] {
  return [...chapters].sort((a, b) => {
    const typeDifference = Number(Boolean(a.is_extra)) - Number(Boolean(b.is_extra));
    if (typeDifference !== 0) return typeDifference;

    const indexDifference = (a.chapter_index ?? 0) - (b.chapter_index ?? 0);
    if (indexDifference !== 0) return indexDifference;

    return 0;
  });
}
