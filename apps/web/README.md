# kivori-web

The Kivori website: Next.js 16 (App Router) on Cloudflare Workers through `@opennextjs/cloudflare`.
Brand art is imported from the repo-root `assets/` via the `@brand` alias and is never copied.

## Commands

Run from the repo root (or `cd apps/web`).

| Task                                   | Command                                                    |
| -------------------------------------- | ---------------------------------------------------------- |
| Dev server (port 3000)                 | `just web-dev` / `bun --filter kivori-web dev`             |
| Plain Next build (no Cloudflare creds) | `bun --filter kivori-web build`                            |
| Tests, typecheck                       | `bun --filter kivori-web test` / `... typecheck`           |
| Preview in the local Workers runtime   | `just web-preview` (serves http://localhost:8787)          |
| Deploy to Cloudflare                   | `just web-deploy`                                          |
| Regenerate Worker binding types        | `bun --filter kivori-web cf-typegen` (output is gitignored) |

`SITE_URL` (e.g. `https://kivori.example`) sets `metadataBase`; it defaults to `http://localhost:3000`.

## One-time Cloudflare setup (owner)

1. Create a Cloudflare account and run `bunx wrangler login` once for local deploys.
2. Add GitHub repository secrets `CLOUDFLARE_API_TOKEN` (Workers edit permission) and `CLOUDFLARE_ACCOUNT_ID`.
3. Push to `main`: `.github/workflows/web.yml` deploys to `https://kivori-web.<your-subdomain>.workers.dev`.
   Without the secrets the workflow still builds and reports the Worker size, then skips deploy.
4. PR previews use `wrangler versions upload`, which requires the Worker to exist, so the first `main` deploy comes first.

W1 binds no resources. R2, D1, Turnstile, rate limiting and Analytics Engine arrive in later slices.
