# Kivori platform, products, and release publishing

Status: base implementation merged as PR #174; production HTTP 403 prompted the authenticated D1 snapshot follow-up, 2026-11-04.

The branch implements catalog-linked KV-P1 presentation, compatible product URL/waitlist handling,
published changelog rendering, stable desktop download selection, bounded release caching and
metadata reads, and draft-release metadata generation. The findings below describe the baseline.
See the [website runbook](../apps/web/README.md#products-and-published-releases) for the implemented
limits. The live feed's original failure remains unconfirmed until runtime verification after an
approved deployment. The follow-up adds the narrow durable snapshot described below after confirming production HTTP 403; it uses the existing D1 database and Actions credentials.

Desktop 0.3.1 / Pip firmware 0.1.4 is now [published](https://github.com/cunilab/Kivori/releases/tag/v0.3.1). Work continues on `feat/web-multiproduct-releases`.

## Findings in the current system

- The shared catalog already defines internal product `pip` (wire ID 1), model `KV-P1`, board `c3-supermini` (wire ID 1). Its customer name is currently Kivori Pip. See [registry](../products/registry.toml) and [product](../products/pip/product.toml).
- The website has a separate product record named simply Kivori, without a model or shared catalog reference: [product content](../apps/web/content/products/kivori.ts), [types](../apps/web/content/products/types.ts). Product cards use the common hero visual rather than product-selected art.
- [The release loader](../apps/web/lib/releases.ts) reads GitHub at runtime. It excludes drafts, but includes prereleases and has no desktop tag-family filter. A future firmware-only GitHub Release could therefore become the website's apparent latest app version. Only the first 20 releases are requested, so future filtering and history also need pagination.
- [Website notes](../apps/web/lib/release-notes.ts) explicitly ignore the GitHub body. Only 0.1.0 has curated notes; other versions show a generic fallback. Firmware versions are absent from the web release model, although [release tooling](../scripts/release-info.mjs) already obtains them from the shared catalog.
- [GitHub response caching](../apps/web/lib/github-cache.ts) uses 600-second per-isolate memory and Workers Cache API entries. Stale fallback exists only in an isolate that previously fetched successfully; there is no durable last-known-good release snapshot.
- [Web deployment](../.github/workflows/web.yml) responds to selected code paths on main, not release publication. Dynamic release data should not require a website rebuild. Catalog changes currently fall outside those deployment path filters.
- A read-only live check returned HTTP 200 from the public `/download` page but rendered “Downloads are temporarily unavailable” and an unavailable-history message, with no 0.3.1. The public GitHub API returned published 0.3.1 and its assets from this Mac. This establishes a mismatch, not its cause: Worker egress, rate limiting, deployment drift, and caching need runtime evidence.
- The website deliberately keeps repository URLs out of its public surface and streams downloads through first-party routes. [The leak guard](../apps/web/scripts/check-leaks.mjs) must remain in place when notes are integrated.

## Recommended product and version model

Use Kivori as the shared platform/app brand and KV-P1 as the first device's public model name. Present the page title as **Kivori KV-P1** and the selector as **KV-P1**. Preserve `pip`, numeric IDs, firmware directories, and existing tags; changing marketing presentation does not require a protocol migration. Changing the shared customer name used by the desktop app is a separate coordinated decision.

| Concept | Current example | Website treatment |
| --- | --- | --- |
| Platform/app | Kivori desktop 0.3.1 | One shared download per supported OS/architecture |
| Product model | KV-P1 | Product page, selected visual, specifications, support status |
| Firmware target | `pip` / `c3-supermini`, 0.1.4 | Model-specific firmware information, updated through the desktop app |
| Hardware revision | Catalog board identity | Compatibility information when needed; not an app version |
| Wire protocol | 0.3 | Internal compatibility contract; not the customer-facing firmware version |

App and firmware versions advance independently. Show **“Kivori app 0.3.1 · KV-P1 bundled firmware 0.1.4”**, rather than one ambiguous product version. Use “bundled firmware” until independent firmware releases actually exist. Do not infer compatibility or minimum app versions by comparing unrelated version numbers.

Reuse the shared catalog for identity, model, and art references; retain editorial descriptions/specifications in website content keyed by catalog identity. Do not copy firmware version strings into marketing content. Validate unknown/duplicate product references during the build. A second product should add a catalog-linked content entry, not another download implementation.

Use `/products/kv-p1` for the public page and preserve the existing product URL with a redirect. Keep `/download` as the shared app download and current version-history entry point. Add product filtering or a product updates route only when it helps users distinguish real releases. Keep the home page focused on the available product while explaining the shared platform; empty future-product cards are unnecessary.

## Publishing once

1. Generate a small, versioned release metadata asset from the **exact release commit**, using the existing catalog/release tools. It should identify the desktop version, commit, channel, and bundled firmware entries with product/board IDs and versions. Resolve artifacts through the release asset list. Do not derive firmware versions from a title, prose, or current main.
2. Keep the Tauri `latest.json` updater contract separate. Website metadata is not an updater authority or permission to flash a device.
3. Review one public changelog source and reuse it in GitHub and the website. GitHub is the authoritative published copy; the website reads its public-facing section automatically, including later corrections. The GitHub body may also include developer links, but public notes must respect the website's repository-link policy. Render a restricted safe format without raw HTML or unsafe links. Do not maintain a second handwritten copy for each web release.
4. Attach and validate all assets and metadata while the release is a draft; publish after review. Only published releases enter the website's release view. Default downloads select a valid **stable desktop** release with the requested artifact. Beta releases appear separately and are explicitly labeled.
5. Load the same release view for download buttons, history, product firmware summaries, and metadata. Historical releases without the new asset still show their app version; show firmware as unrecorded instead of inventing it. Validate response/metadata shapes, artifact ambiguity, and supported schema versions. If an OS lacks an installer in the newest release, show it as unavailable or explicitly offer an older supported version with its own label; never label an old binary with the new version.

An immutable-release setting would strengthen the draft-first process by locking tags and assets after publication. Enabling it is a separate repository configuration change; it was not enabled or verified in this research.

## Synchronization: start small, measure the guarantee

First repair and instrument the existing live feed. A cached runtime read is sufficient to make ordinary publication appear without a web deployment. Establish a normal-operation goal of appearing within ten minutes and show last-successful-sync time and a clear unavailable/stale state. This is a proposed target, not today's guarantee: an edge hit currently starts a fresh 600-second memory TTL, and stale memory has no age limit. Track original fetch time across cache layers and bound stale use before claiming a freshness limit. Distinguish an upstream error from “no releases yet” and from an unsupported OS. Add bounded fetch timeouts, shape validation, conditional requests, and server-side status diagnostics as needed by the diagnosis.

Do not promise instantaneous, outage-proof synchronization from a short-lived edge cache. If immediate updates or durable outage fallback are required, add a publish-triggered refresh and a durable, validated last-known-good snapshot in storage already available to the site. A notification should cause a refetch of authoritative published state, not blindly overwrite it from an event payload. Reconciliation handles missed events, corrections, deletion, and unpublishing; it also prevents out-of-order events from rolling back the view. A stale snapshot needs a visible age and must not silently advertise withdrawn downloads indefinitely.

Choose **one** event mechanism when needed: a release workflow invoking a narrowly authorized refresh, or a signed GitHub webhook. For a webhook, validate the raw-body HMAC, repository and action, reject oversized input, and process duplicate deliveries idempotently. Support public edits and withdrawal, not only first publication. Keep secrets server-side. A release created/published by `GITHUB_TOKEN` does not reliably trigger another event workflow; an explicit dispatch/call is needed in that automation path.

Cloudflare Cache API entries are local to a data center. Calling `cache.delete` in one webhook request is not a global invalidation mechanism. The current OpenNext static-assets incremental cache is read-only; generic `revalidateTag` examples are not a drop-in solution. Keep static marketing pages separate from runtime release summaries, or add a deliberately configured writable revalidation setup only if justified.

## Implementation sequence and acceptance

1. **Release reliability:** identify the deployed build and Worker-side upstream status; fix the actual feed failure. Verify live 0.3.1 Windows and Apple Silicon downloads, error versus empty UI, and stable/beta selection. Preserve first-party streaming and leak checks.
2. **Product presentation:** connect website content to catalog identity; show KV-P1 with its product visual and retain the old URL redirect. Verify model-specific content, unknown-product handling, and catalog-triggered web builds.
3. **Metadata and notes:** generate the release metadata during draft creation; consume it for model firmware summaries and shared release history. Verify exact-commit versions, multiple-product fixtures, safe notes, older releases, and no mixing of app/firmware tag families.
4. **Freshness, if required:** add the chosen event refresh plus reconciliation only after deciding the latency/outage requirement. Verify duplicate/out-of-order events, missed refresh recovery, publication edits/withdrawal, invalid metadata, and upstream failures. Check the deployed site after publication rather than equating a green release workflow with a successful web update.

No second product, CMS, separate application per device, generic firmware download portal, or new database is needed for the first step.

## Research basis

These are recommendations for Kivori drawn from the repository and primary documentation, not rules imposed by the examples.

- [Logi Options+](https://www.logitech.com/en-us/software/logi-options-plus) and its [device capability matrix](https://support.logi.com/hc/en-gb/articles/25528092585879-Supported-devices-Logi-Options) demonstrate a shared application with device-specific support. This supports the platform/product split without implying every product has the same features.
- [GitHub Releases API](https://docs.github.com/en/rest/releases/releases) is repository-scoped; tags without GitHub Releases are not release-list entries. Product-specific selection must be explicit.
- [GitHub release events](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#release) and [workflow triggering rules](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/trigger-a-workflow) establish publication triggers and token-related limitations.
- [GitHub API practices](https://docs.github.com/en/rest/using-the-rest-api/best-practices-for-using-the-rest-api) recommend event notifications and conditional requests. [Webhook practices](https://docs.github.com/en/webhooks/using-webhooks/best-practices-for-using-webhooks) and [failed deliveries](https://docs.github.com/en/webhooks/using-webhooks/handling-failed-webhook-deliveries) explain validation, duplicate handling, and why reconciliation is needed.
- [Cloudflare Cache API](https://developers.cloudflare.com/workers/runtime-apis/cache/) and [OpenNext Cloudflare caching](https://opennext.js.org/cloudflare/caching) explain the current invalidation and read-only-storage constraints.
- [Immutable releases](https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases) support assembling all assets before publication while allowing notes to be corrected afterward.
- [Semantic Versioning](https://semver.org/) concerns a declared public compatibility contract; it does not make hardware revision, desktop, firmware, and protocol one version stream. [LVFS metadata](https://lvfs.readthedocs.io/en/latest/metainfo.html) illustrates explicit firmware-to-device/revision matching; adopting LVFS itself is unnecessary here.
