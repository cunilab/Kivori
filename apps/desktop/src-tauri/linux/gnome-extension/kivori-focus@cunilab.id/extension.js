import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';
import { Extension } from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const BUS = 'id.cunilab.KivoriFocus';
const PATH = '/id/cunilab/KivoriFocus';
const XML = `<node><interface name="${BUS}">
  <method name="GetFocusedApp">
    <arg name="appId" type="s" direction="out"/>
    <arg name="wmClass" type="s" direction="out"/>
    <arg name="pid" type="u" direction="out"/>
    <arg name="locked" type="b" direction="out"/>
  </method>
  <signal name="FocusChanged">
    <arg name="appId" type="s"/><arg name="wmClass" type="s"/>
    <arg name="pid" type="u"/><arg name="locked" type="b"/>
  </signal>
</interface></node>`;

export default class KivoriFocus extends Extension {
  enable() {
    this._object = Gio.DBusExportedObject.wrapJSObject(XML, this);
    this._object.export(Gio.DBus.session, PATH);
    this._owner = Gio.bus_own_name_on_connection(
      Gio.DBus.session,
      BUS,
      Gio.BusNameOwnerFlags.NONE,
      null,
      null,
    );
    this._focusSignal = global.display.connect('notify::focus-window', () => this._changed());
    // Kept alive on unlock-dialog solely to report Protected; no app identity while locked.
    this._lockSignal = Main.screenShield.connect('locked-changed', () => this._changed());
  }

  GetFocusedApp() {
    if (Main.screenShield.locked) return ['', '', 0, true];
    const window = global.display.focus_window;
    if (!window) return ['', '', 0, false];
    const app = Shell.WindowTracker.get_default().get_window_app(window);
    return [app?.get_id() ?? '', window.get_wm_class() ?? '', Math.max(0, window.get_pid()), false];
  }

  _changed() {
    this._object.emit_signal('FocusChanged', new GLib.Variant('(ssub)', this.GetFocusedApp()));
  }

  disable() {
    if (this._focusSignal) global.display.disconnect(this._focusSignal);
    if (this._lockSignal) Main.screenShield.disconnect(this._lockSignal);
    this._focusSignal = null;
    this._lockSignal = null;
    this._object?.unexport();
    this._object = null;
    if (this._owner) Gio.bus_unown_name(this._owner);
    this._owner = null;
  }
}
