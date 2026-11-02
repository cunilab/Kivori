# Windows desktop validation

Use an installed release build and a connected Kivori device. Record the Windows version, app
version, and build source with the results.

## Launch and background operation

1. Launch Kivori from the Start menu. Check whether an extra console appears and, if it does,
   note its title, process, and when it appears. Repeat with the development launch separately.
2. Minimize the window; check the device buttons and knob. Close the window with **X** and check
   them again, then unplug and reconnect the device and repeat.
3. Open Kivori from the tray and by launching it again. Confirm the existing window opens and the
   tray icon is discoverable. Choose **Quit** from the tray and confirm the process exits.
4. Flash firmware and confirm the update completes without an extra console. Development builds
   retain their console for diagnostic output.

## Windows security warning

Record the exact warning and its source: SmartScreen, Microsoft Defender Antivirus (see Windows
Security → Protection history), or Smart App Control. Include the detection name if present, the
affected file name, app version, and steps to reproduce. Avoid sharing personal file paths.
Check whether the file is `Kivori.exe`, `espflash.exe`, the installer, or another component;
the current report has not established which one is affected.

Calculate the affected file's SHA-256 with PowerShell's `Get-FileHash -Algorithm SHA256` and
compare it with the release artifact or the corresponding bundled file before investigating it.
Do not assume a warning is a false positive. If a specific clean file is detected by Defender
Antivirus, submit it for review through [Microsoft's file submission portal](https://www.microsoft.com/en-us/wdsi/filesubmission).
That review does not establish SmartScreen reputation. Do not disable Windows protection or add
broad exclusions as a workaround.

[SmartScreen reputation](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation)
and Smart App Control are separate from antivirus detections. The warning must be identified before
choosing a mitigation; the current report does not yet identify it.

A possible no-certificate distribution route is an MSIX submitted to the Microsoft Store, which
signs it after certification. Investigate whether USB access, firmware flashing, startup, and updates
work before choosing that route. The Store's existing EXE/MSI route still requires the publisher to
sign the installer and executable files; uploading the current unsigned NSIS installer does not
avoid that requirement. See [Microsoft's distribution comparison](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/choose-distribution-path).
