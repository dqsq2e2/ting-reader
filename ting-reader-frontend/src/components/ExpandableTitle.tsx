import React, { useState, useRef, useEffect } from 'react';

interface ExpandableTitleProps {
  title: string;
  className?: string;
  maxLines?: number;
}

const ExpandableTitle: React.FC<ExpandableTitleProps> = ({ title, className = "", maxLines = 1 }) => {
  const [isExpanded, setIsExpanded] = useState(false);
  const [isOverflowing, setIsOverflowing] = useState(false);
  const textRef = useRef<HTMLHeadingElement>(null);

  useEffect(() => {
    const checkOverflow = () => {
      if (textRef.current) {
        const { scrollHeight, clientHeight } = textRef.current;
        setIsOverflowing(scrollHeight > clientHeight);
      }
    };

    checkOverflow();
    window.addEventListener('resize', checkOverflow);
    return () => window.removeEventListener('resize', checkOverflow);
  }, [title, maxLines]);

  return (
    <div className="relative group">
      <h3 
        ref={textRef}
        onClick={() => isOverflowing && setIsExpanded(!isExpanded)}
        className={`
          ${className} 
          ${!isExpanded ? 'line-clamp-' + maxLines : ''} 
          ${isOverflowing ? 'cursor-pointer hover:text-primary-600 dark:hover:text-primary-400 hover:scale-[1.02] hover:font-black' : ''}
          transition-all duration-200 origin-left
        `}
        style={{
          display: !isExpanded ? '-webkit-box' : 'block',
          WebkitLineClamp: !isExpanded ? maxLines : 'none',
          WebkitBoxOrient: 'vertical',
          overflow: !isExpanded ? 'hidden' : 'visible'
        }}
      >
        {title}
      </h3>
    </div>
  );
};

export default ExpandableTitle;
