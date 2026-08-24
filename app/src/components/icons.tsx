/** Inline SVGs — no icon font, so the app stays self-contained and offline. */
const s = { width: 24, height: 24, fill: "none", stroke: "currentColor", strokeWidth: 1.7, strokeLinecap: "round" as const, strokeLinejoin: "round" as const };

export const IconApps = () => (
  <svg viewBox="0 0 24 24" {...s}><rect x="3" y="3" width="7.5" height="7.5" rx="2" /><rect x="13.5" y="3" width="7.5" height="7.5" rx="2" /><rect x="3" y="13.5" width="7.5" height="7.5" rx="2" /><rect x="13.5" y="13.5" width="7.5" height="7.5" rx="2" /></svg>
);

export const IconRemaining = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M6 3h7l5 5v9a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2Z" /><path d="M13 3v5h5" /><path d="M8.5 12.5h7M8.5 16h4" /></svg>
);

export const IconSearch = () => (
  <svg viewBox="0 0 24 24" {...s}><circle cx="11" cy="11" r="7" /><path d="m20 20-3.5-3.5" /></svg>
);

export const IconTrash = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M4 6h16M9 6V4h6v2M6 6l1 14h10l1-14" /><path d="M10 10v6M14 10v6" /></svg>
);

export const IconWarn = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M12 4 3 19h18L12 4Z" /><path d="M12 10v4M12 17h.01" /></svg>
);

export const IconFolder = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M3 7a2 2 0 0 1 2-2h4l2 2.5h8a2 2 0 0 1 2 2V18a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7Z" /></svg>
);

export const IconStartup = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M12 3c2.6 2 4.2 5 4.2 8.4V15H7.8v-3.6C7.8 8 9.4 5 12 3Z" /><path d="M7.8 15 5.4 17.4V20l3-1.4M16.2 15l2.4 2.4V20l-3-1.4" /><circle cx="12" cy="10" r="1.6" /></svg>
);

export const IconPuzzle = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M10 4a2 2 0 1 1 4 0v1.5h2.5A1.5 1.5 0 0 1 18 7v2.5h1.5a2 2 0 1 1 0 4H18V16a1.5 1.5 0 0 1-1.5 1.5H14V19a2 2 0 1 1-4 0v-1.5H7.5A1.5 1.5 0 0 1 6 16v-2.5H4.5a2 2 0 1 1 0-4H6V7a1.5 1.5 0 0 1 1.5-1.5H10V4Z" /></svg>
);

export const IconLock = () => (
  <svg viewBox="0 0 24 24" {...s}><rect x="4.5" y="10.5" width="15" height="10" rx="2.4" /><path d="M8 10.5V7.5a4 4 0 0 1 8 0v3" /><path d="M12 14.5v2.5" /></svg>
);

export const IconCheck = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M4 12.5 9.5 18 20 6.5" /></svg>
);

export const IconBroom = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M15.5 3.5 20 8" /><path d="M13 6 6.8 12.2a3 3 0 0 0-.7 3l.4 1.4 6.1-6.1" /><path d="M4 20c1.6-.4 2.6-1 3.4-1.8l3-3 3.4 3.4-3 3C10 22.4 8.9 23 7.3 23" /><path d="M17.5 5.5 21 2" /></svg>
);

export const IconUpdates = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M20 12a8 8 0 1 1-2.5-5.8" /><path d="M20 4v4.5h-4.5" /></svg>
);

export const IconHistory = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M3.5 12a8.5 8.5 0 1 0 2.6-6.1" /><path d="M3.5 4.5V9H8" /><path d="M12 7.5V12l3 1.8" /></svg>
);

export const IconSettings = () => (
  <svg viewBox="0 0 24 24" {...s}><circle cx="12" cy="12" r="3.2" /><path d="M19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1.03 1.56V21a2 2 0 1 1-4 0v-.09A1.7 1.7 0 0 0 8.9 19.3a1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.7 1.7 0 0 0 4.7 15a1.7 1.7 0 0 0-1.56-1.03H3a2 2 0 1 1 0-4h.09A1.7 1.7 0 0 0 4.7 8.9a1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.7 1.7 0 0 0 9 4.7a1.7 1.7 0 0 0 1.03-1.56V3a2 2 0 1 1 4 0v.09A1.7 1.7 0 0 0 15.1 4.7a1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.7 1.7 0 0 0 19.3 9v0c.24.6.8 1.01 1.45 1.03H21a2 2 0 1 1 0 4h-.09A1.7 1.7 0 0 0 19.4 15Z" /></svg>
);

export const IconDashboard = () => (
  <svg viewBox="0 0 24 24" {...s}><rect x="3" y="3" width="7" height="9" rx="1.5" /><rect x="14" y="3" width="7" height="5" rx="1.5" /><rect x="14" y="12" width="7" height="9" rx="1.5" /><rect x="3" y="16" width="7" height="5" rx="1.5" /></svg>
);

export const IconTerminal = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="m4 17 6-5-6-5M12 19h8" /></svg>
);

export const IconShield = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10Z" /><path d="m9 12 2 2 4-4" /></svg>
);

/** The illustration on empty right-hand panes. */
export const ArtEmpty = () => (
  <svg viewBox="0 0 120 120" className="empty-art" width={80} height={80} fill="none" stroke="currentColor">
    <rect x="25" y="25" width="70" height="70" rx="16" strokeWidth="2.5" strokeOpacity="0.15" fill="#f8fafc" />
    <path d="M40 50h40M40 65h25M40 80h15" strokeWidth="3" strokeLinecap="round" strokeOpacity="0.3" />
    <circle cx="85" cy="85" r="14" fill="#6366f1" fillOpacity="0.1" stroke="#6366f1" strokeWidth="2.5" />
    <path d="m95 95 6 6" stroke="#6366f1" strokeWidth="2.5" strokeLinecap="round" />
  </svg>
);
