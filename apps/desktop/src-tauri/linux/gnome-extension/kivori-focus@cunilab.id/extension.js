import Gio from 'gi://Gio';
import Shell from 'gi://Shell';
import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import { Extension } from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as ModalDialog from 'resource:///org/gnome/shell/ui/modalDialog.js';
import * as Dialog from 'resource:///org/gnome/shell/ui/dialog.js';
import { FocusBridge } from './bridge.js';
import { Identity, Keyboard, isProtected, consentDescription } from './security.js';

/**
 * Protected: the screen is locked or locking, or the Shell holds a modal grab. Shell-drawn
 * password prompts (polkit, keyring, Wi-Fi/VPN secrets) are modal grabs, not windows: the app
 * behind them is still `focus_window`, so that app must never receive shortcuts or typed macros.
 * Menus, the overview and the Alt+Tab switcher are grabs too and count, which only ever
 * suspends shortcuts briefly.
 */
// No app identity while protected, and never a window title.
function readFocus() {
  if (isProtected(Main, global.stage)) return ['', '', 0, true];
  const window = global.display.focus_window;
  if (!window) return ['', '', 0, false];
  const app = Shell.WindowTracker.get_default().get_window_app(window);
  return [app?.get_id() ?? '', window.get_wm_class() ?? '', Math.max(0, window.get_pid()), false];
}

export default class KivoriFocus extends Extension {
  enable() {
    this._keyboard ??= new Keyboard(() => {
      const device = global.backend
        .get_default_seat()
        .create_virtual_device(Clutter.InputDeviceType.KEYBOARD_DEVICE);
      return {
        notify: (key, down) =>
          device.notify_keyval(
            GLib.get_monotonic_time(),
            key,
            down ? Clutter.KeyState.PRESSED : Clutter.KeyState.RELEASED,
          ),
        dispose: () => device.run_dispose(),
      };
    });
    // Retain the snapshot across disable/re-enable; disk changes need a new Shell session.
    this._identity ??= new Identity(this.path);
    this._identity.watch();
    this._bridge = new FocusBridge(
      readFocus,
      Gio.DBus.session,
      this._identity,
      (executable) => this._requestApproval(executable),
      this._keyboard,
      () => this._approval?.cancel(),
    );
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

  _requestApproval(executable) {
    return new Promise((resolve) => {
      const dialog = new ModalDialog.ModalDialog();
      let finished = false;
      const finish = (approved) => {
        if (finished) return;
        finished = true;
        this._approval = null;
        dialog.close(); // ModalDialog destroys itself after its close transition.
        resolve(approved);
      };
      this._approval = { dialog, cancel: () => finish(false) };
      dialog.contentLayout.add_child(
        new Dialog.MessageDialogContent({
          title: 'Allow Kivori to press keyboard shortcuts?',
          description: consentDescription(executable),
        }),
      );
      dialog.setButtons([
        { label: 'Deny', action: () => finish(false), key: Clutter.KEY_Escape },
        { label: 'Allow', action: () => finish(true) },
      ]);
      if (!dialog.open()) {
        finish(false);
        dialog.destroy();
      }
    });
  }

  disable() {
    for (const [object, id] of this._connections ?? []) {
      try {
        object.disconnect(id);
      } catch {
        /* An already-destroyed source must not skip cleanup. */
      }
    }
    this._connections = null;
    try {
      this._bridge?.stop();
    } finally {
      this._bridge = null;
      try {
        this._approval?.cancel();
      } finally {
        try {
          this._identity?.pause();
        } finally {
          this._keyboard?.reset();
          // Preserve uncertain releases across disable/re-enable; never silently forget held keys.
          if (!this._keyboard?.suspended) this._keyboard = null;
        }
      }
    }
  }
}
