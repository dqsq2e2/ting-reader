import type { Chapter } from '../types';

export function sortChaptersForPlayback(chapters: Chapter[]): Chapter[] {
  return [...chapters].sort((a, b) => {
    const typeDifference = Number(Boolean(a.isExtra)) - Number(Boolean(b.isExtra));
    if (typeDifference !== 0) return typeDifference;

    const indexDifference = (a.chapterIndex ?? 0) - (b.chapterIndex ?? 0);
    if (indexDifference !== 0) return indexDifference;

    return 0;
  });
}
