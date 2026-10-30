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
2. Add the GitHub repository secret `CLOUDFLARE_API_TOKEN` (Workers edit permission) and the repository
   variable `CLOUDFLARE_ACCOUNT_ID` (not secret). Both are set (2026-10-12).
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

### Local signup and deletion rehearsal

Run `bun --filter kivori-web test lib/waitlist.integration.test.ts` from the repository root.
This calls the real API route with dummy `example.com` addresses, a disposable local D1 database
using the committed migration, and mocked Turnstile, rate limiting and analytics. The isolated
Wrangler configuration disables remote bindings, persistence and environment-file loading.
The fixture disposes the local runtime and removes its temporary configuration afterward.

It verifies normalized signup, identical duplicate responses, oversized attribution handling,
parameterized deletion across products while preserving another address, failure paths and the
rendered privacy contact link/fallback. No public mailbox, Cloudflare credentials, email delivery
or production deployment is needed. This is an API/server-rendering test, not a browser or live
Turnstile test; it does not close the public launch checks below.

### Public launch and deletion verification

The [M2.9 readiness audit](../../docs/validation.md#m29-public-website-readiness-audit-open)
records the current gaps. Test keys and a successful local form response do not establish readiness.

1. Confirm the owner's public privacy mailbox and a consented test email. Set the public mailbox
   in `wrangler.jsonc` `vars.CONTACT_EMAIL`; do not infer it from deployment credentials.
2. Configure the real Turnstile widget for the intended hostname, commit its public site key,
   and provision its secret with the interactive command above. Keep secrets out of Git, chat
   and evidence. Widget inspection currently needs additional Cloudflare access from the owner.
3. Review the configuration change and deploy through the approved production workflow, which
   applies D1 migrations. PR version uploads are previews, not production releases. Inspect the
   active production version for `WAITLIST_DB`, `WAITLIST_RATE_LIMITER`, `SIGNUPS`, the public
   variables and the secret binding; secret presence alone does not prove the widget works.
4. Confirm the intended public access policy with the owner. After the approved launch, check
   the homepage and `/privacy` anonymously, including the real widget and correct `mailto:` link.
   A Cloudflare Access error blocks this check; do not remove Access merely to run the test.
5. Submit one approved test signup through the real form. Verify its normalized email and product
   in D1 with a bound-parameter query. A success response alone is insufficient: honeypot requests
   intentionally succeed without storing a row. Record UTC time, deployed version and row count,
   without committing the address, token or database export.
6. With authorization to send it, email a deletion request to the published mailbox and confirm
   receipt. Using a database client with bound parameters, delete all rows for that normalized
   test email across products, then verify zero remain:

   ```sql
   SELECT COUNT(*) FROM waitlist WHERE email = ?1;
   DELETE FROM waitlist WHERE email = ?1;
   SELECT COUNT(*) FROM waitlist WHERE email = ?1;
   ```

   Bind `?1` to the approved test address; these are parameterized SQL templates, not standalone
   Wrangler commands. Check the address and initial count before deletion. Record receipt and
   deletion outcomes without publishing personal data. Close the roadmap item only when every
   live check passes.

Sign-up counts (product and utm_source only, no email) go to the `SIGNUPS` Analytics Engine dataset
`kivori_signups`. R2 and Cloudflare Web Analytics are not bound yet.
