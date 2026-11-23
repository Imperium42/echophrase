# Trademarks

Echophrase's source code is licensed under the [Mozilla Public License 2.0](LICENSE).
**The license covers the code, not the name or the logo.** MPL-2.0 section 2.3 says it
grants no trademark rights.

- **Echophrase™** (US trademark application, serial no. 50228738) and the Echophrase
  logo are trademarks of Imperium42 LLC.
- **Imperium42®** is a registered trademark of Imperium42 LLC (USPTO serial no. 87242524).

## What you can do

- Say that your project is "based on Echophrase" or "a fork of Echophrase".
- Use the name to refer to this project accurately (articles, reviews, tutorials, package
  manager entries that install the official builds).
- Build and run Echophrase from source for yourself or your organization.

## What needs a different name

If you **distribute** a modified build or a fork, give it a different name and icon, and
don't present it as official or endorsed by Imperium42 LLC. That keeps users from
confusing your build, and its bugs and support channels, with ours. In practice:

1. Change `productName`, `identifier` and the window title in `src-tauri/tauri.conf.json5`.
2. Replace the icons in `src-tauri/icons/` and `static/`.
3. Point the updater (`plugins.updater` in `tauri.conf.json5`) at your own update feed, or
   disable it. Official builds update from the official channel only.

Questions or permission requests: https://echophrase.com/contact
