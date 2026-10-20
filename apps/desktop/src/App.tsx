import { useEffect, useRef, useState } from 'react';
import type { ReactElement } from 'react';
import { toast } from 'sonner';
import {
  Cpu,
  Gamepad2,
  House,
  LayoutGrid,
  Monitor,
  Moon,
  Sun,
  Wrench,
  type LucideIcon,
} from 'lucide-react';
import { Badge } from '@kivori/ui/components/badge';
import { Button } from '@kivori/ui/components/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from '@kivori/ui/components/dropdown-menu';
import { Separator } from '@kivori/ui/components/separator';
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
  SidebarRail,
  SidebarTrigger,
  useSidebar,
} from '@kivori/ui/components/sidebar';
import { Toaster } from '@kivori/ui/components/sonner';
import { TooltipProvider } from '@kivori/ui/components/tooltip';
import { ControlsPage } from './features/controls/ControlsPage';
import { DeveloperPage } from './features/developer/DeveloperPage';
import { DevicePage } from './features/device/DevicePage';
import { DisplayPage } from './features/display/DisplayPage';
import { migrateLegacyBuddy } from './features/display/legacy-buddy';
import { HomePage } from './features/home/HomePage';
import { Onboarding } from './features/onboarding/Onboarding';
import { FirmwareOfferDialog } from './features/updates/FirmwareOfferDialog';
import { UpdateDialog } from './features/updates/UpdateDialog';
import { useOnboarding } from './features/onboarding/use-onboarding';
import {
  uiConnection,
  useAppInfo,
  useConnectionStatus,
  useConfig,
  useDeskStatus,
  useUpdateAvailable,
  type UiConnection,
} from './hooks/use-kivori';
import { brandIconUrl } from './lib/brand';
import { appReady } from './lib/ipc';
import { useDevMode } from './lib/dev-mode';
import { format, strings } from './lib/i18n/strings';
import { setTheme, useTheme, type ThemeChoice } from './lib/theme';
import { cn } from '@kivori/ui/lib/utils';

type PageId = 'home' | 'controls' | 'display' | 'developer' | 'device';

const nav = strings.navigation;
const PRIMARY: { id: PageId; label: string; Icon: LucideIcon }[] = [
  { id: 'home', label: nav.home, Icon: House },
  { id: 'controls', label: nav.controls, Icon: Gamepad2 },
  { id: 'display', label: nav.display, Icon: LayoutGrid },
];
const SECONDARY: { id: PageId; label: string; Icon: LucideIcon }[] = [
  { id: 'device', label: nav.device, Icon: Cpu },
  { id: 'developer', label: nav.developer, Icon: Wrench },
];

function NavigationItem({
  entry: { id, label, Icon },
  page,
  onNavigate,
}: {
  entry: (typeof PRIMARY)[number];
  page: PageId;
  onNavigate: (page: PageId) => void;
}): ReactElement {
  const { setOpenMobile } = useSidebar();
  return (
    <SidebarMenuItem>
      <SidebarMenuButton
        isActive={page === id}
        aria-current={page === id ? 'page' : undefined}
        tooltip={label}
        onClick={() => {
          setOpenMobile(false);
          onNavigate(id);
        }}
      >
        <Icon aria-hidden="true" />
        <span>{label}</span>
      </SidebarMenuButton>
    </SidebarMenuItem>
  );
}

const DOT: Record<UiConnection, string> = {
  loading: 'bg-muted-foreground/40',
  disconnected: 'bg-muted-foreground/60',
  connecting: 'bg-warning motion-safe:animate-pulse',
  reconnecting: 'bg-warning motion-safe:animate-pulse',
  connected: 'bg-success',
  incompatible: 'bg-destructive',
};

function ConnectionPill({ ui }: { ui: UiConnection }): ReactElement | null {
  if (ui === 'loading') return null;
  return (
    <Badge variant="outline" className="h-7 gap-2 rounded-full px-3" data-state={ui}>
      <span aria-hidden="true" className={cn('size-2 rounded-full', DOT[ui])} />
      {strings.connection.status[ui]}
    </Badge>
  );
}

