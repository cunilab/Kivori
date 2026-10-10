import type { Product } from './types';

// Every fact comes from the repo (README.md, docs/product.md, docs/user-guide.md, docs/bom.md,
// hardware/enclosure/README.md). Unknowns are written as "TBC" instead of guessed.
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
  gallery: [
    {
      src: '/generated/soft-arc-faceplate-blueprint-1600.webp',
      width: 1600,
      height: 1174,
      alt: 'Blueprint of the Kivori faceplate: the 48 mm knob, the square display opening and the three button openings, with dimensions.',
      caption:
        'Faceplate layout from the early blueprint, drawn at 100 mm wide. The current prototype is 110 mm wide so the knob clears the left button.',
    },
    {
      src: '/generated/image-1600.webp',
      width: 1600,
      height: 1200,
      alt: 'Five-view blueprint of the Kivori enclosure: front, right side, back, top back and top side.',
      caption:
        'Five views of the enclosure blueprint: the wedge profile (30 mm at the far edge, 18 mm at the near edge) and the USB-C opening.',
    },
  ],
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
      title: 'BOOT recovery',
      body: 'Hold the knob for about 10 seconds to restart Kivori even if the app is closed. If a firmware update fails, the BOOT button restores the device in three steps.',
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
        { label: 'Rotary encoder', value: 'HW-040 with push switch (beta part)' },
        { label: 'Knob', value: '48 mm' },
        { label: 'Buttons', value: '3 contextual buttons, Press and Hold each' },
        { label: 'Recovery', value: 'Hold the knob about 10 s to reboot' },
      ],
    },
    {
      group: 'Display',
      rows: [
        { label: 'Panel', value: '1.3" ST7789, 240 × 240' },
        { label: 'Views', value: 'Buddy, Clock, Volume, Media, System' },
      ],
    },
    {
      group: 'Electronics',
      rows: [
        { label: 'MCU', value: 'ESP32-C3' },
        { label: 'Connection', value: 'USB-C: power, flashing and data' },
        { label: 'Not included', value: 'Buzzer, haptics, light sensor' },
      ],
    },
    {
      group: 'Enclosure',
      rows: [
        { label: 'Size', value: '110 × 70 mm wedge' },
        { label: 'Thickness', value: '18 mm near edge, 30 mm far edge' },
        { label: 'Face tilt', value: 'About 10° towards you' },
        { label: 'Build', value: 'Printed shell, snap-fit, no screws (prototype v3)' },
        { label: 'Weight', value: 'TBC' },
      ],
    },
    {
      group: 'Software',
      rows: [
        { label: 'Desktop app', value: 'Kivori Desktop' },
        { label: 'Privacy', value: 'Offline, no account, no telemetry' },
        { label: 'Configuration', value: 'Stored on your computer, per user' },
      ],
    },
  ],
  compatibility: [
    { os: 'Windows', status: 'supported-beta' },
    { os: 'macOS', status: 'later' },
    { os: 'Linux', status: 'not-planned' },
  ],
  inTheBox: [
    'Kivori unit (knob, three buttons, screen): TBC',
    'USB-C data cable: TBC',
    'Installer download link or USB stick: TBC',
    'Quick-start card: TBC',
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
      question: 'What if the app crashes or a firmware update fails?',
      answer:
        'If the app stops, the screen shows Offline within about 4 seconds. Holding the knob for about 10 seconds restarts the device without the app. After a failed update, the BOOT button restores it in three steps.',
    },
    {
      question: 'What is in the box?',
      answer: 'To be confirmed. The contents of the beta kit are not final yet.',
    },
    {
      question: 'How much is it and when does it ship?',
      answer:
        'There is no price or ship date yet. Join the waitlist and we will tell you when there is.',
    },
  ],
};
