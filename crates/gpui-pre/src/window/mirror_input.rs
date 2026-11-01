use super::*;

#[derive(Clone)]
pub(crate) struct MirrorScope {
    pub(crate) parent: Option<usize>,
    pub(crate) delta: Option<Pixels>,
}

#[derive(Clone)]
pub(crate) struct MirrorHit {
    pub(crate) scope: usize,
    pub(crate) physical: Bounds<Pixels>,
}

#[derive(Default)]
pub(crate) struct MirrorInput {
    pub(crate) enabled: bool,
    pub(crate) width: Pixels,
    pub(crate) scopes: Vec<MirrorScope>,
    pub(crate) hits: Vec<Option<MirrorHit>>,
    pub(crate) listeners: Vec<Option<usize>>,
    pub(crate) node_scopes: FxHashMap<DispatchNodeId, usize>,
    pub(crate) deferred_parents: Vec<(DispatchNodeId, usize)>,
    pending_parents: FxHashMap<DispatchNodeId, usize>,
    pub(crate) quads: Vec<MirrorQuad>,
}

impl MirrorInput {
    pub(crate) fn deferred_parent_scope(&mut self, node: DispatchNodeId) -> usize {
        if let Some(scope) = self
            .node_scopes
            .get(&node)
            .or(self.pending_parents.get(&node))
        {
            return *scope;
        }
        let scope = self.push_scope(None, None);
        self.pending_parents.insert(node, scope);
        scope
    }

    pub(crate) fn bind_node_scope(&mut self, node: DispatchNodeId, scope: usize) {
        self.node_scopes.insert(node, scope);
        if let Some(pending) = self.pending_parents.remove(&node) {
            self.scopes[pending].parent = Some(scope);
        }
    }

    pub(crate) fn ancestor(&self, ancestor: Option<usize>, mut child: Option<usize>) -> bool {
        while child.is_some() {
            if child == ancestor {
                return true;
            }
            child = child
                .and_then(|id| self.scopes.get(id))
                .and_then(|scope| scope.parent);
        }
        ancestor.is_none()
    }

    pub(crate) fn push_scope(&mut self, parent: Option<usize>, delta: Option<Pixels>) -> usize {
        let id = self.scopes.len();
        self.scopes.push(MirrorScope { parent, delta });
        id
    }

    pub(crate) fn stamp(&mut self, index: usize, scope: usize, physical: Bounds<Pixels>) {
        if self.hits.len() <= index {
            self.hits.resize_with(index + 1, || None);
        }
        self.hits[index] = Some(MirrorHit { scope, physical });
    }

    pub(crate) fn bounds(&self, index: usize, hitbox: &Hitbox) -> Bounds<Pixels> {
        self.hits
            .get(index)
            .and_then(Option::as_ref)
            .map(|hit| hit.physical)
            .unwrap_or_else(|| {
                let bounds = hitbox.bounds.intersect(&hitbox.content_mask.bounds);
                bounds + point(self.width - bounds.left() - bounds.right(), px(0.))
            })
    }

    pub(crate) fn hit_scope(&self, hitboxes: &[Hitbox], id: HitboxId) -> Option<usize> {
        hitboxes
            .iter()
            .position(|hitbox| hitbox.id == id)
            .and_then(|index| self.hits.get(index))
            .and_then(Option::as_ref)
            .map(|hit| hit.scope)
    }

    pub(crate) fn branches(&self, left: Option<usize>, right: Option<usize>) -> bool {
        self.ancestor(left, right) || self.ancestor(right, left)
    }

    fn related(&self, owner: Option<usize>, unit: &MirrorUnit) -> bool {
        owner.is_none()
            || self.ancestor(owner, unit.scope)
            || unit.region
                && self.ancestor(unit.scope, owner)
                && owner
                    .and_then(|scope| self.scopes.get(scope))
                    .and_then(|scope| scope.delta)
                    == Some(unit.delta)
    }

    fn contains(&self, unit: &MirrorUnit, position: Point<Pixels>) -> bool {
        unit.physical().contains(&position)
            && unit.quad.is_none_or(|index| {
                self.quads
                    .get(index)
                    .is_some_and(|quad| quad.contains(position))
            })
    }

