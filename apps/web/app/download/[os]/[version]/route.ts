import type { NextRequest } from 'next/server';
import { downloadResponse } from '@/lib/download';

export const dynamic = 'force-dynamic';

interface Context {
  params: Promise<{ os: string; version: string }>;
}

/** `/download/windows/latest` or `/download/windows/<semver>`: the installer, streamed (see lib/download.ts). */
export async function GET(request: NextRequest, { params }: Context): Promise<Response> {
  const { os, version } = await params;
  return downloadResponse(request, os, version);
}
