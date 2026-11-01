use super::*;
use crate::window::mirror_quad::MirrorQuad;
use crate::{Background, Quad, ScaledPixels, transparent_black};

fn outline_geometry(scale: f32) -> MirrorQuad {
    let bounds = bounds(0., 0., 20., 20.).scale(scale);
    let radii: crate::Corners<Pixels> = px(8.).into();
    let widths: crate::Edges<Pixels> = px(1.).into();
    MirrorQuad {
        quad: Quad {
            bounds,
            content_mask: ContentMask { bounds },
            border_color: red(),
            corner_radii: radii.scale(scale),
            border_widths: widths.scale(scale),
            ..Default::default()
        },
        scale,
    }
}

#[test]
fn quad_coverage_matches_clear_curves_strokes_scale_and_clipping() {
    for scale in [1., 1.5, 2.] {
        let mut geometry = outline_geometry(scale);
        for at in [
            point(px(10.), px(5.)),
            point(px(5.), px(5.)),
            point(px(0.5), px(0.5)),
        ] {
            assert!(!geometry.contains(at), "clear {at:?} at scale {scale}");
        }
        assert!(geometry.contains(point(px(10.), px(0.5))));
        geometry.quad.content_mask.bounds = bounds(0., 5., 20., 15.).scale(scale);
        assert!(!geometry.contains(point(px(10.), px(0.5))));
        geometry.quad.content_mask.bounds = geometry.quad.bounds;
        geometry.quad.background = red().into();
        assert!(geometry.contains(point(px(10.), px(5.))));
        assert!(!geometry.contains(point(px(0.5), px(0.5))));
    }
}

#[test]
fn asymmetric_and_zero_width_edges_do_not_gain_coverage() {
    let mut geometry = outline_geometry(1.);
    geometry.quad.border_widths = crate::Edges {
        top: ScaledPixels(1.),
        left: ScaledPixels(3.),
        right: ScaledPixels(0.),
        bottom: ScaledPixels(2.),
    };
    assert!(geometry.contains(point(px(0.5), px(10.))));
    assert!(!geometry.contains(point(px(19.5), px(10.))));
    assert!(!geometry.contains(point(px(5.), px(5.))));
    assert!(geometry.contains(point(px(10.), px(19.5))));
}

#[test]
fn dashed_stroke_gaps_remain_clear() {
    let mut geometry = outline_geometry(1.);
    geometry.quad.bounds = bounds(0., 0., 100., 20.).scale(1.);
    geometry.quad.content_mask.bounds = geometry.quad.bounds;
    geometry.quad.corner_radii = Default::default();
    geometry.quad.border_widths = crate::Edges {
        top: ScaledPixels(2.),
        left: ScaledPixels(0.),
        right: ScaledPixels(0.),
        bottom: ScaledPixels(0.),
    };
    geometry.quad.border_style = crate::BorderStyle::Dashed;
    assert!(geometry.contains(point(px(2.), px(0.5))));
    assert!(!geometry.contains(point(px(5.), px(0.5))));
    assert!(geometry.contains(point(px(8.), px(0.5))));
}

struct CachedShape {
    points: Rc<RefCell<Vec<Point<Pixels>>>>,
    width: Rc<Cell<Pixels>>,
    renders: Rc<Cell<usize>>,
}
impl Render for CachedShape {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        SameScope {
            points: self.points.clone(),
            width: self.width.clone(),
            border_only: true,
            capture: false,
            corner_radii: px(8.),
            border_widths: px(1.).into(),
        }
    }
}
struct ShapeHost {
    child: crate::Entity<CachedShape>,
    prefix: bool,
}
impl Render for ShapeHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .children(
                self.prefix
                    .then(|| div().absolute().size(px(100.)).bg(red())),
            )
            .child(
                self.child
                    .clone()
                    .cached(crate::StyleRefinement::default().size(px(100.))),
            )
    }
}

#[gpui::test]
fn cached_quad_coverage_rebases_after_prefix_insert_and_remove(cx: &mut TestAppContext) {
    let points = Rc::new(RefCell::new(Vec::new()));
    let width = Rc::new(Cell::new(px(0.)));
    let renders = Rc::new(Cell::new(0));
    let (log, line_width, count) = (points.clone(), width.clone(), renders.clone());
    let child = cx.new(move |_| CachedShape {
        points: log,
        width: line_width,
        renders: count,
    });
    let (host, cx) = cx.add_window_view(move |_, _| ShapeHost {
        child,
        prefix: false,
    });
    let viewport = cx.update(|window, _| {
        window.set_layout_mirrored(true);
        window.viewport_size().width
    });
    cx.run_until_parked();
    let at = point(viewport - width.get() + px(5.), px(5.));
    cx.simulate_event(crate::MouseMoveEvent {
        position: at,
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
        assert_eq!(renders.get(), before, "the shape owner reused cached paint");
        let expected = cx.update(|window, _| {
            let text = window
                .rendered_frame
                .mirror_units
                .iter()
                .find(|u| !u.region && !u.reflected)
                .unwrap();
            at - point(text.delta, px(0.))
        });
        cx.simulate_click(at, Modifiers::none());
        assert_eq!(*points.borrow(), [expected], "prefix {prefix}");
        points.borrow_mut().clear();
        let stroke = point(viewport - width.get() + px(10.), px(0.5));
        cx.simulate_click(stroke, Modifiers::none());
        assert_eq!(
            *points.borrow(),
            [point(viewport - stroke.x, stroke.y)],
            "stroke prefix {prefix}"
        );
    }
}

#[test]
fn transparent_gradient_stops_and_checker_gaps_remain_clear() {
    let mut geometry = outline_geometry(1.);
    geometry.quad.corner_radii = Default::default();
    geometry.quad.border_widths = Default::default();
    let mut background = Background::default();
    background.tag = crate::color::BackgroundTag::LinearGradient;
    background.gradient_angle_or_pattern_height = 90.;
    background.colors = [
        crate::LinearColorStop {
            color: transparent_black(),
            percentage: 0.25,
        },
        crate::LinearColorStop {
            color: red(),
            percentage: 0.75,
        },
    ];
    geometry.quad.background = background;
    assert!(!geometry.contains(point(px(1.), px(10.))));
    assert!(geometry.contains(point(px(19.), px(10.))));
    background.tag = crate::color::BackgroundTag::Checkerboard;
    background.solid = red();
    background.gradient_angle_or_pattern_height = 4.;
    geometry.quad.background = background;
    assert!(!geometry.contains(point(px(1.), px(1.))));
    assert!(geometry.contains(point(px(5.), px(1.))));
}
