// Smooth scroll for wheel input.
//
// gpui-pre applies wheel deltas to the scroll offset immediately, so every
// wheel notch is a hard jump. This module adds a frame-clock eased approach on
// top of the existing scroll machinery, without touching gpui-pre:
//
// * The wheel handler runs in the capture phase and consumes the event, so the
//   immediate jump never happens for line-based (real wheel) deltas.
// * Pixel-precise deltas (touch pads / high resolution wheels) are left alone
//   and keep the native feel.
// * Each frame the app advances the offset towards the target. A non-decaying
//   exponential approach is used: it can never overshoot, it is frame-rate
//   independent (driven by elapsed time, not a fixed per-frame step) and it
//   converges quickly enough to feel immediate.

use std::rc::Rc;

use std::cell::RefCell;

use gpui_kit::{
    AnyElement, App, Bounds, DispatchPhase, Div, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, ParentElement, Pixels, ScrollDelta, ScrollHandle,
    ScrollWheelEvent, Stateful, Styled, StyleRefinement, Window, px,
};

/// Time constant of the exponential approach.
///
/// The offset closes `1 - 1/e` of the remaining distance every
/// `SMOOTH_SCROLL_TAU`. 60ms keeps the perceptible part of one wheel notch
/// inside ~150-250ms (95% of the distance is covered in ~180ms) while the tail
/// is only sub-pixel movement.
pub const SMOOTH_SCROLL_TAU: f32 = 0.06;

/// Below this distance the offset snaps to the target so the animation settles
/// instead of asymptotically creeping towards the end. 0.35px is far below one
/// device pixel, so the snap is invisible.
const SMOOTH_SCROLL_SETTLE: f32 = 0.35;

/// Exponential approach factor for a frame that took `dt_seconds`.
///
/// * `dt == 0` -> `1.0`, e.g. stay put.
/// * `dt` large -> `0.0`, e.g. land exactly on the target.
fn approach_fraction(dt_seconds: f32, tau_seconds: f32) -> f32 {
    // `f32::INFINITY.exp()` is `0.0`, so a frame that took "forever" lands
    // exactly on the target; only `NaN` has to be rejected.
    if dt_seconds.is_nan() || dt_seconds <= 0.0 {
        return 1.0;
    }
    if !tau_seconds.is_finite() || tau_seconds <= 0.0 {
        return 0.0;
    }
    (-dt_seconds / tau_seconds).exp()
}

/// One step of the eased scroll: move `current` towards `target` by the
/// fraction implied by `dt_seconds`.
///
/// The result is always on the segment between `current` and `target` (no
/// overshoot), never moves away from the target, and equals `target` exactly
/// once the remaining distance is below `settle`.
pub fn smooth_scroll_step(current: f32, target: f32, dt_seconds: f32) -> f32 {
    smooth_scroll_step_with(current, target, dt_seconds, SMOOTH_SCROLL_TAU, SMOOTH_SCROLL_SETTLE)
}

fn smooth_scroll_step_with(
    current: f32,
    target: f32,
    dt_seconds: f32,
    tau_seconds: f32,
    settle: f32,
) -> f32 {
    if current == target {
        return target;
    }
    let remaining = target - current;
    let next = current + remaining * (1.0 - approach_fraction(dt_seconds, tau_seconds));
    if (target - next).abs() <= settle {
        target
    } else {
        next
    }
}

/// The animated scroll offset for one wheel event: `target` moved out of
/// `current` by the event's pixel delta, then clamped to the scrollable range.
///
/// `max_offset` is passed as the positive distance the content can move, i.e.
/// the target is clamped to `[-max, 0]`, mirroring gpui's own clamping. At the
/// very top or bottom the target simply stops growing, so a flick does not pull
/// the content past the end and snap back.
pub fn smooth_scroll_target(current: f32, delta: f32, max_offset: f32) -> f32 {
    if !delta.is_finite() || !current.is_finite() {
        return current;
    }
    let max = if max_offset.is_finite() { max_offset.max(0.0) } else { 0.0 };
    (current + delta).clamp(-max, 0.0)
}

