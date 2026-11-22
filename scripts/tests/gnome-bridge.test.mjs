// Run: node --experimental-vm-modules --test scripts/tests/gnome-bridge.test.mjs
import { readFileSync } from 'node:fs';
import { SourceTextModule, SyntheticModule } from 'node:vm';
import { test } from 'node:test';
import assert from 'node:assert/strict';

const root = new URL(
  '../../apps/desktop/src-tauri/linux/gnome-extension/kivori-focus@cunilab.id/',
  import.meta.url,
);
let now = 1000000;
let nextWatch = 0;
const watches = new Map();
const files = new Map();
const monitors = [];
let exeLink = '/opt/kivori';
let stamp = 'original';
const Gio = {
  FileMonitorFlags: { WATCH_MOVES: 1 },
  FileMonitorEvent: { UNMOUNTED: 1, DELETED: 2, CHANGED: 3 },
  DBusCallFlags: { NONE: 0 },
  FileQueryInfoFlags: { NONE: 0 },
  FileCreateFlags: { PRIVATE: 1, REPLACE_DESTINATION: 2 },
  DBus: { session: {} },
  BusNameWatcherFlags: { NONE: 0 },
  bus_watch_name_on_connection: (_, sender, _flags, _appeared, lost) => {
    const id = ++nextWatch;
    watches.set(id, { sender, lost });
    return id;
  },
  bus_unwatch_name: (id) => watches.delete(id),
  File: {
    new_for_path: (path) => ({
      monitor_directory: () => {
        const monitor = {
          connect: (_signal, changed) => {
            monitor.changed = changed;
          },
          cancel: () => {},
        };
        monitors.push(monitor);
        return monitor;
      },
      load_contents: () => {
        if (!files.has(path)) throw new Error('missing file');
        return [true, new TextEncoder().encode(files.get(path))];
      },
      query_info: () => ({ has_attribute: () => true, get_attribute_as_string: () => stamp }),
      replace_contents: (bytes) => {
        files.set(path, new TextDecoder().decode(bytes));
        return [true, 'etag'];
      },
    }),
  },
};
const Clutter = { InputDeviceType: { KEYBOARD_DEVICE: 1 }, KeyState: { PRESSED: 1, RELEASED: 0 } };
const GLib = {
  get_monotonic_time: () => now,
  file_read_link: () => exeLink,
  VariantType: class {},
  Variant: class {
    constructor(_, value) {
      this.value = value;
    }
  },
};
const modules = new Map();
async function load(name) {
  if (modules.has(name)) return modules.get(name);
  const module = new SourceTextModule(readFileSync(new URL(name, root), 'utf8'));
  modules.set(name, module);
  await module.link(async (specifier) => {
    if (specifier.startsWith('./')) return load(specifier.slice(2));
    return new SyntheticModule(['default'], function () {
      this.setExport(
        'default',
        specifier === 'gi://Gio' ? Gio : specifier === 'gi://Clutter' ? Clutter : GLib,
      );
    });
  });
  await module.evaluate();
  return module;
}
const security = (await load('security.js')).namespace;
const { FocusBridge, VERSION } = (await load('bridge.js')).namespace;
const { allowedShortcut, isProtected, Keyboard, Identity } = security;
function invocation(sender = ':1.42') {
  return {
    get_sender: () => sender,
    return_value(value) {
      this.value = value?.value;
      this.ok = true;
    },
    return_dbus_error(name) {
      this.error = name;
    },
  };
}
async function fixture({
  read = () => ['', '', 0, false],
  approve = async () => true,
  identity,
  keyboard,
} = {}) {
  now += 1000000;
  const approved = new Set();
  const peers = new Map([
    [':1.42', 'exe-v1'],
    [':1.43', 'exe-v1'],
  ]);
  identity ??= {
    resolve: async (sender) => peers.get(sender) ?? null,
    approved: (stamp) => approved.has(stamp),
    save: (stamp) => approved.add(stamp),
  };
  let calls = 0,
    resets = 0;
  keyboard ??= {
    send: () => calls++,
    reset: () => resets++,
    modifiersHeld: () => false,
    suspended: false,
  };
  const bridge = new FocusBridge(read, { emit_signal: () => {} }, identity, approve, keyboard);
  await bridge.SubscribeAsync([], invocation());
  return { bridge, identity, peers, approved, calls: () => calls, resets: () => resets };
}
async function authorize(bridge, sender = ':1.42', interactive = true) {
  const call = invocation(sender);
  await bridge.AuthorizeAsync([interactive], call);
  return call;
}
async function context(bridge, sender = ':1.42') {
  const call = invocation(sender);
  await bridge.GetShortcutContextAsync([], call);
  return call.value;
}
async function send(
  bridge,
  { sender = ':1.42', modifiers = 1, key = 97, deadline, generation } = {},
) {
  const ctx = await context(bridge, sender);
  const call = invocation(sender);
  await bridge.SendShortcutAsync(
    [modifiers, key, deadline ?? ctx[2] + 50000, generation ?? ctx[1]],
    call,
  );
  return call;
}

