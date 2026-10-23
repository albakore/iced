# iced_winit 0.14.1 for Pyzed

Copied from crates.io (MIT, © Héctor Ramón and the iced contributors) for the Android prototype.
The changes are marked `pyzed:`:

- `src/lib.rs`: `android::set_app` keeps the `AndroidApp` that `android_main` receives, and
  `run` hands it to winit's event loop (winit needs it on Android).
- `src/conversion.rs`: Android has no `KeyEventExtModifierSupplement`, so keys are read as on Wasm.
- `src/lib.rs`: on Android the events wait for the first `resumed` (a window has nothing to draw on
  before it). When the app goes to the background (`suspended`) the windows drop their surfaces,
  and when it comes back they get new ones and redraw. `Window::surface` is an `Option` for that.
- `src/window.rs`: a tap while an input keeps the focus shows the soft keyboard again (the user
  may have closed it with the back button).
