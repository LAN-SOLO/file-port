/**
 * App-Icon als Inline-SVG (vereinfachte, flache Fassung von
 * public/brand/fileport.svg der Website): zwei Transfer-Pfeile ⇄ mit
 * Punkt, auf abgerundeter Kachel — skaliert sauber im Header.
 */
export default function Logo({ size = 26 }: { size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 1024 1024"
      aria-hidden="true"
      focusable="false"
    >
      <rect x="14" y="14" width="996" height="996" rx="226" fill="#14213a" stroke="#2b3d61" strokeWidth="28" />
      <path
        d="M 316 416 L 644 416 M 576 336 L 660 416 L 576 496"
        stroke="#38bdf8"
        strokeWidth="72"
        strokeLinecap="round"
        strokeLinejoin="round"
        fill="none"
      />
      <path
        d="M 708 632 L 380 632 M 448 552 L 364 632 L 448 712"
        stroke="#0284c7"
        strokeWidth="72"
        strokeLinecap="round"
        strokeLinejoin="round"
        fill="none"
      />
      <circle cx="512" cy="524" r="52" fill="#eef4fa" />
    </svg>
  );
}
