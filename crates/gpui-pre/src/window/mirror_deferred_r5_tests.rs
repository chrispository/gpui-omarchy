use super::*;

struct DeferredAncestor {
    points: Rc<RefCell<Vec<Point<Pixels>>>>,
    paint_delta: Rc<Cell<Pixels>>,
    renders: Rc<Cell<usize>>,
    nested_late: bool,
}
impl Render for DeferredAncestor {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        let paint_delta = self.paint_delta.clone();
        let text = canvas(
            |_, _, _| {},
            move |_, _, window, cx| {
                let line = super::super::text::line(window, "ab", 0..0);
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
                if window.layout_mirrored() {
                    let unit = window
                        .next_frame
                        .mirror_units
                        .iter()
                        .rev()
                        .find(|unit| {
                            !unit.region && !unit.reflected && unit.scope == window.mirror.scope
                        })
                        .expect("the actual painted deferred text unit");
                    paint_delta.set(unit.delta);
                }
            },
        )
        .size_full();
        let points = self.points.clone();
        let parent = div()
            .id("interactive-parent")
            .absolute()
            .left(px(30.))
            .top_0()
            .w(px(20.))
            .h(px(20.))
            .occlude()
            .on_mouse_down(MouseButton::Left, move |event, window, _| {
                assert_eq!(window.mouse_position(), event.position);
                points.borrow_mut().push(event.position);
            })
            .child(deferred(text));
        let parent = if self.nested_late {
            deferred(parent).with_priority(1).into_any_element()
        } else {
            parent.into_any_element()
        };
        div()
            .size_full()
            .child(Region(div().w(px(50.)).h(px(20.)).into_any_element()))
            .child(parent)
    }
}
#[gpui::test]
fn deferred_hitless_text_keeps_its_interactive_ancestor(cx: &mut TestAppContext) {
    assert_deferred_ancestor(cx, false);
}
#[gpui::test]
fn deferred_text_keeps_an_ancestor_painted_at_a_later_priority(cx: &mut TestAppContext) {
    assert_deferred_ancestor(cx, true);
}
fn assert_deferred_ancestor(cx: &mut TestAppContext, nested_late: bool) {
    let points = Rc::new(RefCell::new(Vec::new()));
    let paint_delta = Rc::new(Cell::new(px(0.)));
    let log = points.clone();
    let line_width = paint_delta.clone();
    let (_, cx) = cx.add_window_view(move |_, _| DeferredAncestor {
        points: log,
        paint_delta: line_width,
        renders: Rc::new(Cell::new(0)),
        nested_late,
    });
    let viewport = cx.update(|window, _| {
        window.set_layout_mirrored(true);
        window.viewport_size().width
    });
    cx.run_until_parked();
    cx.simulate_click(point(viewport - px(49.), px(10.)), Modifiers::none());
    let points = points.borrow();
    assert_eq!(points.len(), 1);
    assert_eq!(points[0].y, px(10.));
    assert!(
        (points[0].x - (viewport - px(49.) - paint_delta.get()))
            .0
            .abs()
            < 0.001,
        "{points:?} versus {:?}",
        viewport - px(49.) - paint_delta.get()
    );
}

struct CachedHost {
    child: crate::Entity<DeferredAncestor>,
    prefix: bool,
}
impl Render for CachedHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .children(
                self.prefix
                    .then(|| div().absolute().top(px(40.)).size(px(5.)).occlude()),
            )
            .child(
                self.child
                    .clone()
                    .cached(crate::StyleRefinement::default().size(px(100.))),
            )
    }
}
#[gpui::test]
fn cached_deferred_text_restores_its_parent_after_prefix_changes(cx: &mut TestAppContext) {
    assert_cached_deferred_ancestor(cx, false);
}
#[gpui::test]
fn cached_deferred_text_keeps_an_ancestor_painted_later(cx: &mut TestAppContext) {
    assert_cached_deferred_ancestor(cx, true);
}
fn assert_cached_deferred_ancestor(cx: &mut TestAppContext, nested_late: bool) {
    let points = Rc::new(RefCell::new(Vec::new()));
    let paint_delta = Rc::new(Cell::new(px(0.)));
    let renders = Rc::new(Cell::new(0));
    let log = points.clone();
    let line_width = paint_delta.clone();
    let count = renders.clone();
    let child = cx.new(move |_| DeferredAncestor {
        points: log,
        paint_delta: line_width,
        renders: count,
        nested_late,
    });
    let (host, cx) = cx.add_window_view(move |_, _| CachedHost {
        child,
        prefix: false,
    });
    let viewport = cx.update(|window, _| {
        window.set_layout_mirrored(true);
        window.viewport_size().width
    });
    cx.run_until_parked();
    cx.simulate_event(crate::MouseMoveEvent {
        position: point(viewport - px(49.), px(10.)),
        ..Default::default()
    });
    cx.run_until_parked();
    for prefix in [true, false] {
        cx.run_until_parked();
        let before = renders.get();
        points.borrow_mut().clear();
        host.update(cx, |host, cx| {
            host.prefix = prefix;
            cx.notify();
        });
        cx.run_until_parked();
        assert_eq!(
            renders.get(),
            before,
            "the deferred owner reused cached paint"
        );
        cx.simulate_click(point(viewport - px(49.), px(10.)), Modifiers::none());
        let points = points.borrow();
        assert_eq!(points.len(), 1);
        assert!(
            (points[0].x - (viewport - px(49.) - paint_delta.get()))
                .0
                .abs()
                < 0.001,
            "prefix={prefix}: {points:?}"
        );
        assert_eq!(points[0].y, px(10.));
    }
}
struct DeferredTarget(Rc<RefCell<Vec<Point<Pixels>>>>);
impl Render for DeferredTarget {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let points = self.0.clone();
        let target = div()
            .id("deferred-target")
            .w(px(20.))
            .h(px(20.))
            .occlude()
            .on_mouse_down(MouseButton::Left, move |event, _, _| {
                points.borrow_mut().push(event.position)
            });
        Region(
            div()
                .flex()
                .w(px(50.))
                .h(px(20.))
                .child(div().w(px(30.)))
                .child(deferred(target))
                .into_any_element(),
        )
    }
}
#[gpui::test]
fn deferred_target_does_not_inherit_an_expired_unmirrored_paint_frame(cx: &mut TestAppContext) {
    let points = Rc::new(RefCell::new(Vec::new()));
    let log = points.clone();
    let (_, cx) = cx.add_window_view(move |_, _| DeferredTarget(log));
    let viewport = cx.update(|window, _| {
        window.set_layout_mirrored(true);
        window.viewport_size().width
    });
    cx.run_until_parked();
    cx.simulate_click(point(viewport - px(40.), px(10.)), Modifiers::none());
    assert_eq!(*points.borrow(), [point(px(40.), px(10.))]);
}
