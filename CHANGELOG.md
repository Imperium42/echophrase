# Changelog

All notable changes to Echophrase will be documented in this file.

## [Unreleased]

### Fixed
- Fixed a rare crash on Windows when signing out, shutting down or restarting with Echophrase running.
- Clearer microphone error messages: an unplugged mic no longer shows a misleading WSL message on macOS, and the permission hint no longer points Mac users to Windows settings.

## [v1.0.0] - 2026-11-23

### Added
- **Echophrase is now free and open source** under the [Mozilla Public License 2.0](LICENSE) (MPL-2.0).
- **Everything is free and unlimited, Pro features included.** No account, no sign-in, no Pro tier and no usage limits. Everything that used to be Pro now works for everyone: custom ONNX models, Transform (grammar cleanup), the MCP server and CLI `transcribe`, JSON output with timestamps, compute-type selection, and the Customize and Recording settings.
- A "Support Me" button at the bottom of Settings, if you'd like to buy me a coffee via GitHub Sponsors.
- A "Source code" link in Settings > About.
- Push to Talk hotkey can now be tapped or held: tap to start and tap to stop, or hold it while you talk and let go to stop. Pick "Tap", "Hold" or both under Push to Talk in Settings.
- "Restore Clipboard After Pasting" in Settings > Output: after Paste at cursor, put back whatever you had copied before. Off by default; on macOS it may need extra clipboard permissions.

### Changed
- No more accounts or sign-in server. Account settings and server cloud sync are gone, and the app never forces you to update. If you used cloud sync, your settings stay on this computer as they are. Echophrase only connects to download models and check for updates.
- If you signed in before, a leftover Echophrase sign-in entry may still be in your system keychain. It does nothing, and you can delete it if you like. To have your old account data deleted, contact us via Settings > About > Support.
- Linux is now community-supported: build it from source. There are no more official Linux downloads or auto-updates, so v0.9.9 is the last official Linux build.

### Improved
- Windows 11 is the officially supported Windows version. Windows 10 should work but is unofficial.
- Bundled ONNX Runtime 1.24.4 and DirectML 1.15.4, so GPU transcription no longer depends on the older DirectML that ships with Windows.
- Open Mic is now marked Beta.
- The start hint now tells you whether to tap or hold your hotkey, based on your setting.
- When Paste at cursor leaves the text on your clipboard, Settings now shows Copy to Clipboard as on, with a tip explaining why.
- Small branding touch-up: a trademark mark next to the Echophrase name.
- Settings > Model shows your model's name and size instead of its internal id.

### Fixed
- Open mic now follows your output setting, so "Paste at cursor" pastes when the countdown ends instead of only copying.
- On Windows, pasting no longer lands in Echophrase itself when it has focus - the text goes to the app you were using.
- A brief audio glitch no longer cancels your recording with a "Not recording" error.
- Grammar correction works again on installs where a settings mix-up pointed it at the speech model.
- The "Language (UI)" label in Settings is no longer cut off.
- Restore Clipboard After Pasting is more reliable: it puts back what you had copied before.
- The tap/hold Push to Talk hotkey is more reliable: fixed missed or stuck starts and stops.
- Open mic no longer opens a save dialog for every phrase.

### Removed
- Removed anonymous usage stats entirely. Echophrase now only connects to download models and check for updates.
- The unused CUDA plugin and all CUDA code paths. Windows uses DirectML, macOS uses CoreML, Linux runs on CPU.

## [v0.9.9] - 2026-11-20

### Improved
- Minor stability improvements, so every platform gets this update together.

## [v0.9.8] - 2026-11-20

### Fixed
- If you installed Echophrase with Homebrew, Check for updates no longer says "Latest" when a newer version exists. It now tells you which version is out and shows the command to upgrade.
- Long recordings no longer make memory use balloon. Anything over about 40 seconds is now transcribed in pieces, which keeps memory low and finishes roughly twice as fast, with the same accuracy.

### Improved
- Long recordings with the multilingual model are now split at natural pauses instead of mid-word, so words at the seams are no longer cut in half.
- Hovering a Pro-only model now just shows it is a Pro perk, without sending you to the upgrade screen when you click.
- New Optimization section in Settings with an optional "Unload model, idle for X mins" setting (off by default). It frees the speech model's memory after that many idle minutes, which helps when Echophrase sits in the tray while you game. The next recording reloads it in the background (about 7 seconds).
- The longest recording you can set is now 3 minutes (down from 5), to keep memory use from spiking too high on very long clips. For best results, keep each recording under 1 minute. If you had a longer limit saved, it is lowered to 3 minutes automatically.

## [v0.9.7] - 2026-11-10

