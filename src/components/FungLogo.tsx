import React from "react";

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
  wordmarkColor = "#6F897E",
}: FungLogoProps) {
  const isDark = variant === "dark";
  const mainColor = isDark ? "#171918" : "#FAF8F3";
  const shadowColor = isDark ? "rgba(0, 0, 0, 0.4)" : "rgba(23, 25, 24, 0.25)";
  const foldColor = isDark ? "#282C29" : "#E2DDD1";

  return (
    <div
      className={`fung-logo-container ${className}`}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: Math.max(8, size * 0.3),
        userSelect: "none",
      }}
    >
      <svg
        width={size}
        height={size}
        viewBox="0 0 200 200"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
        style={{ filter: `drop-shadow(0px ${size * 0.08}px ${size * 0.12}px ${shadowColor})` }}
      >
        <defs>
          <linearGradient id={`porcelainGrad-${variant}`} x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor={isDark ? "#202321" : "#FFFFFF"} />
            <stop offset="100%" stopColor={mainColor} />
          </linearGradient>

          <linearGradient id={`foldGrad-${variant}`} x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor={foldColor} />
            <stop offset="100%" stopColor={mainColor} />
          </linearGradient>

          <filter id={`insetShadow-${variant}`} x="-10%" y="-10%" width="120%" height="120%">
            <feOffset dx="0" dy="3" />
            <feGaussianBlur stdDeviation="3" result="offset-blur" />
            <feComposite operator="out" in="SourceGraphic" in2="offset-blur" result="inverse" />
            <feFlood floodColor="#000" floodOpacity="0.2" result="color" />
            <feComposite operator="in" in="color" in2="inverse" result="shadow" />
            <feComposite operator="over" in="shadow" in2="SourceGraphic" />
          </filter>
        </defs>

        {/* Base Squircle Loop Body */}
        <path
          d="M 50 20 H 150 A 30 30 0 0 1 180 50 V 150 A 30 30 0 0 1 150 180 H 50 A 30 30 0 0 1 20 150 V 50 A 30 30 0 0 1 50 20 Z"
          fill={`url(#porcelainGrad-${variant})`}
        />

        {/* Inner Hole Cutout */}
        <path
          d="M 80 60 H 120 A 15 15 0 0 1 135 75 V 125 A 25 25 0 0 1 110 150 H 80 A 15 15 0 0 1 65 135 V 75 A 15 15 0 0 1 80 60 Z"
          fill={isDark ? "#FAF8F3" : "#171918"}
        />

        {/* Top-Left Ribbon Fold Transition */}
        <path
          d="M 20 60 C 20 37.9 37.9 20 60 20 L 120 20 C 85 20 55 45 45 80 L 20 60 Z"
          fill={`url(#foldGrad-${variant})`}
          opacity="0.9"
        />

        {/* Bottom-Right Ribbon Fold Overlay */}
        <path
          d="M 180 140 C 180 162.1 162.1 180 140 180 L 80 180 C 115 180 145 155 155 120 L 180 140 Z"
          fill={`url(#foldGrad-${variant})`}
          opacity="0.85"
        />
      </svg>

      {showWordmark && (
        <span
          className="fung-wordmark-text"
          style={{
            color: wordmarkColor,
            fontFamily: '"SF Pro Display", "Avenir Next", system-ui, sans-serif',
            fontSize: `${size * 0.55}px`,
            fontWeight: 700,
            letterSpacing: "0.35em",
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
