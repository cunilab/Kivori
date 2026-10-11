# ADR-0011: Mic mute mutes every active capture device

- **Status:** Accepted
- **Date:** 2026-10-11
- **Milestone:** M2.6
- **Related:** [ADR-0008](./0008-desk-state-as-two-new-messages.md)

## Context

"Muted" must mean no microphone can hear you.

## Decision

**Windows.** Enumerate active capture endpoints (`IMMDeviceEnumerator::EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)`), `IAudioEndpointVolume::SetMute` on each, then `GetMute` on each; State Confirmed only if all read muted. An `IAudioEndpointVolumeCallback` per endpoint reports outside changes as External, like volume today. While mic mute is on, a capture device that arrives (`IMMNotificationClient`) is muted and read back too, because the user's intent covers it; if that fails the state is live, not muted. "Mic in use" reads the privacy consent store (`CapabilityAccessManager\ConsentStore\microphone`, an app entry with `LastUsedTimeStop` 0); it is undocumented, so unreadable means unknown.

**macOS.** `kAudioDevicePropertyMute` on the input scope. "In use" is any process object other than Kivori's own (`kAudioHardwarePropertyProcessObjectList`) with `kAudioProcessPropertyIsRunningInput` set, so a headset that only plays audio is not Live; the devices a running-input process lists on `kAudioProcessPropertyDevices` (input scope, aggregates expanded) are the ones being captured. Without the process API it falls back to `kAudioDevicePropertyDeviceIsRunningSomewhere` on input-only devices; a duplex device is then unknown. A device with no mute property (the iPhone's Continuity microphone is always listed while the phone is near) counts as muted only while observed not capturing, and only beside at least one device that reads muted; capturing or unknown makes the state live and the press Error naming it.

The desk re-reads capture state on its 1 s sample, so a guarded device that starts recording turns the state live within about a second.

**Wire.** Mic state travels behind a new capability in its own message, because `Status` is a fixed struct (ADR-0008).

Still never a quiet switch to input volume 0 (invariant 19): it is not mute and automatic gain can raise it.

## Alternatives considered

- App shortcuts: rejected, per app, Unverified.
- Muting only the default device: rejected, another app may record from a different mic, so "muted" would be false.
- Input volume 0: rejected, not mute; automatic gain can raise it.
