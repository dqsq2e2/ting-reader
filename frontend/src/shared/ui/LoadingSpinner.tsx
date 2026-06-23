import React from 'react';

interface LoadingSpinnerProps {
  className?: string;
  size?: 'sm' | 'md' | 'lg';
}

const SIZE_MAP = {
  sm: 'h-6 w-6',
  md: 'h-12 w-12',
  lg: 'h-16 w-16',
} as const;

/**
 * Reusable loading spinner for the entire app.
 * Previously duplicated across 10+ pages.
 */
const LoadingSpinner: React.FC<LoadingSpinnerProps> = ({ className = '', size = 'md' }) => (
  <div className={`flex items-center justify-center h-full ${className}`}>
    <div className={`animate-spin rounded-full border-b-2 border-primary-600 ${SIZE_MAP[size]}`} />
  </div>
);

export default LoadingSpinner;