### Fixed
- Minor bug fixes and stability improvements.

## [v0.9.6] - 2026-11-10

### Added
- Custom models are now labeled "Custom ONNX Models", and on the free plan hovering the tile flips it to show it is a Pro perk - click to see what Pro includes.

### Fixed
- Finishing (or skipping) the welcome tour now plays the main screen's entrance animation instead of just showing the finished, static screen.

### Improved
- Our community links now point to Bluesky instead of X.
- The macOS Accessibility permission prompt is shorter and easier to read, links straight to System Settings, and all pop-up windows now scroll instead of getting cut off when the app window is small.
- The app no longer loads its emoji font from Google at launch. The font now ships inside the app, so the only servers Echophrase ever contacts are the ones listed in the Privacy Policy.

## [v0.9.4] - 2026-09-19

### Fixed
- The Quit button on the "update required" screen now actually quits the app.
- macOS: Homebrew installs no longer update themselves; the app points you to `brew upgrade --cask echophrase` instead.
- Windows: uninstalling or updating through Scoop no longer leaves a broken entry that made the regular installer fail with "Unable to uninstall!".

### Improved
- The "press the mic or hotkey" hint now shows at every launch until your first recording, instead of disappearing forever.
- Settings > Language (UI) moved up to sit right under Account and is now a dropdown. Simplified Chinese is labelled correctly in the list.
- The "update required" screen explains forced updates correctly for beta and non-beta users, and the Copy command button gives clearer feedback after copying.
- Pro features in Settings keep their Pro badge after you upgrade, so it is always clear what is part of Pro.

## [v0.9.3] - 2026-09-11

### Added
- Simplified Chinese (简体中文) UI. Auto-selected for mainland China, Singapore and Malaysia system locales; switchable in Settings > Language.
- Switch from monthly to annual Pro from Settings > Account or the Pro window. You see exactly what you will be charged today before confirming.

### Improved
- Settings > Custom Models now shows just your active model and a Manage button instead of repeating the full list.
- Docs links in the app now open the docs site in your app language instead of always opening the English page.

### Fixed
- Windows: the tray icon no longer drops back into the hidden overflow after an update. The app now keeps a fixed identity for the icon and the app itself is code-signed, so your "always show" choice survives even when the install folder changes (for example Scoop installs).
- Renewal and cancellation dates in Settings > Account and the Pro window now follow the app language instead of the OS language.
- You can no longer accidentally buy Pro twice on the same account; the app tells you that you already have it and shows your plan instead of a buy button.
- The Pro window now shows an error line when checkout cannot open, instead of silently doing nothing.

## [v0.9.2] - 2026-09-06

### Added
- Your account details now show your renewal date under the plan, or the end date if you have cancelled.
- A Verbose logging switch in Settings > About, for when you are reporting a bug.

### Improved
- The logo, record button and "Press the mic" hint are now centered as one group, so the main screen no longer sits low in the window.
- Buttons across the app now show a pointer cursor on hover, and the hotkey field no longer looks clickable while it is waiting for you to press keys.
- Bringing your own model is now marked Alpha instead of Coming Soon - it is available today, just still rough around the edges.

### Fixed
- The splash screen no longer fails to appear while the app is starting up.
- If your hotkey cannot be registered (usually because another app or a second copy of Echophrase holds it), Settings > Hotkeys now says so instead of silently doing nothing.
- "Check for updates" no longer reports Latest when the check itself failed, such as while offline.
- The Upgrade button in the Pro window now opens sign-in when you are signed out, instead of doing nothing.
- The PRO badge and the cancellation date no longer linger after you sign out or switch accounts.

## [v0.9.1] - 2026-08-21

### Fixed
- Entering your 6-digit email code could fail with a session error even when the
  code was correct.
- A failed sign-in attempt left its error message pinned above the form, so it
  still looked broken while you were retrying.

### Improved
- Choosing email sign-in now tucks the Google button away so the 6-digit code
  entry has room, instead of leaving both on screen at once.
- The email sign-in panel now expands and collapses smoothly instead of
  snapping open.

## [v0.9.0] - 2026-08-20

<!-- critical -->

### Added
- Version numbers no longer carry a `-beta` suffix. That is a numbering change only: Echophrase is still in beta, and the free Pro features you have during the beta are unchanged.
- You can now rename the keys in your hotkey. If your hotkey uses a key your keyboard labels differently - a mouse side button that reports as F15, say - hit the note icon next to the hotkey in Settings > Hotkeys and call it whatever you like ("M5"). Echophrase then uses that name everywhere it shows the hotkey. It is a label only and does not change the binding.

