import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';

export function createKeyboardDevice(stage) {
  // Shell's global.backend is Meta.Backend, not Clutter.Backend. Only the stage's
  // Clutter context exposes the seat that can create virtual input devices.
  // GNOME 46/47 predate the stage context API.
  const backend = stage.context?.get_backend() ?? Clutter.get_default_backend();
  const device = backend
    .get_default_seat()
    .create_virtual_device(Clutter.InputDeviceType.KEYBOARD_DEVICE);
  return {
    notify: (key, down) =>
      device.notify_keyval(
        GLib.get_monotonic_time(), // Clutter virtual input takes microseconds.
        key,
        down ? Clutter.KeyState.PRESSED : Clutter.KeyState.RELEASED,
      ),
    dispose: () => device.run_dispose(),
  };
}
