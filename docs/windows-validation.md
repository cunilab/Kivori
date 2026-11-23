# Windows desktop validation

## Windows security warning

Record the exact warning and its source: SmartScreen, Microsoft Defender Antivirus (see Windows
Security → Protection history), or Smart App Control. Include the detection name if present, the
affected file name, app version, and steps to reproduce. Avoid sharing personal file paths.
Check whether the file is `Kivori.exe`, the installer, or another component.

Calculate the affected file's SHA-256 with PowerShell's `Get-FileHash -Algorithm SHA256` and
compare it with the release artifact or the corresponding bundled file before investigating it.
Do not assume a warning is a false positive. If a specific clean file is detected by Defender
Antivirus, submit it for review through [Microsoft's file submission portal](https://www.microsoft.com/en-us/wdsi/filesubmission).
That review does not establish SmartScreen reputation. Do not disable Windows protection or add
broad exclusions as a workaround.

[SmartScreen reputation](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation)
and Smart App Control are separate from antivirus detections. Identify the warning before choosing a mitigation.

The Microsoft Store signs MSIX packages after certification. The Store's existing EXE/MSI route still requires the publisher to
sign the installer and executable files; uploading the current unsigned NSIS installer does not
avoid that requirement. See [Microsoft's distribution comparison](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/choose-distribution-path).