test('focus subscription alone cannot inject and an explicit denied approval cannot grant it', async () => {
  const f = await fixture({ approve: async () => false });
  assert.match((await send(f.bridge)).error, /AccessDenied$/);
  assert.deepEqual((await authorize(f.bridge)).value, [false]);
  assert.match((await send(f.bridge)).error, /AccessDenied$/);
  assert.equal(f.calls(), 0);
});

test('unauthenticated clients and sandbox proxies cannot consume subscriber slots', async () => {
  const f = await fixture();
  for (let i = 100; i < 130; i++) {
    const call = invocation(`:1.${i}`);
    await f.bridge.SubscribeAsync([], call);
    assert.match(call.error, /AccessDenied$/);
  }
  assert.equal(f.bridge._subscribers.size, 1);
  assert.deepEqual((await authorize(f.bridge)).value, [true]);
  assert.equal((await send(f.bridge)).ok, true);
  f.peers.delete(':1.42');
  const changed = invocation();
  await f.bridge.SubscribeAsync([], changed);
  assert.match(changed.error, /AccessDenied$/);
  assert.equal(f.bridge._subscribers.size, 0);
  assert.equal(f.bridge._injector, null);
});

test('only one injector; owner loss and unsubscribe revoke injection and reset the device', async () => {
  const f = await fixture();
  await authorize(f.bridge);
  await f.bridge.SubscribeAsync([], invocation(':1.43'));
  assert.match((await authorize(f.bridge, ':1.43')).error, /AccessDenied$/);
  assert.match((await send(f.bridge, { sender: ':1.43' })).error, /AccessDenied$/);
  [...watches.values()].findLast((watch) => watch.sender === ':1.42').lost();
  assert.match((await send(f.bridge)).error, /AccessDenied$/);
  assert.equal(f.resets(), 1);
  await authorize(f.bridge, ':1.43');
  f.bridge.UnsubscribeAsync([], invocation(':1.43'));
  assert.equal(f.resets(), 2);
});

test('approval is remembered for a fingerprint across bridge restarts, replacement requires approval', async () => {
  let prompts = 0;
  const f = await fixture({
    approve: async () => {
      prompts++;
      return true;
    },
  });
  await authorize(f.bridge);
  f.bridge.stop();
  const second = await fixture({
    identity: f.identity,
    approve: async () => {
      prompts++;
      return false;
    },
  });
  assert.deepEqual((await authorize(second.bridge)).value, [true]);
  assert.equal(prompts, 1);
  f.peers.set(':1.42', 'exe-v2');
  assert.match((await send(second.bridge)).error, /AccessDenied$/);
  assert.equal(second.resets(), 1);
  await second.bridge.SubscribeAsync([], invocation());
  assert.deepEqual((await authorize(second.bridge)).value, [false]);
  assert.equal(prompts, 2);
});

