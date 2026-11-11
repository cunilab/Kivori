import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import { allowedShortcut } from './security.js';

export const BUS = 'id.cunilab.KivoriFocus';
export const PATH = '/id/cunilab/KivoriFocus';
// Bump with metadata.json and desktop gnome_focus.rs on every protocol/behavior change.
export const VERSION = 7;
const MAX_SUBSCRIBERS = 4;
const XML = `<node><interface name="${BUS}">
  <property name="Version" type="u" access="read"/>
  <method name="Subscribe">
    <arg name="appId" type="s" direction="out"/><arg name="wmClass" type="s" direction="out"/>
    <arg name="pid" type="u" direction="out"/><arg name="protected" type="b" direction="out"/>
  </method>
  <method name="Unsubscribe"/>
  <method name="Authorize"><arg name="interactive" type="b" direction="in"/><arg name="approved" type="b" direction="out"/></method>
  <method name="GetShortcutContext">
    <arg name="authorized" type="b" direction="out"/>
    <arg name="generation" type="u" direction="out"/><arg name="clock" type="t" direction="out"/>
  </method>
  <method name="SendShortcut">
    <arg name="modifiers" type="u" direction="in"/><arg name="keysym" type="u" direction="in"/>
    <arg name="deadline" type="t" direction="in"/><arg name="generation" type="u" direction="in"/>
  </method>
  <signal name="FocusChanged"><arg name="appId" type="s"/><arg name="wmClass" type="s"/>
    <arg name="pid" type="u"/><arg name="protected" type="b"/></signal>
</interface></node>`;
const UNREADABLE = ['', '', 0, true];

