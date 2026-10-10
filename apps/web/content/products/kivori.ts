import type { Product } from './types';

// Public selling points only. Keep part names, exact dimensions and build details out of this file;
// a fact that is not decided yet is left out rather than guessed.
export const kivori: Product = {
  slug: 'kivori',
  name: 'Kivori',
  edition: 'Beta',
  status: 'coming-soon',
  tagline: 'Control the desktop physically. Understand the desktop visually.',
  summary:
    'A desk buddy you control your computer with: a physical knob and three buttons, and a small screen where the buddy shows what your computer is really doing.',
  hero: {
    art: 'device',
    alt: 'Illustration of the Kivori wedge: a big knob on the left, a square screen and three buttons on the right.',
  },
  features: [
    {
      title: 'App-aware profiles',
      body: 'The controls change to suit the app in front of you: media keys in a player, tab switching in a browser. Built in: General, Browser, Code, Media, Zoom and Teams. Hold the middle button to pin one.',
      icon: 'layers',
    },
    {
      title: 'Honest confirmations',
      body: 'Every action ends in a badge that says what Kivori actually knows: Confirmed, Started, Unverified or Error. A shortcut is never shown as a success it cannot prove.',
      icon: 'shield-check',
    },
    {
      title: 'Macros',
      body: 'A macro runs up to 8 steps in order, with optional waits. It stops at the first step that fails and reports its least-confirmed step.',
      icon: 'list',
    },
    {
      title: 'Display views',
      body: 'The screen is home to your buddy, with the profile, clock and control labels around it. Switch between Buddy, Clock, Volume, Media and System (CPU and memory) with a double press.',
      icon: 'monitor',
    },
    {
      title: 'Buddy reactions',
      body: 'The buddy looks strained under heavy load, muted when you are muted and happy after a confirmed action. Turn reactions on or off and set their intensity to Low, Normal or High.',
      icon: 'smile',
    },
    {
      title: 'Always recoverable',
      body: 'Hold the knob for about 10 seconds to restart Kivori even if the app is closed. If an update ever goes wrong, the device can always be restored from the app.',
      icon: 'life-buoy',
    },
    {
      title: 'Offline by design',
      body: 'No account, no cloud and no telemetry. Nothing in the app needs the internet, and your settings stay on your computer.',
      icon: 'wifi-off',
    },
  ],
  specs: [
    {
      group: 'Controls',
      rows: [
        { label: 'Controls', value: 'Clicky knob with press, plus three buttons' },
        { label: 'Buttons', value: 'Press and Hold on each button' },
        { label: 'Restart', value: 'Hold the knob for about 10 seconds' },
      ],
    },
    {
      group: 'Display',
      rows: [
        { label: 'Display', value: 'Bright colour screen' },
        { label: 'Views', value: 'Buddy, Clock, Volume, Media, System' },
      ],
    },
    {
      group: 'Connection',
      rows: [{ label: 'Connection', value: 'USB-C (power and data)' }],
    },
    {
      group: 'Design',
      rows: [
        { label: 'Size', value: 'Fits beside your keyboard' },
        { label: 'Angle', value: 'Tilted towards you' },
      ],
    },
    {
      group: 'Software',
      rows: [
        { label: 'Desktop app', value: 'Kivori Desktop' },
        { label: 'Works with', value: 'Windows 10 and 11 · macOS coming later' },
        { label: 'Privacy', value: 'Works offline, no account, no telemetry' },
      ],
    },
  ],
  compatibility: [
    { os: 'Windows', status: 'supported-beta' },
    { os: 'macOS', status: 'later' },
    { os: 'Linux', status: 'not-planned' },
  ],
  faq: [
    {
      question: 'Does Kivori need an account or the internet?',
      answer:
        'No. Kivori works fully offline with no account, no cloud and no telemetry. Your settings and the activity list stay on your computer.',
    },
    {
      question: 'Which computers does it work with?',
      answer:
        'The beta is for Windows. macOS support comes after the beta. Linux is not planned until there is demand.',
    },
    {
      question: 'How does it connect?',
      answer:
        'With a USB-C data cable, which also powers it. Kivori finds the device by itself, so there is no port to choose. Some USB-C cables only carry power; use a data cable.',
    },
    {
      question: 'How do I know an action really worked?',
      answer:
        'The screen shows a badge after each action. Green is Confirmed (Kivori read the new state back), blue is Started, amber is Unverified (sent, but the computer cannot confirm it, as with shortcuts and media keys) and red is Error.',
    },
    {
      question: 'What if the app crashes or an update fails?',
      answer:
        'If the app stops, the screen shows Offline within about 4 seconds. Holding the knob for about 10 seconds restarts the device without the app. After a failed update, the app can always restore the device.',
    },
    {
      question: 'How much is it and when does it ship?',
      answer:
        'There is no price or ship date yet. Join the waitlist and we will tell you when there is.',
    },
  ],
};
