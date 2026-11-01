use super::*;
use crate::{Hitbox, HitboxBehavior, MouseMoveEvent};

struct Probe {
    checks: Rc<Cell<usize>>,
    capture: bool,
    scroll: bool,
    unmirrored: bool,
}
impl IntoElement for Probe {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for Probe {
    type RequestLayoutState = AnyElement;
    type PrepaintState = Hitbox;
    fn id(&self) -> Option<crate::ElementId> {
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
        let mut child = div().w(px(50.)).h(px(20.)).into_any_element();
        (child.request_layout(window, cx), child)
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        frame: Bounds<Pixels>,
        child: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) -> Hitbox {
        child.prepaint(window, cx);
        window.insert_hitbox(frame, HitboxBehavior::BlockMouseExceptScroll)
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        frame: Bounds<Pixels>,
        child: &mut AnyElement,
        hit: &mut Hitbox,
        window: &mut Window,
        cx: &mut App,
    ) {
        let physical = point(window.viewport_size().width - px(40.), px(10.));
        if window.mouse_position == physical {
            assert!(window.mirror.scope.is_some());
            assert_eq!(
                window.mouse_hit_test.ids.first(),
                Some(&hit.id),
                "physical target participates in the cache identity check"
            );
            for (id, index) in window
                .mouse_hit_test
                .ids
                .iter()
                .zip(&window.mouse_hit_test.indices)
            {
                assert_eq!(
                    *id, window.next_frame.hitboxes[*index].id,
                    "cached hit index belongs to next_frame during paint"
                );
            }
            let previous = window.captured_hitbox;
            if self.capture {
                window.capture_pointer(hit.id);
            }
            let (logical, in_region) =
                window.mirror_coordinates(physical, window.mirror.scope, self.scroll);
            let expected_x = if self.unmirrored {
                frame.origin.x * 2. + px(10.)
            } else {
                px(40.)
            };
            assert!(
                (logical.x - expected_x).0.abs() < 0.001,
                "{logical:?} versus {expected_x:?}"
            );
            assert_eq!(logical.y, px(10.));
            assert_eq!(in_region, self.unmirrored);
            window.captured_hitbox = previous;
            self.checks.set(self.checks.get() + 1);
        }
        child.paint(window, cx);
    }
}
struct Topology {
    prefix: bool,
    checks: Rc<Cell<usize>>,
    capture: bool,
    scroll: bool,
    unmirrored: bool,
}
impl Render for Topology {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let probe = Probe {
            checks: self.checks.clone(),
            capture: self.capture,
            scroll: self.scroll,
            unmirrored: self.unmirrored,
        }
        .into_any_element();
        let probe = if self.unmirrored {
            Region(probe).into_any_element()
        } else {
            probe
        };
        let mut row = div().flex();
        if self.prefix {
            row = row.child(
                div()
                    .id("new-prefix")
                    .w(px(5.))
                    .h(px(20.))
                    .on_mouse_move(|_, _, _| {}),
            );
        }
        row.child(probe)
    }
}
#[gpui::test]
fn cached_hit_indices_match_next_frame_after_topology_changes(cx: &mut TestAppContext) {
    for unmirrored in [false, true] {
        for capture in [false, true] {
            for scroll in [false, true] {
                let checks = Rc::new(Cell::new(0));
                let count = checks.clone();
                let (view, cx) = cx.add_window_view(move |_, _| Topology {
                    prefix: false,
                    checks: count,
                    capture,
                    scroll,
                    unmirrored,
                });
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
                for prefix in [true, false] {
                    let before = checks.get();
                    view.update(cx, |view, cx| {
                        view.prefix = prefix;
                        cx.notify();
                    });
                    cx.run_until_parked();
                    assert!(checks.get() > before, "topology change painted the control");
                }
            }
        }
    }
}
