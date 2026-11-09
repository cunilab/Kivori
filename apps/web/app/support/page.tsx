import type { Metadata } from 'next';
import type { ReactElement, ReactNode } from 'react';
import { contactEmail } from '@/lib/contact';
import { pageMetadata } from '@/lib/metadata';

export const metadata: Metadata = pageMetadata({
  title: 'Support',
  description:
    'Help for Kivori: getting started, the knob and buttons, status lights, profiles and fixes.',
  path: '/support',
});

// Rendered per request so CONTACT_EMAIL is read from the Worker env at runtime.
export const dynamic = 'force-dynamic';

function Steps({ items }: { items: ReactNode[] }): ReactElement {
  return (
    <ol className="list-decimal space-y-2 pl-5 marker:font-medium marker:text-primary">
      {items.map((item, index) => (
        <li key={index}>{item}</li>
      ))}
    </ol>
  );
}

function Bullets({ items }: { items: ReactNode[] }): ReactElement {
  return (
    <ul className="list-disc space-y-2 pl-5 marker:text-primary">
      {items.map((item, index) => (
        <li key={index}>{item}</li>
      ))}
    </ul>
  );
}

const CONTROLS: [string, string][] = [
  ['Turn the knob', 'Volume. Faster turns take bigger steps.'],
  ['Press the knob', 'Play or pause.'],
  ['Hold the knob, then let go', 'Mute.'],
  ['Double-press the knob', 'Switch the screen view.'],
  ['Left, middle, right button', 'Previous, play or pause, next.'],
  ['Hold the middle button', 'Pin a profile.'],
  ['Hold the knob for about 10 seconds', 'Restart Kivori, even if the app is closed.'],
];

const LIGHTS: [string, string][] = [
  ['Green check: Confirmed', 'Kivori read the new state back from your computer.'],
  ['Blue arrow: Started', 'Your computer accepted it and started it, for example opening an app.'],
  [
    'Amber question mark: Unverified',
    'It was sent, but your computer cannot confirm what happened. Shortcuts and media keys always show this.',
  ],
  ['Red cross: Error', 'It did not work, and nothing was changed.'],
];

