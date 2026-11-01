// A mirrored window reflects boxes, moves an unmirrored region whole, and maps pointer input back.
use crate::{
    AnyElement, App, AppContext as _, Bounds, ContentMask, Context, Corners, Edges, Element,
    GlobalElementId, InspectorElementId, InteractiveElement as _, IntoElement, LayoutId, Modifiers,
    MouseButton, MouseExitEvent, ParentElement as _, Pixels, Render, ScrollDelta, ScrollWheelEvent,
    Styled as _, TestAppContext, Window, black, canvas, deferred, div, fill, point, px, red, size,
};
use std::{cell::Cell, cell::RefCell, rc::Rc};

use super::mirror::Mirror;

struct Paint(Rc<dyn Fn(&mut Window, &mut App)>);

impl Render for Paint {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let paint = self.0.clone();
        canvas(|_, _, _| {}, move |_, _, window, cx| paint(window, cx)).size_full()
    }
}

fn bounds(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(w), px(h)))
}

/// The primitives a frame painted.
struct Painted {
    quads: Vec<crate::Quad>,
    underlines: Vec<crate::Underline>,
    monochrome_sprites: Vec<crate::MonochromeSprite>,
    subpixel_sprites: Vec<crate::SubpixelSprite>,
}

/// Draws `paint` plain and mirrored at `scale`: what each frame painted and the device viewport width.
fn frames(
    cx: &mut TestAppContext,
    scale: f32,
    paint: impl Fn(&mut Window, &mut App) + 'static,
) -> (Painted, Painted, f32) {
    let handle = cx.add_window(move |_, _| Paint(Rc::new(paint)));
    cx.simulate_window_scale_factor_change(handle.into(), scale);
    let mut scenes = Vec::new();
    let mut width = 0.;
    for mirrored in [false, true] {
        cx.update_window(handle.into(), |_, window, cx| {
            window.set_layout_mirrored(mirrored);
            window.draw(cx).clear(cx);
            width = (window.viewport_size().width.0 * scale).round();
            let scene = &window.rendered_frame.scene;
            scenes.push(Painted {
                quads: scene.quads.clone(),
                underlines: scene.underlines.clone(),
                monochrome_sprites: scene.monochrome_sprites.clone(),
                subpixel_sprites: scene.subpixel_sprites.clone(),
            });
        })
        .unwrap();
    }
    let mirrored = scenes.pop().unwrap();
    (scenes.pop().unwrap(), mirrored, width)
}

#[gpui::test]
fn a_mirrored_quad_reflects_its_box_and_swaps_its_sides(cx: &mut TestAppContext) {
    for scale in [1., 1.5, 2.] {
        let (plain, mirrored, width) = frames(cx, scale, |window, _| {
            let mut quad = fill(bounds(10., 20., 50., 30.), red());
            quad.corner_radii = Corners {
                top_left: px(4.),
                ..Corners::default()
            };
            quad.border_widths = Edges {
                left: px(2.),
                ..Edges::default()
            };
            quad.border_color = black();
            window.paint_quad(quad);
        });
        let (plain, mirrored) = (&plain.quads[0], &mirrored.quads[0]);
        assert_eq!(mirrored.bounds.size, plain.bounds.size);
        assert_eq!(
            mirrored.bounds.origin.x.0,
            width - plain.bounds.right().0,
            "scale {scale}"
        );
        assert_eq!(mirrored.bounds.origin.y, plain.bounds.origin.y);
        assert_eq!(mirrored.corner_radii.top_right, plain.corner_radii.top_left);
        assert_eq!(mirrored.corner_radii.top_left.0, 0.);
        assert_eq!(mirrored.border_widths.right, plain.border_widths.left);
        assert_eq!(mirrored.border_widths.left.0, 0.);
    }
}