test('disconnect or disable during a pending approval cannot authorize a stale peer', async () => {
  for (const stop of [false, true]) {
    let finish;
    const f = await fixture({
      approve: () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    });
    const pending = authorize(f.bridge);
    await new Promise((resolve) => setImmediate(resolve));
    if (stop) f.bridge.stop();
    else f.bridge.UnsubscribeAsync([], invocation());
    finish(true);
    assert.deepEqual((await pending).value, [false]);
    assert.equal(f.bridge._injector, null);
  }
});

test('real protection predicate fails closed for missing, invalid and throwing fields', () => {
  const main = () => ({
    screenShield: { locked: false, active: false },
    sessionMode: { isLocked: false },
    modalCount: 0,
  });
  const stage = { get_grab_actor: () => null };
  assert.equal(isProtected(main(), stage), false);
  for (const path of ['screenShield', 'sessionMode', 'modalCount']) {
    const state = main();
    delete state[path];
    assert.equal(isProtected(state, stage), true);
  }
  for (const [object, field] of [
    ['screenShield', 'locked'],
    ['screenShield', 'active'],
    ['sessionMode', 'isLocked'],
  ]) {
    const state = main();
    delete state[object][field];
    assert.equal(isProtected(state, stage), true);
    state[object][field] = 'false';
    assert.equal(isProtected(state, stage), true);
    state[object][field] = true;
    assert.equal(isProtected(state, stage), true);
  }
  for (const count of [-1, NaN, 0.5, '0', 1]) {
    const state = main();
    state.modalCount = count;
    assert.equal(isProtected(state, stage), true);
  }
  for (const value of [
    {},
    { get_grab_actor: () => undefined },
    { get_grab_actor: () => ({}) },
    {
      get_grab_actor: () => {
        throw new Error();
      },
    },
  ]) {
    assert.equal(isProtected(main(), value), true);
  }
});

test('protection is checked after asynchronous identity resolution, invalid read fields refuse', async () => {
  let protectedState = false;
  const f = await fixture({ read: () => ['', '', 0, protectedState] });
  await authorize(f.bridge);
  const ctx = await context(f.bridge);
  f.identity.resolve = async () => {
    protectedState = true;
    return 'exe-v1';
  };
  const call = invocation();
  await f.bridge.SendShortcutAsync([1, 97, ctx[2] + 50000, ctx[1]], call);
  assert.match(call.error, /AccessDenied$/);
  assert.equal(f.calls(), 0);
  for (const bad of [[], ['', '', 0, undefined], ['', '', -1, false], ['', '', 0, 'false']]) {
    const f = await fixture({ read: () => bad });
    assert.match((await authorize(f.bridge)).error, /AccessDenied$/);
  }
});

test('all prohibited chords are refused, non-text keys and ordinary shortcuts are allowed', async () => {
  const blocked = [
    [0, 97],
    [4, 97],
    [0, 0xff0d],
    [4, 0xff0d],
    [2, 0xffbf],
    [1 | 2, 0xffff],
    [1 | 2, 0xff08],
    [8, 108],
    [8, 0xffeb],
    [0, 0xffeb],
    [1, 118],
    [5, 118],
    [8, 118],
    [0, 0x10000e9],
    [16, 97],
    [-1, 97],
    [0, 0xffca],
  ];
  for (let key = 0xffbe; key <= 0xffc9; key++) blocked.push([3, key], [7, key]);
  const f = await fixture();
  await authorize(f.bridge);
  for (const [modifiers, key] of blocked) {
    assert.equal(allowedShortcut(modifiers, key), false, `${modifiers}/${key}`);
    assert.match((await send(f.bridge, { modifiers, key })).error, /InvalidArgs$/);
  }
  for (const [modifiers, key] of [
    [1, 97],
    [5, 0xff09],
    [2, 0xff51],
    [8, 97],
    [0, 0xffbe],
    [0, 0xffc9],
    [0, 0xff1b],
    [0, 0xffff],
    [0, 0x1008ff14],
    [0, 0x1008ff16],
    [0, 0x1008ff17],
    [0, 0x1008ff13],
  ]) {
    assert.equal(allowedShortcut(modifiers, key), true);
    now += 20000;
    assert.equal((await send(f.bridge, { modifiers, key })).ok, true);
  }
});

