// Runs only in the isolated session created by scripts/gnome-shortcut-smoke.sh.
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Gtk from 'gi://Gtk?version=4.0';
import Gdk from 'gi://Gdk?version=4.0';

Gio._promisify(Gio.DBusConnection.prototype, 'call', 'call_finish');
const directory = ARGV[1];
if (ARGV[0] === 'prepare') {
  // Fixture approval belongs to this disposable Shell only, never the user's session.
  const executable = GLib.file_read_link('/proc/self/exe');
  GLib.file_set_contents(`${directory}/trusted.json`, JSON.stringify({ executable }));
  const { Identity } = await import(`file://${directory}/security.js`);
  const identity = new Identity(directory);
  identity.save(identity._record);
  identity.pause();
} else {
  const bus = 'id.cunilab.KivoriFocus';
  const path = '/id/cunilab/KivoriFocus';
  const call = async (method, signature, args, result) => {
    const reply = await Gio.DBus.session.call(
      bus,
      path,
      bus,
      method,
      new GLib.Variant(signature, args),
      new GLib.VariantType(result),
      Gio.DBusCallFlags.NONE,
      2000,
      null,
    );
    const values = reply.deep_unpack();
    print(`${method}: ${JSON.stringify(values)}`);
    return values;
  };
  const sleep = (ms) =>
    new Promise((resolve) =>
      GLib.timeout_add(GLib.PRIORITY_DEFAULT, ms, () => {
        resolve();
        return GLib.SOURCE_REMOVE;
      }),
    );
  const events = [];
  let passed = false;
  const app = new Gtk.Application({ application_id: 'id.cunilab.KivoriShortcutSmoke' });
  app.connect('activate', () => {
    const window = new Gtk.ApplicationWindow({
      application: app,
      title: 'Kivori shortcut smoke',
      default_width: 400,
      default_height: 200,
    });
    window.set_child(new Gtk.Label({ label: 'Isolated keyboard delivery probe' }));
    const keys = new Gtk.EventControllerKey();
    for (const signal of ['key-pressed', 'key-released']) {
      keys.connect(signal, (_controller, key, code, state) => {
        const event = [signal, Gdk.keyval_name(key), code, Number(state)];
        events.push(event);
        print(`EVENT ${JSON.stringify(event)}`);
        return false;
      });
    }
    window.add_controller(keys);
    window.present();
    void (async () => {
      try {
        await sleep(4000);
        const focus = await call('Subscribe', '()', [], '(ssub)');
        if (focus[3]) throw new Error('Shell is protected');
        const [approved] = await call('Authorize', '(b)', [false], '(b)');
        if (!approved) throw new Error('Fixture identity was not authorized');
        for (const [modifiers, key] of [
          [2, 0xff51],
          [1, 114],
          [2, 0xff53],
        ]) {
          const [authorized, generation, clock] = await call(
            'GetShortcutContext',
            '()',
            [],
            '(but)',
          );
          if (!authorized) throw new Error('Shortcut context not authorized');
          await call('SendShortcut', '(uutu)', [modifiers, key, clock + 100000, generation], '()');
          await sleep(250);
        }
        const expected = [
          ['key-pressed', 'Alt_L', 0],
          ['key-pressed', 'Left', Gdk.ModifierType.ALT_MASK],
          ['key-released', 'Left', Gdk.ModifierType.ALT_MASK],
          ['key-released', 'Alt_L', Gdk.ModifierType.ALT_MASK],
          ['key-pressed', 'Control_L', 0],
          ['key-pressed', 'r', Gdk.ModifierType.CONTROL_MASK],
          ['key-released', 'r', Gdk.ModifierType.CONTROL_MASK],
          ['key-released', 'Control_L', Gdk.ModifierType.CONTROL_MASK],
          ['key-pressed', 'Alt_L', 0],
          ['key-pressed', 'Right', Gdk.ModifierType.ALT_MASK],
          ['key-released', 'Right', Gdk.ModifierType.ALT_MASK],
          ['key-released', 'Alt_L', Gdk.ModifierType.ALT_MASK],
        ];
        const actual = events.map(([signal, key, , state]) => [signal, key, state]);
        if (JSON.stringify(actual) !== JSON.stringify(expected))
          throw new Error(`Unexpected events: ${JSON.stringify(actual)}`);
        passed = true;
        print('PASS: all 12 Wayland key events and modifier states received');
      } catch (error) {
        printerr(`FAIL: ${error.message}`);
      } finally {
        app.quit();
      }
    })();
  });
  app.run([]);
  if (!passed) imports.system.exit(1);
}
