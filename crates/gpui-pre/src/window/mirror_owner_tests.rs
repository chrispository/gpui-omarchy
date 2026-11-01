use super::*;
use crate::{ElementId, MouseMoveEvent, Point};

type Points = Rc<RefCell<Vec<(&'static str, Point<Pixels>)>>>;

struct Layers {
    points: Points,
    text: bool,
    blocking: bool,
    consume: bool,
}

impl Render for Layers {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let moves = self.points.clone();
        let clicks = self.points.clone();
        let a = div()
            .id("a")
            .w(px(20.))
            .h(px(20.))
            .on_mouse_move(move |event, window, _| {
                assert_eq!(window.mouse_position(), event.position);
                moves.borrow_mut().push(("a-move", event.position));
            })
            .on_mouse_down(MouseButton::Left, move |event, window, _| {
                assert_eq!(window.mouse_position(), event.position);
                clicks.borrow_mut().push(("a-click", event.position));
            });
        let region = Region(
            div()
                .flex()
                .w(px(50.))
                .h(px(20.))
                .child(a)
                .child(div().w(px(30.)))
                .into_any_element(),
        );
        let moves = self.points.clone();
        let clicks = self.points.clone();
        let consume = self.consume;
        let mut overlay = div()
            .id("overlay")
            .absolute()
            .left(px(10.))
            .top_0()
            .w(px(40.))
            .h(px(20.))
            .on_mouse_move(move |event, window, _| {
                assert_eq!(window.mouse_position(), event.position);
                moves.borrow_mut().push(("overlay-move", event.position));
            });
        if self.consume {
            overlay = overlay.on_mouse_down(MouseButton::Left, move |event, _, cx| {
                clicks.borrow_mut().push(("overlay-click", event.position));
                if consume {
                    cx.stop_propagation();
                }
            });
        }
        if self.blocking {
            overlay = overlay.occlude();
        }
        if self.text {
            overlay = overlay.child(
                canvas(
                    |_, _, _| {},
                    |_, _, window, cx| {
                        let line = super::text::line(window, "ab", 0..0);
                        assert!(line.width > px(2.) && line.width < px(40.));
                        line.paint(
                            point(px(50.) - line.width, px(0.)),
                            px(20.),
                            crate::TextAlign::Left,
                            None,
                            window,
                            cx,
                        )
                        .unwrap();
                    },
                )
                .size_full(),
            );
        }
        div().size_full().child(region).child(deferred(overlay))
    }
}

fn layered(
    cx: &mut TestAppContext,
    text: bool,
    blocking: bool,
    consume: bool,
) -> (Points, &mut crate::VisualTestContext, Point<Pixels>, Pixels) {
    let points = Points::default();
    let shared = points.clone();
    let (_, cx) = cx.add_window_view(move |_, _| Layers {
        points: shared,
        text,
        blocking,
        consume,
    });
    let width = cx.update(|window, _| {
        window.set_layout_mirrored(true);
        window.viewport_size().width
    });
    cx.run_until_parked();
    let glyph_x = cx.update(|window, _| {
        window
            .rendered_frame
            .mirror_units
            .iter()
            .find(|unit| !unit.region && !unit.reflected)
            .map(|unit| width - px(49.) - unit.delta)
            .unwrap_or(px(49.))
    });
    (points, cx, point(width - px(49.), px(10.)), glyph_x)
}

#[gpui::test]
fn normal_overlay_and_region_receive_their_own_coordinates(cx: &mut TestAppContext) {
    let (points, cx, at, _) = layered(cx, false, false, false);
    cx.simulate_event(MouseMoveEvent {
        position: at,
        ..Default::default()
    });
    assert!(
        points
            .borrow()
            .contains(&("overlay-move", point(px(49.), px(10.))))
    );
    assert!(
        points
            .borrow()
            .contains(&("a-move", point(px(1.), px(10.))))
    );
    points.borrow_mut().clear();
    cx.simulate_click(at, Modifiers::default());
    assert_eq!(*points.borrow(), [("a-click", point(px(1.), px(10.)))]);
}

#[gpui::test]
fn normal_clickable_overlay_can_consume_its_own_click(cx: &mut TestAppContext) {
    let (points, cx, at, _) = layered(cx, false, false, true);
    cx.simulate_click(at, Modifiers::default());
    assert_eq!(
        *points.borrow(),
        [("overlay-click", point(px(49.), px(10.)))]
    );
}

