use super::*;
use crate::window::{HitboxId, MirrorInput, MirrorUnit};
use crate::{DispatchPhase, Hitbox, HitboxBehavior, MouseDownEvent};

fn empty(cx: &mut TestAppContext) -> &mut crate::VisualTestContext {
    let (_, cx) = cx.add_window_view(|_, _| Paint(Rc::new(|_, _| {})));
    cx.update(|window, _| window.set_layout_mirrored(true));
    cx.run_until_parked();
    cx
}

fn hit(id: u64, logical: Bounds<Pixels>, behavior: HitboxBehavior) -> Hitbox {
    Hitbox {
        id: HitboxId(id),
        bounds: logical,
        content_mask: ContentMask {
            bounds: bounds(0., 0., 2000., 100.),
        },
        behavior,
    }
}

#[gpui::test]
fn registered_hit_order_survives_parent_and_child_paint_scopes(cx: &mut TestAppContext) {
    let cx = empty(cx);
    cx.update(|window, _| {
        let width = window.viewport_size().width;
        let frame = &mut window.rendered_frame;
        frame.mirror_input = MirrorInput::default();
        frame.mirror_input.enabled = true;
        frame.mirror_input.width = width;
        let parent = frame.mirror_input.push_scope(None, None);
        let child = frame.mirror_input.push_scope(Some(parent), None);
        frame.hitboxes = vec![
            hit(10, bounds(0., 0., 20., 20.), HitboxBehavior::Normal),
            hit(11, bounds(0., 0., 20., 20.), HitboxBehavior::BlockMouse),
        ];
        let physical = Bounds::new(point(width - px(20.), px(0.)), size(px(20.), px(20.)));
        frame.mirror_input.stamp(0, child, physical);
        frame.mirror_input.stamp(1, parent, physical);
        assert_eq!(
            frame
                .hit_test(point(width - px(10.), px(10.)))
                .ids
                .as_slice(),
            [HitboxId(11)]
        );
    });
}

#[gpui::test]
fn an_unstamped_later_overlay_still_blocks_during_incremental_hit_testing(cx: &mut TestAppContext) {
    let cx = empty(cx);
    cx.update(|window, _| {
        let width = window.viewport_size().width;
        let frame = &mut window.rendered_frame;
        frame.mirror_input = MirrorInput::default();
        frame.mirror_input.enabled = true;
        frame.mirror_input.width = width;
        let row = frame.mirror_input.push_scope(None, Some(width - px(50.)));
        frame.hitboxes = vec![
            hit(20, bounds(0., 0., 20., 20.), HitboxBehavior::Normal),
            hit(21, bounds(10., 0., 40., 20.), HitboxBehavior::BlockMouse),
        ];
        frame.mirror_input.stamp(
            0,
            row,
            Bounds::new(point(width - px(50.), px(0.)), size(px(20.), px(20.))),
        );
        assert_eq!(
            frame
                .hit_test(point(width - px(49.), px(10.)))
                .ids
                .as_slice(),
            [HitboxId(21)]
        );
    });
}

#[gpui::test]
fn a_covered_descendant_does_not_supply_its_ancestors_pointer_transform(cx: &mut TestAppContext) {
    let cx = empty(cx);
    cx.update(|window, _| {
        let width = window.viewport_size().width;
        let frame = &mut window.rendered_frame;
        frame.mirror_input = MirrorInput::default();
        frame.mirror_input.enabled = true;
        frame.mirror_input.width = width;
        let root = frame.mirror_input.push_scope(None, None);
        let region = frame
            .mirror_input
            .push_scope(Some(root), Some(width - px(50.)));
        let card = frame.mirror_input.push_scope(Some(root), None);
        frame.hitboxes = vec![
            hit(30, bounds(0., 0., 20., 20.), HitboxBehavior::Normal),
            hit(31, bounds(10., 0., 40., 20.), HitboxBehavior::BlockMouse),
        ];
        frame.mirror_input.stamp(
            0,
            region,
            Bounds::new(point(width - px(50.), px(0.)), size(px(20.), px(20.))),
        );
        frame.mirror_input.stamp(
            1,
            card,
            Bounds::new(point(width - px(50.), px(0.)), size(px(40.), px(20.))),
        );
        frame.mirror_units = vec![MirrorUnit {
            logical: bounds(0., 0., 50., 20.),
            delta: width - px(50.),
            region: true,
            reflected: false,
            quad: None,
            scope: Some(region),
        }];
        let physical = point(width - px(49.), px(10.));
        assert_eq!(
            window.mirror_point_for_scope(physical, Some(card)),
            point(px(49.), px(10.))
        );
        assert_eq!(
            window.mirror_point_for_scope(physical, Some(root)),
            point(px(49.), px(10.))
        );
    });
}

