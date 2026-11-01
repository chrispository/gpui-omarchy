use super::*;

#[gpui::test]
fn captured_text_extrapolates_over_its_earlier_background(cx: &mut TestAppContext) {
    let cx = empty(cx);
    cx.update(|window, _| {
        let width = window.viewport_size().width;
        let frame = &mut window.rendered_frame;
        frame.mirror_input = MirrorInput::default();
        frame.mirror_input.enabled = true;
        frame.mirror_input.width = width;
        let owner = frame.mirror_input.push_scope(None, None);
        frame.hitboxes = vec![hit(70, bounds(0., 0., 100., 20.), HitboxBehavior::Normal)];
        frame.mirror_input.stamp(
            0,
            owner,
            Bounds::new(point(width - px(100.), px(0.)), size(px(100.), px(20.))),
        );
        frame.mirror_units = vec![
            MirrorUnit {
                logical: bounds(0., 0., 100., 20.),
                delta: width - px(100.),
                region: false,
                reflected: true,
                quad: None,
                scope: Some(owner),
            },
            MirrorUnit {
                logical: bounds(30., 0., 10., 20.),
                delta: width - px(70.),
                region: false,
                reflected: false,
                quad: None,
                scope: Some(owner),
            },
        ];
        window.mouse_position = point(width - px(35.), px(10.));
        window.mouse_hit_test = window.rendered_frame.hit_test(window.mouse_position);
        window.capture_pointer(HitboxId(70));
        assert_eq!(
            window.mirror_point_for_scope(point(width - px(20.), px(10.)), Some(owner)),
            point(px(50.), px(10.))
        );
    });
}

#[gpui::test]
fn a_captured_foreground_keeps_its_pointer_coordinates(cx: &mut TestAppContext) {
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
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    cx.simulate_event(crate::MouseMoveEvent {
        position: at,
        pressed_button: Some(MouseButton::Left),
        ..Default::default()
    });
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    let points = points.borrow();
    assert_eq!(points.len(), 2);
    assert!((points[0].x - (width.get() - px(10.))).0.abs() < 0.001);
    assert_eq!(
        points[1], points[0],
        "capture must not move an unchanged pointer"
    );
}

#[gpui::test]
fn a_border_only_surface_preserves_text_in_its_transparent_interior(cx: &mut TestAppContext) {
    let points = Rc::new(RefCell::new(Vec::new()));
    let width = Rc::new(Cell::new(px(0.)));
    let log = points.clone();
    let line_width = width.clone();
    let (_, cx) = cx.add_window_view(move |_, _| SameScopeView {
        points: log,
        width: line_width,
        border_only: true,
        capture: false,
        corner_radii: px(0.),
        border_widths: px(1.).into(),
    });
    let viewport = cx.update(|window, _| {
        window.set_layout_mirrored(true);
        window.viewport_size().width
    });
    cx.run_until_parked();
    let at = point(viewport - width.get() + px(10.), px(10.));
    let expected_text = cx.update(|window, _| {
        let text = window
            .rendered_frame
            .mirror_units
            .iter()
            .find(|unit| !unit.region && !unit.reflected)
            .unwrap();
        at - point(text.delta, px(0.))
    });
    assert!(expected_text.x < px(20.));
    cx.simulate_click(at, Modifiers::none());
    assert_eq!(*points.borrow(), [expected_text]);
    points.borrow_mut().clear();
    let border = point(at.x, px(0.5));
    cx.simulate_click(border, Modifiers::none());
    let points = points.borrow();
    assert_eq!(points.len(), 1);
    assert_eq!(points[0], point(viewport - border.x, border.y));
}

#[path = "mirror_shape_r7_tests.rs"]
mod shape_r7;