### Improved
- The app background now has a faint blue-violet glow behind the record button, instead of being flat black.
- If you installed Echophrase through a package manager, it now leaves updating to that package manager instead of updating itself, and tells you the exact command to run.

### Fixed
- Pasting at your cursor no longer requires an account. It was never a paid feature, but the setting was locked until you signed in.
- Release notes in the update window were missing their bullet points.
- Settings sections that need Pro now say so up front, instead of asking you to sign in first and only then revealing they are Pro.

## [v0.8.0-beta.75] - 2026-08-14

### Added
- Recordings can now run up to 5 minutes. The Max Duration slider in Settings > Recording fades from green to yellow to red past 1 minute, with a note explaining that longer recordings take longer to transcribe and show nothing until they finish.

### Improved
- The Voice Threshold slider is now called "Open Mic Threshold" and only shows up in Open Mic mode, since it never did anything in Push to Talk. It fades from green to red as it gets more aggressive, and warns when it is set high enough to start missing quiet speech.
- The custom model import window now starts with a simple choice (browse a local file or download from HuggingFace) instead of showing both forms at once.
- The compatibility requirements for custom models are shown as a clearer, collapsible checklist.
- Model names in download windows now link to their source page, so you can verify where the file comes from.
- Download windows now have a folder icon to jump straight to where the file is being saved.
- Pressing Escape in the custom model import window now steps back one screen at a time, instead of always closing everything.

### Fixed
- The Max Duration setting had no effect - every recording stopped at 30 seconds no matter what you picked. It also required signing in, even though the limit was never account-based.
- Picking a HuggingFace file to import could show the wrong screen instead of the download progress.
- Pausing a custom model download could show garbled model info; resuming and deleting a paused download now work correctly.
- The custom model import window could grow too tall and hide its buttons and text off-screen.
- Fixed a duplicate "back" button in the custom model import window.
- Download windows were missing download speed and time remaining while a download was in progress; custom model downloads were also missing the file size.

## [v0.8.0-beta.74] - 2026-08-10

### Added
- macOS: the welcome flow now walks through both permissions up front - Microphone (required), and Accessibility for pasting at your cursor (optional) - explaining what each is for, with a "Copy to clipboard, instead" option that skips the Accessibility request entirely, links to review either permission in System Settings, and a link to the new permissions docs page. If a paste is ever blocked, Echophrase explains why rather than leaving you with the system's vague "control your computer" prompt, and only ever asks for the narrow permission to send a paste shortcut - never to read your screen or your keystrokes.
- Anonymous app stats are now shown and confirmed on the welcome screen instead of only living in Settings, with a book icon linking to a page listing every event we send. The box starts checked, and on a new install nothing is sent until you answer it - including the one-time install count, which used to be sent regardless. You can also disable everything before the app starts by launching with `DO_NOT_TRACK=1`.

### Improved
- Turning off anonymous usage stats no longer pops up a window asking you to reconsider, and the setting is now named "Send anonymous app stats" so it says what it actually does.

### Fixed
- The Discord link in the account menu now opens a working invite.

## [v0.8.0-beta.73] - 2026-08-09

### Fixed
- Imported custom models are now always run by the correct transcription engine - some models could previously fail to load or produce garbage output.
- Fixed hover explanations in the custom model picker being cut off mid-sentence.

## [v0.8.0-beta.72] - 2026-08-08

### Added
- You can now bring your own ONNX transcription models (Pro) - import as many as you like from a local .onnx file, a direct link to one, or a HuggingFace repo URL (shows a picker to choose from when a repo has more than one .onnx file, with a hover explanation for recognized filename suffixes like int8/fp16/q4), with friendly validation, duplicate detection, a management window to switch between and remove them, and a link to docs explaining where to find models. Downloads show progress in the same minimizable indicator as other model downloads and resume automatically if interrupted.
- If you imported a custom model from HuggingFace on another device, the import window prefills its URL for you (marked with a Cloud Sync badge) so one click brings it to this machine.
- Anonymous crash reporting: if the app crashes, it sends a privacy-safe report (source location and thread name only - never the crash message, your files, or any personal data). Covered by the same telemetry opt-out in Settings > Data, and documented in the updated privacy policy.

### Improved
- Anonymous telemetry now waits until the app has been idle for 10+ minutes (or is closing) before sending anything, so no network request ever happens during or right after a transcription. Your audio and transcripts never leave your machine either way - this just makes it visibly true in any network monitor.
- All Settings sections can now be collapsed and expanded, with a chevron that clearly shows whether a section is open or closed.
- Settings are reorganized into a more intuitive order - capture settings first (microphone, recording, hotkeys), then models, then output.

