import { useEffect, useRef, useState } from 'react';
import type { ReactElement } from 'react';
import { toast } from 'sonner';
import {
  Activity,
  Cpu,
  FlaskConical,
  Gamepad2,
  House,
  LayoutGrid,
  Monitor,
  Moon,
  Sun,
  type LucideIcon,
} from 'lucide-react';
import { Badge } from './components/ui/badge';
import { Button } from './components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from './components/ui/dropdown-menu';
import { Separator } from './components/ui/separator';
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
} from './components/ui/sidebar';
import { Toaster } from './components/ui/sonner';
import { TooltipProvider } from './components/ui/tooltip';
import { ActivityPage } from './features/activity/ActivityPage';
import { ControlsPage } from './features/controls/ControlsPage';
import { DevicePage } from './features/device/DevicePage';
import { DeviceStudio } from './features/device-studio/DeviceStudio';
import { DisplayPage } from './features/display/DisplayPage';
import { HomePage } from './features/home/HomePage';
import {
  uiConnection,
  useAppInfo,
  useConnectionStatus,
  useDeskStatus,
  type UiConnection,
} from './hooks/use-kivori';
import { brandIconUrl } from './lib/brand';
import { useDevMode } from './lib/dev-mode';
import { format, strings } from './lib/i18n/strings';
import { setTheme, useTheme, type ThemeChoice } from './lib/theme';
import { cn } from './lib/utils';

// The Device Studio route is served in dev builds only (FR-028). The backend independently gates its
// dev-only IPC commands behind the `device-studio` Cargo feature, so both layers must agree.
const DEVICE_STUDIO_BUILD = import.meta.env.DEV;

type PageId = 'home' | 'controls' | 'display' | 'activity' | 'device' | 'studio';

const nav = strings.navigation;
const PRIMARY: { id: PageId; label: string; Icon: LucideIcon }[] = [
  { id: 'home', label: nav.home, Icon: House },
  { id: 'controls', label: nav.controls, Icon: Gamepad2 },
  { id: 'display', label: nav.display, Icon: LayoutGrid },
];
const SECONDARY: { id: PageId; label: string; Icon: LucideIcon }[] = [
  { id: 'activity', label: nav.activity, Icon: Activity },
  { id: 'device', label: nav.device, Icon: Cpu },
];

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
  const [page, setPage] = useState<PageId>('home');
  const connection = useConnectionStatus();
  const desk = useDeskStatus();
  const appInfo = useAppInfo();
  const ui = uiConnection(connection);
  useConnectionToasts(ui);

  const devMode = useDevMode();
  // Developer mode never reveals Device Studio in a production build: the build gate comes first.
  const showStudio = DEVICE_STUDIO_BUILD && devMode && (appInfo?.deviceStudioEnabled ?? false);
  const hidden = (id: PageId): boolean =>
    (id === 'studio' && !showStudio) || (id === 'activity' && !devMode);
  const current = hidden(page) ? 'home' : page;
  // A page that just became hidden (developer mode turned off) lands on Home for good.
  useEffect(() => {
    if (current !== page) setPage(current);
  }, [current, page]);

  const go = (next: PageId): void => {
    setPage(next);
    // Move focus to the new page title so keyboard and screen-reader users land in context.
    requestAnimationFrame(() =>
      document.querySelector<HTMLElement>('[data-page-title]')?.focus({ preventScroll: true }),
    );
  };

  const item = ({ id, label, Icon }: { id: PageId; label: string; Icon: LucideIcon }) => (
    <SidebarMenuItem key={id}>
      <SidebarMenuButton
        isActive={current === id}
        aria-current={current === id ? 'page' : undefined}
        tooltip={label}
        onClick={() => go(id)}
      >
        <Icon aria-hidden="true" />
        <span>{label}</span>
      </SidebarMenuButton>
    </SidebarMenuItem>
  );

  const allItems = [...PRIMARY, ...SECONDARY];
  const title =
    current === 'studio'
      ? nav.studio
      : (allItems.find((entry) => entry.id === current)?.label ?? '');

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
                  <SidebarMenu>{SECONDARY.filter(({ id }) => !hidden(id)).map(item)}</SidebarMenu>
                </SidebarGroupContent>
              </SidebarGroup>
              {showStudio ? (
                <SidebarGroup>
                  <SidebarGroupLabel>{nav.groups.developer}</SidebarGroupLabel>
                  <SidebarGroupContent>
                    <SidebarMenu>
                      {item({ id: 'studio', label: nav.studio, Icon: FlaskConical })}
                    </SidebarMenu>
                  </SidebarGroupContent>
                </SidebarGroup>
              ) : null}
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
          <header className="sticky top-0 z-20 flex h-14 shrink-0 items-center gap-2 border-b bg-background/80 px-4 backdrop-blur supports-[backdrop-filter]:bg-background/70">
            <SidebarTrigger aria-label={nav.toggle} className="-ml-1" />
            <Separator orientation="vertical" className="mr-1 h-4" />
            <span className="text-sm font-medium" aria-hidden="true">
              {title}
            </span>
            <div className="ml-auto flex items-center gap-2">
              <ConnectionPill ui={ui} />
              <ThemeMenu />
            </div>
          </header>
          <div className="flex-1">
            {current === 'home' ? (
              <HomePage connection={connection} desk={desk} onNavigate={go} />
            ) : current === 'controls' ? (
              <ControlsPage desk={desk} />
            ) : current === 'display' ? (
              <DisplayPage desk={desk} connection={connection} />
            ) : current === 'activity' ? (
              <ActivityPage />
            ) : current === 'device' ? (
              <DevicePage connection={connection} appInfo={appInfo} />
            ) : showStudio ? (
              <DeviceStudio />
            ) : null}
          </div>
        </SidebarInset>
      </SidebarProvider>
      <Toaster position="bottom-right" richColors closeButton />
    </TooltipProvider>
  );
}
