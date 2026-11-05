'use client';

import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { MenuIcon } from 'lucide-react';
import { useState, type ReactElement } from 'react';
import { Button } from '@kivori/ui/components/button';
import {
  NavigationMenu,
  NavigationMenuItem,
  NavigationMenuLink,
  NavigationMenuList,
} from '@kivori/ui/components/navigation-menu';
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from '@kivori/ui/components/sheet';
import { MAIN_NAV } from '@/lib/nav';
import { ThemeMenu } from '@/components/theme-menu';

export function SiteNav(): ReactElement {
  const pathname = usePathname();
  const [open, setOpen] = useState(false);
  const isActive = (href: string): boolean => {
    const path = href.split('#')[0] ?? href;
    return !href.includes('#') && (path === '/' ? pathname === '/' : pathname.startsWith(path));
  };

  return (
    <div className="flex items-center gap-7">
      <NavigationMenu aria-label="Main" className="hidden md:flex">
        <NavigationMenuList className="gap-1">
          {MAIN_NAV.map((item) => (
            <NavigationMenuItem key={item.href}>
              <NavigationMenuLink
                active={isActive(item.href)}
                render={<Link href={item.href} />}
                className="rounded-md px-3.5 py-1.5 text-[0.8125rem] text-muted-foreground hover:text-foreground data-active:bg-transparent data-active:text-foreground"
              >
                {item.label}
              </NavigationMenuLink>
            </NavigationMenuItem>
          ))}
        </NavigationMenuList>
      </NavigationMenu>

      <div className="flex items-center justify-end gap-1.5">
        <Button
          size="sm"
          variant="outline"
          className="hidden h-10 rounded-md border-border px-4 text-sm sm:inline-flex"
          nativeButton={false}
          render={<Link href="/#waitlist" />}
        >
          Get updates
        </Button>
        <ThemeMenu />
        <Sheet open={open} onOpenChange={setOpen}>
          <SheetTrigger
            render={
              <Button
                variant="ghost"
                size="icon"
                className="rounded-md md:hidden"
                aria-label="Open menu"
              />
            }
          >
            <MenuIcon />
          </SheetTrigger>
          <SheetContent side="right" className="md:hidden">
            <SheetHeader>
              <SheetTitle className="font-display text-xl">Kivori</SheetTitle>
              <SheetDescription className="sr-only">Site navigation</SheetDescription>
            </SheetHeader>
            <nav aria-label="Mobile" className="flex flex-col px-4">
              {MAIN_NAV.map((item) => (
                <Link
                  key={item.href}
                  href={item.href}
                  onClick={() => setOpen(false)}
                  className="border-b border-border py-4 font-display text-2xl font-semibold tracking-tight"
                >
                  {item.label}
                </Link>
              ))}
              <Link
                href="/#waitlist"
                onClick={() => setOpen(false)}
                className="border-b border-border py-4 text-2xl font-semibold tracking-tight sm:hidden"
              >
                Get updates
              </Link>
            </nav>
          </SheetContent>
        </Sheet>
      </div>
    </div>
  );
}
