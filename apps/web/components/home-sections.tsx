import Image from 'next/image';
import type { ReactElement } from 'react';
import mascot from '@brand/mascot.svg';
import { Badge } from '@kivori/ui/components/badge';
import { Card } from '@kivori/ui/components/card';
import { Separator } from '@kivori/ui/components/separator';
import { ProductStage } from '@/components/product-shot';
import { Eyebrow, Section } from '@/components/section';
import { WaitlistSection } from '@/components/waitlist-section';
import { moods, renders, type MediaImage } from '@/lib/media';

interface Mood {
  name: string;
  caption: string;
  image: MediaImage;
}

const MOODS: readonly Mood[] = [
  { name: 'Idle', caption: 'Hanging out with you while you work.', image: moods.idle },
  { name: 'Happy', caption: 'Only when an action really went through.', image: moods.happy },
  { name: 'Listening', caption: 'Music is playing.', image: moods.listening },
  { name: 'Attentive', caption: "You're in a meeting.", image: moods.attentive },
  { name: 'Strained', caption: 'Your computer is working hard.', image: moods.busy },
  { name: 'Sleeping', caption: 'Your computer is asleep or locked.', image: moods.sleeping },
];

/** The six real mood frames, each tied to something that really happens on the computer. */
export function MoodsSection(): ReactElement {
  return (
    <Section
      id="moods"
      eyebrow="The buddy"
      title="It has moods. They mean something."
      intro="The face on the screen isn't decoration. Each mood comes from something real happening on your computer."
    >
      <ul className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {MOODS.map((mood) => (
          <li key={mood.name}>
            <Card className="h-full flex-row items-center gap-4 rounded-2xl p-3.5 ring-0">
              <Image
                src={mood.image.src}
                alt={mood.image.alt}
                width={96}
                height={96}
                unoptimized
                className="size-24 shrink-0 rounded-xl [image-rendering:pixelated]"
              />
              <div>
                <h3 className="pixel text-[0.82rem] font-bold text-mint">{mood.name}</h3>
                <p className="mt-2 text-[0.98rem] leading-snug">{mood.caption}</p>
              </div>
            </Card>
          </li>
        ))}
      </ul>
    </Section>
  );
}

const RULES: readonly { app: string; lead: string; rest: string }[] = [
  { app: 'Browser', lead: 'The knob flips through your tabs', rest: '.' },
  { app: 'Music', lead: 'The buddy listens along. Keys skip and pause', rest: '.' },
  { app: 'Meetings', lead: 'A meeting face, and your shortcuts one press away', rest: '.' },
  { app: 'Code', lead: 'Your own keys for your editor, set up in the app', rest: '.' },
  {
    app: 'Everything else',
    lead: 'Volume, play or pause, mute',
    rest: '. The basics, always there.',
  },
];

/** Profile rules as a definition list, plus the pin note. */
export function ProfilesSection(): ReactElement {
  return (
    <Section id="profiles" customHeading title="">
      <div className="grid items-start gap-8 md:grid-cols-[5fr_7fr] md:gap-16">
        <div>
          <Eyebrow>Profiles</Eyebrow>
          <h2 id="profiles-title" className="display-2 mt-3.5">
            It knows where you are.
          </h2>
          <p className="lead mt-4 max-w-[52ch]">
            Switch apps and Kivori switches with you. The labels on the screen always say what each
            control does right now.
          </p>
          <p className="mt-6 inline-flex flex-wrap items-center gap-2.5 text-[0.95rem] text-muted-foreground">
            <kbd className="pixel rounded-md border border-b-[3px] border-border px-2 py-1.5 text-[0.7rem] text-mint">
              hold middle key
            </kbd>
            keeps one profile, whatever is in front.
          </p>
        </div>
        <dl className="border-t border-border">
          {RULES.map((rule) => (
            <div
              key={rule.app}
              className="grid gap-1 border-b border-border py-[18px] sm:grid-cols-[9.5rem_1fr] sm:gap-4"
            >
              <dt className="font-semibold">{rule.app}</dt>
              <dd className="text-muted-foreground">
                <strong className="font-medium text-foreground">{rule.lead}</strong>
                {rule.rest}
              </dd>
            </div>
          ))}
        </dl>
      </div>
    </Section>
  );
}

interface Light {
  glyph: string;
  name: string;
  color: string;
  line: string;
}

