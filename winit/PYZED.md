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
- `src/window.rs`: animation frames (`RedrawRequest`) wait for the frame rate limits of
  `Settings::rendering` (`max_fps`, and `background_fps` without the focus, where 0 pauses them),
  and a window covered whole (`Occluded`) skips its frames; both draw the skipped frame when the
  window gets the focus back or shows again. Input and new content still draw at once.
- `src/lib.rs`: `quiet::set_unchanged` names the windows a message did not change after each
  update; they keep their layout and are not redrawn. `quiet::graphics` returns the adapter and
  backend of the compositor.
