type P = { className?: string };

export const SearchIcon = ({ className }: P) => (
  <svg className={className} viewBox="0 0 16 16" fill="none">
    <circle cx="7" cy="7" r="4.6" stroke="currentColor" strokeWidth="1.6" />
    <path
      d="M10.4 10.4 L14 14"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
    />
  </svg>
);

export const CaretIcon = ({ className }: P) => (
  <svg className={className} viewBox="0 0 10 10" fill="none">
    <path
      d="M3.5 1.5 L7.5 5 L3.5 8.5"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </svg>
);

export const CheckIcon = ({ className }: P) => (
  <svg className={className} viewBox="0 0 12 12" fill="none">
    <path
      d="M2 6.2 L4.6 8.8 L10 3.2"
      stroke="currentColor"
      strokeWidth="1.9"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </svg>
);

export const MinusIcon = ({ className }: P) => (
  <svg className={className} viewBox="0 0 12 12" fill="none">
    <path d="M2.5 6 H9.5" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" />
  </svg>
);

export const CloseIcon = ({ className }: P) => (
  <svg className={className} viewBox="0 0 14 14" fill="none">
    <path
      d="M3 3 L11 11 M11 3 L3 11"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
    />
  </svg>
);

export const BackIcon = ({ className }: P) => (
  <svg className={className} viewBox="0 0 14 14" fill="none">
    <path
      d="M8.5 2.5 L4.5 7 L8.5 11.5"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </svg>
);