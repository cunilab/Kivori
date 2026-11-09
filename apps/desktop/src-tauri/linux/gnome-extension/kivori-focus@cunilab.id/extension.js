import Shell from 'gi://Shell';
import { Extension } from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import { FocusBridge } from './bridge.js';

/**
 * Protected: the screen is locked or locking, or the Shell holds a modal grab. Shell-drawn
 * password prompts (polkit, keyring, Wi-Fi/VPN secrets) are modal grabs, not windows: the app
 * behind them is still `focus_window`, so that app must never receive shortcuts or typed macros.
 * Menus, the overview and the Alt+Tab switcher are grabs too and count, which only ever
 * suspends shortcuts briefly.
 */
function isProtected() {
  const shield = Main.screenShield;
  return Boolean(
    shield?.locked || shield?.active || Main.sessionMode?.isLocked || Main.modalCount > 0,
  );
}

// No app identity while protected, and never a window title.
function readFocus() {
  if (isProtected()) return ['', '', 0, true];
  const window = global.display.focus_window;
  if (!window) return ['', '', 0, false];
  const app = Shell.WindowTracker.get_default().get_window_app(window);
  return [app?.get_id() ?? '', window.get_wm_class() ?? '', Math.max(0, window.get_pid()), false];
}

export default class KivoriFocus extends Extension {
  enable() {
    this._bridge = new FocusBridge(readFocus);
    this._bridge.start();
    const changed = () => this._bridge.notify();
    // A modal grab moves key focus and a lock changes the shield; Kivori also re-reads every
    // second, so a missed signal costs at most that long.
    this._connections = [
      [global.display, 'notify::focus-window'],
      [global.stage, 'notify::key-focus'],
      [Main.layoutManager, 'system-modal-opened'],
      [Main.sessionMode, 'updated'],
      ...(Main.screenShield
        ? [
            [Main.screenShield, 'locked-changed'],
            [Main.screenShield, 'active-changed'],
          ]
        : []),
    ].map(([object, name]) => [object, object.connect(name, changed)]);
  }

  disable() {
    for (const [object, id] of this._connections ?? []) object.disconnect(id);
    this._connections = null;
    this._bridge?.stop();
    this._bridge = null;
  }
}
