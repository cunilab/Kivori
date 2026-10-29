/** Strings support local fixtures; deployed bindings resolve through Secrets Store. */
export type TurnstileValue = string | { get(): Promise<string> };

export async function readTurnstileValue(
  value: TurnstileValue | undefined,
): Promise<string | undefined> {
  if (value === undefined) return undefined;
  const resolved = typeof value === 'string' ? value : await value.get();
  if (typeof resolved !== 'string' || !resolved.trim()) {
    throw new Error('Turnstile configuration unavailable');
  }
  return resolved.trim();
}
