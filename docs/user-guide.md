# Kivori user guide

A short guide for beta customers. Kivori Pip (model KV-P1) is a desk buddy with a knob, three
buttons and a small screen. It controls your computer and shows what is going on. Windows and Apple Silicon macOS builds share the same controls; platform differences are called out.

## What's in the box

- Kivori Pip unit (knob, three buttons, screen) _(to be confirmed)_
- USB-C data cable _(to be confirmed)_
- Installer download link or USB stick _(to be confirmed)_
- Quick-start card _(to be confirmed)_

## Install

1. Run the Kivori installer. It installs for your user only and does not ask for administrator
   rights.
2. This beta installer is not signed yet, so Windows SmartScreen may show "Windows protected your
   PC". This is expected for an unsigned app. If you got the installer from us, choose **More
   info**, then **Run anyway**.
3. Finish the installer and open Kivori.

**macOS (Apple Silicon):** open the `.dmg`, drag Kivori to Applications, eject the disk image, then
open the installed app. Keep Kivori in Applications so
its Accessibility permission belongs to the installed copy. Install a newer `.dmg` manually to
update the app.

The first time you open it, macOS says it cannot verify the app (Kivori is not notarized yet).
This is expected. Close the message, open **System Settings**, go to **Privacy & Security**, scroll
down to the message about Kivori and choose **Open Anyway**, then confirm with your password or
Touch ID. You only do this once. Do not turn Gatekeeper off. Public distribution readiness is
tracked in [macOS support](./macos.md).

Most things work without any permission on a Mac. Only keyboard shortcuts need **Accessibility**.
Kivori asks once if it is not on yet (in setup, or in one card the first time you open an updated
Kivori), and never when it is already on. Choose **Allow**, then switch Kivori on in System
Settings, Privacy & Security, Accessibility. If you choose **Not now**, it will not ask again by
itself: the Controls page reminds you when you bind a shortcut, and Settings shows whether it is
**Allowed** and lets you allow it. Play/pause, next, previous, volume and mute never need it.

## First run and setup

Kivori walks you through a short setup. You can skip any step, and "Skip setup" is always at the top.

1. **Welcome.**
2. **Plug in.** Connect Kivori with a USB-C **data** cable. Kivori finds it by itself; there is no
   port to choose.
3. **Allow Accessibility (macOS only).** Shown only if it is not on yet. **Allow** opens System
   Settings; the step continues by itself once you switch Kivori on. **Not now** skips it.
4. **Try it.** Follow the guidance for your device's supported controls and check their assigned
   actions in Controls. This step does not automatically tick or verify the hardware; continue
   when you are ready.
5. **Make it yours.** Pick the starting screen, how lively the buddy is, and whether Kivori starts
   at login.

You can repeat setup any time: Device page, **Run setup again**.

## The controls

| Control | What it does by default |
|---|---|
| Turn the knob | Volume (faster turns take bigger steps) |
| Press the knob | Play / Pause |
| Hold the knob (about 1 second, then let go) | Mute |
| Double-press the knob | Switch screen view |
| Left / middle / right button | Previous track / Play-Pause / Next track |
| Hold the middle button | Pin a profile (see below) |
| Hold the knob for about 10 seconds | Restarts Kivori, even if the app is closed |

A short press means pressed and released within half a second. Holding the knob past about 2
seconds does nothing until the 10-second restart, and the screen shows its progress. The left and
right buttons can also be given a Hold action in the app; holding them for a long time never
restarts anything.

With double press enabled, a single knob press waits about a quarter second after release so
Kivori can distinguish it from a double press.

## Profiles

Kivori changes what the controls do to suit the app in front of you, for example media keys in a
player or tab switching in a browser. This is **Auto**, and the profile name shows at the top left of
the screen. The built-in profiles are General, Browser, Code, Media, Zoom and Teams. In Zoom and Teams the left
button mutes your microphones.

To stay on one profile, hold the middle button. Each hold moves on: General, Browser, Code, Media,
Zoom, Teams, then back to Auto. A dot next to the name shows it is pinned. On a protected window
(such as a Windows permission prompt or the lock screen) shortcuts and app launches pause, while
volume, media keys and mute keep working.

## Muting your microphones

The **Mic mute** action (the left button in Zoom and Teams, or any control you give it on the
Controls page) mutes every microphone your computer has, not just one app's, and then checks each
one. The buddy covers its mouth and a slashed microphone shows in the corner only when every
microphone reads muted. If one stays live, Kivori says so and shows it as failed, and the Home page
names the microphone. It is the computer's microphone, not Zoom's or Teams' own mute button, so
those apps can still show their own mute as off. A red microphone ("Live") shows when an app is
using an unmuted microphone, where your computer reports that; if it does not, nothing shows.

