import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

export const BUS = 'id.cunilab.KivoriFocus';
export const PATH = '/id/cunilab/KivoriFocus';
// Bump together with metadata.json "version" on every behaviour or D-Bus change. Kivori trusts a
// running copy only when its Version is at least the one it embeds, and installs an update
// otherwise. Keep newer versions backward compatible with Subscribe and FocusChanged.
export const VERSION = 2;
const MAX_SUBSCRIBERS = 8;

const XML = `<node><interface name="${BUS}">
  <property name="Version" type="u" access="read"/>
  <method name="Subscribe">
    <arg name="appId" type="s" direction="out"/>
    <arg name="wmClass" type="s" direction="out"/>
    <arg name="pid" type="u" direction="out"/>
    <arg name="protected" type="b" direction="out"/>
  </method>
  <method name="Unsubscribe"/>
  <signal name="FocusChanged">
    <arg name="appId" type="s"/><arg name="wmClass" type="s"/>
    <arg name="pid" type="u"/><arg name="protected" type="b"/>
  </signal>
</interface></node>`;

// Fail closed: a state that cannot be read is reported as protected, never as the last app.
const UNREADABLE = ['', '', 0, true];

/**
 * The D-Bus half, free of Shell internals. Focus is never queryable or broadcast: a caller must
 * Subscribe, which returns the current state, and changes are then sent only to that caller's
 * unique name. `read()` returns [appId, wmClass, pid, protected].
 */
export class FocusBridge {
  constructor(read, connection = Gio.DBus.session) {
    this._read = read;
    this._connection = connection;
    this._subscribers = new Map();
    this._last = null;
  }

  get Version() {
    return VERSION;
  }

  start() {
    this._object = Gio.DBusExportedObject.wrapJSObject(XML, this);
    this._object.export(this._connection, PATH);
    this._owner = Gio.bus_own_name_on_connection(
      this._connection,
      BUS,
      Gio.BusNameOwnerFlags.NONE,
      null,
      null,
    );
  }

  _state() {
    try {
      return this._read();
    } catch (error) {
      console.error(`Kivori focus: ${error}`);
      return UNREADABLE;
    }
  }

  SubscribeAsync(_params, invocation) {
    const sender = invocation.get_sender();
    const known = this._subscribers.has(sender);
    if (!sender || (!known && this._subscribers.size >= MAX_SUBSCRIBERS)) {
      invocation.return_dbus_error(
        'org.freedesktop.DBus.Error.LimitsExceeded',
        'Too many subscribers',
      );
      return;
    }
    if (!known) {
      const watch = Gio.bus_watch_name_on_connection(
        this._connection,
        sender,
        Gio.BusNameWatcherFlags.NONE,
        null,
        () => this._drop(sender),
      );
      this._subscribers.set(sender, watch);
    }
    invocation.return_value(new GLib.Variant('(ssub)', this._state()));
  }

  UnsubscribeAsync(_params, invocation) {
    this._drop(invocation.get_sender());
    invocation.return_value(null);
  }

  _drop(sender) {
    const watch = this._subscribers.get(sender);
    if (watch === undefined) return;
    this._subscribers.delete(sender);
    Gio.bus_unwatch_name(watch);
  }

  /** Call whenever focus or protection may have changed; sends only real changes. */
  notify() {
    const state = this._state();
    const key = JSON.stringify(state);
    if (key === this._last) return;
    this._last = key;
    const body = new GLib.Variant('(ssub)', state);
    for (const sender of this._subscribers.keys()) {
      try {
        this._connection.emit_signal(sender, PATH, BUS, 'FocusChanged', body);
      } catch (error) {
        console.error(`Kivori focus: ${error}`);
      }
    }
  }

  stop() {
    for (const watch of this._subscribers.values()) Gio.bus_unwatch_name(watch);
    this._subscribers.clear();
    this._object?.unexport();
    this._object = null;
    if (this._owner) Gio.bus_unown_name(this._owner);
    this._owner = null;
  }
}
