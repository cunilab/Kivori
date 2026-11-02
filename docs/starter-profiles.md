# Starter profiles

The Controls page includes editable starter profiles. Each profile can be pinned with **Use this profile**, then released with **Follow foreground app**. The original device button pin cycle is unchanged by default; turn on **In pin cycle** for any starter you also want to select from the device. Pinning lasts until it is released or the application restarts. Protected foregrounds still suspend shortcut actions.

The creative profiles match known application IDs; if a version is not recognized, use **Add app** and choose the running app. Shortcuts depend on the application's version, focused panel and keymap. Review and edit the controls before using them; reset the profile to restore its starter settings. Defaults below use Ctrl on Windows and Command on macOS where shown as Ctrl/Command.

| Profile | Buttons (left, middle, right) | Knob |
| --- | --- | --- |
| CapCut | Play (Space), split (Ctrl/Command+B), unbound | System volume |
| Photoshop | Undo (Ctrl/Command+Z), redo (Ctrl/Command+Shift+Z), brush (B) | System volume |
| After Effects | Undo, redo, unbound | System volume |
| Premiere Pro | Unbound, play (Space), unbound | Frame step (left/right arrows) |
| Blender | Undo, redo, unbound | System volume |
| Claude Code | Search history (Ctrl+R), transcript (Ctrl+O), unbound | Prompt history (up/down) |
| Codex | Search history (Ctrl+R), copy last output (Ctrl+O), unbound | Prompt history (up/down) |
| OpenCode | Unbound | Navigate (up/down, context-dependent) |

Agent profiles have no automatic app match: Kivori sees the terminal or editor window, not necessarily the agent inside it. Pin the right profile before focusing the agent; choose **Follow foreground app** when finished. Terminal or agent keymap changes may require editing the defaults. OpenCode's leader-key sequences cannot be represented as a single shortcut, so those controls start unbound.

References: [CapCut](https://www.capcut.com/resource/pc-professional-video-editor), [Photoshop](https://helpx.adobe.com/photoshop/desktop/get-started/set-up-toolbars-panels/use-undo-redo-commands.html), [After Effects](https://helpx.adobe.com/after-effects/desktop/get-started/keyboard-shortcuts/keyboard-shortcuts-reference.html), [Premiere Pro](https://helpx.adobe.com/premiere/desktop/get-started/set-up-accessibility-features/navigate-premiere-using-the-keyboard.html), [Blender](https://docs.blender.org/manual/en/5.2/interface/keymap/blender_default.html), [Claude Code](https://code.claude.com/docs/en/interactive-mode), [Codex](https://developers.openai.com/codex/cli/reference/), and [OpenCode](https://opencode.ai/docs/keybinds/).