#[gpui::test]
fn a_mirrored_mask_clips_the_reflected_side(cx: &mut TestAppContext) {
    let (plain, mirrored, width) = frames(cx, 1., |window, _| {
        window.with_content_mask(
            Some(ContentMask {
                bounds: bounds(0., 0., 100., 300.),
            }),
            |window| {
                window.paint_quad(fill(bounds(0., 0., 200., 10.), red()));
            },
        );
    });
    assert_eq!(plain.quads[0].content_mask.bounds.origin.x.0, 0.);
    let mask = mirrored.quads[0].content_mask.bounds;
    assert_eq!((mask.origin.x.0, mask.size.width.0), (width - 100., 100.));
    assert_eq!(mirrored.quads[0].bounds.origin.x.0, width - 200.);
}

#[gpui::test]
fn an_unmirrored_region_moves_whole_and_maps_the_pointer_back(cx: &mut TestAppContext) {
    let region = bounds(10., 0., 100., 20.);
    let (plain, mirrored, width) = frames(cx, 1., move |window, _| {
        window.with_unmirrored(region, |window| {
            window.paint_quad(fill(bounds(10., 0., 20., 20.), red()));
            window.paint_quad(fill(bounds(60., 0., 50., 20.), red()));
        });
    });
    let shift = width - 10. - 10. - 100.;
    assert_eq!((plain.quads.len(), mirrored.quads.len()), (2, 2));
    for (plain, mirrored) in plain.quads.iter().zip(&mirrored.quads) {
        assert_eq!(mirrored.bounds.origin.x.0 - plain.bounds.origin.x.0, shift);
    }
    let mut mirror = Mirror::default();
    mirror.enabled = true;
    mirror.begin_frame(size(px(width), px(300.)), 1.);
    let mut input = super::mirror_input::MirrorInput::default();
    input.width = px(width);
    let unit = mirror.enter(region, true).unwrap();
    mirror.exit(Some(unit));
    let inside = point(px(60. + shift + 5.), px(5.));
    assert_eq!(
        input.point(inside, None, &[unit], None, None),
        (point(px(65.), px(5.)), true, false)
    );
    let outside = point(px(5.), px(5.));
    assert_eq!(
        input.point(outside, None, &[unit], None, None),
        (point(px(width - 5.), px(5.)), false, true)
    );
    let caret = bounds(65., 2., 1., 16.);
    assert_eq!(
        mirror.physical_bounds(caret, &[unit]),
        caret + point(px(shift), px(0.))
    );
    assert_eq!(
        mirror.physical_bounds(bounds(0., 2., 30., 16.), &[unit]),
        bounds(width - 30., 2., 30., 16.)
    );
}

#[gpui::test]
fn a_nested_region_follows_the_outermost(cx: &mut TestAppContext) {
    let (plain, mirrored, width) = frames(cx, 1., move |window, _| {
        window.with_unmirrored(bounds(0., 0., 200., 20.), |window| {
            window.with_unmirrored(bounds(150., 0., 50., 20.), |window| {
                window.paint_quad(fill(bounds(150., 0., 50., 20.), red()));
            });
        });
    });
    assert_eq!(
        mirrored.quads[0].bounds.origin.x.0 - plain.quads[0].bounds.origin.x.0,
        width - 200.
    );
}

#[gpui::test]
fn a_click_in_a_mirrored_window_lands_on_the_logical_element(cx: &mut TestAppContext) {
    struct Target(Rc<Cell<u32>>);
    impl Render for Target {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let hits = self.0.clone();
            div().size_full().child(
                div()
                    .id("start")
                    .w(px(50.))
                    .h(px(20.))
                    .on_mouse_down(crate::MouseButton::Left, move |_, _, _| {
                        hits.set(hits.get() + 1)
                    }),
            )
        }
    }
    let hits = Rc::new(Cell::new(0));
    let target = hits.clone();
    let (_, cx) = cx.add_window_view(move |_, _| Target(target));
    let width = cx.update(|window, _| {
        window.set_layout_mirrored(true);
        window.viewport_size().width
    });
    cx.run_until_parked();
    cx.simulate_click(point(px(25.), px(10.)), Modifiers::default());
    assert_eq!(hits.get(), 0, "the physical left is the logical right");
    cx.simulate_click(point(width - px(25.), px(10.)), Modifiers::default());
    assert_eq!(hits.get(), 1);
}

