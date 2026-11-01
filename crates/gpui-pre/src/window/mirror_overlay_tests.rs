use super::*;

#[derive(Clone, Copy)]
enum OverlayKind {
    Transparent,
    MouseOnly,
    HitlessText,
}

struct Overlay(Log, OverlayKind);

impl Render for Overlay {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let target = |name: &'static str| {
            let clicks = self.0.clone();
            let scrolls = self.0.clone();
            div()
                .id(name)
                .w(px(20.))
                .h(px(20.))
                .on_mouse_down(MouseButton::Left, move |_, _, _| {
                    clicks.borrow_mut().push(name.into())
                })
                .on_scroll_wheel(move |event, _, _| {
                    scrolls
                        .borrow_mut()
                        .push(format!("{name} {:?}", event.delta))
                })
        };
        let row = div()
            .flex()
            .w(px(50.))
            .h(px(20.))
            .child(target("a"))
            .child(div().w(px(10.)))
            .child(target("b"));
        let overlay = match self.1 {
            OverlayKind::Transparent => deferred(
                div()
                    .id("transparent")
                    .absolute()
                    .left(px(30.))
                    .top_0()
                    .w(px(20.))
                    .h(px(20.))
                    .on_mouse_move(|_, _, _| {}),
            )
            .into_any_element(),
            OverlayKind::MouseOnly => deferred(
                div()
                    .id("mouse-only")
                    .absolute()
                    .left(px(30.))
                    .top_0()
                    .w(px(20.))
                    .h(px(20.))
                    .block_mouse_except_scroll(),
            )
            .into_any_element(),
            OverlayKind::HitlessText => canvas(
                |_, _, _| {},
                |_, _, window, cx| {
                    let line = super::text::line(window, "abc", 0..0);
                    assert!(line.width > px(10.));
                    line.paint(
                        point(px(30.), px(0.)),
                        px(20.),
                        crate::TextAlign::Left,
                        None,
                        window,
                        cx,
                    )
                    .unwrap();
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .w(px(100.))
            .h(px(20.))
            .into_any_element(),
        };
        div()
            .size_full()
            .child(Region(row.into_any_element()))
            .child(overlay)
    }
}

fn covered(cx: &mut TestAppContext, kind: OverlayKind, scroll: bool) -> Vec<String> {
    let log = Log::default();
    let shared = log.clone();
    let (_, cx) = cx.add_window_view(move |_, _| Overlay(shared, kind));
    let width = cx.update(|window, _| {
        window.set_layout_mirrored(true);
        window.viewport_size().width
    });
    cx.run_until_parked();
    let at = point(width - px(40.), px(10.));
    if scroll {
        cx.simulate_event(ScrollWheelEvent {
            position: at,
            delta: ScrollDelta::Pixels(point(px(3.), px(-2.))),
            ..Default::default()
        });
    } else {
        cx.simulate_click(at, Modifiers::default());
    }
    let found = log.borrow().clone();
    found
}

#[gpui::test]
fn a_transparent_deferred_hitbox_keeps_the_underlying_regions_click_mapping(
    cx: &mut TestAppContext,
) {
    assert_eq!(covered(cx, OverlayKind::Transparent, false), ["a"]);
}

#[gpui::test]
fn a_mouse_only_occluder_passes_scroll_to_the_underlying_region(cx: &mut TestAppContext) {
    assert!(covered(cx, OverlayKind::MouseOnly, false).is_empty());
    assert_eq!(
        covered(cx, OverlayKind::MouseOnly, true),
        [format!(
            "a {:?}",
            ScrollDelta::Pixels(point(px(3.), px(-2.)))
        )]
    );
}

#[test]
fn hitless_overlay_text_does_not_replace_an_underlying_regions_mapping() {
    assert_eq!(
        covered(&mut super::text::text_cx(), OverlayKind::HitlessText, false),
        ["a"]
    );
}

struct InlineCard(Log, Rc<Cell<usize>>);