    pub(crate) fn point(
        &self,
        position: Point<Pixels>,
        owner: Option<usize>,
        units: &[MirrorUnit],
        capture: Option<MirrorCapture>,
        anchor: Option<usize>,
    ) -> (Point<Pixels>, bool, bool) {
        let reflected = (point(self.width - position.x, position.y), false, true);
        if capture.is_some_and(|capture| capture.reflected) {
            return reflected;
        }
        let eligible = |unit: &&MirrorUnit| {
            self.related(owner, unit)
                && self.branches(anchor, unit.scope)
                && (capture.is_none() || !unit.reflected)
        };
        let unit = units
            .iter()
            .rev()
            .filter(eligible)
            .find(|unit| self.contains(unit, position))
            .or_else(|| {
                capture.and_then(|_| {
                    units.iter().rev().filter(eligible).find(|unit| {
                        !unit.region
                            && unit.logical.size.width > px(0.)
                            && position.y >= unit.physical().top()
                            && position.y <= unit.physical().bottom()
                    })
                })
            });
        if let Some(unit) = unit {
            return if unit.reflected {
                reflected
            } else {
                (position - point(unit.delta, px(0.)), unit.region, false)
            };
        }
        match owner
            .and_then(|id| self.scopes.get(id))
            .and_then(|scope| scope.delta)
        {
            Some(delta) => (position - point(delta, px(0.)), true, false),
            None => capture
                .map(|capture| {
                    (
                        position - point(capture.delta, px(0.)),
                        capture.unmirrored,
                        false,
                    )
                })
                .unwrap_or(reflected),
        }
    }

    pub(crate) fn physical_bounds(
        &self,
        bounds: Bounds<Pixels>,
        owner: Option<usize>,
        units: &[MirrorUnit],
    ) -> Bounds<Pixels> {
        let contains = |unit: &&MirrorUnit| {
            !unit.reflected && self.related(owner, unit) && unit.logical.contains(&bounds.origin)
        };
        let unit = units
            .iter()
            .rev()
            .filter(|unit| unit.region)
            .find(contains)
            .or_else(|| units.iter().rev().find(contains));
        match unit.map(|unit| unit.delta).or_else(|| {
            owner
                .and_then(|id| self.scopes.get(id))
                .and_then(|scope| scope.delta)
        }) {
            Some(delta) => bounds + point(delta, px(0.)),
            None => bounds + point(self.width - bounds.left() - bounds.right(), px(0.)),
        }
    }
}

impl Window {
    pub(crate) fn with_mirror_hitbox_range<R>(
        &mut self,
        range: Range<usize>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let previous = (
            self.mirror.scope,
            self.mirror.hitbox_start,
            self.mirror.hitbox_end,
        );
        self.mirror.hitbox_start = range.start;
        self.mirror.hitbox_end = range.end;
        if self.mirror.enabled {
            let delta = self.mirror.region_delta();
            let scope = self
                .next_frame
                .mirror_input
                .push_scope(self.mirror.scope, delta);
            self.mirror.scope = Some(scope);
            if let Some(node) = self.next_frame.dispatch_tree.active_node_id() {
                self.next_frame.mirror_input.bind_node_scope(node, scope);
            }
            self.stamp_mirror_hits(range, None);
        }
        let result = f(self);
        (
            self.mirror.scope,
            self.mirror.hitbox_start,
            self.mirror.hitbox_end,
        ) = previous;
        result
    }

    pub(crate) fn stamp_mirror_hits(
        &mut self,
        range: Range<usize>,
        inside: Option<Bounds<Pixels>>,
    ) {
        let Some(scope) = self.mirror.scope else {
            return;
        };
        let mut changed = false;
        for index in range {
            let hitbox = &self.next_frame.hitboxes[index];
            if inside.is_some_and(|bounds| {
                hitbox.bounds.left() < bounds.left()
                    || hitbox.bounds.top() < bounds.top()
                    || hitbox.bounds.right() > bounds.right()
                    || hitbox.bounds.bottom() > bounds.bottom()
            }) {
                continue;
            }
            let physical = self
                .mirror
                .bounds(hitbox.bounds.intersect(&hitbox.content_mask.bounds))
                .intersect(&self.mirror.physical_mask());
            changed |= self.next_frame.mirror_input.bounds(index, hitbox) != physical;
            self.next_frame.mirror_input.stamp(index, scope, physical);
        }
        if changed {
            self.mouse_hit_test = self.next_frame.hit_test(self.mouse_position);
        }
    }

