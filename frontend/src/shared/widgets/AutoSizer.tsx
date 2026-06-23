import React, { useRef, useState, useEffect } from 'react';
import type { ReactNode } from 'react';

interface Size {
  width: number;
  height: number;
}

interface Props {
  children: (size: Size) => ReactNode;
  className?: string;
  style?: React.CSSProperties;
}

const AutoSizer = ({ children, className, style }: Props) => {
  const ref = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState<Size>({ width: 0, height: 0 });

  useEffect(() => {
    const element = ref.current;
    if (!element) return;

    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        // Use contentRect for accurate inner dimensions
        const { width, height } = entry.contentRect;
        // Check if size actually changed to avoid loop
        setSize(prev => {
            if (prev.width === width && prev.height === height) return prev;
            return { width, height };
        });
      }
    });

    observer.observe(element);
    
    // Initial measurement
    const rect = element.getBoundingClientRect();
    setSize({ width: rect.width, height: rect.height });

    return () => {
      observer.disconnect();
    };
  }, []);

  return (
    <div 
        ref={ref} 
        className={className} 
        style={{ width: '100%', height: '100%', overflow: 'hidden', ...style }}
    >
      {/* Only render children when we have a valid size to prevent initial flash or 0-size rendering */}
      {size.width > 0 && size.height > 0 ? children(size) : null}
    </div>
  );
};

export default AutoSizer;
