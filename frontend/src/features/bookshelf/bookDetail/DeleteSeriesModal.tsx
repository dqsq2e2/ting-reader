import React from 'react';
import { AlertTriangle, Loader2, Trash2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { Series } from '../../../core/types';

interface Props {
  series: Series;
  deleting: boolean;
  onClose: () => void;
  onConfirm: () => void;
}

const DeleteSeriesModal: React.FC<Props> = ({
  series,
  deleting,
  onClose,
  onConfirm,
}) => {
  const { t } = useTranslation();

  return (
    <div className="fixed inset-0 z-[300] flex items-center justify-center p-4">
      <div className="absolute inset-0 bg-slate-900/60 backdrop-blur-sm" onClick={onClose}></div>
      <div className="relative w-full max-w-md bg-white dark:bg-slate-900 rounded-3xl shadow-2xl overflow-hidden animate-in zoom-in-95 duration-200">
        <div className="p-8">
          <div className="w-16 h-16 bg-red-50 dark:bg-red-900/20 rounded-2xl flex items-center justify-center text-red-500 mx-auto mb-6">
            <AlertTriangle size={32} />
          </div>

          <h3 className="text-xl font-bold text-center dark:text-white mb-2">{t('bookshelf.deleteSeries')}</h3>
          <p className="text-slate-500 dark:text-slate-400 text-center mb-8">
            {series.id === 'bulk'
              ? t('bookshelf.deleteSeriesConfirmBulk', { count: series.title })
              : `${t('bookshelf.deleteSeriesConfirm')} (${series.title})`
            }
          </p>

          <div className="flex gap-4">
            <button
              onClick={onClose}
              className="flex-1 py-3 font-bold text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-xl transition-all"
            >
              {t('common.cancel')}
            </button>
            <button
              onClick={onConfirm}
              disabled={deleting}
              className="flex-1 py-3 bg-red-500 hover:bg-red-600 text-white font-bold rounded-xl shadow-lg shadow-red-500/30 flex items-center justify-center gap-2 transition-all disabled:opacity-50"
            >
              {deleting ? <Loader2 className="animate-spin" size={20} /> : <Trash2 size={20} />}
              {t('bookshelf.confirmDelete')}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};

export default DeleteSeriesModal;