    pub(crate) fn mirror_owner(&self, position: Point<Pixels>) -> Option<usize> {
        let frame = &self.rendered_frame;
        self.captured_hitbox
            .or_else(|| frame.hit_test(position).ids.first().copied())
            .and_then(|id| frame.mirror_input.hit_scope(&frame.hitboxes, id))
    }

    pub(crate) fn mirror_bounds_out_for_scope(
        &self,
        bounds: Bounds<Pixels>,
        owner: Option<usize>,
    ) -> Bounds<Pixels> {
        if !self.mirror.enabled {
            return bounds;
        }
        self.rendered_frame.mirror_input.physical_bounds(
            bounds,
            owner,
            &self.rendered_frame.mirror_units,
        )
    }

    pub(crate) fn reuse_mirror_input(&mut self, range: &Range<PaintIndex>) {
        let old_root = range.start.mirror_scope;
        let new_root = self.mirror.scope;
        let old_start = range.start.mirror_scopes_index;
        let new_start = self.next_frame.mirror_input.scopes.len();
        let remap = |id: Option<usize>| {
            id.map(|id| {
                if Some(id) == old_root {
                    new_root.expect("cached mirror root")
                } else if id >= old_start && id < range.end.mirror_scopes_index {
                    new_start + id - old_start
                } else {
                    new_root.expect("cached mirror ancestor")
                }
            })
        };
        let scopes = self.rendered_frame.mirror_input.scopes
            [old_start..range.end.mirror_scopes_index]
            .iter()
            .cloned()
            .map(|mut scope| {
                scope.parent = remap(scope.parent);
                scope
            })
            .collect::<Vec<_>>();
        self.next_frame.mirror_input.scopes.extend(scopes);
        let mut parents = mem::take(&mut self.next_frame.mirror_input.deferred_parents);
        parents.retain(|(node, scope)| {
            let included = Some(*scope) == old_root
                || *scope >= old_start && *scope < range.end.mirror_scopes_index;
            if included {
                self.next_frame
                    .mirror_input
                    .bind_node_scope(*node, remap(Some(*scope)).unwrap());
            }
            !included
        });
        self.next_frame.mirror_input.deferred_parents = parents;
        let old_hits = range.start.mirror_hitbox_start..range.start.mirror_hitbox_end;
        assert_eq!(
            old_hits.len(),
            self.mirror.hitbox_end - self.mirror.hitbox_start
        );
        for (offset, index) in old_hits.enumerate() {
            if let Some(hit) = self
                .rendered_frame
                .mirror_input
                .hits
                .get(index)
                .and_then(Option::as_ref)
            {
                self.next_frame.mirror_input.stamp(
                    self.mirror.hitbox_start + offset,
                    remap(Some(hit.scope)).unwrap(),
                    hit.physical,
                );
            }
        }
        let old_quads = range.start.mirror_quads_index..range.end.mirror_quads_index;
        let new_quad_start = self.next_frame.mirror_input.quads.len();
        self.next_frame.mirror_input.quads.extend(
            self.rendered_frame.mirror_input.quads[old_quads.clone()]
                .iter()
                .copied(),
        );
        self.next_frame.mirror_units.extend(
            self.rendered_frame.mirror_units
                [range.start.mirror_units_index..range.end.mirror_units_index]
                .iter()
                .map(|unit| MirrorUnit {
                    scope: remap(unit.scope),
                    quad: unit
                        .quad
                        .map(|index| new_quad_start + index - old_quads.start),
                    ..*unit
                }),
        );
        self.next_frame.mirror_input.listeners.extend(
            self.rendered_frame.mirror_input.listeners
                [range.start.mouse_listeners_index..range.end.mouse_listeners_index]
                .iter()
                .map(|scope| remap(*scope)),
        );
        for handler in self.rendered_frame.input_handlers
            [range.start.input_handlers_index..range.end.input_handlers_index]
            .iter_mut()
            .flatten()
        {
            handler.mirror_scope = remap(handler.mirror_scope);
        }
        self.mouse_hit_test = self.next_frame.hit_test(self.mouse_position);
    }
}
