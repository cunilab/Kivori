import { useEffect, useState, type ReactElement } from 'react';
import { strings } from '@/lib/i18n/strings';

export interface DeviceScreenLine {
  gesture: string;
  action: string;
}

const clockFormat = new Intl.DateTimeFormat([], {
  hour: '2-digit',
  minute: '2-digit',
  hour12: false,
});

function useClock(): string {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const timer = setInterval(() => setNow(new Date()), 15_000);
    return () => clearInterval(timer);
  }, []);
  return clockFormat.format(now);
}

/**
 * What the device's display shows, drawn in the illustration: a status row (profile left, time
 * right) over the focused control and its gestures. With no `title` it shows the profile (or the
 * Kivori name) and a hint. Sizes are in `cqw`, so it scales with the drawing.
 */
export function DeviceScreen({
  profile,
  title,
  lines = [],
  hint,
}: {
  profile?: string | undefined;
  title?: string | undefined;
  lines?: DeviceScreenLine[] | undefined;
  hint?: string | undefined;
}): ReactElement {
  const time = useClock();
  const idle = !title;
  return (
    <div className="device-screen" data-testid="device-screen">
      <div className="device-screen__status">
        <span className="device-screen__profile">{profile ?? ''}</span>
        <time className="device-screen__time">{time}</time>
      </div>
      {/* Keyed so a change of control replays the fade-in. */}
      <div className="device-screen__body" key={title ?? 'idle'}>
        {idle ? (
          <>
            <p className="device-screen__title">
              <span className="device-screen__dot" aria-hidden="true" />
              {profile ?? strings.appTitle}
            </p>
            {hint ? <p className="device-screen__hint">{hint}</p> : null}
          </>
        ) : (
          <>
            <p className="device-screen__title">
              <span className="device-screen__dot" aria-hidden="true" />
              {title}
            </p>
            <dl className="device-screen__lines">
              {lines.slice(0, 3).map((line) => (
                <div key={line.gesture} className="device-screen__line">
                  <dt>{line.gesture}</dt>
                  <dd>{line.action}</dd>
                </div>
              ))}
            </dl>
          </>
        )}
      </div>
    </div>
  );
}
