import type { BookMetadata } from './book';
import type { ChapterChange } from './chapter';

export interface ScrapeDiff {
  current: BookMetadata;
  scraped: BookMetadata;
  chapter_changes: ChapterChange[];
}