function ThemeMenu(): ReactElement {
  const { theme, resolved } = useTheme();
  const Icon = resolved === 'dark' ? Moon : Sun;
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={<Button variant="ghost" size="icon" aria-label={strings.theme.label} />}
      >
        <Icon aria-hidden="true" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-40">
        <DropdownMenuGroup>
          <DropdownMenuLabel>{strings.theme.label}</DropdownMenuLabel>
          <DropdownMenuRadioGroup
            value={theme}
            onValueChange={(value) => setTheme(value as ThemeChoice)}
          >
            <DropdownMenuRadioItem value="light">
              <Sun aria-hidden="true" />
              {strings.theme.light}
            </DropdownMenuRadioItem>
            <DropdownMenuRadioItem value="dark">
              <Moon aria-hidden="true" />
              {strings.theme.dark}
            </DropdownMenuRadioItem>
            <DropdownMenuRadioItem value="system">
              <Monitor aria-hidden="true" />
              {strings.theme.system}
            </DropdownMenuRadioItem>
          </DropdownMenuRadioGroup>
        </DropdownMenuGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function BrandMark(): ReactElement {
  // The icon's own tile has a transparent margin, so it sits a little larger than a 32 px box.
  return (
    <img
      src={brandIconUrl}
      alt=""
      data-testid="brand-mark"
      draggable={false}
      className="-m-1 size-10 shrink-0 select-none"
    />
  );
}

/** Announces real connection transitions (never the first snapshot) as toasts. */
function useConnectionToasts(ui: UiConnection): void {
  const previous = useRef<UiConnection>('loading');
  useEffect(() => {
    const before = previous.current;
    previous.current = ui;
    if (before === 'loading' || before === ui) return;
    if (ui === 'connected') toast.success(strings.connection.toastConnected);
    else if (before === 'connected') toast.warning(strings.connection.toastLost);
  }, [ui]);
}

export function App(): ReactElement {
  const { resolved: resolvedTheme } = useTheme();
  const [page, setPage] = useState<PageId>('home');
  const connection = useConnectionStatus();
  const desk = useDeskStatus();
  const updateAvailable = useUpdateAvailable(connection);
  const config = useConfig();
  // Buddy settings used to live in localStorage; carry them over once the config is known.
  const legacyChecked = useRef(false);
  useEffect(() => {
    if (!config || legacyChecked.current) return;
    legacyChecked.current = true;
    migrateLegacyBuddy(config);
  }, [config]);
  const appInfo = useAppInfo();
  // `null` (not answered yet) shows the normal layout: the native answer takes a few milliseconds.
  const onboarding = useOnboarding();
  const ui = uiConnection(connection);
  useConnectionToasts(ui);

  // The first native answer is in: the page has real content, so the core may show the window.
  const reportedReady = useRef(false);
  useEffect(() => {
    if (ui === 'loading' || reportedReady.current) return;
    reportedReady.current = true;
    void appReady();
  }, [ui]);

  const devMode = useDevMode();
  const lastFocused = useRef<HTMLElement | null>(null);
  useEffect(() => {
    const remember = (event: FocusEvent): void => {
      if (event.target instanceof HTMLElement) lastFocused.current = event.target;
    };
    document.addEventListener('focusin', remember);
    return () => document.removeEventListener('focusin', remember);
  }, []);
  const supportKey = `${connection?.connection}:${connection?.connectionGeneration}:${connection?.device?.identity?.known}:${connection?.capabilities.join(',')}:${devMode}:${appInfo?.deviceStudioEnabled}`;
  useEffect(() => {
    if (
      lastFocused.current &&
      !lastFocused.current.isConnected &&
      document.activeElement === document.body
    ) {
      document.querySelector<HTMLElement>('[data-page-title]')?.focus({ preventScroll: true });
    }
  }, [supportKey]);

  const go = (next: PageId): void => {
    setPage(next);
    // A new page starts at its top, not at the previous page's scroll position.
    window.scrollTo({ top: 0 });
    // Move focus to the new page title so keyboard and screen-reader users land in context.
    requestAnimationFrame(() =>
      document.querySelector<HTMLElement>('[data-page-title]')?.focus({ preventScroll: true }),
    );
  };

  const item = (entry: (typeof PRIMARY)[number]) => (
    <NavigationItem key={entry.id} entry={entry} page={page} onNavigate={go} />
  );

  const allItems = [...PRIMARY, ...SECONDARY];
  const title = allItems.find((entry) => entry.id === page)?.label ?? '';

  if (onboarding.completed === false) {
    return (
      <TooltipProvider>
        <Onboarding
          connection={connection}
          config={config}
          updateAvailable={updateAvailable}
          onComplete={onboarding.complete}
        />
        <UpdateDialog />
        <FirmwareOfferDialog />
        <Toaster theme={resolvedTheme} position="bottom-right" richColors closeButton />
      </TooltipProvider>
    );
  }

  return (
    <TooltipProvider>
      <SidebarProvider>
        <Sidebar collapsible="icon">
          <SidebarHeader>
            <div className="flex items-center gap-2.5 px-1 py-1.5">
              <BrandMark />
              <div className="grid leading-tight group-data-[collapsible=icon]:hidden">
                <span className="font-semibold tracking-tight">{strings.appTitle}</span>
                <span className="text-xs text-muted-foreground">{strings.tagline}</span>
              </div>
            </div>
          </SidebarHeader>
          <SidebarContent>
            <nav aria-label={nav.label}>
              <SidebarGroup>
                <SidebarGroupLabel>{nav.groups.device}</SidebarGroupLabel>
                <SidebarGroupContent>
                  <SidebarMenu>{PRIMARY.map(item)}</SidebarMenu>
                </SidebarGroupContent>
              </SidebarGroup>
              <SidebarGroup>
                <SidebarGroupLabel>{nav.groups.system}</SidebarGroupLabel>
                <SidebarGroupContent>
                  <SidebarMenu>{SECONDARY.map(item)}</SidebarMenu>
                </SidebarGroupContent>
              </SidebarGroup>
            </nav>
          </SidebarContent>
          <SidebarFooter>
            {appInfo ? (
              <p className="px-2 pb-1 text-xs text-muted-foreground group-data-[collapsible=icon]:hidden">
                {format(nav.version, { version: appInfo.appVersion })}
              </p>
            ) : null}
          </SidebarFooter>
          <SidebarRail />
        </Sidebar>

        <SidebarInset>
          <header className="sticky top-0 z-20 flex h-14 shrink-0 items-center gap-2 border-b bg-background/95 px-4 backdrop-blur-md supports-[backdrop-filter]:bg-background/85">
            <SidebarTrigger aria-label={nav.toggle} className="-ml-1" />
            <Separator
              orientation="vertical"
              className="mr-1 data-vertical:h-4 data-vertical:self-center"
            />
            <span className="text-sm font-medium" aria-hidden="true">
              {title}
            </span>
            <div className="ml-auto flex items-center gap-2">
              <ConnectionPill ui={ui} />
              <ThemeMenu />
            </div>
          </header>
          <div className="flex-1">
            {page === 'home' ? (
              <HomePage
                connection={connection}
                desk={desk}
                updateAvailable={updateAvailable}
                onNavigate={go}
              />
            ) : page === 'controls' ? (
              <ControlsPage desk={desk} config={config} connection={connection} />
            ) : page === 'display' ? (
              <DisplayPage desk={desk} config={config} connection={connection} />
            ) : page === 'developer' ? (
              <DeveloperPage connection={connection} appInfo={appInfo} />
            ) : page === 'device' ? (
              <DevicePage
                connection={connection}
                appInfo={appInfo}
                onRunSetupAgain={() => {
                  onboarding.restart().catch(() => toast.error(strings.onboarding.runAgain.failed));
                }}
              />
            ) : null}
          </div>
        </SidebarInset>
      </SidebarProvider>
      <UpdateDialog />
      <FirmwareOfferDialog />
      <Toaster theme={resolvedTheme} position="bottom-right" richColors closeButton />
    </TooltipProvider>
  );
}
