import type { ReactElement } from 'react';

export function OfflinePromise(): ReactElement {
  return (
    <section
      id="offline"
      aria-labelledby="offline-title"
      className="scroll-mt-20 overflow-x-clip border-t border-home-hairline bg-home-paper px-5 py-14 sm:px-8 sm:py-20"
    >
      <div className="mx-auto grid max-w-[1180px] gap-8 lg:grid-cols-[minmax(0,3fr)_minmax(0,2fr)] lg:items-end lg:gap-16">
        <h2
          id="offline-title"
          className="home-display text-[clamp(2.5rem,6vw,5.5rem)] text-home-ink"
        >
          <span className="block">No account.</span>
          <span className="block">No cloud.</span>
          <span className="block">No telemetry.</span>
        </h2>
        <p className="max-w-[40ch] text-lg text-home-graphite lg:pb-3">
          Nothing in the app needs the internet. Works with Windows 10 and 11; Linux and Apple
          Silicon Macs are in validation.
        </p>
      </div>
    </section>
  );
}
