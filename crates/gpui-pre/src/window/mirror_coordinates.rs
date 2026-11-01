use super::*;

#[derive(Clone, Copy)]
pub(crate) struct MirrorCapture {
    pub(crate) hitbox: HitboxId,
    pub(crate) reflected: bool,
    pub(crate) delta: Pixels,
    pub(crate) unmirrored: bool,
}

impl Window {
    pub(crate) fn mirror_point_for_scope(
        &self,
        position: Point<Pixels>,
        owner: Option<usize>,
    ) -> Point<Pixels> {
        self.mirror_coordinates(position, owner, false).0
    }

    pub(crate) fn mirror_coordinates(
        &self,
        position: Point<Pixels>,
        owner: Option<usize>,
        scroll: bool,
    ) -> (Point<Pixels>, bool) {
        let captured = self.captured_hitbox.map(|hitbox| {
            self.captured_mirror
                .filter(|capture| capture.hitbox == hitbox)
                .unwrap_or(MirrorCapture {
                    hitbox,
                    reflected: false,
                    delta: px(0.),
                    unmirrored: false,
                })
        });
        let (point, unmirrored, _) = self.mirror_mapping(position, owner, scroll, captured);
        (point, unmirrored)
    }

    pub(crate) fn mirror_mapping(
        &self,
        position: Point<Pixels>,
        owner: Option<usize>,
        scroll: bool,
        capture: Option<MirrorCapture>,
    ) -> (Point<Pixels>, bool, bool) {
        if !self.mirror.enabled {
            return (position, false, false);
        }
        let frame = if self.mirror.scope.is_some() {
            &self.next_frame
        } else {
            &self.rendered_frame
        };
        let input = &frame.mirror_input;
        let captured = capture
            .and_then(|capture| {
                input
                    .hit_scope(&frame.hitboxes, capture.hitbox)
                    .map(|scope| (scope, capture))
            })
            .filter(|(scope, _)| input.branches(owner, Some(*scope)));
        let queried;
        let hits = if position == self.mouse_position {
            &self.mouse_hit_test
        } else {
            queried = frame.hit_test(position);
            &queried
        };
        let count = if scroll {
            hits.indices.len()
        } else {
            hits.hover_hitbox_count
        };
        let anchor = captured
            .map(|(scope, _)| scope)
            .or_else(|| {
                hits.indices.iter().take(count).find_map(|index| {
                    let scope = input
                        .hits
                        .get(*index)
                        .and_then(Option::as_ref)
                        .map(|hit| hit.scope);
                    scope.filter(|scope| input.branches(owner, Some(*scope)))
                })
            })
            .or(owner);
        input.point(
            position,
            owner,
            &frame.mirror_units,
            captured.map(|(_, capture)| capture),
            anchor,
        )
    }

    pub(crate) fn record_mirror_quad(&mut self, logical: Bounds<ScaledPixels>, quad: Quad) {
        if quad.background.is_transparent()
            && (quad.border_color.a == 0.
                || [
                    quad.border_widths.left,
                    quad.border_widths.right,
                    quad.border_widths.top,
                    quad.border_widths.bottom,
                ]
                .iter()
                .all(|width| width.0 == 0.))
        {
            return;
        }
        let before = self.next_frame.mirror_units.len();
        self.record_mirror_surface(logical, quad.bounds);
        if self.next_frame.mirror_units.len() > before {
            let index = self.next_frame.mirror_input.quads.len();
            let scale = self.scale_factor();
            self.next_frame
                .mirror_input
                .quads
                .push(MirrorQuad { quad, scale });
            self.next_frame.mirror_units.last_mut().unwrap().quad = Some(index);
        }
    }

    pub(crate) fn record_mirror_surface(
        &mut self,
        logical: Bounds<ScaledPixels>,
        physical: Bounds<ScaledPixels>,
    ) {
        if !self.mirror.enabled || self.mirror.in_unit() {
            return;
        }
        let scale = self.scale_factor();
        let logical = Bounds::new(
            point(
                px(logical.origin.x.0 / scale),
                px(logical.origin.y.0 / scale),
            ),
            size(
                px(logical.size.width.0 / scale),
                px(logical.size.height.0 / scale),
            ),
        );
        let delta = px(physical.origin.x.0 / scale) - logical.origin.x;
        let unit = MirrorUnit {
            logical,
            delta,
            region: false,
            scope: self.mirror.scope,
            reflected: true,
            quad: None,
        };
        let visible = self.mirror.visible(unit);
        if !visible.logical.is_empty() {
            self.next_frame.mirror_units.push(visible);
        }
    }
}
