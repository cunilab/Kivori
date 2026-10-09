; Tauri NSIS installer hooks (bundle.windows.nsis.installerHooks).
;
; Launch at login (M3 S4) writes HKCU\...\Run\Kivori through tauri-plugin-autostart (auto-launch 0.5:
; the value name is the app's productName, "Kivori"). The installer is per-user (installMode
; currentUser), so HKCU is the right hive. Without this hook an uninstall would leave a Run entry that
; points at a deleted exe. The second key is where Windows records the Startup-apps toggle.
!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Kivori"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "Kivori"
!macroend
