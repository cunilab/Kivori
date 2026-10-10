'use client';

import type { ReactElement, ReactNode } from 'react';
import { Button } from '@kivori/ui/components/button';
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from '@kivori/ui/components/sheet';

interface BuyBarProps {
  name: string;
  price?: string | undefined;
  buyUrl?: string | undefined;
  /** The waitlist form, server-rendered by the page with the edition preselected. */
  waitlist: ReactNode;
}

/** Sticky bar under the site nav: name, price or "Coming soon", Buy, and Notify me (opens the waitlist). */
export function BuyBar({ name, price, buyUrl, waitlist }: BuyBarProps): ReactElement {
  return (
    <div className="sticky top-14 z-40 border-b border-border/70 bg-background/85 backdrop-blur-xl">
      <div className="mx-auto flex h-14 max-w-[1200px] items-center gap-3 px-5 sm:px-8">
        <p className="font-display text-lg font-bold tracking-tight">{name}</p>
        <p className="pixel hidden text-[0.7rem] text-muted-foreground sm:block">
          {price ?? 'Coming soon'}
        </p>
        <div className="ml-auto flex items-center gap-2">
          {buyUrl ? (
            <Button
              size="sm"
              className="keycap h-8 px-4"
              nativeButton={false}
              render={<a href={buyUrl} />}
            >
              Buy
            </Button>
          ) : (
            <Button size="sm" variant="secondary" className="h-8 rounded-full px-4" disabled>
              Buy &middot; Coming soon
            </Button>
          )}
          <Sheet>
            <SheetTrigger render={<Button size="sm" className="keycap h-8 px-4" />}>
              Notify me
            </SheetTrigger>
            <SheetContent side="right" className="overflow-y-auto sm:max-w-md">
              <SheetHeader>
                <SheetTitle className="font-display text-2xl font-bold tracking-tight">
                  Be the first to know.
                </SheetTitle>
                <SheetDescription>
                  Leave your email and we will tell you when {name} is ready.
                </SheetDescription>
              </SheetHeader>
              <div className="px-4 pb-6">{waitlist}</div>
            </SheetContent>
          </Sheet>
        </div>
      </div>
    </div>
  );
}