test('expired, overlong, changed-focus and delayed-in-authorization requests never inject', async () => {
  const f = await fixture();
  await authorize(f.bridge);
  for (const deadline of [now, now - 1, now + 100001, NaN]) {
    assert.match((await send(f.bridge, { deadline })).error, /AccessDenied$/);
  }
  const ctx = await context(f.bridge);
  f.bridge.notify(); // same-app window changes must invalidate the generation too
  assert.match((await send(f.bridge, { generation: ctx[1] })).error, /AccessDenied$/);
  f.identity.resolve = async () => {
    now += 100000;
    return 'exe-v1';
  };
  const call = invocation();
  await f.bridge.SendShortcutAsync([1, 97, now + 50000, (await context(f.bridge))[1]], call);
  assert.match(call.error, /AccessDenied$/);
  assert.equal(f.calls(), 0);
});

test('rate limit is retained for the authorized injector', async () => {
  const f = await fixture();
  await authorize(f.bridge);
  assert.equal((await send(f.bridge)).ok, true);
  assert.match((await send(f.bridge)).error, /LimitsExceeded$/);
  now += 20000;
  assert.equal((await send(f.bridge)).ok, true);
});

test('failed releases remain tracked, dispose is attempted and subsequent injection is suspended', () => {
  let disposed = 0;
  const events = [];
  const keyboard = new Keyboard(() => ({
    notify: (key, down) => {
      events.push([key, down]);
      if (!down && key === 0xff09) throw new Error('failed release');
    },
    dispose: () => disposed++,
  }));
  assert.throws(() => keyboard.send(5, 0xff09));
  assert.equal(keyboard.suspended, true);
  assert.deepEqual([...keyboard.held], [0xff09]);
  assert.equal(disposed, 1);
  const count = events.length;
  assert.throws(() => keyboard.send(1, 97));
  assert.equal(events.length, count);
  keyboard.reset();
  assert.deepEqual([...keyboard.held], [0xff09]);
  assert.equal(keyboard.suspended, true);
  assert.ok(events.some(([key, down]) => key === 0xffe3 && !down));
});

test('failed press and transient release errors release all tracked keys and reset the device', () => {
  for (const failure of ['press', 'release']) {
    let failed = false,
      disposed = 0,
      created = 0;
    const keyboard = new Keyboard(() => {
      created++;
      return {
        notify: (key, down) => {
          if (!failed && key === 0xff09 && down === (failure === 'press')) {
            failed = true;
            throw new Error();
          }
        },
        dispose: () => disposed++,
      };
    });
    assert.throws(() => keyboard.send(5, 0xff09));
    assert.equal(keyboard.held.size, 0);
    assert.equal(keyboard.suspended, false);
    assert.equal(disposed, 1);
    keyboard.send(1, 97);
    assert.equal(created, 2);
    keyboard.reset();
    assert.equal(disposed, 2);
  }
});

