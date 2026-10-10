/** Native rejections arrive as plain strings; show them verbatim. */
export function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