export default function SupportPage(): ReactElement {
  const email = contactEmail();
  const sections: { id: string; title: string; body: ReactNode }[] = [
    {
      id: 'getting-started',
      title: 'Getting started',
      body: (
        <Steps
          items={[
            'Install the Kivori app on your computer (Windows, macOS or Linux) and open it.',
            'Plug Kivori into a USB-C data cable. The app finds it by itself, so there is no port to choose.',
            'Follow the short setup. You can skip any step, and you can run it again any time from the Device page.',
            'Try it: turn the knob, press the knob and press a side button. Each step ticks when Kivori sees it.',
            'Pick your starting screen, how lively the buddy is, and whether Kivori starts at login.',
          ]}
        />
      ),
    },
    {
      id: 'controls',
      title: 'The knob and buttons',
      body: (
        <>
          <dl className="divide-y divide-border">
            {CONTROLS.map(([control, action]) => (
              <div key={control} className="grid gap-1 py-3 sm:grid-cols-[2fr_3fr] sm:gap-6">
                <dt className="font-medium text-foreground">{control}</dt>
                <dd>{action}</dd>
              </div>
            ))}
          </dl>
          <p className="mt-4">
            You can change what any control does on the Controls page, or chain steps into a macro
            and run it with one press.
          </p>
        </>
      ),
    },
    {
      id: 'status-lights',
      title: 'What the status lights mean',
      body: (
        <>
          <p>
            After every action the screen shows a short badge that says what Kivori really knows.
          </p>
          <dl className="mt-3 divide-y divide-border">
            {LIGHTS.map(([light, meaning]) => (
              <div key={light} className="grid gap-1 py-3 sm:grid-cols-[2fr_3fr] sm:gap-6">
                <dt className="font-medium text-foreground">{light}</dt>
                <dd>{meaning}</dd>
              </div>
            ))}
          </dl>
        </>
      ),
    },
    {
      id: 'profiles',
      title: 'Profiles',
      body: (
        <>
          <p>
            Kivori changes what the controls do to suit the app in front of you, such as media keys
            in a player or tab switching in a browser. This is Auto, and the profile name shows at
            the top left of the screen.
          </p>
          <p className="mt-3">
            The built-in profiles are General, Browser, Code, Media, Zoom and Teams. To stay on one,
            hold the middle button. Each hold moves on to the next, and after Teams it goes back to
            Auto. A dot next to the name shows it is pinned.
          </p>
        </>
      ),
    },
    {
      id: 'troubleshooting',
      title: 'Troubleshooting',
      body: (
        <Bullets
          items={[
            <>
              <strong className="font-medium text-foreground">Kivori does not connect.</strong> Wait
              a few seconds, then try another USB port, plugged straight into your computer rather
              than a hub.
            </>,
            <>
              <strong className="font-medium text-foreground">Use a data cable.</strong> Some USB-C
              cables only carry power. A data USB-C cable is needed.
            </>,
            <>
              <strong className="font-medium text-foreground">
                Close other apps that use the device.
              </strong>{' '}
              Programs that talk to USB devices can keep Kivori busy. Close them and try again.
            </>,
            <>
              <strong className="font-medium text-foreground">
                Windows shows a protection notice.
              </strong>{' '}
              The beta installer is not signed yet. Choose More info, then Run anyway.
            </>,
            <>
              <strong className="font-medium text-foreground">Restore your Kivori.</strong> If an
              update ever goes wrong or the device will not connect, open the Device page in the app
              and choose Restore. It walks you through every step.
            </>,
            <>
              <strong className="font-medium text-foreground">Asking for help.</strong> On the
              Device page choose Copy diagnostics and paste it into your message. It holds versions
              and status only, never personal details.
            </>,
          ]}
        />
      ),
    },
    {
      id: 'linux',
      title: 'Linux setup',
      body: (
        <p>
          On Ubuntu / Debian, install the .deb with your package installer; it includes the USB
          access rule. AppImage users must install the Kivori udev rule separately, make the file
          executable and install libfuse2 (libfuse2t64 on Ubuntu 24.04+) if needed. Reconnect the
          device after installing the rule. Volume and MPRIS media work on Wayland; shortcuts and
          automatic app profiles require an X11 session. GNOME needs the AppIndicator extension for
          the tray icon. See{' '}
          <a
            href="https://github.com/cunilab/Kivori/blob/main/docs/linux.md"
            className="text-primary underline"
          >
            Linux setup and troubleshooting
          </a>
          .
        </p>
      ),
    },
    {
      id: 'contact',
      title: 'Contact',
      body: email ? (
        <p>
          Email us at{' '}
          <a href={`mailto:${email}`} className="text-primary underline">
            {email}
          </a>{' '}
          and include your diagnostics if you can.
        </p>
      ) : (
        <p>Support contact coming before launch.</p>
      ),
    },
  ];

  return (
    <>
      <section aria-labelledby="support-title" className="px-5 pt-14 pb-8 sm:px-8 sm:pt-20">
        <div className="mx-auto max-w-[1180px]">
          <h1 id="support-title" className="display-1 mt-3">
            How can we help?
          </h1>
          <p className="lead mt-6 max-w-xl">
            Quick answers for setting up and living with your Kivori.
          </p>
        </div>
      </section>
      <nav aria-label="Help topics" className="px-5 sm:px-8">
        <ul className="mx-auto flex max-w-[1180px] flex-wrap gap-x-7 gap-y-3 border-y border-border py-5 text-sm font-semibold">
          {sections.map((section) => (
            <li key={section.id}>
              <a className="text-teal underline-offset-4 hover:underline" href={`#${section.id}`}>
                {section.title}
              </a>
            </li>
          ))}
        </ul>
      </nav>
      <section id="help" aria-label="Help topics" className="px-5 pt-8 pb-24 sm:px-8 sm:pb-32">
        <div className="mx-auto max-w-3xl">
          {sections.map((section) => (
            <section
              key={section.id}
              id={section.id}
              aria-labelledby={`${section.id}-title`}
              className="scroll-mt-24 border-t border-border py-8"
            >
              <h2 id={`${section.id}-title`} className="text-2xl font-semibold tracking-tight">
                {section.title}
              </h2>
              <div className="mt-4 leading-relaxed text-muted-foreground">{section.body}</div>
            </section>
          ))}
        </div>
      </section>
    </>
  );
}