/// True while `current` still has to move towards `target`.
pub fn smooth_scroll_animating(current: f32, target: f32) -> bool {
    (target - current).abs() > f32::EPSILON && current != target
}

/// Per-container scroll animation state shared with the element tree.
#[derive(Debug, Default)]
pub struct SmoothScrollState {
    /// Scroll handle tracked by the div, used to read and write the offset.
    pub handle: ScrollHandle,
    /// The offset the animation is easing towards.
    pub target: f32,
    /// Set by the wheel listener when it starts an animation.
    ///
    /// The listener cannot call `Window::request_animation_frame` itself: that
    /// unwraps the current view, which platform wheel dispatch does not push.
    /// `Render::render` picks the request up on the next frame instead, where
    /// the view context is guaranteed.
    pub request_frame: bool,
    /// True while the offset is still easing towards the target.
    pub animating: bool,
    /// The last offset handed to `handle`. `NEG_INFINITY` means "nothing
    /// sampled yet"; `advance_smooth_scroll` adopts the handle's offset then.
    sampled: f32,
}

impl SmoothScrollState {
    pub fn new() -> Self {
        Self {
            handle: ScrollHandle::new(),
            target: 0.0,
            request_frame: false,
            animating: false,
            sampled: f32::NEG_INFINITY,
        }
    }

    /// Start easing towards `target` from wherever the content currently sits.
    pub fn set_target(&mut self, target: f32) {
        self.target = target;
        self.sampled = f32::NEG_INFINITY;
        self.request_frame = true;
        self.animating = true;
    }

    /// Add `delta` pixels (positive scrolls the view down) on top of the
    /// pending target, clamped to the scrollable range.
    pub fn scroll_by(&mut self, delta: f32) {
        let current = self.handle.offset().y.as_f32();
        let max = self.handle.max_offset().y.as_f32();
        self.set_target(smooth_scroll_target(current, delta, max));
    }

    pub fn offset(&self) -> f32 {
        self.handle.offset().y.as_f32()
    }

    pub fn max_offset(&self) -> f32 {
        self.handle.max_offset().y.as_f32()
    }

    /// Consume a pending frame request and report whether the frame clock has
    /// to keep running for this container.
    pub fn take_frame_request(&mut self) -> bool {
        let request = self.request_frame;
        self.request_frame = false;
        request || self.animating
    }
}

/// Step every container's animation towards its target.
///
/// Returns `true` while at least one container still has to move, which the
/// caller should turn into another `Window::request_animation_frame`.
pub fn advance_smooth_scroll(scrolls: &[Rc<RefCell<SmoothScrollState>>], dt_seconds: f32) -> bool {
    let mut animating = false;
    for state in scrolls {
        let mut state = state.borrow_mut();
        let actual = state.offset();
        let max = state.max_offset().max(0.0);
        // The only writer of `sampled` is this function, and it writes the
        // exact `f32` it handed to the handle, so a real divergence means
        // somebody else moved the content: a scrollbar drag, a
        // `scroll_to_item`, a changed scroll range. Yield to that move instead
        // of easing back to a stale target.
        if state.sampled.is_finite() && (state.sampled - actual).abs() > 0.5 {
            state.target = actual;
            state.sampled = actual;
            state.animating = false;
        }
        let target = state.target.clamp(-max, 0.0);
        state.target = target;
        let from = if state.sampled.is_finite() {
            state.sampled
        } else {
            actual
        };
        let next = smooth_scroll_step(from, target, dt_seconds);
        state.sampled = next;
        if next != actual {
            let mut offset = state.handle.offset();
            offset.y = px(next);
            state.handle.set_offset(offset);
        }
        state.animating = smooth_scroll_animating(next, target);
        animating |= state.animating;
    }
    animating
}

/// Wrapper that makes a scroll div ease wheel input.
///
/// The wrapped div stays the scroll container, so layout, scrollbars and the
/// tracked `ScrollHandle` behave exactly as before. The wrapper only adds a
/// capture-phase wheel listener, which lets it consume real wheel deltas
/// *before* gpui applies them. Pixel-precise deltas are deliberately not
/// consumed and fall through to gpui's own immediate handling.
pub struct SmoothScrollDiv {
    inner: Stateful<Div>,
    on_wheel: Rc<dyn Fn(&ScrollWheelEvent, &mut Window, &mut App)>,
}