### Fixed
- The Pro upgrade window now shows a robot icon next to the MCP feature (it was missing) and rewords the line to make clear the MCP server is the Pro feature, not the CLI itself.
- Fixed text alignments in the top banner.

## [v0.8.0-beta.71] - 2026-08-04

### Added
- The command-line helper (`echophrase` CLI) can now act as an MCP server, so AI agents like Claude Code can record and transcode audio for you from the terminal.
- You can now transcribe audio from the command line or via AI agents while the app is running (`echophrase transcribe`).
- New Settings section, "MCP Server" (Pro), lets you turn on a local server so the CLI and AI agents can transcribe through the running app.
- The MCP Server settings section now has a "Copy setup prompt" button that copies ready-to-paste instructions for an AI agent to configure the MCP server for you.

### Improved
- CLI and AI-agent transcription now explains exactly why it is unavailable: upgrade to Pro, enable the MCP Server setting, or start the app - instead of one generic error.
- WSL users now see a penguin icon with an explanation next to the input device and Open Mic settings, since the microphone is locked to whatever Windows default mic was active when WSL started (change it and run `wsl --shutdown` to switch).
- The input device list in Settings now refreshes every time you open Settings, so newly plugged-in or removed microphones show up without restarting the app.
- The input device dropdown in Settings is now wider so longer device names are readable instead of cut off.
- Silent or near-silent recordings now show "(No sound detected - check mic settings)" instead of pasting garbled text like "Current." or "None".

### Fixed
- Fixed Qwen3-ASR sometimes returning an empty transcription in auto-detect language mode, even for clearly spoken audio. It now retries with a default language before giving up, instead of silently dropping your speech.
- Open mic no longer discards your sentence when the voice detector briefly wavers mid-speech.
- Fixed garbled `<asr_text>` text sometimes showing up in transcriptions when using Qwen3-ASR.
- Open mic no longer silently stops transcribing after an unusually slow chunk.
- Fixed open mic occasionally double-starting from a single hotkey press and losing the recording session.
- Fixed Pro users in the beta program being unable to enable the MCP Server.
- Fixed a rare "File not found" error page showing up at startup on Windows.

## [v0.8.0-beta.70] - 2026-07-28

### Improved
- Behind-the-scenes improvements to how updates are delivered.

## [v0.8.0-beta.69] - 2026-07-28

### Added
- Mac users can now install Echophrase with Homebrew: `brew install imperium42/tap/echophrase`. Updates work the same as before.

## [v0.8.0-beta.68] - 2026-07-27

### Added
- A folder button next to Voice Model in Settings opens the folder where your downloaded models are stored, so you can check what is taking up disk space without hunting for the path.

### Improved
- Transcribed text now appears centered in the preview.

### Fixed
- The app window is no longer huge on first launch on Mac (and on Windows displays set above 100% scaling). The interface was doubling the system's display scaling on top of itself; it now opens at the normal size, and you can still make everything bigger or smaller under Settings > Accessibility > Manual.
- On Mac, the window now opens centered on screen instead of down toward the bottom.
- Your chosen voice model now sticks after a restart. Picking Qwen3-ASR (including when the app recommended it for you) would quietly go back to Parakeet the next time you opened the app, so dictation used a different model than the one Settings showed.
- The app no longer warns that the model failed to load when it actually loaded fine. On startup two parts of the app could try to load the model at once, and the loading indicator could get stuck even though your first dictation was ready to go.
- On systems set to Simplified Chinese, output now defaults to Simplified characters instead of Traditional.
- Linux and WSL machines with no language configured are no longer treated as non-English, so they get the recommended English model on first launch instead of the multilingual one.

## [v0.8.0-beta.66] - 2026-07-24

### Added
- APAC and Chinese language support. Dictate in Mandarin and get Chinese characters, with your choice of Traditional or Simplified output in Settings.
- New Qwen3-ASR 0.6B voice model, available on every tier. It handles Chinese and other non-European languages that the existing models could not, and you can tell it which language you are speaking or let it detect that for you.
- Pro cloud settings sync. Turn it on per device to keep your preferences in sync across your computers - it stays off until you enable it.

### Fixed
- Changing one setting no longer resets another. Picking a voice model and then changing your app language could silently switch the model back, so dictation kept using the previous model no matter what the app displayed.
- Switching voice models now loads the new model right away instead of leaving the old one in memory, so your first dictation after a switch is fast and the loading indicator reflects what is actually happening.
- Fixed the app sometimes staying hidden after you tried to open it, leaving it stuck minimized where you could not see the window.

## [v0.8.0-beta.65] - 2026-07-19

