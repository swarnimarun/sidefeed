# sidefeed ui

SolidJS + Vite frontend for the reader. The Rust service serves the build
output and owns every API route; there is no second server in production.

```sh
cd web
npm install
npm run dev      # hot reload on :5173, /api and /feeds proxied to the service
npm run build    # writes ../src/web/dist, which the binary embeds
npm run check    # typecheck
```

`dist/` is committed on purpose: the deployment host has no Node, so the box
build stays a plain `cargo build --release`. Rebuild and commit `dist/`
whenever the UI changes.

Filters are route state, not component state: `?tag=`, `?host=`, `?matching=all`,
`?order=oldest`, `?unread=1`, `?hours=` — so any filtered view is a link.