impl IntoElement for SmoothScrollDiv {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl SmoothScrollDiv {
    pub fn new(inner: Stateful<Div>, state: &Rc<RefCell<SmoothScrollState>>) -> Self {
        let wheel_state = state.clone();
        let on_wheel: Rc<dyn Fn(&ScrollWheelEvent, &mut Window, &mut App)> =
            Rc::new(move |event, window, cx| {
                let delta = match event.delta {
                    ScrollDelta::Lines(lines) => (lines.y * window.line_height()).as_f32(),
                    // Touch pads and high-resolution wheels already stream
                    // pixel deltas at input rate; leave the native path alone.
                    ScrollDelta::Pixels(_) => return,
                };
                if !delta.is_finite() || delta == 0.0 {
                    return;
                }
                wheel_state.borrow_mut().scroll_by(delta);
                // `Render::render` turns this into a `request_animation_frame`
                // on the next frame; see `SmoothScrollState::request_frame`.
                // Consume the event so gpui's immediate scroll never runs.
                cx.stop_propagation();
            });
        Self { inner, on_wheel }
    }
}

impl Styled for SmoothScrollDiv {
    fn style(&mut self) -> &mut StyleRefinement {
        self.inner.style()
    }
}

impl ParentElement for SmoothScrollDiv {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.inner.extend(elements);
    }
}

impl Element for SmoothScrollDiv {
    type RequestLayoutState = <Stateful<Div> as Element>::RequestLayoutState;
    type PrepaintState = <Stateful<Div> as Element>::PrepaintState;

