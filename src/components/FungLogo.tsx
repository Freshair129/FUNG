type FungLogoProps = {
  size?: number;
  className?: string;
  variant?: "porcelain" | "dark" | "sage";
  showWordmark?: boolean;
  wordmarkColor?: string;
};

export function FungLogo({
  size = 40,
  className = "",
  variant = "porcelain",
  showWordmark = false,
  wordmarkColor,
}: FungLogoProps) {
  const markColor = variant === "porcelain"
    ? "var(--fung-brand-porcelain, #FAF8F3)"
    : "var(--fung-brand-ink, #171918)";
  const resolvedWordmarkColor = wordmarkColor ?? "var(--fung-logo-wordmark, #6F897E)";

  return (
    <div
      className={`fung-logo-container ${className}`}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: Math.max(8, size * 0.3),
        userSelect: "none",
        color: markColor,
      }}
    >
      <svg
        width={size}
        height={size}
        viewBox="0 0 100 100"
        xmlns="http://www.w3.org/2000/svg"
        role={showWordmark ? undefined : "img"}
        aria-hidden={showWordmark ? true : undefined}
        aria-label={showWordmark ? undefined : "FUNG"}
      >
        <path
          fill="currentColor"
          fillRule="evenodd"
          d="M32 8H68A24 24 0 0 1 92 32V68A24 24 0 0 1 68 92H32A24 24 0 0 1 8 68V32A24 24 0 0 1 32 8ZM47 22H53A14 14 0 0 1 67 36V64A14 14 0 0 1 53 78H47A14 14 0 0 1 33 64V36A14 14 0 0 1 47 22Z"
        />
      </svg>

      {showWordmark && (
        <span
          className="fung-wordmark-text"
          style={{
            color: resolvedWordmarkColor,
            fontFamily: 'var(--fung-font-latin, "DM Sans", sans-serif)',
            fontSize: `${size * 0.55}px`,
            fontWeight: 500,
            letterSpacing: "var(--fung-wordmark-tracking, 0.34em)",
            textTransform: "uppercase",
            lineHeight: 1,
            marginLeft: "0.1em",
          }}
        >
          FUNG
        </span>
      )}
    </div>
  );
}