test('Identity derives the pid from the bus daemon, rejects proxy/mismatched executable, persists fingerprint', async () => {
  files.clear();
  files.set('/extension/trusted.json', JSON.stringify({ executable: '/opt/kivori' }));
  exeLink = '/opt/kivori';
  stamp = 'original';
  const conn = {
    call: (dest, path, iface, method, params, _type, _flags, _timeout, _cancel, callback) => {
      assert.equal(dest, 'org.freedesktop.DBus');
      assert.equal(method, 'GetConnectionUnixProcessID');
      assert.deepEqual(params.value, [':1.42']);
      callback(conn, {});
    },
    call_finish: () => ({ deep_unpack: () => [42] }),
  };
  const identity = new Identity('/extension', conn);
  const fingerprint = await identity.resolve(':1.42');
  assert.ok(fingerprint);
  assert.equal(identity.approved(fingerprint), false);
  identity.save(fingerprint);
  assert.equal(new Identity('/extension', conn).approved(fingerprint), true);
  stamp = 'replaced';
  assert.notEqual(await identity.resolve(':1.42'), fingerprint);
  assert.equal(identity.approved(await identity.resolve(':1.42')), false);
  for (const path of ['/usr/bin/xdg-dbus-proxy', '/opt/kivori (deleted)', '/opt/other']) {
    exeLink = path;
    assert.equal(await identity.resolve(':1.42'), null);
  }
  assert.equal(await identity.resolve('caller-supplied-pid'), null);
  files.delete('/extension/trusted.json');
  exeLink = '/opt/kivori';
  assert.ok(await identity.resolve(':1.42'), 'the existing session never reloads trusted.json');
  assert.equal(await new Identity('/extension', conn).resolve(':1.42'), null);
});

test('metadata, bridge version, and all extension ES modules parse', () => {
  assert.equal(JSON.parse(readFileSync(new URL('metadata.json', root), 'utf8')).version, VERSION);
  new SourceTextModule(readFileSync(new URL('extension.js', root), 'utf8'));
});

test('stop revokes the injector, disposes the keyboard and retains unconfirmed releases', async () => {
  let disposed = 0;
  const keyboard = new Keyboard(
    () => ({
      notify: (_key, down) => {
        if (!down) throw new Error('release unavailable');
      },
      dispose: () => disposed++,
    }),
    () => 0,
  );
  const f = await fixture({ keyboard });
  await authorize(f.bridge);
  assert.match((await send(f.bridge)).error, /Failed$/);
  const held = [...keyboard.held];
  f.bridge.stop();
  assert.equal(f.bridge._injector, null);
  assert.equal(keyboard.suspended, true);
  assert.deepEqual([...keyboard.held], held);
  assert.equal(disposed, 1);
  assert.match((await send(f.bridge)).error, /AccessDenied$/);
});

test('a failed device disposal suspends injection until reset succeeds', () => {
  let fail = true;
  const keyboard = new Keyboard(() => ({
    notify: () => {},
    dispose: () => {
      if (fail) throw new Error('dispose unavailable');
    },
  }));
  keyboard.send(1, 97);
  keyboard.reset();
  assert.equal(keyboard.suspended, true);
  assert.throws(() => keyboard.send(1, 97));
  fail = false;
  keyboard.reset();
  assert.equal(keyboard.suspended, false);
  keyboard.send(1, 97);
});

function identityFixture() {
  files.clear();
  exeLink = '/opt/kivori';
  stamp = 'original';
  files.set('/extension/trusted.json', JSON.stringify({ executable: exeLink }));
  const conn = {
    call: (_dest, _path, _iface, _method, _params, _type, _flags, _timeout, _cancel, callback) =>
      callback(conn, {}),
    call_finish: () => ({ deep_unpack: () => [42] }),
  };
  return new Identity('/extension', conn);
}
function trustChanged() {
  monitors
    .at(-1)
    .changed(
      null,
      { get_basename: () => 'trusted.json', get_path: () => '/extension/trusted.json' },
      null,
      Gio.FileMonitorEvent.CHANGED,
    );
}

