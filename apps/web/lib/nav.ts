export interface NavLink {
  href: string;
  label: string;
}

export const MAIN_NAV: readonly NavLink[] = [
  { href: '/', label: 'Overview' },
  { href: '/products/kivori#specs', label: 'Specs' },
  { href: '/download', label: 'Download' },
  { href: '/support', label: 'Support' },
];

export const FOOTER_NAV: readonly { title: string; links: readonly NavLink[] }[] = [
  {
    title: 'Product',
    links: [
      { href: '/', label: 'Overview' },
      { href: '/products', label: 'Lineup' },
      { href: '/products/kivori#specs', label: 'Specs' },
      { href: '/download', label: 'Download' },
    ],
  },
  {
    title: 'Help',
    links: [
      { href: '/support', label: 'Support' },
      { href: '/privacy', label: 'Privacy' },
    ],
  },
];
