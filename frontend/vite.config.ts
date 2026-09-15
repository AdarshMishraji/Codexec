import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

// Dev-only: codexec-api runs on :8080 (see DEPLOYMENT.md). `npm run dev`
// serves the SPA on Vite's own port, so these paths need to be proxied
// through to the real backend instead of 404ing - the production build
// never uses this, since then everything is same-origin (codexec-api
// serves the built frontend directly via tower-http's ServeDir).
//
// "/admin/" (trailing slash, not "/admin") deliberately only matches the
// nested JSON API (/admin/languages, /admin/api-keys, ...) - the bare
// "/admin" path is the React Router *page* and must stay unproxied so
// Vite's own dev-server SPA fallback serves index.html for it instead.
const API_PROXY_PATHS = ['/admin/', '/submissions', '/stats', '/languages']

export default defineConfig({
  plugins: [react()],
  server: {
    proxy: Object.fromEntries(
      API_PROXY_PATHS.map((path) => [path, 'http://localhost:8080']),
    ),
  },
})
