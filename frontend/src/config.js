// Centralized runtime configuration.
// These read from Vite env vars when provided, and safely fall back to
// undefined so the UI can degrade gracefully (demo mode) instead of crashing.
export const UPLOAD_URL = import.meta.env.VITE_UPLOAD_URL
export const WS_URL = import.meta.env.VITE_WS_URL

// Whether we have a real backend configured at all.
export const HAS_UPLOAD_BACKEND = Boolean(UPLOAD_URL)
export const HAS_WS_BACKEND = Boolean(WS_URL)
