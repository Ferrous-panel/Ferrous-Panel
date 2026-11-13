import React from 'react';

interface FerrousLogoProps {
  size?: 'sm' | 'md' | 'lg' | 'xl';
  showWordmark?: boolean;
  className?: string;
}

export const FerrousLogo: React.FC<FerrousLogoProps> = ({
  size = 'md',
  showWordmark = true,
  className = ''
}) => {
  const dimensions = {
    sm: 'w-7 h-7',
    md: 'w-8 h-8',
    lg: 'w-11 h-11',
    xl: 'w-14 h-14'
  }[size];

  const textSize = {
    sm: 'text-base',
    md: 'text-lg',
    lg: 'text-xl',
    xl: 'text-2xl'
  }[size];

  return (
    <span className={`inline-flex items-center gap-2.5 select-none ${className}`}>
      <svg
        viewBox="0 0 64 64"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
        className={`${dimensions} shrink-0`}
        aria-hidden="true"
      >
        <defs>
          <linearGradient
            id="ferrous-gear-grad"
            x1="8"
            y1="6"
            x2="56"
            y2="58"
            gradientUnits="userSpaceOnUse"
          >
            <stop offset="0%" stopColor="#FB923C" />
            <stop offset="50%" stopColor="#EA580C" />
            <stop offset="100%" stopColor="#9A3412" />
          </linearGradient>
          <linearGradient
            id="ferrous-core-grad"
            x1="12"
            y1="10"
            x2="52"
            y2="54"
            gradientUnits="userSpaceOnUse"
          >
            <stop offset="0%" stopColor="#1E293B" />
            <stop offset="100%" stopColor="#0F172A" />
          </linearGradient>
        </defs>
        {/* Outer Precision Rust Gear */}
        <path
          d="M29 3h6l1.2 4.6a22.5 22.5 0 0 1 5.1 2.1l4.1-2.4 4.2 4.2-2.4 4.1a22.5 22.5 0 0 1 2.1 5.1L54 22v6l-4.6 1.2a22.5 22.5 0 0 1 0 5.6L54 36v6l-4.7 1.3a22.5 22.5 0 0 1-2.1 5.1l2.4 4.1-4.2 4.2-4.1-2.4a22.5 22.5 0 0 1-5.1 2.1L35 61h-6l-1.2-4.6a22.5 22.5 0 0 1-5.1-2.1l-4.1 2.4-4.2-4.2 2.4-4.1a22.5 22.5 0 0 1-2.1-5.1L10 42v-6l4.6-1.2a22.5 22.5 0 0 1 0-5.6L10 28v-6l4.7-1.3a22.5 22.5 0 0 1 2.1-5.1l-2.4-4.1 4.2-4.2 4.1 2.4a22.5 22.5 0 0 1 5.1-2.1L29 3Z"
          fill="url(#ferrous-gear-grad)"
        />
        {/* Hexagonal Server Core Shield */}
        <polygon
          points="32,11 49,21 49,43 32,53 15,43 15,21"
          fill="url(#ferrous-core-grad)"
          stroke="#F97316"
          strokeWidth="1.75"
        />
        {/* Forged F + Polyglot Node Core */}
        <path
          d="M23 22h18v4.5H28v5h10v4.5H28V44h-5V22Z"
          fill="#F8FAFC"
        />
        <path d="M36 36h6v8h-6v-8Z" fill="#F97316" />
        <circle cx="41" cy="24.25" r="2.25" fill="#FB923C" />
      </svg>
      {showWordmark && (
        <span
          className={`${textSize} font-bold tracking-tight text-slate-900 dark:text-slate-100 whitespace-nowrap font-display`}
        >
          Ferrous<span className="text-orange-600 dark:text-orange-500">Panel</span>
        </span>
      )}
    </span>
  );
};