const LIGHTS: readonly Light[] = [
  {
    glyph: '✓',
    name: 'Confirmed',
    color: 'var(--ok)',
    line: 'Kivori read the new state back from your computer. It worked, and it knows.',
  },
  {
    glyph: '→',
    name: 'Started',
    color: 'var(--sky)',
    line: 'Your computer accepted it and got going, like opening an app.',
  },
  {
    glyph: '?',
    name: 'Unverified',
    color: 'var(--amber)',
    line: 'Sent, but your computer cannot confirm it. Shortcuts and media keys always say so.',
  },
  {
    glyph: '✕',
    name: 'Error',
    color: '#e5484d',
    line: 'It did not work, and nothing was changed. No pretending.',
  },
];

/** The four status lights: the real differentiator, one line each. */
export function LightsSection(): ReactElement {
  return (
    <Section
      id="honest"
      tone="surface"
      eyebrow="Status lights"
      title="Honest by design."
      intro="After every action the screen shows a little light that says how sure Kivori really is. A shortcut is never dressed up as a success it cannot prove."
    >
      <ul className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        {LIGHTS.map((light) => (
          <li key={light.name}>
            <Card className="h-full gap-0 rounded-2xl bg-background p-6 ring-1 ring-border">
              <span
                aria-hidden="true"
                className="grid size-12 place-items-center rounded-full text-xl font-bold text-stage"
                style={{ background: light.color, boxShadow: `0 0 28px ${light.color}55` }}
              >
                {light.glyph}
              </span>
              <h3 className="mt-5 font-display text-xl font-bold tracking-tight">{light.name}</h3>
              <p className="mt-2 text-[0.98rem] text-muted-foreground">{light.line}</p>
            </Card>
          </li>
        ))}
      </ul>
    </Section>
  );
}

const KEYS: readonly { key: string; action: string }[] = [
  { key: 'Left', action: 'Previous track' },
  { key: 'Middle', action: 'Run my meeting macro' },
  { key: 'Right', action: 'Next track' },
];

/** Customise: three key chips with action names, and a macro in one press. */
export function CustomizeSection(): ReactElement {
  return (
    <Section
      id="customize"
      eyebrow="Your rules"
      title="Make it yours."
      intro="Give the knob and each of the three keys their own job in every app. Chain a few steps into a macro and run the whole thing with one press."
    >
      <div className="grid items-center gap-6 md:grid-cols-2">
        <Card className="gap-5 rounded-2xl p-6 ring-1 ring-border">
          <p className="pixel text-xs text-muted-foreground">Example: keys in Meetings</p>
          <ul className="grid grid-cols-3 gap-3">
            {KEYS.map((item) => (
              <li key={item.key} className="flex flex-col items-center gap-3 text-center">
                <span
                  aria-hidden="true"
                  className="block aspect-[1.08] w-full max-w-24 rounded-[16%] bg-linear-to-b from-[#9ff7d4] to-mint shadow-[inset_0_-0.55rem_0_var(--mint-deep),0_0.3rem_0_#05070c]"
                />
                <span className="text-sm font-semibold">{item.key}</span>
                <Badge variant="secondary" className="h-auto whitespace-normal py-1 text-center">
                  {item.action}
                </Badge>
              </li>
            ))}
          </ul>
        </Card>
        <Card className="gap-4 rounded-2xl p-6 ring-1 ring-border">
          <p className="pixel text-xs text-muted-foreground">Example: one-press macro</p>
          <ol className="flex flex-wrap items-center gap-2 text-sm">
            {['Mute mic', 'Open notes', 'Start timer'].map((step, index) => (
              <li key={step} className="flex items-center gap-2">
                {index > 0 ? (
                  <span aria-hidden="true" className="text-mint">
                    &rarr;
                  </span>
                ) : null}
                <span className="rounded-full border border-border bg-secondary px-3 py-1.5">
                  {step}
                </span>
              </li>
            ))}
          </ol>
          <Separator />
          <p className="text-muted-foreground">
            A macro stops at the first step that fails and tells you how sure it is about the rest.
          </p>
        </Card>
      </div>
    </Section>
  );
}

