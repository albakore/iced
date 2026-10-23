# Iced for Pyzed

The `pyzed-0.14` branch is iced 0.14 (the `0.14` branch: 0.14.1 and one fix) with Pyzed's own
changes and pull requests from the iced community that were still open upstream. Pyzed pins a
commit of this branch in its `Cargo.toml`.

Each pull request is one commit that keeps its author. When one had to change to fit 0.14, the
commit says so, or a following commit named `Adapt #N` does it. The branch builds with all
features and the workspace tests pass.

## Pyzed's changes

They are the first commits of the branch, marked `(Pyzed)`, and the code says `pyzed:` where it
changes iced.

- Effect layers in both renderers: a widget can draw part of the tree into a layer composited
  with a rotation, an opacity, a blur, a bloom or a blurred backdrop (`effect.rs` in each crate).
- tiny-skia clips box shadows to the region being redrawn.
- Both renderers keep the alpha of an SVG's tint color.
- wgpu starts the native GPU APIs (Vulkan, Metal, DirectX 12) before OpenGL, and steps aside for
  tiny-skia when the adapter runs on the CPU (llvmpipe, lavapipe, WARP, SwiftShader).
- `iced_graphics::text::set_tab_width` sets the columns between tab stops of editors and paragraphs.
- `iced_winit::quiet` lets the app handle a message before the windows are rebuilt, and
  `iced_winit::android::set_app` hands the Android activity to the event loop; on Android the
  windows wait for the activity to resume, drop their surfaces in the background and get new ones
  when the app comes back, and a tap on a focused input shows the soft keyboard again (see
  `winit/PYZED.md`).

- `iced::Settings::rendering` (`iced::settings::Rendering`) tunes how windows are drawn and paced:
  the samples of the antialiasing (2x to 16x), the presentation mode (fifo, fifo_relaxed, mailbox,
  immediate, falling back when the surface lacks it), the power preference, a GPU picked by name,
  the GPU memory hints, the frame latency, and frame rate limits for animations in focused and
  unfocused windows (0 pauses them). Environment variables override each field (`ICED_MSAA`,
  `ICED_PRESENT_MODE`, `WGPU_POWER_PREF`, `WGPU_ADAPTER_NAME`, `ICED_MEMORY`, `ICED_FRAME_LATENCY`,
  `ICED_MAX_FPS`, `ICED_BACKGROUND_FPS`).
- Windows covered whole by others (`Occluded`, on X11, macOS, iOS and the Web) skip their frames and
  draw once when they show again.
- `iced_winit::quiet::set_unchanged` lets the app name the windows a message did not change: they
  keep their layout (`UserInterface::build_unchanged`, the cache now keeps it) and are not redrawn.
  `iced_winit::quiet::graphics` tells which GPU and backend draw the windows.

## Community pull requests (81)

