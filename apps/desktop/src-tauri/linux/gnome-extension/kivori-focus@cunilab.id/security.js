import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

// Missing or invalid Shell security state is protected, never a usable desktop.
export function isProtected(main, stage) {
  try {
    const shield = main.screenShield;
    const mode = main.sessionMode;
    if (
      typeof shield?.locked !== 'boolean' ||
      typeof shield?.active !== 'boolean' ||
      typeof mode?.isLocked !== 'boolean' ||
      !Number.isInteger(main.modalCount) ||
      main.modalCount < 0 ||
      typeof stage?.get_grab_actor !== 'function'
    )
      return true;
    const grab = stage.get_grab_actor();
    if (grab === undefined) return true;
    return shield.locked || shield.active || mode.isLocked || main.modalCount > 0 || grab !== null;
  } catch {
    return true;
  }
}

export function allowedShortcut(modifiers, key) {
  if (!Number.isInteger(modifiers) || modifiers < 0 || modifiers > 15 || !Number.isInteger(key))
    return false;
  const ctrl = Boolean(modifiers & 1),
    alt = Boolean(modifiers & 2),
    superKey = Boolean(modifiers & 8);
  const fn = key >= 0xffbe && key <= 0xffc9; // F1–F12 only
  const lone =
    fn ||
    [
      0xff1b, 0xff09, 0xff51, 0xff52, 0xff53, 0xff54, 0xff50, 0xff57, 0xff55, 0xff56, 0xffff,
      0x1008ff11, 0x1008ff12, 0x1008ff13, 0x1008ff14, 0x1008ff15, 0x1008ff16, 0x1008ff17,
    ].includes(key);
  const printable =
    (key >= 97 && key <= 122) || (key >= 48 && key <= 57) || [0x60, 0x5b, 0x5d, 0x20].includes(key);
  if (!(lone || printable || [0xff0d, 0xff08].includes(key))) return false;
  if (
    (alt && key === 0xffbf) ||
    (ctrl && alt && (fn || [0xffff, 0xff08].includes(key))) ||
    (superKey && key === 108) ||
    ((ctrl || superKey) && key === 118)
  )
    return false;
  return ctrl || alt || superKey || (modifiers === 0 && lone);
}

function readJson(path) {
  const [ok, bytes] = Gio.File.new_for_path(path).load_contents(null);
  if (!ok) throw new Error('Cannot read keyboard authorization');
  return JSON.parse(new TextDecoder().decode(bytes));
}

/** Identity and disk approval are frozen for this extension instance's Shell session. */
export class Identity {
  constructor(directory, connection = Gio.DBus.session) {
    this.directory = directory;
    this.connection = connection;
    this.onChange = () => {};
    this._executable = null;
    this._record = null;
    this._approval = null;
    try {
      const path = readJson(`${directory}/trusted.json`).executable;
      if (typeof path === 'string' && path.startsWith('/')) {
        this._executable = path;
        this._record = this._fingerprint(path, path);
      }
    } catch {
      /* Missing/invalid install data cannot grant injection. */
    }
    try {
      this._approval = readJson(`${directory}/approval.json`).fingerprint;
    } catch {
      /* No consent. */
    }
    this._observed = this._record;
    this.watch();
  }

  get executable() {
    return this._executable;
  }

  _fingerprint(path, file) {
    const attributes = [
      'unix::device',
      'unix::inode',
      'time::modified',
      'time::modified-usec',
      'standard::size',
    ];
    const info = Gio.File.new_for_path(file).query_info(
      attributes.join(','),
      Gio.FileQueryInfoFlags.NONE,
      null,
    );
    const stamp = attributes.map((name) => {
      if (!info.has_attribute(name)) throw new Error('Missing executable identity');
      return info.get_attribute_as_string(name);
    });
    return JSON.stringify([path, ...stamp]);
  }

  invalidate() {
    this._approval = null;
    this.onChange();
  }

  watch() {
    if (this._monitor) return;
    // Watch the directory so atomic replacement/removal of trusted.json is also observed.
    // The monitor invalidates consent; it NEVER loads new trust or approval data.
    this._monitor = Gio.File.new_for_path(this.directory).monitor_directory(
      Gio.FileMonitorFlags.WATCH_MOVES,
      null,
    );
    this._monitor.connect('changed', (_monitor, file, other, event) => {
      if (
        file?.get_basename() === 'trusted.json' ||
        other?.get_basename() === 'trusted.json' ||
        event === Gio.FileMonitorEvent.UNMOUNTED ||
        (event === Gio.FileMonitorEvent.DELETED && file?.get_path() === this.directory)
      ) {
        this.invalidate();
      }
    });
  }

