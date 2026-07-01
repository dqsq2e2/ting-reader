import React, { useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { HelpCircle } from 'lucide-react';
import { useTranslation } from 'react-i18next';

const TOOLTIP_WIDTH = 288;
const VIEWPORT_PADDING = 12;

const HelpHint: React.FC<{ text: string }> = ({ text }) => {
  const { t } = useTranslation();
  const [visible, setVisible] = useState(false);
  const [position, setPosition] = useState<{ left: number; top: number; width: number; placement: 'top' | 'bottom' } | null>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);

  const updatePosition = useCallback(() => {
    const button = buttonRef.current;
    if (!button) return;

    const rect = button.getBoundingClientRect();
    const width = Math.min(TOOLTIP_WIDTH, window.innerWidth - VIEWPORT_PADDING * 2);
    const halfWidth = width / 2;
    const maxLeft = window.innerWidth - VIEWPORT_PADDING - halfWidth;
    const minLeft = VIEWPORT_PADDING + halfWidth;
    const centeredLeft = rect.left + rect.width / 2;
    const left = maxLeft >= minLeft
      ? Math.min(Math.max(centeredLeft, minLeft), maxLeft)
      : window.innerWidth / 2;
    const placeTop = rect.bottom + 160 > window.innerHeight && rect.top > 120;

    setPosition({
      left,
      top: placeTop ? rect.top - 8 : rect.bottom + 8,
      width,
      placement: placeTop ? 'top' : 'bottom',
    });
  }, []);

  const showHint = () => {
    updatePosition();
    setVisible(true);
  };

  useEffect(() => {
    if (!visible) return;

    let frame = 0;
    const scheduleUpdate = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(updatePosition);
    };

    scheduleUpdate();
    window.addEventListener('resize', scheduleUpdate);
    window.addEventListener('scroll', scheduleUpdate, true);

    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener('resize', scheduleUpdate);
      window.removeEventListener('scroll', scheduleUpdate, true);
    };
  }, [visible, updatePosition]);

  return (
    <span className="inline-flex align-middle">
      <button
        ref={buttonRef}
        type="button"
        aria-label={t('helpHint.ariaLabel')}
        aria-expanded={visible}
        onMouseEnter={showHint}
        onMouseLeave={() => setVisible(false)}
        onFocus={showHint}
        onBlur={() => setVisible(false)}
        onClick={(event) => {
          event.preventDefault();
          event.stopPropagation();
          showHint();
        }}
        className="inline-flex h-5 w-5 items-center justify-center rounded-full text-slate-400 transition-colors hover:bg-slate-100 hover:text-primary-600 focus:outline-none focus:ring-2 focus:ring-primary-500/30 dark:hover:bg-slate-800"
      >
        <HelpCircle size={14} />
      </button>
      {visible && position
        ? createPortal(
            <span
              role="tooltip"
              style={{
                left: position.left,
                top: position.top,
                width: position.width,
                transform: position.placement === 'top' ? 'translate(-50%, -100%)' : 'translateX(-50%)',
              }}
              className="pointer-events-none fixed z-[1000] rounded-lg border border-slate-200 bg-white px-3 py-2 text-xs font-medium leading-relaxed tracking-normal text-slate-600 shadow-xl normal-case dark:border-slate-700 dark:bg-slate-900 dark:text-slate-300"
            >
              {text}
            </span>,
            document.body
          )
        : null}
    </span>
  );
};

export default HelpHint;
