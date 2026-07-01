import React from 'react';
import { useTranslation } from 'react-i18next';
import { Check, SkipBack, SkipForward, X } from 'lucide-react';

interface Props {
  editSkipIntro: number;
  editSkipOutro: number;
  onChangeSkipIntro: (value: number) => void;
  onChangeSkipOutro: (value: number) => void;
  onClose: () => void;
  onSave: () => void;
}

const PlayerSettingsModal: React.FC<Props> = ({
  editSkipIntro,
  editSkipOutro,
  onChangeSkipIntro,
  onChangeSkipOutro,
  onClose,
  onSave,
}) => {
  const { t } = useTranslation();

  return (
    <div className="fixed inset-0 z-[300] flex items-center justify-center p-4">
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" onClick={onClose}></div>
      <div className="relative w-full max-w-sm bg-white dark:bg-slate-900 rounded-[32px] shadow-2xl overflow-hidden animate-in zoom-in-95 duration-200">
        <div className="p-6 sm:p-8">
          <div className="flex items-center justify-between mb-6">
            <h3 className="text-xl font-bold text-slate-900 dark:text-white">{t('player.settings')}</h3>
            <button onClick={onClose} className="p-2 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-full">
              <X size={20} className="text-slate-400" />
            </button>
          </div>

          <div className="space-y-6">
            <div className="space-y-2">
              <label className="text-xs font-bold text-slate-500 uppercase tracking-wider flex items-center gap-2">
                <SkipBack size={14} />
                {t('player.skipIntro')}
              </label>
              <input
                type="number"
                value={editSkipIntro}
                onChange={e => onChangeSkipIntro(parseInt(e.target.value) || 0)}
                className="w-full px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-2xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                placeholder={t('player.secondsExample', { seconds: 30 })}
              />
            </div>

            <div className="space-y-2">
              <label className="text-xs font-bold text-slate-500 uppercase tracking-wider flex items-center gap-2">
                <SkipForward size={14} />
                {t('player.skipOutro')}
              </label>
              <input
                type="number"
                value={editSkipOutro}
                onChange={e => onChangeSkipOutro(parseInt(e.target.value) || 0)}
                className="w-full px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-2xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white"
                placeholder={t('player.secondsExample', { seconds: 15 })}
              />
            </div>
          </div>

          <div className="mt-8 flex gap-3">
            <button
              onClick={onClose}
              className="flex-1 py-3.5 font-bold text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-2xl transition-all"
            >
              {t('common.cancel')}
            </button>
            <button
              onClick={onSave}
              className="flex-1 py-3.5 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-2xl shadow-lg shadow-primary-500/30 flex items-center justify-center gap-2 transition-all"
            >
              <Check size={20} />
              {t('mine.save')}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};

export default PlayerSettingsModal;
