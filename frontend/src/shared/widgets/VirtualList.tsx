import React, { useState, useRef } from 'react';

interface Props {
  height: number;
  width: number | string;
  itemCount: number;
  itemSize: number;
  children: (props: { index: number; style: React.CSSProperties }) => React.ReactNode;
  className?: string;
  style?: React.CSSProperties;
}

export const FixedSizeList: React.FC<Props> = ({ 
  height, 
  width, 
  itemCount, 
  itemSize, 
  children,
  className,
  style 
}) => {
  const [scrollTop, setScrollTop] = useState(0);
  const scrollRef = useRef<HTMLDivElement>(null);

  const totalHeight = itemCount * itemSize;
  
  // Buffer items to prevent flickering
  const buffer = 5;
  const startIndex = Math.max(0, Math.floor(scrollTop / itemSize) - buffer);
  const endIndex = Math.min(
    itemCount - 1,
    Math.floor((scrollTop + (typeof height === 'number' ? height : 0)) / itemSize) + buffer
  );

  const items = [];
  // Only render items if we have a valid range
  if (itemCount > 0) {
      for (let i = startIndex; i <= endIndex; i++) {
        items.push(
          <React.Fragment key={i}>
            {children({
              index: i,
              style: {
                position: 'absolute',
                top: i * itemSize,
                left: 0,
                width: '100%',
                height: itemSize,
              },
            })}
          </React.Fragment>
        );
      }
  }

  return (
    <div
      ref={scrollRef}
      className={className}
      style={{ 
        height, 
        width, 
        overflow: 'auto', 
        position: 'relative',
        ...style
      }}
      onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}
    >
      <div style={{ height: totalHeight, width: '100%', position: 'relative' }}>
        {items}
      </div>
    </div>
  );
};

export default FixedSizeList;