export class FocusBridge {
  constructor(read, connection, identity, approve, keyboard, cancelApproval = () => {}) {
    this._read = read;
    this._connection = connection;
    this._identity = identity;
    this._approve = approve;
    this._cancelApproval = cancelApproval;
    this._keyboard = keyboard;
    this._subscribers = new Map();
    this._injector = null;
    this._pending = null;
    this._stopped = false;
    this._generation = 0;
    this._last = null;
    this._lastShortcut = null;
    this._identityRevoker = () => {
      this._injector = null;
      this._pending = null;
      try {
        this._cancelApproval();
      } finally {
        this._keyboard.reset();
      }
    };
    this._identity.onChange = this._identityRevoker;
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
    let state;
    try {
      state = this._read();
    } catch {
      state = UNREADABLE;
    }
    if (
      !Array.isArray(state) ||
      state.length !== 4 ||
      typeof state[0] !== 'string' ||
      typeof state[1] !== 'string' ||
      !Number.isInteger(state[2]) ||
      state[2] < 0 ||
      state[2] > 0xffffffff ||
      typeof state[3] !== 'boolean'
    )
      state = UNREADABLE;
    const key = JSON.stringify(state);
    if (key !== this._last) {
      this._last = key;
      this._generation = (this._generation + 1) >>> 0;
    }
    return state;
  }
  _refuse(invocation, name, message) {
    invocation.return_dbus_error(`org.freedesktop.DBus.Error.${name}`, message);
  }
  async SubscribeAsync(_params, invocation) {
    const sender = invocation.get_sender();
    // Unauthenticated clients never occupy a slot (focus subscription grants no injection).
    if (!(await this._identity.resolve(sender)) || this._stopped) {
      this._drop(sender);
      this._refuse(invocation, 'AccessDenied', 'The installed Kivori executable is required');
      return;
    }
    if (!this._subscribers.has(sender)) {
      if (this._subscribers.size >= MAX_SUBSCRIBERS) {
        const oldest = [...this._subscribers.keys()].find(
          (peer) => peer !== this._injector?.sender,
        );
        if (oldest) this._drop(oldest);
      }
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
  async AuthorizeAsync([interactive], invocation) {
    const sender = invocation.get_sender();
    const fingerprint = await this._identity.resolve(sender);
    if (!fingerprint) this._drop(sender);
    if (
      !fingerprint ||
      !this._subscribers.has(sender) ||
      this._stopped ||
      this._state()[3] ||
      this._pending ||
      (this._injector && this._injector.sender !== sender)
    ) {
      this._refuse(invocation, 'AccessDenied', 'Keyboard authorization is unavailable');
      return;
    }
    const pending = { sender };
    this._pending = pending;
    let approved;
    try {
      approved =
        this._identity.approved(fingerprint) ||
        (interactive === true && (await this._approve(this._identity.executable)));
      // A disconnect, executable replacement, or disable during the dialog revokes its result.
      approved =
        approved &&
        !this._stopped &&
        this._pending === pending &&
        this._subscribers.has(sender) &&
        (await this._identity.resolve(sender)) === fingerprint;
      approved =
        approved && !this._stopped && this._pending === pending && this._subscribers.has(sender);
      if (approved) {
        this._identity.save(fingerprint);
        this._injector = { sender, fingerprint };
      }
    } catch {
      approved = false;
    }
    if (this._pending === pending) this._pending = null;
    invocation.return_value(new GLib.Variant('(b)', [Boolean(approved)]));
  }
  async _authorized(sender) {
    const injector = this._injector;
    if (this._stopped || !injector || injector.sender !== sender || !this._subscribers.has(sender))
      return false;
    const fingerprint = await this._identity.resolve(sender);
    if (this._stopped || this._injector !== injector || fingerprint !== injector.fingerprint) {
      if (this._injector === injector) this._drop(sender);
      return false;
    }
    return true;
  }
  async GetShortcutContextAsync(_params, invocation) {
    const authorized = await this._authorized(invocation.get_sender());
    this._state();
    invocation.return_value(
      new GLib.Variant('(but)', [
        authorized && !this._keyboard.suspended,
        this._generation,
        GLib.get_monotonic_time(),
      ]),
    );
  }
  async SendShortcutAsync([modifiers, keysym, deadline, generation], invocation) {
    if (!(await this._authorized(invocation.get_sender()))) {
      this._refuse(invocation, 'AccessDenied', 'Keyboard authorization is required');
      return;
    }
    if (!allowedShortcut(modifiers, keysym)) {
      this._refuse(invocation, 'InvalidArgs', 'Shortcut not allowed by the GNOME keyboard policy');
      return;
    }
    const state = this._state();
    const now = GLib.get_monotonic_time();
    // No awaits below this point: validate the target and expiry immediately before the chord.
    if (
      state[3] ||
      generation !== this._generation ||
      !Number.isSafeInteger(deadline) ||
      deadline <= now ||
      deadline - now > 100000
    ) {
      this._refuse(
        invocation,
        'AccessDenied',
        'Shortcut expired, focus changed, or desktop is protected',
      );
      return;
    }
    if (this._keyboard.modifiersHeld()) {
      invocation.return_dbus_error(`${BUS}.Error.ModifiersHeld`, 'Modifier keys are held down');
      return;
    }
    if (this._lastShortcut !== null && now - this._lastShortcut < 20000) {
      this._refuse(invocation, 'LimitsExceeded', 'Keyboard shortcuts are arriving too quickly');
      return;
    }
    this._lastShortcut = now;
    try {
      this._keyboard.send(modifiers, keysym);
      invocation.return_value(null);
    } catch (error) {
      console.error(`[Kivori] Keyboard shortcut failed: ${error.message}`);
      this._refuse(invocation, 'Failed', 'Keyboard shortcut could not be sent');
    }
  }
  UnsubscribeAsync(_params, invocation) {
    this._drop(invocation.get_sender());
    invocation.return_value(null);
  }
  _drop(sender) {
    if (this._pending?.sender === sender) {
      this._pending = null;
      this._cancelApproval();
    }
    if (this._injector?.sender === sender) {
      this._injector = null;
      this._keyboard.reset();
    }
    const watch = this._subscribers.get(sender);
    if (watch === undefined) return;
    this._subscribers.delete(sender);
    Gio.bus_unwatch_name(watch);
  }
  notify() {
    this._generation = (this._generation + 1) >>> 0; // includes windows of the same application
    const state = this._state();
    const body = new GLib.Variant('(ssub)', state);
    for (const sender of this._subscribers.keys()) {
      try {
        this._connection.emit_signal(sender, PATH, BUS, 'FocusChanged', body);
      } catch {
        this._drop(sender);
      }
    }
  }
  stop() {
    this._stopped = true;
    if (this._identity.onChange === this._identityRevoker) this._identity.onChange = () => {};
    for (const sender of [...this._subscribers.keys()]) this._drop(sender);
    this._pending = null;
    this._keyboard.reset();
    this._object?.unexport();
    this._object = null;
    if (this._owner) Gio.bus_unown_name(this._owner);
    this._owner = null;
  }
}
