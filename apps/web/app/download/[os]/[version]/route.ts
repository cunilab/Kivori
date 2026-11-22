import type { NextRequest } from 'next/server';
import { downloadResponse } from '@/lib/download';

export const dynamic = 'force-dynamic';

interface Context {
  params: Promise<{ os: string; version: string }>;
}

/** Streams a published installer; Linux selects .deb by default or ?format=appimage. */
export async function GET(request: NextRequest, { params }: Context): Promise<Response> {
  const { os, version } = await params;
  return downloadResponse(request, os, version);
}
