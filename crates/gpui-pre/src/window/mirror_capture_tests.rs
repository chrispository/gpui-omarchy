use super::*;
use crate::{
    DispatchPhase, ElementId, Hitbox, HitboxBehavior, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    Point, TouchEvent, TouchId, TouchPhase,
};

type Points = Rc<RefCell<Vec<Point<Pixels>>>>;
struct Capture(Points);

impl IntoElement for Capture {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for Capture {
    type RequestLayoutState = AnyElement;
    type PrepaintState = Hitbox;
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        let mut element = div().w(px(50.)).h(px(20.)).into_any_element();
        (element.request_layout(window, cx), element)
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        frame: Bounds<Pixels>,
        element: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) -> Hitbox {
        element.prepaint(window, cx);
        window.insert_hitbox(
            Bounds::new(frame.origin, size(px(20.), px(20.))),
            HitboxBehavior::Normal,
        )
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        element: &mut AnyElement,
        hit: &mut Hitbox,
        window: &mut Window,
        cx: &mut App,
    ) {
        element.paint(window, cx);
        let log = self.0.clone();
        let hit = hit.clone();
        let down = hit.clone();
        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, _| {
            if phase == DispatchPhase::Bubble && down.is_hovered(window) {
                window.capture_pointer(down.id);
                log.borrow_mut().push(event.position);
            }
        });
        let moves = self.0.clone();
        let moved = hit.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, _| {
            if phase == DispatchPhase::Bubble && moved.is_hovered(window) {
                assert_eq!(window.mouse_position(), event.position);
                moves.borrow_mut().push(event.position);
            }
        });
        let up = self.0.clone();
        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, _| {
            if phase == DispatchPhase::Bubble && hit.is_hovered(window) {
                up.borrow_mut().push(event.position);
            }
        });
    }
}

struct CaptureView(Points);
impl Render for CaptureView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Region(Capture(self.0.clone()).into_any_element())
    }
}

#[gpui::test]
fn captured_region_keeps_its_coordinates_outside_the_viewport(cx: &mut TestAppContext) {
    let points = Points::default();
    let log = points.clone();
    let (_, cx) = cx.add_window_view(move |_, _| CaptureView(log));
    let width = cx.update(|window, _| {
        window.set_layout_mirrored(true);
        window.viewport_size().width
    });
    cx.run_until_parked();
    cx.simulate_event(MouseMoveEvent {
        position: point(width - px(40.), px(10.)),
        ..Default::default()
    });
    cx.run_until_parked();
    points.borrow_mut().clear();
    cx.simulate_mouse_down(
        point(width - px(40.), px(10.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    let outside = point(width + px(10.), px(10.));
    cx.simulate_event(MouseMoveEvent {
        position: outside,
        pressed_button: Some(MouseButton::Left),
        ..Default::default()
    });
    cx.simulate_mouse_up(outside, MouseButton::Left, Modifiers::none());
    assert_eq!(
        *points.borrow(),
        [
            point(px(10.), px(10.)),
            point(px(60.), px(10.)),
            point(px(60.), px(10.))
        ]
    );
    assert!(cx.update(|window, _| window.captured_hitbox()).is_none());
}

#[gpui::test]
fn a_touch_tap_maps_synthetic_mouse_events_once(cx: &mut TestAppContext) {
    let points = Points::default();
    let log = points.clone();
    let (_, cx) = cx.add_window_view(move |_, _| CaptureView(log));
    let width = cx.update(|window, _| {
        window.set_layout_mirrored(true);
        window.viewport_size().width
    });
    cx.run_until_parked();
    for phase in [TouchPhase::Started, TouchPhase::Ended] {
        cx.simulate_event(TouchEvent {
            id: TouchId(1),
            phase,
            position: point(width - px(40.), px(10.)),
            ..Default::default()
        });
    }
    assert_eq!(
        *points.borrow(),
        [point(px(10.), px(10.)), point(px(10.), px(10.))]
    );
}