test('rewriting trust and computable approval after enable never authorizes an attacker or sends keys', async () => {
  const identity = identityFixture();
  let prompts = 0;
  const f = await fixture({
    identity,
    approve: async () => {
      prompts++;
      return false;
    },
  });
  const legitimate = await identity.resolve(':1.42');
  files.set('/extension/trusted.json', JSON.stringify({ executable: '/opt/attacker' }));
  files.set(
    '/extension/approval.json',
    JSON.stringify({ fingerprint: JSON.stringify(['/opt/attacker', ...Array(5).fill(stamp)]) }),
  );
  exeLink = '/opt/attacker';
  const attacker = invocation();
  await f.bridge.SubscribeAsync([], attacker);
  assert.match(attacker.error, /AccessDenied$/);
  assert.match((await authorize(f.bridge)).error, /AccessDenied$/);
  assert.match((await send(f.bridge)).error, /AccessDenied$/);
  assert.equal(f.calls(), 0);
  assert.equal(prompts, 0);
  // Forging consent for the ORIGINAL path is also ineffective, even before a monitor event.
  exeLink = '/opt/kivori';
  files.set('/extension/approval.json', JSON.stringify({ fingerprint: legitimate }));
  await f.bridge.SubscribeAsync([], invocation());
  assert.deepEqual((await authorize(f.bridge, ':1.42', false)).value, [false]);
  assert.equal(prompts, 0);
  assert.deepEqual((await authorize(f.bridge)).value, [false]);
  assert.equal(prompts, 1);
  assert.match((await send(f.bridge)).error, /AccessDenied$/);
  assert.equal(f.calls(), 0);
});

test('trust monitor revokes live authorization and only new Shell consent can restore the cached path', async () => {
  const identity = identityFixture();
  let prompts = 0;
  identity.save(await identity.resolve(':1.42'));
  const f = await fixture({
    identity,
    approve: async () => {
      prompts++;
      return true;
    },
  });
  assert.deepEqual((await authorize(f.bridge, ':1.42', false)).value, [true]);
  files.set('/extension/trusted.json', JSON.stringify({ executable: '/opt/attacker' }));
  trustChanged();
  assert.match((await send(f.bridge)).error, /AccessDenied$/);
  assert.equal(f.calls(), 0);
  assert.deepEqual((await authorize(f.bridge, ':1.42', false)).value, [false]);
  assert.equal(prompts, 0);
  assert.deepEqual((await authorize(f.bridge)).value, [true]);
  assert.equal(prompts, 1);
  assert.equal(identity.executable, '/opt/kivori');
  assert.equal((await send(f.bridge)).ok, true);
});

test('a changed fingerprint cannot use forged disk approval and disable/re-enable never reloads trust', async () => {
  const identity = identityFixture();
  identity.save(await identity.resolve(':1.42'));
  const f = await fixture({ identity, approve: async () => false });
  stamp = 'replacement';
  const changed = await identity.resolve(':1.42');
  files.set('/extension/approval.json', JSON.stringify({ fingerprint: changed }));
  assert.equal(identity.approved(changed), false);
  assert.deepEqual((await authorize(f.bridge)).value, [false]);
  assert.equal(f.calls(), 0);
  identity.pause();
  files.set('/extension/trusted.json', JSON.stringify({ executable: '/opt/attacker' }));
  identity.watch();
  assert.equal(identity.executable, '/opt/kivori');
  exeLink = '/opt/attacker';
  assert.equal(await identity.resolve(':1.42'), null);
});

test('the actual Shell dialog description contains the complete executable path and keyboard capability', async () => {
  let content, dialog;
  class FakeDialog {
    constructor() {
      dialog = { buttons: [] };
      this.contentLayout = { add_child: () => {} };
    }
    setButtons(buttons) {
      dialog.buttons = buttons;
    }
    open() {
      return true;
    }
    close() {}
  }
  const module = new SourceTextModule(readFileSync(new URL('extension.js', root), 'utf8'));
  await module.link(async (specifier) => {
    if (specifier.startsWith('./')) return load(specifier.slice(2));
    let exports;
    if (specifier.endsWith('/extension.js')) exports = { Extension: class {} };
    else if (specifier.endsWith('/modalDialog.js')) exports = { ModalDialog: FakeDialog };
    else if (specifier.endsWith('/dialog.js'))
      exports = {
        MessageDialogContent: class {
          constructor(props) {
            content = props;
          }
        },
      };
    else if (specifier.endsWith('/main.js')) exports = {};
    else
      exports = { default: specifier === 'gi://Gio' ? Gio : specifier === 'gi://GLib' ? GLib : {} };
    return new SyntheticModule(Object.keys(exports), function () {
      for (const [key, value] of Object.entries(exports)) this.setExport(key, value);
    });
  });
  await module.evaluate();
  const extension = new module.namespace.default();
  const path = '/home/example/Applications/Kivori Desktop/kivori-desktop';
  const result = extension._requestApproval(path);
  assert.ok(content.description.includes(path));
  assert.match(content.description, /will be able to press keyboard shortcuts/);
  dialog.buttons.find((button) => button.label === 'Allow').action();
  assert.equal(await result, true);
});

