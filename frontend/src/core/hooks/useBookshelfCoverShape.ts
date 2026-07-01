import { useEffect, useState } from 'react';
import apiClient from '../api/client';

export type CoverShape = 'rect' | 'square';

const COVER_SHAPE_EVENT = 'ting-reader-bookshelf-cover-shape-change';
let cachedCoverShape: CoverShape = 'rect';

export const normalizeCoverShape = (value: unknown): CoverShape => (
  value === 'square' ? 'square' : 'rect'
);

export const getCoverAspectClass = (coverShape: CoverShape) => (
  coverShape === 'square' ? 'aspect-square' : 'aspect-[3/4]'
);

export const publishBookshelfCoverShape = (coverShape: CoverShape) => {
  cachedCoverShape = coverShape;
  window.dispatchEvent(new CustomEvent(COVER_SHAPE_EVENT, { detail: coverShape }));
};

export const useBookshelfCoverShape = (enabled = true) => {
  const [coverShape, setCoverShape] = useState<CoverShape>(cachedCoverShape);

  useEffect(() => {
    let cancelled = false;
    if (!enabled) return;

    apiClient.get('/api/settings')
      .then(res => {
        const nextCoverShape = normalizeCoverShape(res.data.settings_json?.bookshelf_cover_shape);
        cachedCoverShape = nextCoverShape;
        if (!cancelled) setCoverShape(nextCoverShape);
      })
      .catch(err => console.error('加载书架封面比例失败', err));

    const handleCoverShapeChange = (event: Event) => {
      const nextCoverShape = normalizeCoverShape((event as CustomEvent).detail);
      cachedCoverShape = nextCoverShape;
      setCoverShape(nextCoverShape);
    };

    window.addEventListener(COVER_SHAPE_EVENT, handleCoverShapeChange);
    return () => {
      cancelled = true;
      window.removeEventListener(COVER_SHAPE_EVENT, handleCoverShapeChange);
    };
  }, [enabled]);

  return coverShape;
};