  pause() {
    this.invalidate();
    this._monitor?.cancel();
    this._monitor = null;
  }

  async resolve(sender) {
    try {
      if (
        !this._monitor ||
        !this._executable ||
        typeof sender !== 'string' ||
        !/^:\d+\.\d+$/.test(sender)
      )
        return null;
      const pid = await new Promise((resolve, reject) => {
        this.connection.call(
          'org.freedesktop.DBus',
          '/org/freedesktop/DBus',
          'org.freedesktop.DBus',
          'GetConnectionUnixProcessID',
          new GLib.Variant('(s)', [sender]),
          new GLib.VariantType('(u)'),
          Gio.DBusCallFlags.NONE,
          2000,
          null,
          (conn, result) => {
            try {
              resolve(conn.call_finish(result).deep_unpack()[0]);
            } catch (error) {
              reject(error);
            }
          },
        );
      });
      if (
        !Number.isInteger(pid) ||
        pid <= 0 ||
        GLib.file_read_link(`/proc/${pid}/exe`) !== this._executable
      )
        return null;
      const fingerprint = this._fingerprint(this._executable, this._executable);
      if (fingerprint !== this._fingerprint(this._executable, `/proc/${pid}/exe`)) return null;
      if (fingerprint !== this._observed) {
        this._observed = fingerprint;
        this.invalidate(); // A changed executable requires fresh Shell consent, not disk data.
      }
      return fingerprint;
    } catch {
      return null;
    }
  }

  approved(fingerprint) {
    return Boolean(this._monitor) && fingerprint === this._record && fingerprint === this._approval;
  }

  save(fingerprint) {
    if (!this._monitor || fingerprint !== this._observed)
      throw new Error('Executable changed during consent');
    const [ok] = Gio.File.new_for_path(`${this.directory}/approval.json`).replace_contents(
      new TextEncoder().encode(JSON.stringify({ fingerprint })),
      null,
      false,
      Gio.FileCreateFlags.PRIVATE | Gio.FileCreateFlags.REPLACE_DESTINATION,
      null,
    );
    if (!ok) throw new Error('Could not persist keyboard approval');
    this._record = fingerprint;
    this._approval = fingerprint; // Only this Shell dialog can add consent during this session.
  }
}

export function consentDescription(executable) {
  return `Executable: ${executable}\n\nThis executable will be able to press keyboard shortcuts in your apps. Keyboard control is paused while GNOME is locked or showing a modal prompt. This approval is remembered for this executable.`;
}

// Clutter.ModifierType Shift, Control, Mod1 (Alt), Mod3, Mod4, Mod5, Super, Hyper, Meta.
// Caps Lock (Lock) and Num Lock (Mod2) do not change a chord and are ignored.
const CHORD_MODIFIERS = 1 | 4 | 8 | 32 | 64 | 128 | (1 << 26) | (1 << 27) | (1 << 28);

/** Failed releases remain tracked. No new chord can run while cleanup is uncertain. */
export class Keyboard {
  constructor(create, readModifiers = () => null) {
    this.create = create;
    this.readModifiers = readModifiers;
    this.device = null;
    this.held = new Set();
    this.suspended = false;
    this.disposed = false;
  }

  release() {
    if (this.disposed) return;
    for (const key of [...this.held].reverse()) {
      try {
        this.device.notify(key, false);
        this.held.delete(key);
      } catch {
        this.suspended = true;
      }
    }
  }

  reset() {
    this.release();
    if (this.device && !this.disposed) {
      try {
        this.device.dispose();
        this.disposed = true;
      } catch {
        this.suspended = true;
      }
    }
    if (this.held.size === 0 && (!this.device || this.disposed)) {
      this.device = null;
      this.disposed = false;
      this.suspended = false;
    }
  }

  /** Seat modifiers already down would join the injected chord (held Alt + lone F2 is Alt+F2),
   *  bypassing the policy. An unreadable seat state counts as held. */
  modifiersHeld() {
    try {
      const mods = this.readModifiers();
      return !Number.isInteger(mods) || (mods & CHORD_MODIFIERS) !== 0;
    } catch {
      return true;
    }
  }

  send(modifiers, key) {
    if (this.suspended || this.held.size) throw new Error('Keyboard cleanup is required');
    this.device ??= this.create();
    const keys = [0xffe3, 0xffe9, 0xffe1, 0xffeb].filter((_, bit) => modifiers & (1 << bit));
    keys.push(key);
    let failed = false;
    try {
      for (const value of keys) {
        this.held.add(value);
        this.device.notify(value, true);
      }
    } catch {
      failed = true;
    }
    this.release();
    if (failed || this.held.size) {
      this.reset();
      throw new Error('Keyboard shortcut could not be sent');
    }
  }
}
