import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// `npm run dev` serves the UI with hot reload and proxies the API to a running
// `navgator-web` (default port 4747). The server checks Host and Origin, so
// the proxy rewrites both to the server's own address.
const server = `http://127.0.0.1:${process.env.NAVGATOR_WEB_PORT ?? "4747"}`;

export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      "^/(api/|theme\\.css)": {
        target: server,
        changeOrigin: true,
        headers: { origin: server },
      },
    },
  },
});