#[gpui::test]
fn capture_prefers_a_containing_line_before_other_same_y_lines(cx: &mut TestAppContext) {
    let cx = empty(cx);
    cx.update(|window, _| {
        let width = window.viewport_size().width;
        let frame = &mut window.rendered_frame;
        frame.mirror_input = MirrorInput::default();
        frame.mirror_input.enabled = true;
        frame.mirror_input.width = width;
        let owner = frame.mirror_input.push_scope(None, None);
        frame.mirror_units = vec![
            MirrorUnit {
                logical: bounds(0., 0., 10., 20.),
                delta: width - px(10.),
                region: false,
                reflected: false,
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
        frame.hitboxes = vec![hit(40, bounds(0., 0., 100., 20.), HitboxBehavior::Normal)];
        frame.mirror_input.stamp(
            0,
            owner,
            Bounds::new(point(width - px(100.), px(0.)), size(px(100.), px(20.))),
        );
        let physical = point(width - px(5.), px(10.));
        let before = window.mirror_point_for_scope(physical, Some(owner));
        assert_eq!(before, point(px(5.), px(10.)));
        window.captured_hitbox = Some(HitboxId(40));
        assert_eq!(window.mirror_point_for_scope(physical, Some(owner)), before);
    });
}

struct SameScope {
    points: Rc<RefCell<Vec<Point<Pixels>>>>,
    width: Rc<Cell<Pixels>>,
    border_only: bool,
    capture: bool,
    corner_radii: Pixels,
    border_widths: crate::Edges<Pixels>,
}

impl IntoElement for SameScope {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for SameScope {
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
        let mut child = div().w(px(200.)).h(px(20.)).into_any_element();
        (child.request_layout(window, cx), child)
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        child: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) -> Hitbox {
        child.prepaint(window, cx);
        let line = super::super::text::line(window, "abcdefgh", 0..0);
        assert!(line.width > px(40.) && line.width < px(200.));
        self.width.set(line.width);
        window.insert_hitbox(
            Bounds::new(point(line.width - px(20.), px(0.)), size(px(20.), px(20.))),
            HitboxBehavior::BlockMouse,
        )
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        child: &mut AnyElement,
        hit: &mut Hitbox,
        window: &mut Window,
        cx: &mut App,
    ) {
        child.paint(window, cx);
        let line = super::super::text::line(window, "abcdefgh", 0..0);
        line.paint(
            point(px(0.), px(0.)),
            px(20.),
            crate::TextAlign::Left,
            None,
            window,
            cx,
        )
        .unwrap();
        let foreground = if self.border_only {
            crate::outline(hit.bounds, red(), Default::default()).border_widths(self.border_widths)
        } else {
            fill(hit.bounds, red())
        };
        window.paint_quad(foreground.corner_radii(self.corner_radii));
        let log = self.points.clone();
        let capture = self.capture;
        let moved = hit.clone();
        let hit = hit.clone();
        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, _| {
            if phase == DispatchPhase::Bubble && hit.is_hovered(window) {
                log.borrow_mut().push(event.position);
                if capture {
                    window.capture_pointer(hit.id);
                }
            }
        });
        let log = self.points.clone();
        window.on_mouse_event(move |event: &crate::MouseMoveEvent, phase, window, _| {
            if phase == DispatchPhase::Bubble && window.captured_hitbox() == Some(moved.id) {
                log.borrow_mut().push(event.position);
            }
        });
    }
}

struct SameScopeView {
    points: Rc<RefCell<Vec<Point<Pixels>>>>,
    width: Rc<Cell<Pixels>>,
    border_only: bool,
    capture: bool,
    corner_radii: Pixels,
    border_widths: crate::Edges<Pixels>,
}
impl Render for SameScopeView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        SameScope {
            points: self.points.clone(),
            width: self.width.clone(),
            border_only: self.border_only,
            capture: self.capture,
            corner_radii: self.corner_radii,
            border_widths: self.border_widths,
        }
    }
}

#[gpui::test]
fn a_foreground_painted_in_the_text_owners_element_gets_its_own_coordinates(
    cx: &mut TestAppContext,
) {
    let points = Rc::new(RefCell::new(Vec::new()));
    let width = Rc::new(Cell::new(px(0.)));
    let log = points.clone();
    let line_width = width.clone();
    let (_, cx) = cx.add_window_view(move |_, _| SameScopeView {
        points: log,
        width: line_width,
        border_only: false,
        capture: false,
        corner_radii: px(0.),
        border_widths: px(1.).into(),
    });
    let viewport = cx.update(|window, _| {
        window.set_layout_mirrored(true);
        window.viewport_size().width
    });
    cx.run_until_parked();
    cx.simulate_click(
        point(viewport - width.get() + px(10.), px(10.)),
        Modifiers::none(),
    );
    let points = points.borrow();
    assert_eq!(points.len(), 1);
    assert_eq!(points[0].y, px(10.));
    assert!((points[0].x - (width.get() - px(10.))).0.abs() < 0.001);
}

#[path = "mirror_surface_tests.rs"]
mod surface;
