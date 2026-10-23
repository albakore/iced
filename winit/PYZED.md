# iced_winit 0.14.1 for Pyzed

Copied from crates.io (MIT, © Héctor Ramón and the iced contributors) for the Android prototype.
The changes are marked `pyzed:`:

- `src/lib.rs`: `android::set_app` keeps the `AndroidApp` that `android_main` receives, and
  `run` hands it to winit's event loop (winit needs it on Android).
- `src/conversion.rs`: Android has no `KeyEventExtModifierSupplement`, so keys are read as on Wasm.
