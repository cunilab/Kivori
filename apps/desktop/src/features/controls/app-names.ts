import type { StartupPlatform } from '@/lib/ipc/types';

/** Lowercased id without the `.exe` / `.app` suffix: `Chrome.exe` -> `chrome`. */
function normalize(id: string): string {
  return id
    .trim()
    .toLowerCase()
    .replace(/\.(exe|app)$/, '');
}

/** Apps the built-in profiles know, under every process name, exe name and bundle id they use. */
const KNOWN: Record<string, readonly string[]> = {
  Chrome: ['chrome', 'com.google.chrome'],
  Edge: ['msedge', 'com.microsoft.edgemac'],
  Firefox: ['firefox', 'org.mozilla.firefox'],
  Brave: ['brave', 'com.brave.browser'],
  Safari: ['safari', 'com.apple.safari'],
  Arc: ['arc', 'company.thebrowser.browser'],
  'VS Code': ['code', 'com.microsoft.vscode'],
  Cursor: ['cursor', 'com.todesktop.230313mzl4w4u92'],
  'Visual Studio': ['devenv'],
  Xcode: ['xcode', 'com.apple.dt.xcode'],
  'IntelliJ IDEA': ['idea', 'idea64', 'com.jetbrains.intellij'],
  PyCharm: ['pycharm', 'pycharm64', 'com.jetbrains.pycharm'],
  WebStorm: ['webstorm', 'webstorm64', 'com.jetbrains.webstorm'],
  GoLand: ['goland', 'goland64', 'com.jetbrains.goland'],
  CLion: ['clion', 'clion64', 'com.jetbrains.clion'],
  Rider: ['rider', 'rider64', 'com.jetbrains.rider'],
  RustRover: ['rustrover', 'rustrover64', 'com.jetbrains.rustrover'],
  Spotify: ['spotify', 'com.spotify.client'],
  Zoom: ['zoom', 'zoom.us', 'us.zoom.xos'],
  Teams: ['ms-teams', 'teams', 'com.microsoft.teams', 'com.microsoft.teams2'],
  'OBS Studio': ['obs', 'obs64', 'com.obsproject.obs-studio'],
  Figma: ['figma', 'com.figma.desktop'],
  Slack: ['slack', 'com.tinyspeck.slackmacgap'],
  Discord: ['discord', 'com.hnc.discord'],
};

const BY_ID: ReadonlyMap<string, string> = new Map(
  Object.entries(KNOWN).flatMap(([name, ids]) => ids.map((id) => [id, name] as const)),
);

/** A reverse-DNS bundle id such as `com.google.chrome` (an `.exe` name never is one). */
export function isBundleId(id: string): boolean {
  return !/\.exe$/i.test(id) && /^[a-z0-9_-]+(\.[a-z0-9_-]+){2,}$/i.test(id.trim());
}

function titleCase(text: string): string {
  return text
    .split(/[\s_-]+/)
    .filter(Boolean)
    .map((word) => word[0]!.toUpperCase() + word.slice(1))
    .join(' ');
}

/** What an app id reads as: a known name, else its last segment, de-numbered and title-cased. */
export function friendlyAppName(id: string): string {
  const key = normalize(id);
  const known = BY_ID.get(key);
  if (known) return known;
  const last = isBundleId(key) ? (key.split('.').at(-1) ?? key) : key;
  const base = last.replace(/(?<=[a-z])(64|32)$/, '');
  return titleCase(base) || id;
}

/** True when the id belongs to the other OS (an exe name on macOS, a bundle id elsewhere). */
function foreign(id: string, platform: StartupPlatform | null): boolean {
  if (platform === 'macos') return /\.exe$/i.test(id);
  if (platform === null) return false;
  return isBundleId(id);
}

/** One app as the user knows it, with every stored id (alias) that refers to it. */
export interface AppGroup {
  /** The friendly name. */
  name: string;
  /** The alias to draw the icon from and to label the row with: the first one for this OS. */
  primary: string;
  /** Every stored id of this app in the profile, hidden ones included. */
  ids: string[];
}

/** Merges the aliases of one app into a single entry; the stored ids are untouched. */
export function groupApps(ids: string[], platform: StartupPlatform | null): AppGroup[] {
  const groups = new Map<string, AppGroup>();
  for (const id of ids) {
    const name = friendlyAppName(id);
    const key = name.toLowerCase();
    const group = groups.get(key);
    if (group) group.ids.push(id);
    else groups.set(key, { name, primary: id, ids: [id] });
  }
  for (const group of groups.values()) {
    group.primary = group.ids.find((id) => !foreign(id, platform)) ?? group.ids[0]!;
  }
  return [...groups.values()];
}
