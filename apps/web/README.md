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

## Waitlist (W4)

Sign-ups go to `POST /api/waitlist` and are stored in D1 (`WAITLIST_DB`, database `kivori-web`).
Checks, in order: honeypot, field lengths, email, product, Turnstile, per-IP rate limit (5 per
minute, `WAITLIST_RATE_LIMITER`). Repeat sign-ups return the same response as new ones. No IP is stored.

### Migrations

```sh
cd apps/web
bunx wrangler d1 migrations apply kivori-web --local    # local preview database
bunx wrangler d1 migrations apply kivori-web --remote   # production (the deploy workflow also runs this)
```

### Exporting the list

```sh
bunx wrangler d1 execute kivori-web --remote --command "SELECT email, product, created_at FROM waitlist ORDER BY created_at"
# add --json for machine-readable output
```

### Turnstile keys

The repo ships Cloudflare's always-pass **test** keys: the site key is in `wrangler.jsonc` `vars`
(`TURNSTILE_SITE_KEY`), and the server falls back to the test secret only while the site key is the
test key. A real site key with no `TURNSTILE_SECRET_KEY` rejects every sign-up (fail closed). To go live:

1. Cloudflare dashboard, Turnstile, Add widget, for hostname `kivori-web.andres12holivin.workers.dev`
   (and the custom domain later).
2. Put the widget's site key in `wrangler.jsonc` `vars.TURNSTILE_SITE_KEY`.
3. `bunx wrangler secret put TURNSTILE_SECRET_KEY` and paste the widget's secret.
4. Deploy.

For local testing the test token `XXXX.DUMMY.TOKEN.XXXX` passes with the test secret.

Sign-up counts (product and utm_source only, no email) go to the `SIGNUPS` Analytics Engine dataset
`kivori_signups`. R2 and Cloudflare Web Analytics are not bound yet.