impl Render for InlineCard {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.1.set(self.1.get() + 1);
        let target = |name: &'static str| {
            let log = self.0.clone();
            div()
                .id(name)
                .w(px(20.))
                .h(px(20.))
                .on_mouse_down(MouseButton::Left, move |_, _, _| {
                    log.borrow_mut().push(name.into())
                })
        };
        let row = div()
            .flex()
            .w(px(50.))
            .h(px(20.))
            .child(target("a"))
            .child(div().w(px(10.)))
            .child(target("b"));
        div()
            .size_full()
            .child(Region(row.into_any_element()))
            .child(target("card").absolute().left(px(30.)).top_0().occlude())
    }
}

#[gpui::test]
fn a_later_sibling_in_the_same_layer_takes_clicks_over_an_unmirrored_region(
    cx: &mut TestAppContext,
) {
    let log = Log::default();
    let shared = log.clone();
    let (_, cx) = cx.add_window_view(move |_, _| InlineCard(shared, Rc::new(Cell::new(0))));
    let width = cx.update(|window, _| {
        window.set_layout_mirrored(true);
        window.viewport_size().width
    });
    cx.run_until_parked();
    cx.simulate_click(point(width - px(40.), px(10.)), Modifiers::default());
    assert_eq!(*log.borrow(), ["card"]);
}

struct CachedRegion(Log, Rc<Cell<usize>>);

impl Render for CachedRegion {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.1.set(self.1.get() + 1);
        let target = |name: &'static str| {
            let log = self.0.clone();
            div()
                .id(name)
                .w(px(20.))
                .h(px(20.))
                .on_mouse_down(MouseButton::Left, move |_, _, _| {
                    log.borrow_mut().push(name.into());
                })
        };
        Region(
            div()
                .flex()
                .w(px(50.))
                .h(px(20.))
                .child(target("a"))
                .child(div().w(px(10.)))
                .child(target("b").occlude())
                .into_any_element(),
        )
    }
}

struct CachedHost {
    child: crate::Entity<CachedRegion>,
    prefix: bool,
    mirrored: bool,
}

impl Render for CachedHost {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        window.set_layout_mirrored(self.mirrored);
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
fn a_cached_unit_tracks_its_hitbox_boundary_after_a_sibling_is_inserted(cx: &mut TestAppContext) {
    let log = Log::default();
    let renders = Rc::new(Cell::new(0));
    let child = cx.new(|_| CachedRegion(log.clone(), renders.clone()));
    let (host, cx) = cx.add_window_view(move |_, _| CachedHost {
        child,
        prefix: false,
        mirrored: true,
    });
    let width = cx.update(|window, _| {
        window.set_layout_mirrored(true);
        window.viewport_size().width
    });
    cx.run_until_parked();
    for prefix in [true, false] {
        cx.run_until_parked();
        let count = renders.get();
        log.borrow_mut().clear();
        host.update(cx, |host, cx| {
            host.prefix = prefix;
            cx.notify();
        });
        cx.run_until_parked();
        assert_eq!(
            renders.get(),
            count,
            "prefix={prefix}: the child reused its cached paint"
        );
        cx.simulate_click(point(width - px(40.), px(10.)), Modifiers::default());
        assert_eq!(*log.borrow(), ["a"], "prefix={prefix}");
    }
}

#[gpui::test]
fn a_direction_change_during_render_invalidates_cached_geometry(cx: &mut TestAppContext) {
    let log = Log::default();
    let renders = Rc::new(Cell::new(0));
    let child = cx.new(|_| CachedRegion(log.clone(), renders.clone()));
    let (host, cx) = cx.add_window_view(move |_, _| CachedHost {
        child,
        prefix: false,
        mirrored: false,
    });
    cx.run_until_parked();
    for mirrored in [true, false] {
        let count = renders.get();
        host.update(cx, |host, cx| {
            host.mirrored = mirrored;
            cx.notify();
        });
        cx.run_until_parked();
        assert!(
            renders.get() > count,
            "direction changes invalidate cached paint"
        );
        log.borrow_mut().clear();
        let at = cx.update(|window, _| {
            point(
                if mirrored {
                    window.viewport_size().width - px(40.)
                } else {
                    px(10.)
                },
                px(10.),
            )
        });
        cx.simulate_click(at, Modifiers::default());
        assert_eq!(*log.borrow(), ["a"]);
        cx.run_until_parked();
    }
}