### Added
- You can now adjust the size of the app's interface from Settings. Make text and controls larger for easier reading, or smaller to fit more on screen - great for high-resolution displays or accessibility.

### Improved
- Pro features now keep working offline for much longer before you need to reconnect, so travel or spotty internet won't interrupt your Pro experience.

## [v0.8.0-beta.64] - 2026-07-15

### Improved
- Behind-the-scenes stability and performance improvements, including a faster and more reliable app foundation.

## [v0.8.0-beta.62] - 2026-07-14

### Fixed
- Fixed microphone recording on Windows Subsystem for Linux (WSL), where the app couldn't detect or record from your microphone. Recording now works out of the box, and you can pick a specific input device if you have more than one.

## [v0.8.0-beta.60] - 2026-07-14

### Improved
- The macOS app is now fully signed and notarized by Apple, so it installs cleanly without the "damaged" or "unidentified developer" warning.
- Your saved sign-in is now protected using your system's built-in secure keychain, adding an extra layer of security to your account.

### Fixed
- General security and stability hardening under the hood.

## [v0.8.0-beta.59] - 2026-07-13

### Improved
- macOS builds are now signed and notarized with our official Apple Developer ID, so Gatekeeper recognizes the app as coming from a verified developer instead of showing an "unidentified developer" warning.

## [v0.8.0-beta.58] - 2026-07-03

### Fixed
- Restored the Linux download. A hiccup while publishing the previous build left Linux without an available download; this release brings it back and makes publishing more resilient so it doesn't happen again.

## [v0.8.0-beta.56] - 2026-07-02

### Fixed
- The "Append space to incomplete sentences" setting now actually works. Previously, even with it on, a dictation that didn't end with a period, question mark, or exclamation point was copied without a trailing space, so pausing mid-sentence made your words run together. Pauses now flow naturally as intended.

## [v0.8.0-beta.55] - 2026-07-02

### Improved
- The live transcript preview now appears just below the record button, wider and centered, so it fits much more of what you said before trimming. The text is crisper and easier to read, and you can click it to expand the full transcript in a scrollable panel right where you're looking.

### Fixed
- Fixed the startup screen showing a "File not found" error on Windows.

## [v0.8.0-beta.54] - 2026-06-30

### Added
- New "Append space to incomplete sentences" setting (on by default): when a dictation doesn't end with a period, question mark, or exclamation point, a space is added automatically so you can pause mid-sentence and keep dictating without your words running together. You can turn it off in Settings under Recording.

### Fixed
- While an update is installing, the app now shows a full-screen "Installing" screen so you can't accidentally start a recording in the brief moment before the app restarts.
## [v0.8.0-beta.53] - 2026-06-23

### Changed
- **macOS is now in Beta** (promoted from Alpha). Apple Silicon (M1+) joins Windows on the Beta channel; the download page, docs, and in-app messaging reflect the new status.

### Added
- macOS: a polished new Dock icon that automatically matches your light or dark appearance.
- macOS: the menu bar icon now reflects what the app is doing, showing when it's recording or transcribing at a glance.
- macOS: the record hotkey now shows native Mac modifier glyphs (e.g. ⇧ ⌘ Space) in the correct Apple order, and the default hotkey on macOS is Cmd+Shift+A. You can also use the Mac dictation key (F5/mic) to start recording.

### Fixed
- macOS: you can now set hotkeys that include the spacebar (e.g. Cmd+Opt+Space) - the spacebar was previously dropped and produced an invalid shortcut. Pressing an impossible combo now shakes the button instead of saving it.
- Signing in is now more reliable and no longer occasionally stalls when opening your browser.
- Recording is now correctly paused while you're signed out, so you're prompted to sign in instead of recording silently.

## [v0.8.0-beta.52] - 2026-06-10

### Fixed
- Fixed an issue where the upgrade button could fail to open the checkout page if you had been signed in for a while. It now reliably takes you to checkout.

## [v0.8.0-beta.51] - 2026-06-08

### Fixed
- Your first recording right after installing no longer fails with a "model still loading" error while the app warms up on Windows. It now waits for warmup to finish and transcribes automatically, no manual retry needed.
- Tidied up the upgrade screen so it loads cleanly without anything popping in after it opens.

## [v0.8.0-beta.50] - 2026-06-06

### Fixed
- Fixed an issue where free beta accounts were shown as fully paid Pro members. Beta users now correctly see the "Pro (Beta)" label and the option to upgrade to full Pro again.

## [v0.8.0-beta.49] - 2026-06-01

### Fixed
- Further fix for the "model still loading" error some Windows users hit on the first recording after launching the app. The installer now ships the correct graphics acceleration files in the right place, so the app picks them up reliably on every start.