/// An element that paints its child left to right, as Flea's `Unmirrored` does.
struct Region(AnyElement);

impl IntoElement for Region {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for Region {
    type RequestLayoutState = ();
    type PrepaintState = ();
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
        self.0.prepaint(window, cx);
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_unmirrored(bounds, |window| self.0.paint(window, cx));
    }
}

type Log = Rc<RefCell<Vec<String>>>;

// Two targets in a 50 px region at the left, and with `cover` a deferred card over the first one's painted place.
struct Targets(Log, bool);

impl Render for Targets {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let log = self.0.clone();
        let target = |name: &'static str| {
            let (clicks, scrolls) = (log.clone(), log.clone());
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
        let region = div()
            .flex()
            .w(px(50.))
            .h(px(20.))
            .child(target("a"))
            .child(div().w(px(10.)))
            .child(target("b"));
        let card = target("card").absolute().left(px(30.)).top_0().occlude();
        div()
            .size_full()
            .child(Region(region.into_any_element()))
            .child(
                div()
                    .id("outside")
                    .absolute()
                    .top(px(40.))
                    .w(px(20.))
                    .h(px(20.))
                    .child(target("c")),
            )
            .children(self.1.then(|| deferred(card)))
    }
}

fn targets(cx: &mut TestAppContext, cover: bool) -> (Log, &mut crate::VisualTestContext, Pixels) {
    let log = Log::default();
    let shared = log.clone();
    let (_, cx) = cx.add_window_view(move |_, _| Targets(shared, cover));
    let width = cx.update(|window, _| {
        window.set_layout_mirrored(true);
        window.viewport_size().width
    });
    cx.run_until_parked();
    (log, cx, width)
}

#[gpui::test]
fn a_click_in_an_unmirrored_region_lands_on_its_logical_child(cx: &mut TestAppContext) {
    let (log, cx, width) = targets(cx, false);
    // a paints at the region's mirrored left edge, where a reflection would find b.
    cx.simulate_click(point(width - px(40.), px(10.)), Modifiers::default());
    assert_eq!(*log.borrow(), ["a"]);
}

#[gpui::test]
fn a_card_painted_over_an_unmirrored_region_takes_its_clicks(cx: &mut TestAppContext) {
    let (log, cx, width) = targets(cx, true);
    cx.simulate_click(point(width - px(40.), px(10.)), Modifiers::default());
    assert_eq!(*log.borrow(), ["card"]);
}

#[gpui::test]
fn a_horizontal_scroll_runs_the_other_way_outside_an_unmirrored_region(cx: &mut TestAppContext) {
    let (log, cx, width) = targets(cx, false);
    for y in [10., 50.] {
        cx.simulate_event(ScrollWheelEvent {
            position: point(width - px(10.), px(y)),
            delta: ScrollDelta::Pixels(point(px(3.), px(-2.))),
            ..Default::default()
        });
    }
    let kept = format!("b {:?}", ScrollDelta::Pixels(point(px(3.), px(-2.))));
    let turned = format!("c {:?}", ScrollDelta::Pixels(point(px(-3.), px(-2.))));
    assert_eq!(*log.borrow(), [kept, turned]);
}

