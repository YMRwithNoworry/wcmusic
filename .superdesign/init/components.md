# UI Primitives

The Windows desktop UI is native Rust GPUI, not a web component framework. Reusable primitives are imported from `gpui_kit` in `native/wcmusic_ui/src/main.rs`.

## Button
Source: `native/wcmusic_ui/src/main.rs` (player bar usage)
Purpose: compact action button with accessible label, tooltip, icon or text, and ghost/primary/secondary/selected variants.

```rust
Button::new("player-volume")
    .ghost()
    .small()
    .icon(gpui_kit::assets::IconName::Volume2)
    .tooltip("静音 / 取消静音")
    .accessibility_label("静音")
    .on_click(cx.listener(|this, _, _, cx| this.toggle_mute(cx)))
```

## Slider
Source: `native/wcmusic_ui/src/main.rs` (player bar usage)
Purpose: controlled audio progress/volume slider.

```rust
div().w(px(72.0)).h(px(18.0)).flex().items_center().child(
    Slider::new(self.volume_slider.as_ref().expect("volume slider initialized")).w_full(),
)
```

## Layout primitives
`div`, `h_flex`, `v_flex`, `Button`, `Slider` and theme palette helpers compose all native views. Prefer these project conventions over web-only components. Theme colors are accessed through `Palette::new(cx)`.