## [v0.8.0-beta.47] - 2026-05-30

### Fixed
- Fixed model load hanging on Windows when CUDA was installed alongside DirectML. The app now respects the GPU you selected in Settings instead of silently loading a CUDA-only runtime over your DirectML choice.

## [v0.8.0-beta.46] - 2026-05-30

### Fixed
- Fixed an issue on Windows where the "Loading model..." screen could hang forever on some GPUs. The model now loads in seconds and the app becomes usable right away.

## [v0.8.0-beta.45] - 2026-05-29

### Fixed
- Reliability fixes for the in-app update flow.

## [v0.8.0-beta.44] - 2026-05-29

### Improved
- During the beta, the app now reliably prompts you when a newer version is out and lets you update with a single click, right inside the app - no trip to the website needed.

## [v0.8.0-beta.43] - 2026-05-29

### Improved
- Faster, more reliable first launch on the newest NVIDIA graphics cards (RTX 50-series). The one-time GPU setup is now allowed to finish properly, so it no longer gets cut short and forced to start over on every launch. After the first successful setup, the app starts quickly as expected.
- The update prompt now makes it clear that updates are required while Echophrase is in beta.

### Fixed
- Fixed a misleading "model ready" message that could appear while the app was actually still loading.
- If you start recording before the model has finished loading, the app now lets you know it's still getting ready instead of appearing to hang.

## [v0.8.0-beta.40] - 2026-05-29

### Added
- You can now manage your subscription right from inside the app.

### Improved
- Upgrading to Pro is smoother, and your Pro status now shows up reliably right after you subscribe.
- Clearer messaging when the beta period ends.

## [v0.8.0-beta.39] - 2026-05-25

### Improved
- "Paste at cursor" output mode is now available to all signed-in users, not just Pro subscribers. Sign in for free to use it.
- The branding area at the top of the app is now cleaner and more compact.

### Fixed
- Fixed a rare case where the record button could appear clickable during the moment the app was still loading, before it was ready to accept input.
- Fixed an intermittent hang where transcription would freeze if two recordings finished very close together. The second one would get stuck and never complete.

## [v0.8.0-beta.38] - 2026-05-21

### Added
- New "Reset Window Size" button in Settings > Customize that snaps the app back to its default size and re-centers it on screen.

### Improved
- Windows GPU acceleration is more reliable - DirectML now picks the right graphics device more consistently on first launch.
- Transcription preview text automatically scales down for longer lines and wraps more gracefully, so nothing gets clipped off the side.
- The version badge in the corner is now compact (just "beta.38" instead of the full string) so it stays out of the way.
- Release notes window is a touch wider for easier reading.
- Cold server connections on slow networks get a bit more breathing room before timing out, so the connection indicator is less jumpy right after launch.
- The app now always starts connected to our staging service, which fixes a confusing "Dev Server Offline" message some users saw after a fresh install.

### Fixed
- Quitting from the system tray menu now closes the app cleanly.

## [v0.8.0-beta.37] - 2026-05-18

### Fixed
- macOS (alpha): build packaging fix so macOS users can install the latest fixes.

## [v0.8.0-beta.36] - 2026-05-18

### Fixed
- macOS (alpha): catches macOS users up to the fixes that shipped to Windows and Linux in beta.35.

## [v0.8.0-beta.35] - 2026-05-18

### Fixed
- Windows: transcription no longer freezes on first use. The "processing" spinner used to get stuck forever and eventually fail with a timeout - now it waits properly while the GPU warms up the first time and then transcribes as expected.
- Transcribing long recordings on slower hardware no longer cuts off at five minutes. The timeout now scales with how long you spoke and how fast your machine is, so a one-hour recording on a CPU has room to finish.
- Recordings on Windows now get cleaned up after transcription, instead of slowly piling up in your app data folder.

### Added
- A "Setting up GPU acceleration" hint appears during the first launch on Windows so it's clear the app is doing one-time setup, not frozen.
- macOS: clearer permission prompts the first time you use the global hotkey or paste-at-cursor, with proper reason text explaining what the app needs.

### Improved
- The "Update Now" button in the update window now has visible hover and click feedback.
- Behind-the-scenes stability and performance improvements.

## [v0.8.0-beta.34] - 2026-05-16

### Fixed
- macOS: after granting microphone permission, the app no longer falsely claims "Microphone not detected" - clicking Record now works straight through.
- macOS: the "Audio Settings" button in the microphone error now actually responds to clicks and hover, and jumps to System Settings > Privacy & Security > Microphone.

## [v0.8.0-beta.33] - 2026-05-15

### Fixed
- Re-shipping the macOS microphone permission fix from beta.32 - the previous build didn't compile for macOS due to a small type mismatch in the new permission bridge.

