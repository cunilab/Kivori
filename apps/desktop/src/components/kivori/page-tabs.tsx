import { useState } from 'react';
import type { ReactElement, ReactNode } from 'react';
import type { LucideIcon } from 'lucide-react';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@kivori/ui/components/tabs';

export interface PageTab {
  value: string;
  label: string;
  Icon?: LucideIcon;
  content: ReactNode;
}

// The tab each page last showed, kept while the app runs so coming back lands in the same place.
const remembered = new Map<string, string>();

/** For tests: forget every page's remembered tab. */
export function resetPageTabs(): void {
  remembered.clear();
}

/**
 * Splits a long page into tabs. The tab bar sticks under the app header, so a page never has to be
 * scrolled to reach another section. Every page uses this one component so the tabs look the same.
 */
export function PageTabs({
  id,
  label,
  tabs,
}: {
  /** Remembers the open tab per page, e.g. "settings". */
  id: string;
  /** Accessible name of the tab list, e.g. "Settings sections". */
  label: string;
  tabs: readonly PageTab[];
}): ReactElement | null {
  const first = tabs[0]?.value;
  const [value, setValue] = useState(() => remembered.get(id) ?? first);
  if (!first) return null;
  // A tab can disappear (e.g. developer mode turned off): fall back to the first one.
  const current = tabs.some((tab) => tab.value === value) ? value : first;
  return (
    <Tabs
      value={current}
      onValueChange={(next) => {
        const tab = String(next);
        remembered.set(id, tab);
        setValue(tab);
      }}
      className="gap-5"
    >
      <TabsList
        variant="line"
        aria-label={label}
        className="sticky top-14 z-10 -mx-4 h-11! w-auto justify-start gap-1 overflow-x-auto border-b bg-background/95 px-4 backdrop-blur-md sm:-mx-6 sm:px-6 lg:-mx-8 lg:px-8"
      >
        {tabs.map(({ value: tab, label: text, Icon }) => (
          <TabsTrigger
            key={tab}
            value={tab}
            className="flex-none px-3 after:bg-primary data-active:text-foreground"
          >
            {Icon ? <Icon aria-hidden="true" /> : null}
            {text}
          </TabsTrigger>
        ))}
      </TabsList>
      {tabs.map(({ value: tab, content }) => (
        <TabsContent key={tab} value={tab} className="flex flex-col gap-6">
          {content}
        </TabsContent>
      ))}
    </Tabs>
  );
}
