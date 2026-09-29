import type { Config } from "tailwindcss";

export default {
  darkMode: ["class"],
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        border: "hsl(var(--border))",
        input: "hsl(var(--input))",
        ring: "hsl(var(--ring))",
        background: "hsl(var(--background))",
        foreground: "hsl(var(--foreground))",
        primary: {
          DEFAULT: "hsl(var(--primary))",
          foreground: "hsl(var(--primary-foreground))",
        },
        secondary: {
          DEFAULT: "hsl(var(--secondary))",
          foreground: "hsl(var(--secondary-foreground))",
        },
        muted: {
          DEFAULT: "hsl(var(--muted))",
          foreground: "hsl(var(--muted-foreground))",
        },
        accent: {
          DEFAULT: "hsl(var(--accent))",
          foreground: "hsl(var(--accent-foreground))",
        },
        card: {
          DEFAULT: "hsl(var(--card))",
          foreground: "hsl(var(--card-foreground))",
        },
        popover: {
          DEFAULT: "hsl(var(--popover))",
          foreground: "hsl(var(--popover-foreground))",
        },
        destructive: {
          DEFAULT: "hsl(var(--destructive))",
          foreground: "hsl(var(--destructive-foreground))",
        },
        panel: "hsl(var(--panel))",
        raised: "hsl(var(--raised))",
        hover: "hsl(var(--hover))",
        line: "hsl(var(--line))",
        dim: "hsl(var(--dim))",
        "accent-soft": "hsl(var(--accent-soft))",
        warning: "hsl(var(--warning))",
        success: "hsl(var(--success))",
        keyframe: "hsl(var(--keyframe))",
        clip: {
          video: "hsl(var(--clip-video))",
          text: "hsl(var(--clip-text))",
          caption: "hsl(var(--clip-caption))",
          audio: "hsl(var(--clip-audio))",
          graphics: "hsl(var(--clip-graphics))",
        },
      },
      borderRadius: {
        lg: "var(--radius)",
        md: "calc(var(--radius) - 2px)",
        sm: "calc(var(--radius) - 4px)",
        panel: "10px",
        control: "8px",
        clip: "6px",
      },
      // Effects tab transition tile previews: shot A holds, the transition runs from 30% to 70%, then
      // shot B holds. Tiles apply them with `motion-safe:` and show a static midpoint otherwise.
      keyframes: {
        "transition-reveal": {
          "0%, 30%": { opacity: "0" },
          "70%, 100%": { opacity: "1" },
        },
        "transition-dip-cut": {
          "0%, 49.9%": { opacity: "0" },
          "50%, 100%": { opacity: "1" },
        },
        "transition-dip-color": {
          "0%, 30%": { opacity: "0" },
          "50%": { opacity: "1" },
          "70%, 100%": { opacity: "0" },
        },
        "transition-wipe": {
          "0%, 30%": { clipPath: "inset(0 100% 0 0)" },
          "70%, 100%": { clipPath: "inset(0 0 0 0)" },
        },
      },
      animation: {
        "transition-reveal": "transition-reveal 2.4s ease-in-out infinite",
        "transition-dip-cut": "transition-dip-cut 2.4s linear infinite",
        "transition-dip-color": "transition-dip-color 2.4s ease-in-out infinite",
        "transition-wipe": "transition-wipe 2.4s ease-in-out infinite",
      },
    },
  },
  plugins: [],
} satisfies Config;