## [v0.8.0-beta.32] - 2026-05-15

### Fixed
- macOS now properly asks for microphone permission the first time you open the app, instead of silently saying "Microphone not detected" with no system prompt ever appearing.
- On macOS, the "Audio Settings" button in the microphone error now jumps straight to System Settings > Privacy & Security > Microphone, where the actual fix lives.

### Added
- A new microphone permission step in first-run setup on macOS, so you can grant access before you even try to record.

## [v0.8.0-beta.31] - 2026-05-14

### Improved
- Slimmed down the macOS download by removing duplicated files inside the app bundle.

## [v0.8.0-beta.30] - 2026-05-14

### Fixed
- Behind-the-scenes packaging fixes so macOS users get proper builds and in-app updates.

## [v0.8.0-beta.29] - 2026-05-14

### Fixed
- Behind-the-scenes packaging fixes so the macOS build includes everything it needs to run.

## [v0.8.0-beta.28] - 2026-05-14

### Added
- Sign-in emails now include a 6-digit code as a backup to the magic link. Enter it directly in the app if the link doesn't open for any reason. Paste it from anywhere - the code fills in instantly and signs you in.

### Improved
- The "Send magic link" button now shows a friendly countdown when you've recently requested one, so you know exactly when you can try again instead of seeing a generic error.

### Fixed
- The "Audio Settings" link in the microphone-not-detected message now opens the correct settings section when clicked.
- First-time microphone access on macOS now properly asks for permission, instead of silently failing with no popup.

## [v0.8.0-beta.27] - 2026-05-13

### Improved
- Polished the installer's appearance for a cleaner first-run experience.

## [v0.8.0-beta.26] - 2026-05-13

### Improved
- Double-click the tray icon to bring Echophrase back to the front when it's tucked away in the background.
- Behind-the-scenes release pipeline improvements.

## [v0.8.0-beta.25] - 2026-05-12

### Added
- New required-update screen that appears when your version is too old to keep using safely. It explains why an update is needed and walks you through downloading the latest version.
- The app now minimizes to the system tray when you close the window, so it keeps quietly running in the background. The first time this happens, a small notification lets you know where to find it.
- Past release notes are now browsable from the Update screen, so you can scroll back through what changed in earlier versions.

### Fixed
- The Update Available and Release Notes screens no longer get hidden behind the Settings panel.
- The tray menu no longer gets stuck saying "Stop Recording" - it now accurately reflects whether recording is on or off.
- Release notes now render with proper formatting (headings, bullets, links) instead of raw text.

## [v0.8.0-beta.24] - 2026-05-09

### Fixed
- The Google logo on the sign-in button now uses Google's official multicolor mark instead of a flat blue silhouette. The same logo also appears on the linked-accounts list in Settings.
- The welcome glow that pulses around your avatar after sign-in is now properly centered and rounded - no more boxed edges or upward drift.

### Improved
- Behind-the-scenes stability and performance improvements across sign-in, settings, and account handling.

## [v0.8.0-beta.23] - 2026-05-08

### Improved
- Icons across the app now look sharper and more consistent - the same crisp icon family used on our website. Affects settings panels, banners, modals, the account menu, the record button, and toasts.
- Cleaned up the account menu in the corner: the Settings and Sign out icons now align in a clean vertical column with your avatar above.
- Tucked a small scroll icon next to the version in the corner that opens the release notes.

### Fixed
- Fixed a glitch that could prevent the Pro upgrade screen and a few banners from rendering.

## [v0.8.0-beta.22] - 2026-05-08

### Added
- New server-controlled MOTD banner. When the server has a message set, an elegant amber banner appears below the top-left badges and above the logo. Supports `[text](url)` markdown links that open in the system browser.
- Debug panel "MOTD" toggle (dev only) to preview the banner with a mock message.

## [v0.8.0-beta.21] - 2026-05-07

### Fixed
- Settings rows now highlight on hover across the entire panel (Output, Recording, Audio Input, Debug). Previously only Transform Model and a few others lit up.
- Settings hover feels snappy instead of laggy - highlight is instant on hover-in with a smooth fade-out on leave.
- Settings scroll smoothness regression from beta.20.
- About section no longer flickers when hovering Docs link.
- GPU section unhandled-rejection on TDZ access (carried over from beta.20 polish).

### Added
- New in-app Update Available modal with current/new version display, release notes, download progress, and retry on failure.
- Localization keys for the new updater UI (en + zh-TW).