test('virtual keyboard uses the stage Clutter backend, with microsecond times and separate modifier events', async () => {
  const { createKeyboardDevice } = (await load('keyboard.js')).namespace;
  const events = [];
  let disposed = false;
  const stage = {
    context: {
      get_backend: () => ({
        get_default_seat: () => ({
          create_virtual_device: (kind) => {
            assert.equal(kind, Clutter.InputDeviceType.KEYBOARD_DEVICE);
            return {
              notify_keyval: (time, key, state) => events.push([time, key, state]),
              run_dispose: () => {
                disposed = true;
              },
            };
          },
        }),
      }),
    },
  };
  const keyboard = new Keyboard(() => createKeyboardDevice(stage));
  keyboard.send(2, 0xff51);
  assert.deepEqual(events, [
    [now, 0xffe9, 1],
    [now, 0xff51, 1],
    [now, 0xff51, 0],
    [now, 0xffe9, 0],
  ]);
  assert.equal(
    disposed,
    false,
    'retain the device until cleanup, not just until events are queued',
  );
  keyboard.reset();
  assert.equal(disposed, true);
});

test('older GNOME uses Clutter.get_default_backend when the stage has no context', async () => {
  const { createKeyboardDevice } = (await load('keyboard.js')).namespace;
  let created = false;
  Clutter.get_default_backend = () => ({
    get_default_seat: () => ({
      create_virtual_device: () => {
        created = true;
        return {};
      },
    }),
  });
  try {
    createKeyboardDevice({});
    assert.equal(created, true);
  } finally {
    delete Clutter.get_default_backend;
  }
});

test('held seat modifiers refuse the chord: held Alt + lone F2 must not become Alt+F2', async () => {
  let held = true;
  let calls = 0;
  const keyboard = {
    send: () => calls++,
    reset: () => {},
    modifiersHeld: () => held,
    suspended: false,
  };
  const f = await fixture({ keyboard });
  await authorize(f.bridge);
  assert.match((await send(f.bridge, { modifiers: 0, key: 0xffbf })).error, /ModifiersHeld$/);
  assert.equal(calls, 0);
  // A refusal does not consume the rate-limit slot.
  held = false;
  assert.equal((await send(f.bridge, { modifiers: 0, key: 0xffbf })).ok, true);
  assert.equal(calls, 1);
});

test('seat modifier reading: chord modifiers held, lock keys ignored, unknown fails closed', () => {
  const held = (read) => new Keyboard(() => ({}), read).modifiersHeld();
  for (const mods of [1, 4, 8, 32, 64, 128, 1 << 26, 1 << 27, 1 << 28, 4 | 8])
    assert.equal(
      held(() => mods),
      true,
      `mods ${mods}`,
    );
  for (const mods of [0, 2, 16, 2 | 16, 1 << 8])
    assert.equal(
      held(() => mods),
      false,
      `mods ${mods}`,
    );
  assert.equal(
    held(() => undefined),
    true,
  );
  assert.equal(
    held(() => {
      throw new Error('no seat');
    }),
    true,
  );
  assert.equal(new Keyboard(() => ({})).modifiersHeld(), true);
});
