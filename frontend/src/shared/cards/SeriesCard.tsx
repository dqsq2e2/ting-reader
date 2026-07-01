import React from 'react';
import type { Series } from '../../core/types';
import { Layers } from 'lucide-react';
import { Link } from 'react-router-dom';
import { getCoverUrl } from '../../core/utils/image';
import ExpandableTitle from '../widgets/ExpandableTitle';
import { useTranslation } from 'react-i18next';

interface SeriesCardProps {
  series: Series;
  onClick?: () => void;
  coverShape?: 'rect' | 'square';
}

const SeriesCard: React.FC<SeriesCardProps> = ({ series, onClick, coverShape = 'rect' }) => {
  const { t } = useTranslation();
  const content = (
    <>
      <div className={`relative ${coverShape === 'square' ? 'aspect-square' : 'aspect-[3/4]'} overflow-visible mb-1`}>
        {/* Stack effect - subtle layers behind */}
        <div className="absolute top-0 inset-x-2 bottom-2 bg-slate-200 dark:bg-slate-700 rounded-md transform -translate-y-1.5 translate-x-1 rotate-1 shadow-sm"></div>
        <div className="absolute top-0 inset-x-1 bottom-1 bg-slate-300 dark:bg-slate-600 rounded-md transform -translate-y-0.5 -translate-x-0.5 -rotate-1 shadow-sm"></div>
        
        {/* Main cover */}
        <div className="relative h-full w-full rounded-md overflow-hidden shadow-md bg-white dark:bg-slate-800">
            <img 
              src={getCoverUrl(series.cover_url, series.library_id)}
              alt={series.title}
              loading="lazy"
              referrerPolicy="no-referrer"
              className="w-full h-full object-cover transition-transform duration-300 group-hover:scale-105"
              onError={(e) => {
                (e.target as HTMLImageElement).src = 'https://placehold.co/300x400?text=Series';
              }}
            />
            
            {/* Overlay on hover */}
            <div className="absolute inset-0 bg-black/40 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center">
              <div className="w-10 h-10 rounded-full bg-primary-600 text-white flex items-center justify-center shadow-lg transform translate-y-4 group-hover:translate-y-0 transition-transform">
                <Layers size={20} />
              </div>
            </div>
            
            {/* Series badge */}
            <div className="absolute top-2 right-2 px-1.5 py-0.5 bg-black/60 backdrop-blur-sm rounded text-[10px] text-white font-medium flex items-center gap-1">
              <Layers size={10} />
              <span>{t('shared.series')}</span>
            </div>

            {/* Book count badge */}
            <div className="absolute bottom-2 right-2 px-1.5 py-0.5 bg-primary-600/90 backdrop-blur-sm rounded text-[10px] text-white font-medium shadow-sm">
               {t('shared.seriesBookCount', { count: series.books?.length || 0 })}
            </div>
        </div>
      </div>
      
      <div className="mt-2 min-w-0">
        <ExpandableTitle 
          title={series.title} 
          className="font-bold text-sm text-slate-900 dark:text-white group-hover:text-primary-600 transition-colors leading-tight" 
          maxLines={1}
        />
        <div className="mt-1 flex flex-col gap-0.5">
          <div className="flex items-center gap-1.5 text-xs text-slate-500 dark:text-slate-400">
            <span className="line-clamp-1">{series.author || t('shared.unknownAuthor')}</span>
          </div>
        </div>
      </div>
    </>
  );

  if (onClick) {
    return (
      <div 
        onClick={onClick}
        className="group flex flex-col cursor-pointer relative"
      >
        {content}
      </div>
    );
  }

  return (
    <Link 
      to={`/series/${series.id}`}
      className="group flex flex-col relative"
    >
      {content}
    </Link>
  );
};

export default SeriesCard;