    fn id(&self) -> Option<ElementId> {
        self.inner.id()
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        self.inner.source_location()
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.inner.request_layout(id, inspector_id, window, cx)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.inner
            .prepaint(id, inspector_id, bounds, request_layout, window, cx)
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.inner
            .paint(id, inspector_id, bounds, request_layout, prepaint, window, cx);

        // Registering the listener in the capture phase is what lets us replace
        // the immediate jump with an animation: gpui applies wheel deltas in a
        // bubble-phase listener, and a capture-phase `stop_propagation` makes
        // sure ours runs first and that one never does. The hitbox check keeps
        // the event with the container under the pointer, so a nested
        // scrollable consumes its own wheel and the outer one stays put.
        if let Some(hitbox) = prepaint.as_ref() {
            let hitbox = hitbox.clone();
            let on_wheel = self.on_wheel.clone();
            window.on_mouse_event(move |event: &ScrollWheelEvent, phase, window, cx| {
                if phase != DispatchPhase::Capture || !hitbox.should_handle_scroll(window) {
                    return;
                }
                on_wheel(event, window, cx);
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The step must stay on the segment between current and target: never
    /// overshoot, never move away from the target.
    #[test]
    fn step_never_overshoots_or_reverses() {
        let current = 0.0_f32;
        let target = -120.0_f32;
        let mut previous = current;
        for _ in 0..120 {
            let next = smooth_scroll_step(previous, target, 1.0 / 60.0);
            assert!(next <= previous + 1e-5, "step moved backwards: {previous} -> {next}");
            assert!(next >= target - 1e-4, "step overshot the target: {next} < {target}");
            previous = next;
        }
        assert_eq!(previous, target, "animation must settle exactly on the target");
    }

    #[test]
    fn step_converges_within_the_target_duration() {
        let start = 0.0_f32;
        let target = -400.0_f32;
        let mut current = start;
        // 60fps for 250ms.
        for _ in 0..15 {
            current = smooth_scroll_step(current, target, 1.0 / 60.0);
        }
        let covered = (current - start).abs();
        let total = (target - start).abs();
        assert!(
            covered >= total * 0.90,
            "after 250ms only {covered} of {total} pixels covered"
        );
    }

    #[test]
    fn step_handles_idle_and_huge_deltas() {
        // Already at the target: nothing moves.
        assert_eq!(smooth_scroll_step(-42.0, -42.0, 0.016), -42.0);
        // A frame that took forever lands exactly on the target.
        assert_eq!(smooth_scroll_step(0.0, -300.0, 10.0), -300.0);
        assert_eq!(smooth_scroll_step(0.0, -300.0, f32::INFINITY), -300.0);
        // A zero-length frame does not move.
        assert_eq!(smooth_scroll_step(0.0, -300.0, 0.0), 0.0);
        // Non-finite input is ignored instead of poisoning the offset.
        assert_eq!(smooth_scroll_step(0.0, -300.0, f32::NAN), 0.0);
    }

    #[test]
    fn target_is_clamped_to_the_scrollable_range() {
        // Scrolling up at the top must not pull the content down.
        assert_eq!(smooth_scroll_target(0.0, -80.0, 500.0), -80.0);
        assert_eq!(smooth_scroll_target(0.0, 80.0, 500.0), 0.0);
        // Scrolling down at the bottom must stop at the end.
        assert_eq!(smooth_scroll_target(-500.0, -80.0, 500.0), -500.0);
        assert_eq!(smooth_scroll_target(-460.0, -80.0, 500.0), -500.0);
        // A container with no overflow never moves.
        assert_eq!(smooth_scroll_target(0.0, 120.0, 0.0), 0.0);
    }

    #[test]
    fn successive_wheel_notches_accumulate_on_the_target() {
        let mut target = 0.0_f32;
        for _ in 0..3 {
            target = smooth_scroll_target(target, -53.0, 1000.0);
        }
        assert_eq!(target, -159.0);
    }

    /// A wheel event asks for a frame, and the request is consumed exactly
    /// once so the frame clock can stop when nothing is moving.
    #[test]
    fn wheel_requests_a_frame_and_consumes_it_once() {
        let mut state = SmoothScrollState::new();

        // Idle: nothing to do.
        assert!(!state.take_frame_request());
        assert!(state.sampled.is_infinite());

        state.scroll_by(-100.0);
        assert!(state.request_frame);
        assert!(state.animating);

        // The first render consumes the request and keeps pumping.
        assert!(state.take_frame_request());
        // With the target still out of reach it keeps asking for frames even
        // though the listener is not setting the flag any more.
        assert!(!state.request_frame);
        assert!(state.take_frame_request());
    }

    /// The frame-clock pump must clear `animating` once the offset settles, so
    /// the render loop stops requesting frames instead of spinning forever.
    #[test]
    fn pump_stops_once_the_offset_settles() {
        let state = Rc::new(RefCell::new(SmoothScrollState::new()));
        // A fresh container has no layout yet, so its scroll range is empty;
        // the target is clamped there and the pump has nothing to animate.
        state.borrow_mut().set_target(-180.0);
        let scrolls = vec![state.clone()];

        assert!(state.borrow_mut().take_frame_request());
        assert!(!advance_smooth_scroll(&scrolls, 1.0 / 60.0));
        assert!(!state.borrow_mut().take_frame_request());
        assert!(!state.borrow().animating);
        assert_eq!(state.borrow().offset(), 0.0);
    }

    /// If something other than the animation moves the content (a scrollbar
    /// drag, `scroll_to_item`, a shrunken scroll range), the animation must
    /// stop and adopt the new position instead of easing back to its old
    /// target.
    #[test]
    fn external_offset_change_cancels_the_animation() {
        let state = Rc::new(RefCell::new(SmoothScrollState::new()));
        {
            let mut state = state.borrow_mut();
            // Pretend we eased the content to -100 last frame...
            state.sampled = -100.0;
            state.target = -100.0;
            state.animating = true;
        }
        // ...but the handle now reports 0: the scroll range collapsed, or a
        // scrollbar drag moved it back.
        let scrolls = vec![state.clone()];
        advance_smooth_scroll(&scrolls, 1.0 / 60.0);

        let state = state.borrow();
        assert!(!state.animating, "animation must yield to the external move");
        assert_eq!(state.target, 0.0);
        assert_eq!(state.sampled, 0.0);
    }
}