#[test]
fn normal_text_overlay_does_not_change_underlying_coordinates() {
    let mut app = super::text::text_cx();
    let (points, cx, at, glyph_x) = layered(&mut app, true, false, false);
    assert_ne!(glyph_x, px(49.));
    cx.simulate_event(MouseMoveEvent {
        position: at,
        ..Default::default()
    });
    assert!(
        points
            .borrow()
            .contains(&("overlay-move", point(glyph_x, px(10.))))
    );
    assert!(
        points
            .borrow()
            .contains(&("a-move", point(px(1.), px(10.))))
    );
    points.borrow_mut().clear();
    cx.simulate_click(at, Modifiers::default());
    assert_eq!(*points.borrow(), [("a-click", point(px(1.), px(10.)))]);
}

#[test]
fn interactive_ancestor_receives_hitless_child_glyph_coordinates() {
    let mut app = super::text::text_cx();
    let (points, cx, at, glyph_x) = layered(&mut app, true, true, true);
    assert_ne!(glyph_x, px(49.));
    cx.simulate_click(at, Modifiers::default());
    assert_eq!(
        *points.borrow(),
        [("overlay-click", point(glyph_x, px(10.)))]
    );
}

struct ParentLine(AnyElement, bool);

impl IntoElement for ParentLine {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for ParentLine {
    type RequestLayoutState = ();
    type PrepaintState = ();
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
    ) -> (LayoutId, ()) {
        (self.0.request_layout(window, cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.1 {
            window.insert_hitbox(bounds(30., 40., 20., 20.), crate::HitboxBehavior::Normal);
        }
        self.0.prepaint(window, cx);
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let line = super::text::line(window, "abcde", 0..0);
        line.paint(
            point(px(0.), px(0.)),
            px(20.),
            crate::TextAlign::Left,
            None,
            window,
            cx,
        )
        .unwrap();
        self.0.paint(window, cx);
    }
}

struct ParentView(Rc<RefCell<Vec<Point<Pixels>>>>);

impl Render for ParentView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let log = self.0.clone();
        ParentLine(
            div()
                .w(px(50.))
                .h(px(20.))
                .child(
                    div()
                        .id("card")
                        .absolute()
                        .left(px(30.))
                        .top_0()
                        .w(px(20.))
                        .h(px(20.))
                        .occlude()
                        .on_mouse_down(MouseButton::Left, move |event, _, _| {
                            log.borrow_mut().push(event.position)
                        }),
                )
                .into_any_element(),
            false,
        )
    }
}

#[test]
fn a_child_painted_after_parent_text_keeps_its_own_coordinates() {
    let app = &mut super::text::text_cx();
    let hits = Rc::new(RefCell::new(Vec::new()));
    let log = hits.clone();
    let (_, cx) = app.add_window_view(move |_, _| ParentView(log));
    let width = cx.update(|window, _| {
        window.set_layout_mirrored(true);
        window.viewport_size().width
    });
    cx.run_until_parked();
    cx.simulate_click(point(width - px(40.), px(10.)), Modifiers::default());
    assert_eq!(*hits.borrow(), [point(px(40.), px(10.))]);
}

struct Unrelated(Points);

impl Render for Unrelated {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let log = self.0.clone();
        let a = div()
            .id("a")
            .w(px(20.))
            .h(px(20.))
            .on_mouse_down(MouseButton::Left, move |event, _, _| {
                log.borrow_mut().push(("a", event.position))
            });
        div()
            .size_full()
            .child(Region(
                div().w(px(50.)).h(px(20.)).child(a).into_any_element(),
            ))
            .child(ParentLine(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w(px(100.))
                    .h(px(60.))
                    .into_any_element(),
                true,
            ))
    }
}

#[test]
fn an_unrelated_owned_hitbox_does_not_give_text_the_underlying_receivers_coordinates() {
    let app = &mut super::text::text_cx();
    let hits = Points::default();
    let log = hits.clone();
    let (_, cx) = app.add_window_view(move |_, _| Unrelated(log));
    let width = cx.update(|window, _| {
        window.set_layout_mirrored(true);
        window.viewport_size().width
    });
    cx.run_until_parked();
    let at = point(width - px(40.), px(10.));
    cx.update(|window, _| {
        let text = window
            .rendered_frame
            .mirror_units
            .iter()
            .find(|unit| !unit.region && !unit.reflected)
            .unwrap();
        assert_ne!(
            at.x - text.delta,
            px(10.),
            "fixture must distinguish the text transform"
        );
    });
    cx.simulate_click(at, Modifiers::default());
    assert_eq!(*hits.borrow(), [("a", point(px(10.), px(10.)))]);
}

#[path = "mirror_r5_tests.rs"]
mod r5;

#[path = "mirror_frame_cache_tests.rs"]
mod frame_cache;

#[path = "mirror_deferred_r5_tests.rs"]
mod deferred_ancestry;