#[gpui::test]
fn a_mouse_exit_reports_the_logical_position(cx: &mut TestAppContext) {
    let seen = Rc::new(Cell::new(None));
    let shared = seen.clone();
    let handle = cx.add_window(move |_, _| {
        Paint(Rc::new(move |window, _| {
            let shared = shared.clone();
            window.on_mouse_event(move |event: &MouseExitEvent, _, _, _| {
                shared.set(Some(event.position))
            });
        }))
    });
    let width = cx
        .update_window(handle.into(), |_, window, cx| {
            window.set_layout_mirrored(true);
            window.draw(cx).clear(cx);
            window.viewport_size().width
        })
        .unwrap();
    let cx = &mut crate::VisualTestContext::from_window(handle.into(), cx);
    cx.simulate_event(MouseExitEvent {
        position: point(px(5.), px(5.)),
        pressed_button: None,
        modifiers: Modifiers::default(),
    });
    assert_eq!(seen.get(), Some(point(width - px(5.), px(5.))));
}

#[gpui::test]
fn a_window_that_stops_mirroring_paints_as_before(cx: &mut TestAppContext) {
    let handle = cx.add_window(|_, _| {
        Paint(Rc::new(|window, _| {
            window.paint_quad(fill(bounds(10., 20., 50., 30.), red()))
        }))
    });
    let mut origins = Vec::new();
    for mirrored in [false, true, false] {
        cx.update_window(handle.into(), |_, window, cx| {
            window.set_layout_mirrored(mirrored);
            window.draw(cx).clear(cx);
            origins.push(window.rendered_frame.scene.quads[0].bounds.origin.x.0);
        })
        .unwrap();
    }
    assert_ne!(origins[1], origins[0]);
    assert_eq!(origins[2], origins[0]);
}

/// Length and capacity of every table in a frame's mirror input.
fn mirror_tables(input: &super::mirror_input::MirrorInput) -> [(usize, usize); 7] {
    [
        (input.scopes.len(), input.scopes.capacity()),
        (input.hits.len(), input.hits.capacity()),
        (input.listeners.len(), input.listeners.capacity()),
        (input.node_scopes.len(), input.node_scopes.capacity()),
        (
            input.deferred_parents.len(),
            input.deferred_parents.capacity(),
        ),
        (
            input.pending_parents.len(),
            input.pending_parents.capacity(),
        ),
        (input.quads.len(), input.quads.capacity()),
    ]
}

#[gpui::test]
fn a_cleared_frame_keeps_its_mirror_tables(cx: &mut TestAppContext) {
    let handle = cx.add_window(|_, _| {
        Paint(Rc::new(|window, _| {
            for _ in 0..4 {
                window.on_mouse_event(|_: &MouseExitEvent, _, _, _| {});
            }
            window.with_unmirrored(bounds(10., 0., 100., 20.), |window| {
                window.paint_quad(fill(bounds(10., 0., 20., 20.), red()));
            });
            window.paint_quad(fill(bounds(10., 40., 20., 20.), red()));
        }))
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.set_layout_mirrored(true);
        for _ in 0..2 {
            window.draw(cx).clear(cx);
        }
        // This scene leaves hits and the deferred tables empty, so they are filled by hand.
        let node = window.rendered_frame.dispatch_tree.root_node_id();
        let input = &mut window.rendered_frame.mirror_input;
        input.stamp(0, 0, bounds(10., 0., 20., 20.));
        input.deferred_parents.push((node, 0));
        input.pending_parents.insert(node, 0);
        assert!(input.enabled && input.width > px(0.));
        let before = mirror_tables(input);
        assert!(before.iter().all(|&(len, _)| len > 0), "{before:?}");
        // Every draw clears the frame it just swapped out, as here.
        window.rendered_frame.clear();
        let input = &window.rendered_frame.mirror_input;
        assert!(!input.enabled && input.width == px(0.));
        let after = mirror_tables(input);
        assert_eq!(
            after,
            before.map(|(_, capacity)| (0, capacity)),
            "a steady frame refills its tables without allocating"
        );
    })
    .unwrap();
}

#[path = "mirror_text_tests.rs"]
mod text;

#[path = "mirror_overlay_tests.rs"]
mod overlay;

#[path = "mirror_owner_tests.rs"]
mod owner;

#[path = "mirror_capture_tests.rs"]
mod capture;
