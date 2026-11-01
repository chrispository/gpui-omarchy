use super::*;

#[path = "mirror_quad_tests.rs"]
mod quad_coverage;

#[gpui::test]
fn captured_text_keeps_its_acquired_frame_outside_all_line_rows(cx: &mut TestAppContext) {
    let cx = empty(cx);
    cx.update(|window, _| {
        let width = window.viewport_size().width;
        let at = point(width - px(35.), px(10.));
        let frame = &mut window.rendered_frame;
        frame.mirror_input = MirrorInput::default();
        frame.mirror_input.enabled = true;
        frame.mirror_input.width = width;
        let owner = frame.mirror_input.push_scope(None, None);
        frame.hitboxes = vec![hit(78, bounds(0., 0., 100., 20.), HitboxBehavior::Normal)];
        frame.mirror_input.stamp(
            0,
            owner,
            Bounds::new(point(width - px(100.), px(0.)), size(px(100.), px(20.))),
        );
        frame.mirror_units = vec![MirrorUnit {
            logical: bounds(30., 0., 10., 20.),
            delta: width - px(70.),
            region: false,
            reflected: false,
            quad: None,
            scope: Some(owner),
        }];
        window.mouse_position = at;
        window.mouse_hit_test = window.rendered_frame.hit_test(at);
        assert_eq!(
            window.mirror_point_for_scope(at, Some(owner)),
            point(px(35.), px(10.))
        );
        window.capture_pointer(HitboxId(78));
        let outside = point(width - px(20.), px(-10.));
        assert_eq!(
            window.mirror_point_for_scope(outside, Some(owner)),
            point(px(50.), px(-10.))
        );
    });
}

#[gpui::test]
fn capture_keeps_a_disjoint_foregrounds_acquired_frame(cx: &mut TestAppContext) {
    let cx = empty(cx);
    cx.update(|window, _| {
        let width = window.viewport_size().width;
        let at = point(width - px(40.), px(10.));
        let frame = &mut window.rendered_frame;
        frame.mirror_input = MirrorInput::default();
        frame.mirror_input.enabled = true;
        frame.mirror_input.width = width;
        let owner = frame.mirror_input.push_scope(None, None);
        frame.hitboxes = vec![hit(
            77,
            bounds(30., 0., 20., 20.),
            HitboxBehavior::BlockMouse,
        )];
        frame.mirror_input.stamp(
            0,
            owner,
            Bounds::new(point(width - px(50.), px(0.)), size(px(20.), px(20.))),
        );
        frame.mirror_units = vec![
            MirrorUnit {
                logical: bounds(30., 0., 20., 20.),
                delta: width - px(80.),
                region: false,
                reflected: true,
                quad: None,
                scope: Some(owner),
            },
            MirrorUnit {
                logical: bounds(0., 0., 20., 20.),
                delta: width - px(20.),
                region: false,
                reflected: false,
                quad: None,
                scope: Some(owner),
            },
        ];
        assert!(frame.mirror_units[0].physical().contains(&at));
        assert!(!frame.mirror_units[1].physical().contains(&at));
        window.mouse_position = at;
        window.mouse_hit_test = window.rendered_frame.hit_test(at);
        let before = window.mirror_point_for_scope(at, Some(owner));
        assert_eq!(before, point(px(40.), px(10.)));
        window.capture_pointer(HitboxId(77));
        assert_eq!(window.mirror_point_for_scope(at, Some(owner)), before);
    });
}

#[gpui::test]
fn captured_foreground_keeps_its_frame_after_leaving_its_bounds(cx: &mut TestAppContext) {
    let points = Rc::new(RefCell::new(Vec::new()));
    let width = Rc::new(Cell::new(px(0.)));
    let log = points.clone();
    let line_width = width.clone();
    let (_, cx) = cx.add_window_view(move |_, _| SameScopeView {
        points: log,
        width: line_width,
        border_only: false,
        capture: true,
        corner_radii: px(0.),
        border_widths: px(1.).into(),
    });
    let viewport = cx.update(|window, _| {
        window.set_layout_mirrored(true);
        window.viewport_size().width
    });
    cx.run_until_parked();
    let at = point(viewport - width.get() + px(10.), px(10.));
    let outside = at + point(px(11.), px(0.));
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    cx.simulate_event(crate::MouseMoveEvent {
        position: outside,
        pressed_button: Some(MouseButton::Left),
        ..Default::default()
    });
    cx.simulate_mouse_up(outside, MouseButton::Left, Modifiers::none());
    let points = points.borrow();
    assert_eq!(points.len(), 2);
    assert_eq!(points[1], point(viewport - outside.x, outside.y));
}

fn clear_shape_points(
    cx: &mut TestAppContext,
    border_only: bool,
    radius: Pixels,
    border_widths: crate::Edges<Pixels>,
    clear: &[(f32, f32)],
    scale: f32,
) {
    let points = Rc::new(RefCell::new(Vec::new()));
    let width = Rc::new(Cell::new(px(0.)));
    let log = points.clone();
    let line_width = width.clone();
    let (_, cx) = cx.add_window_view(move |_, _| SameScopeView {
        points: log,
        width: line_width,
        border_only,
        capture: false,
        corner_radii: radius,
        border_widths,
    });
    cx.simulate_scale_factor_change(scale);
    let viewport = cx.update(|window, _| {
        window.set_layout_mirrored(true);
        window.viewport_size().width
    });
    cx.run_until_parked();
    for &(x, y) in clear {
        points.borrow_mut().clear();
        let at = point(viewport - width.get() + px(x), px(y));
        let expected = cx.update(|window, _| {
            let unit = window
                .rendered_frame
                .mirror_units
                .iter()
                .find(|unit| !unit.reflected && !unit.region)
                .unwrap();
            at - point(unit.delta, px(0.))
        });
        cx.simulate_click(at, Modifiers::none());
        assert_eq!(*points.borrow(), [expected], "clear point {x},{y}");
    }
    points.borrow_mut().clear();
    let stroke = point(viewport - width.get() + px(10.), px(0.5));
    cx.simulate_click(stroke, Modifiers::none());
    assert_eq!(*points.borrow(), [point(viewport - stroke.x, stroke.y)]);
}

#[gpui::test]
fn rounded_outline_clear_pixels_preserve_visible_text(cx: &mut TestAppContext) {
    clear_shape_points(cx, true, px(8.), px(1.).into(), &[(10., 5.), (5., 5.)], 2.);
}

#[gpui::test]
fn zero_width_outline_edges_preserve_visible_text(cx: &mut TestAppContext) {
    clear_shape_points(
        cx,
        true,
        px(0.),
        crate::Edges {
            top: px(1.),
            left: px(0.),
            right: px(0.),
            bottom: px(0.),
        },
        &[(0.5, 10.)],
        1.,
    );
}

#[gpui::test]
fn filled_rounded_quad_clear_corners_preserve_visible_text(cx: &mut TestAppContext) {
    clear_shape_points(cx, false, px(8.), px(0.).into(), &[(0.5, 0.5)], 2.);
}
