export default function FeroHaLogo({ size = 32 }: { size?: number }) {
  return (
    <div style={{ display: "inline-flex", alignItems: "center", gap: 4 }}>
      <svg
        width={size}
        height={size}
        viewBox="0 0 32 32"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
      >
        <style>{`
          @keyframes logo-draw {
            0% { stroke-dashoffset: 100; opacity: 0; }
            30% { opacity: 1; }
            100% { stroke-dashoffset: 0; opacity: 1; }
          }
          .logo-leaf, .logo-vein {
            stroke: #2ae09a;
            stroke-width: 2;
            stroke-linecap: round;
            stroke-linejoin: round;
            fill: none;
          }
          .logo-leaf {
            stroke-dasharray: 100;
            stroke-dashoffset: 100;
            animation: logo-draw 1.5s cubic-bezier(0.16, 1, 0.3, 1) forwards;
          }
          .logo-vein {
            stroke-dasharray: 40;
            stroke-dashoffset: 40;
            animation: logo-draw 1.5s cubic-bezier(0.16, 1, 0.3, 1) 0.3s forwards;
            stroke: rgba(42, 224, 154, 0.7);
          }
          .logo-dot {
            fill: #2ae09a;
            opacity: 0;
            animation: logo-draw 0.5s ease-out 1.2s forwards;
          }
        `}</style>
        <path className="logo-leaf" d="M16 4 C10 4 4 10 4 16 C4 22 10 28 16 28 C22 28 26 24 26 18 C26 12 22 8 16 8" />
        <path className="logo-vein" d="M16 8 L16 24 M16 12 C12 14 16 18 16 20" />
        <circle className="logo-dot" cx="16" cy="26" r="2" />
      </svg>
      <span style={{
        fontSize: size * 0.6,
        fontWeight: 700,
        color: "var(--accent-primary)",
        fontFamily: "var(--font-mono)",
        letterSpacing: "-0.5px",
        opacity: 0,
        animation: "logo-draw 0.8s ease-out 1.4s forwards",
      }}>FeroHa</span>
    </div>
  );
}
