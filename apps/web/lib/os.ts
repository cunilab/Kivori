export type DetectedOs = 'windows' | 'macos';

/**
 * Picks the OS to show from `navigator.userAgentData?.platform` (preferred) or the user agent.
 * Only Windows installers exist, so anything that is not recognisably a Mac maps to Windows.
 */
export function detectOs(platform: string | undefined, userAgent: string | undefined): DetectedOs {
  const hint = `${platform ?? ''} ${userAgent ?? ''}`;
  if (/iphone|ipad|ipod|android|cros/i.test(hint)) return 'windows';
  if (/mac/i.test(platform ?? '') || (!platform && /macintosh|mac os x/i.test(userAgent ?? ''))) {
    return 'macos';
  }
  return 'windows';
}