## Customising controls and macros

Open the **Controls** page. Pick a profile, then **Edit** any control to choose a different action:
system or per-app volume, media, mute, a keyboard shortcut, launching an app, or a macro. **Test**
runs the selected action. Keyboard shortcuts, including those in macros, first show a 3-second
countdown: focus the receiving app, or choose **Cancel** before it runs. The test shows Processing
while it waits for its result. **Reset** puts the built-in action back. Changes are saved on this
computer for your Windows user.

A **macro** runs up to 8 steps in order, with optional waits. It stops at the first step that fails
and does not undo earlier steps. Create macros on the Controls page, then bind one to a control.

## Display views and the buddy

The screen is home to your buddy, with the profile, clock and control labels around it. Under
**Display** you choose the starting view and what a double press switches to. The views are Buddy,
Clock, Volume, Media and System (CPU and memory).

The buddy shows what your computer is really doing: it looks strained under heavy load, muted when
you are muted, and happy after a confirmed action. Under Display you can switch its reactions on or
off and set its intensity (Low, Normal, High).

## What the badges mean

Feedback badges have these meanings:

| Badge | Meaning |
|---|---|
| Green check | **Confirmed.** Kivori read the new state back from your computer. |
| Blue arrow | **Started.** The computer accepted it and started it (for example launching an app). |
| Amber "?" | **Unverified.** Kivori cannot confirm the final effect. |
| Red cross | **Error.** Kivori could not complete the action. Earlier macro steps may already have run; check the computer's state before retrying. |

Keyboard shortcuts and media keys remain **Unverified** in Activity. On the device they use a
quiet press/label pulse instead of an amber badge; that pulse does not confirm the final effect.

## Sleep and lock

- When your computer goes to sleep, the buddy shows **Sleeping**, then Kivori lets go of the USB
  port. On wake it reconnects by itself. Nothing you turned or pressed before sleep runs afterwards.
- Locking the screen puts the buddy to sleep. Unlocking brings back what you had.
- If the Kivori app stops or crashes, the screen shows **Offline** within about 4 seconds.
- Kivori can start with Windows and sits in the tray. The window is optional: close it and Kivori
  keeps working. Use the tray icon to open it or quit.

## Updating firmware

On the **Device** page, Kivori shows the firmware on your device and the firmware inside the app, and
says when an update is available. An update is never installed without you asking.

1. Choose **Flash firmware**, then **Flash now**.
2. Keep USB plugged in. The screen shows **Updating**. This takes a short while.
3. Kivori reconnects when it is done. A failed update says so.

## Recovery with the BOOT button

If an update failed or the device will not connect, use **Restore with the BOOT button** on the
Device page (or in setup). Connect only one Kivori, then follow the three steps in the app:

1. Unplug your Kivori from the computer.
2. Hold down the BOOT button, and keep holding it while you plug the USB cable back in.
3. Let go of the BOOT button, then choose **Restore now**.

BOOT is the pinhole in the base labelled BOOT; a paperclip reaches it.

## Troubleshooting

- **Not connecting.** Wait a few seconds, then try another USB port, plugged straight into the
  computer rather than through a hub.
- **Charge-only cable.** Some USB-C cables only carry power. Use a data cable.
- **Another program has the port.** Close serial monitors and other apps that use USB serial devices.
- **Shortcuts do nothing (macOS).** Allow Kivori under System Settings, Privacy & Security,
  Accessibility. If you switched it on and Kivori still says it is off, choose **Reset and ask
  again** in the notice on the Controls page (or in Settings, General), then switch it on once
  more.
- **macOS will not open Kivori the first time.** See Open Anyway above.
- **Still stuck.** Try the BOOT button recovery above.
- **Asking for help.** On the Device page choose **Copy diagnostics** and paste it into your message.
  It holds versions and status only: no port names, file paths or device IDs.

## Uninstall

Windows: Settings, Apps, Kivori, Uninstall. This also removes the start-with-Windows entry. Your
settings stay in your user data folder (`%LOCALAPPDATA%\id.cunilab.kivori`, plus
`%LOCALAPPDATA%\id.immer.kivori` if you used a version from before November 2026); delete them if you
want them gone. macOS: drag Kivori from Applications to the Bin.

## Privacy

Kivori works fully offline. It has no telemetry, no account and no cloud, and nothing in the app
needs the internet. Your settings and the session activity list stay on your computer.