/** Kivori and your computer, talking to each other directly. The cloud is crossed out. */
function PrivacyDiagram(): ReactElement {
  return (
    <svg
      viewBox="0 0 480 270"
      role="img"
      aria-label="Kivori talks directly to your computer. The cloud is crossed out."
      className="mx-auto w-full max-w-[480px]"
    >
      <g fill="none" strokeLinecap="round" strokeLinejoin="round">
        <path
          d="M196 78h88a24 24 0 0 0 2-47 32 32 0 0 0-62-8 28 28 0 0 0-28 55z"
          stroke="#66728a"
          strokeWidth="3"
        />
        <path d="M192 96 288 14" stroke="#ff6b70" strokeWidth="6" />
        <path d="M150 190h180" stroke="#7df2c4" strokeWidth="4" strokeDasharray="1 11" />
        <path
          d="M150 190l12-9m-12 9 12 9M330 190l-12-9m12 9-12 9"
          stroke="#7df2c4"
          strokeWidth="4"
        />
      </g>
      <rect
        x="36"
        y="140"
        width="114"
        height="100"
        rx="18"
        fill="#131a2b"
        stroke="#222c42"
        strokeWidth="2"
      />
      <rect x="72" y="162" width="42" height="42" rx="10" fill="#7df2c4" />
      <rect x="72" y="194" width="42" height="10" rx="5" fill="#35a87c" />
      <rect x="84" y="174" width="6" height="12" rx="2" fill="#0b0f1a" />
      <rect x="96" y="174" width="6" height="12" rx="2" fill="#0b0f1a" />
      <rect
        x="330"
        y="140"
        width="114"
        height="100"
        rx="14"
        fill="#131a2b"
        stroke="#222c42"
        strokeWidth="2"
      />
      <rect
        x="344"
        y="154"
        width="86"
        height="54"
        rx="6"
        fill="#0b0f1a"
        stroke="#2a3550"
        strokeWidth="1.5"
      />
      <rect x="373" y="220" width="28" height="6" rx="3" fill="#222c42" />
      <g
        fontFamily="var(--font-silkscreen), monospace"
        fontSize="11"
        fill="#8f9bb0"
        textAnchor="middle"
      >
        <text x="93" y="262">
          KIVORI
        </text>
        <text x="387" y="262">
          YOUR COMPUTER
        </text>
        <text x="240" y="176" fill="#7df2c4">
          DIRECT
        </text>
      </g>
    </svg>
  );
}

/** Privacy band: the diagram on one side, the promise on the other. */
export function PrivacySection(): ReactElement {
  const points: readonly string[] = [
    'No account to make',
    'No cloud to trust',
    'No telemetry, nothing phoning home',
    'Your settings stay on your computer',
  ];
  return (
    <Section id="private" tone="surface" customHeading title="">
      <div className="grid items-center gap-10 md:grid-cols-2 md:gap-16">
        <div>
          <Eyebrow>Private</Eyebrow>
          <h2 id="private-title" className="display-2 mt-3.5">
            No account. No cloud. Just a knob and a very small friend.
          </h2>
          <ul className="mt-6 space-y-2.5 text-muted-foreground">
            {points.map((point) => (
              <li key={point}>
                <span aria-hidden="true" className="text-mint">
                  &#10003;{' '}
                </span>
                {point}
              </li>
            ))}
          </ul>
        </div>
        <div>
          <PrivacyDiagram />
        </div>
      </div>
    </Section>
  );
}

/** The device render on the dark stage with the mint glow beneath it. */
export function ProductLookSection(): ReactElement {
  return (
    <Section
      id="look"
      eyebrow="The look"
      title="Fits beside your keyboard."
      intro="A slim wedge that tilts the screen towards you, with a roomy knob and keys you can find by feel."
      align="center"
    >
      <ProductStage
        image={renders.heroTransparent}
        large
        sizes="(min-width: 1000px) 900px, 100vw"
        className="max-w-[900px]"
      />
    </Section>
  );
}

/** Closing band: the buddy and the waitlist again. */
export function FinalBand(): ReactElement {
  return (
    <section
      id="join"
      aria-labelledby="join-title"
      className="scroll-mt-14 border-t border-border px-5 py-16 sm:px-8 sm:py-24"
    >
      <div className="mx-auto flex max-w-xl flex-col items-center text-center">
        <Image src={mascot} alt="" width={84} height={84} />
        <h2 id="join-title" className="display-2 mt-6">
          Want one? Get on the list.
        </h2>
        <p className="lead mt-4">
          There is no price or ship date yet. Leave your email and we will tell you when there is.
        </p>
        <div className="mt-8 flex w-full justify-center text-left">
          <WaitlistSection idPrefix="final" variant="inline" />
        </div>
      </div>
    </section>
  );
}