### Changed
- Continued Svelte 5 runes migration across hot components (AccountMenu, AudioVisualizer, ConfirmModal, ModelDownloadProgress, ModelSelectorButton, OutputToasts, UpdateBanner, HotkeySelector).
- Updater store reworked for the new modal flow.

## [0.8.0-beta] - 2025-12-21

### Core Features

**Local-First Speech-to-Text**
- All voice processing happens on-device - never sent to external servers
- GPU-accelerated transcription via CUDA with ~0.9s average transcription time
- Privacy-first architecture - your voice data stays on your machine

**Transcription Engine**
- Candle ML framework with Distil-Whisper models (safetensors format)
- Automatic silence detection - stops recording when you stop speaking
- Multiple output formats: Plain Text, Timestamps, JSON with metadata
- Real-time word-by-word preview during transcription

**Recording Features**
- Push-to-talk with global hotkey (works in any app)
- Real-time VAD (Voice Activity Detection) meter with dB scale
- Visual recording indicator with animated effects
- Configurable max recording duration (5s-120s)

**Model System**
- Built-in models: distil-whisper/distil-large-v3, openai/whisper-large-v3-turbo
- Model selector with accuracy vs speed tradeoffs
- Automatic model download with progress UI and resumable downloads
- Partial download support for interrupted transfers

**Output Actions**
- Copy to clipboard (always available)
- Paste at cursor (Pro) - auto-types transcription into focused app
- Save to file (Pro) - exports with configurable path

### Settings

**Audio Input**
- Microphone device selection
- Real-time VAD meter visualization
- Voice threshold slider with test mode

**Recording**
- Max duration selector (5s, 15s, 30s, 60s, 90s, 120s)
- Hotkey configuration with visual capture

**Output**
- Format selection (Text, Timestamps, JSON)
- Post-record action toggles
- JSON metadata template editor with live preview

**Model**
- Current model display with stats
- Model change with automatic download

**Data**
- Help improve Echophrase toggle (anonymous telemetry)

**Debug (Dev Only)**
- Server status display
- Cache clearing tools
- Session management

### Account & Authentication

**Google OAuth2**
- One-click Google login via Xsolla
- Secure logout with session cleanup
- Visual login/logout states with loading indicators

**Session Management**
- Access tokens with 24-hour expiry
- Refresh tokens (14-day lifetime) stored server-side only
- Automatic token refresh before expiration
- AES-256-GCM encrypted local token storage

**Offline Policy**
- Full functionality without internet connection
- Graceful degradation when server unavailable
- Optimistic health checks (no flash of "offline" state)

### Subscription & Pricing

**Free Tier**
- Core transcription features
- Copy to clipboard output
- Basic models
- Local-only processing

**Pro Tier**
- Paste at cursor (auto-type)
- Save to file
- All premium models
- Priority support

**Feature Gates**
- Login-required features marked with badge
- Pro-required features marked with upgrade badge
- Upgrade prompts with one-click flow to website

### Backend (echophrase-server)

**Infrastructure**
- Supabase PostgreSQL database
- ASP.NET Core Minimal APIs
- Xsolla integration (Login, Paystation, Entitlements)

**API Endpoints**
- Health check with latency monitoring
- OAuth initiation and callback handling
- Token refresh endpoint
- User entitlements query

### UI/UX

**Visual Design**
- Dark theme with lime-green accent (#84cc16)
- Spotify-inspired minimalist aesthetic
- Smooth animations and transitions
- Responsive hover states

**Components**
- Settings modal with collapsible sections
- Pro upgrade modal with animated background
- Feature gate banners with urgency levels
- Model download progress overlay
- Boring Avatars for user profiles

**Performance (This Release)**
- Eliminated GPU layer flicker on hover
- Replaced backdrop-filter blur with solid backgrounds
- Optimized CSS transitions for specific properties
- Removed transform scale/translateY hover effects

### Platform Support

**Primary**
- Windows 10/11 with WSL2 (GPU passthrough)
- Native Linux with NVIDIA GPU

**Requirements**
- NVIDIA GPU with CUDA 12.0+ and cuDNN 9.5.1+
- Minimum 2GB VRAM for quantized models
- Compute Capability 7.0+ (RTX 20-series or newer)

### Technical Stack

**Frontend**
- Svelte 5 + SvelteKit with TypeScript
- Tailwind CSS 4.x with shadcn-svelte components
- Vite 7.x build system
- Bun package manager

**Desktop**
- Tauri 2.0 (Rust)
- tauri-plugin-mic-recorder for audio capture
- tauri-plugin-clipboard-manager for output
- tauri-plugin-global-shortcut for hotkeys

**AI/ML**
- Candle ML framework (Rust)
- Distil-Whisper INT8 quantized models
- CUDA-accelerated inference
