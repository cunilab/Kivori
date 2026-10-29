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
4. PRs build and size-check without publishing preview versions. The public URL changes only on deployment.

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

### Turnstile keys (Cloudflare Secrets Store)

Both Turnstile values are read at request time from Secrets Store bindings in `wrangler.jsonc`:
`TURNSTILE_SITE_KEY` and `TURNSTILE_SECRET_KEY`. The committed store ID and secret names are
metadata; neither value belongs in Git, chat or deployment logs. The site key is intentionally
passed to the browser; the verification secret stays in the server API route.

The owner has created both account secrets with Workers scope. Deployment credentials need
permission to bind Secrets Store resources. For API tokens, add **Account → Secrets Store → Edit**
for the Kivori account; Read does not permit bindings. Dashboard users need Secrets Store Deployer
or Super Administrator. Keep the existing Worker/deployment permissions when updating the token.
See [Secrets Store access control](https://developers.cloudflare.com/secrets-store/access-control/).
The configuration declares the bindings, so a dashboard-only binding is not required. An approved
deployment is still needed to activate the new configuration/code. Do not run `wrangler secret put`
for these names: that creates per-Worker string secrets rather than using this store.

Configure the Turnstile widget for the actual website hostname, currently
`kivori-web.andres12holivin.workers.dev`, and add the custom domain when used. Change values in
Secrets Store without copying them into the repository. A lookup error or empty stored value
blocks signup with a generic message; it never silently switches to Cloudflare test keys.
See [Cloudflare's binding guide](https://developers.cloudflare.com/secrets-store/integrations/workers/).

Plain Next rendering outside a Worker uses the test site key. Local Worker emulation cannot read
remote store values: seed both bindings locally using `bunx wrangler secrets-store secret create
<store-id> --name <binding-name> --scopes workers` from `apps/web`, **without `--remote`**, entering
Cloudflare's [dummy test values](https://developers.cloudflare.com/turnstile/troubleshooting/testing/)
at the prompts. Use the committed store ID and each binding name. Never use real production values
for local tests. Automated integration tests instead use isolated dummy bindings and local D1.

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

1. The owner selected the public support/privacy address in `wrangler.jsonc` `vars.CONTACT_EMAIL`.
   Confirm mailbox delivery and select a consented test email before live verification.
2. Confirm the real Turnstile widget allows the intended hostname and both Secrets Store entries
   contain its matching keys. Keep their values out of Git, chat and evidence. Active secret metadata
   alone does not verify the widget configuration or that its values match.
3. Review the configuration change and deploy through the approved production workflow, which
   applies D1 migrations. PRs build and size-check without publishing version URLs. Inspect the
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

### Permanent website address

Use `https://kivori-web.andres12holivin.workers.dev` as the public address. The Wrangler configuration
explicitly enables `workers_dev` and disables `preview_urls`; deployment applies that routing setting.
PR CI builds and checks the Worker without uploading preview versions or posting changing URLs.
Pushes to main retain the existing migration/deployment path. A production deployment from a reviewed
branch can be approved separately without merging its PR; testing a branch does not itself authorize
replacing the live version. Until deployment applies this configuration, existing preview URLs remain
available. After the permanent address serves the tested version, temporary preview hostnames can be
removed from the Turnstile widget while retaining the permanent hostname.