- [#3081](https://github.com/iced-rs/iced/pull/3081) Add `Mouse::Scroll` instruction to `tester`
- [#3085](https://github.com/iced-rs/iced/pull/3085) Resume recording interactions after asserting in `tester`
- [#3139](https://github.com/iced-rs/iced/pull/3139) Add window blur support for Windows 11
- [#3140](https://github.com/iced-rs/iced/pull/3140) Add back iter_mut over layers in a `Stack`
- [#3144](https://github.com/iced-rs/iced/pull/3144) add taskbar_icon to PlatformSpecific struct on Windows
- [#3148](https://github.com/iced-rs/iced/pull/3148) Fix drawing order when pasting frames
- [#3157](https://github.com/iced-rs/iced/pull/3157) Ensure widget layout is recomputed on resize
- [#3159](https://github.com/iced-rs/iced/pull/3159) Table styling additions
- [#3169](https://github.com/iced-rs/iced/pull/3169) Fix missing redraw request in `PaneGrid`
- [#3171](https://github.com/iced-rs/iced/pull/3171) add Default derives to text editor Position and Cursor
- [#3172](https://github.com/iced-rs/iced/pull/3172) Mark `Space` widget as `Copy` and `Clone`
- [#3174](https://github.com/iced-rs/iced/pull/3174) Add option to guess image format from file header
- [#3176](https://github.com/iced-rs/iced/pull/3176) Add `Task::discard_{none, err, ok}` helpers
- [#3179](https://github.com/iced-rs/iced/pull/3179) fix: set sensor widget distance to 0 when visible
- [#3180](https://github.com/iced-rs/iced/pull/3180) Switch operations order in text_input
- [#3184](https://github.com/iced-rs/iced/pull/3184) Fix keyed items in list re-creating state when new one is added to the end
- [#3191](https://github.com/iced-rs/iced/pull/3191) Change feature gates from `image` to `image-without-codecs`
- [#3193](https://github.com/iced-rs/iced/pull/3193) Markdown text color
- [#3196](https://github.com/iced-rs/iced/pull/3196) Sort using more deterministic algorithm
- [#3197](https://github.com/iced-rs/iced/pull/3197) Added text_input Arrow Up/Down actions
- [#3215](https://github.com/iced-rs/iced/pull/3215) Fix panic when atlas upload size is zero
- [#3217](https://github.com/iced-rs/iced/pull/3217) Fix unfocus operation not being accessible
- [#3220](https://github.com/iced-rs/iced/pull/3220) Fix: sensor should emit `on_show` every time it pops into view
- [#3221](https://github.com/iced-rs/iced/pull/3221) fix: Use adjusted instead of absolute cursor position in `TextInput`
- [#3225](https://github.com/iced-rs/iced/pull/3225) `on_press_maybe_with` for button
- [#3227](https://github.com/iced-rs/iced/pull/3227) Enable drag updates for double/triple clicks in `TextEditor`
- [#3239](https://github.com/iced-rs/iced/pull/3239) Add `on_submit_with` to `TextInput`
- [#3240](https://github.com/iced-rs/iced/pull/3240) Add custom cursor support
- [#3244](https://github.com/iced-rs/iced/pull/3244) pane_grid: setting to show controls on title hover
- [#3246](https://github.com/iced-rs/iced/pull/3246) Add configurable max_bind_groups setting
- [#3252](https://github.com/iced-rs/iced/pull/3252) add `combo_box::State::insert`
- [#3253](https://github.com/iced-rs/iced/pull/3253) fix #3232
- [#3262](https://github.com/iced-rs/iced/pull/3262) Add From for canvas::Path <-> lyon_path::Path
- [#3270](https://github.com/iced-rs/iced/pull/3270) enable letter spacing
- [#3276](https://github.com/iced-rs/iced/pull/3276) fix(flex): make axis apis public
- [#3280](https://github.com/iced-rs/iced/pull/3280) feat: add size within
- [#3282](https://github.com/iced-rs/iced/pull/3282) feat(text_input): add on focus and blur. Also, resolve clipy lints
- [#3284](https://github.com/iced-rs/iced/pull/3284) feat(combobox): add on_scroll event to combobox
- [#3286](https://github.com/iced-rs/iced/pull/3286) fix(canvas) use into radius instead of radius
- [#3290](https://github.com/iced-rs/iced/pull/3290) Fix IME preedit slicing
- [#3291](https://github.com/iced-rs/iced/pull/3291) Fix geometry clip bounds for infinite rectangles
- [#3307](https://github.com/iced-rs/iced/pull/3307) improve simulator's screenshots to compare images
- [#3308](https://github.com/iced-rs/iced/pull/3308) fix the panic in TextInput Secure
- [#3310](https://github.com/iced-rs/iced/pull/3310) feat: image handle from path with a user provided version
- [#3321](https://github.com/iced-rs/iced/pull/3321) feat(window): expose monitor_position alongside monitor_size
- [#3333](https://github.com/iced-rs/iced/pull/3333) feat(line_height): accept into pixels
- [#3337](https://github.com/iced-rs/iced/pull/3337) Make sensor callbacks optional
- [#3338](https://github.com/iced-rs/iced/pull/3338) Support double click to reset slider to default
- [#3341](https://github.com/iced-rs/iced/pull/3341) Add `COPY_SRC` to surface usages if supported
- [#3349](https://github.com/iced-rs/iced/pull/3349) implement `animation::Interpolable` for `Border` and `Radius`
- [#3352](https://github.com/iced-rs/iced/pull/3352) Invalidate buffer cache when fonts are loaded
- [#3359](https://github.com/iced-rs/iced/pull/3359) Fix culling of shadowed quads in `tiny_skia` backend
- [#3360](https://github.com/iced-rs/iced/pull/3360) Clip text groups in `tiny_skia` backend
- [#3377](https://github.com/iced-rs/iced/pull/3377) Forward trackpad pinch (magnify) gestures as `mouse::Event::PinchGesture`
- [#3378](https://github.com/iced-rs/iced/pull/3378) fix(image): prevent panic on 0x0 images by asserting size early
- [#3379](https://github.com/iced-rs/iced/pull/3379) Fix `Rectangle::offset` for non-overlapping rectangles
- [#3384](https://github.com/iced-rs/iced/pull/3384) Fix tiny-skia damage for vertically centered cached text
- [#3389](https://github.com/iced-rs/iced/pull/3389) Add `Task::blocking` helper
- [#3391](https://github.com/iced-rs/iced/pull/3391) Add per-side border widths and colors
- [#3393](https://github.com/iced-rs/iced/pull/3393) Draw SVG texts with iced fontdb
- [#3394](https://github.com/iced-rs/iced/pull/3394) modify `Shell::merge` to take `FnMut(B) -> Option<Message>`
- [#3395](https://github.com/iced-rs/iced/pull/3395) Make label optional for `radio` widget
- [#3413](https://github.com/iced-rs/iced/pull/3413) Add more slider styles
- [#3414](https://github.com/iced-rs/iced/pull/3414) Don't allocate for zero- or one-stream batches in `Task::batch`
- [#3426](https://github.com/iced-rs/iced/pull/3426) Addes a kinetic scrolling for touchpad scrolling
- [#3428](https://github.com/iced-rs/iced/pull/3428) Switch combo box matchers from ascii-alphanumeric splitting to unicode word splitting
- [#3433](https://github.com/iced-rs/iced/pull/3433) Expose `bounds` and `min_bounds` publicly through `text_editor::Content`
- [#3434](https://github.com/iced-rs/iced/pull/3434) Support SVG `currentColor` being set to renderer text color
- [#3437](https://github.com/iced-rs/iced/pull/3437) Fix scrollable min height with both scrollbars visible.
- [#3439](https://github.com/iced-rs/iced/pull/3439) feat(Viewer): add support for zoom and drag with touch events
- [#3445](https://github.com/iced-rs/iced/pull/3445) Fix `Text::Raw` clipping in the `tiny-skia` renderer
- [#3450](https://github.com/iced-rs/iced/pull/3450) allow Multi-Monitor window management via MonitorIndex
- [#3462](https://github.com/iced-rs/iced/pull/3462) keyed::Column::from_vecs accepts mismatched keys and children
- [#3477](https://github.com/iced-rs/iced/pull/3477) Gamepad/controller support
- [#3482](https://github.com/iced-rs/iced/pull/3482) fix: reject zero-sized rich text spans
- [#3486](https://github.com/iced-rs/iced/pull/3486) Expand text damage by 1px in `tiny_skia` like quads
- [#3487](https://github.com/iced-rs/iced/pull/3487) Detect both `keyboard_modifiers.control` and `command` in slider
- [#3490](https://github.com/iced-rs/iced/pull/3490) move button status to widget tree
- [#3493](https://github.com/iced-rs/iced/pull/3493) Capture scroll event in opaque widget
- [#3498](https://github.com/iced-rs/iced/pull/3498) wgpu: request device feature TEXTURE_FORMAT_NV12
- [#3500](https://github.com/iced-rs/iced/pull/3500) fix: BSD is incorrectly gated

## Tried and left out

Most other open pull requests target master's redesign of the `Widget` trait, which 0.14 does
not have. These were tried on 0.14 and left out:

- [#3021](https://github.com/iced-rs/iced/pull/3021): system tray: brings GTK3/glib (RUSTSEC-2024-0429); waiting for a decision.
- [#3155](https://github.com/iced-rs/iced/pull/3155): changes `slider::Style` (removes `HandleShape` and `border_color`), which Pyzed uses.
- [#3163](https://github.com/iced-rs/iced/pull/3163): device events: master's subscription differs; to be adapted by hand.
- [#3192](https://github.com/iced-rs/iced/pull/3192): changes `Text::Cached` on wgpu only and clashes with #3384.
- [#3283](https://github.com/iced-rs/iced/pull/3283): overlaps with #3377 (pinch), already applied.
- [#3294](https://github.com/iced-rs/iced/pull/3294): needs the newer cosmic-text BiDi API of master.
- [#3315](https://github.com/iced-rs/iced/pull/3315): text selection: Pyzed has its own; adds a `selection` field to `text::Style`.
- [#3324](https://github.com/iced-rs/iced/pull/3324): semantic selectors for iced_test only; clashes with the 0.14 tests.
- [#3325](https://github.com/iced-rs/iced/pull/3325): 0.14 already has the primary clipboard; the PR restores it on master's async clipboard.
- [#3335](https://github.com/iced-rs/iced/pull/3335): needs master's async clipboard.
- [#3346](https://github.com/iced-rs/iced/pull/3346): makes cosmic-text a dependency of iced_core.
- [#3347](https://github.com/iced-rs/iced/pull/3347): needs master's `diff(&mut self)`.
- [#3370](https://github.com/iced-rs/iced/pull/3370): redesigns the slider API (removes `step`/`shift_step`).
- [#3374](https://github.com/iced-rs/iced/pull/3374): needs master's `diff(&mut self)`.
- [#3385](https://github.com/iced-rs/iced/pull/3385): turns `svg::Handle` into an enum and moves resvg into iced_core.
- [#3422](https://github.com/iced-rs/iced/pull/3422): needs master's refactored combo_box.
- [#3423](https://github.com/iced-rs/iced/pull/3423): no Oklch colors in 0.14.
- [#3443](https://github.com/iced-rs/iced/pull/3443): restructures `winit/src/window.rs`; Pyzed does not embed external content.
- [#3447](https://github.com/iced-rs/iced/pull/3447): needs master's cosmic-text (decorations).
- [#3469](https://github.com/iced-rs/iced/pull/3469): the bug only exists on master.
- [#3473](https://github.com/iced-rs/iced/pull/3473): large API change (no `Clone` on messages) Pyzed does not need.
- [#3475](https://github.com/iced-rs/iced/pull/3475): built on master's redesigned system theme; to be rewritten on 0.14's `linux-theme-detection`.
- [#3484](https://github.com/iced-rs/iced/pull/3484): no ellipsis in 0.14.
- [#3494](https://github.com/iced-rs/iced/pull/3494): built on master's `Widget` trait (layout in `Tree`, `Length::Fit`).
- [#3503](https://github.com/iced-rs/iced/pull/3503): already covered by Pyzed's shadow clipping in tiny-skia.
- [#3507](https://github.com/iced-rs/iced/pull/3507): duplicate of #3148.
- [#3514](https://github.com/iced-rs/iced/pull/3514): asks for 7 extra GPU features Pyzed does not use.
- [#3520](https://github.com/iced-rs/iced/pull/3520): Wayland layer shell: niche, 1900 lines and a new dependency.

## Updating

Apply a new pull request with `git cherry-pick` (or `git apply -3` when it targets master), run
`cargo test --workspace --all-features`, push the branch and bump `rev` in Pyzed's `Cargo.toml`.
