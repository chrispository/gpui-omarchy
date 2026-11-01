//! Qt's `LayoutMirroring` for a window: layout stays logical, paint and pointer reflect, and a unit (a text line or an unmirrored region) moves whole.
use crate::{
    Background, BackgroundTag, Bounds, ContentMask, Corners, Edges, Pixels, Point, Primitive, Quad,
    ScaledPixels, ScrollDelta, Size, TransformationMatrix, Underline, point, px,
};
use std::ops::Range;

/// A region painted and hit as one left-to-right unit, translated to its mirrored place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MirrorUnit {
    /// The unit's logical bounds.
    pub(crate) logical: Bounds<Pixels>,
    /// The logical x shift to its painted place, a whole number of device pixels.
    pub(crate) delta: Pixels,
    /// A `with_unmirrored` region, which keeps its scroll direction and IME placement; else a text line.
    pub(crate) region: bool,
    /// The paint scope that owns this unit.
    pub(crate) scope: Option<usize>,
    /// A reflected non-text painted surface rather than a translated line or region.
    pub(crate) reflected: bool,
    pub(crate) quad: Option<usize>,
}

impl MirrorUnit {
    pub(crate) fn physical(&self) -> Bounds<Pixels> {
        self.logical + point(self.delta, px(0.))
    }
}

#[derive(Default)]
pub(crate) struct Mirror {
    pub(crate) enabled: bool,
    /// Physical content masks, parallel to the window's logical mask stack while enabled.
    masks: Vec<Bounds<Pixels>>,
    unit: Option<MirrorUnit>,
    pub(crate) scope: Option<usize>,
    pub(crate) event_scope: Option<usize>,
    pub(crate) hitbox_start: usize,
    pub(crate) hitbox_end: usize,
    viewport: Size<Pixels>,
    scale: f32,
}

impl Mirror {
    /// Takes the frame's viewport; input between frames maps against the frame last painted.
    pub(crate) fn begin_frame(&mut self, viewport: Size<Pixels>, scale: f32) {
        self.viewport = viewport;
        self.scale = scale;
        self.masks.clear();
        self.unit = None;
        self.scope = None;
        self.event_scope = None;
        self.hitbox_start = 0;
        self.hitbox_end = 0;
    }

    fn round(&self, value: Pixels) -> Pixels {
        px((value.0 * self.scale).round() / self.scale)
    }

    /// The x shift that reflects a box of this left edge and width: the box moves, its contents do not.
    fn shift(&self, left: Pixels, width: Pixels) -> Pixels {
        match self.unit {
            Some(unit) => unit.delta,
            None => self.viewport.width - left - left - width,
        }
    }

    pub(crate) fn bounds(&self, bounds: Bounds<Pixels>) -> Bounds<Pixels> {
        if !self.enabled {
            return bounds;
        }
        bounds + point(self.shift(bounds.left(), bounds.size.width), px(0.))
    }

    /// Starts a unit unless one is already open, which then keeps governing: the outermost wins.
    pub(crate) fn enter(&mut self, logical: Bounds<Pixels>, region: bool) -> Option<MirrorUnit> {
        if !self.enabled || self.unit.is_some() {
            return None;
        }
        let delta =
            self.round(self.viewport.width - logical.left() - logical.left() - logical.size.width);
        let unit = MirrorUnit {
            logical,
            delta,
            region,
            scope: self.scope,
            reflected: false,
            quad: None,
        };
        self.unit = Some(unit);
        Some(unit)
    }

    /// The part of a unit its physical mask shows, which is all of it a pointer can reach.
    pub(crate) fn visible(&self, unit: MirrorUnit) -> MirrorUnit {
        let shown = unit.physical().intersect(&self.mask());
        MirrorUnit {
            logical: shown - point(unit.delta, px(0.)),
            ..unit
        }
    }

    pub(crate) fn exit(&mut self, entered: Option<MirrorUnit>) {
        if entered.is_some() {
            self.unit = None;
        }
    }

    pub(crate) fn push_mask(&mut self, logical: Bounds<Pixels>) {
        if self.enabled {
            let physical = self.bounds(logical).intersect(&self.mask());
            self.masks.push(physical);
        }
    }

    pub(crate) fn pop_mask(&mut self) {
        if self.enabled {
            self.masks.pop();
        }
    }

    pub(crate) fn region_delta(&self) -> Option<Pixels> {
        self.unit.filter(|unit| unit.region).map(|unit| unit.delta)
    }

    pub(crate) fn unit_scope(&mut self, scope: usize) {
        if let Some(unit) = self.unit.as_mut() {
            unit.scope = Some(scope);
        }
    }

    pub(crate) fn in_unit(&self) -> bool {
        self.unit.is_some()
    }

    pub(crate) fn physical_mask(&self) -> Bounds<Pixels> {
        self.mask()
    }

    fn mask(&self) -> Bounds<Pixels> {
        self.masks
            .last()
            .copied()
            .unwrap_or_else(|| Bounds::new(Point::default(), self.viewport))
    }

    /// The physical mask, covering whole device pixels the way the window snaps logical masks.
    pub(crate) fn scaled_mask(
        &self,
        cover: impl Fn(Bounds<Pixels>) -> Bounds<ScaledPixels>,
    ) -> ContentMask<ScaledPixels> {
        ContentMask {
            bounds: cover(self.mask()),
        }
    }

    fn scaled_shift(&self, bounds: &Bounds<ScaledPixels>) -> ScaledPixels {
        match self.unit {
            Some(unit) => ScaledPixels((unit.delta.0 * self.scale).round()),
            None => {
                let width = (self.viewport.width.0 * self.scale).round();
                ScaledPixels(width - bounds.origin.x.0 - bounds.origin.x.0 - bounds.size.width.0)
            }
        }
    }

    /// Moves a snapped primitive to its painted place; `mask` is the physical content mask.
    pub(crate) fn primitive(&self, primitive: &mut Primitive, mask: ContentMask<ScaledPixels>) {
        let reflect = self.unit.is_none();
        match primitive {
            Primitive::Shadow(shadow) => {
                shadow.bounds.origin.x += self.scaled_shift(&shadow.bounds);
                shadow.element_bounds.origin.x += self.scaled_shift(&shadow.element_bounds);
                if reflect {
                    shadow.corner_radii = swap_corners(shadow.corner_radii);
                    shadow.element_corner_radii = swap_corners(shadow.element_corner_radii);
                }
                shadow.content_mask = mask;
            }
            Primitive::Quad(quad) => self.quad(quad, mask),
            Primitive::Path(path) => {
                let shift = self.scaled_shift(&path.bounds);
                path.bounds.origin.x += shift;
                for vertex in &mut path.vertices {
                    vertex.xy_position.x += shift;
                    vertex.content_mask = mask;
                }
                if reflect {
                    reflect_background(&mut path.color);
                }
                path.content_mask = mask;
            }
            Primitive::Underline(underline) => self.underline(underline, mask),
            Primitive::MonochromeSprite(sprite) => {
                let shift = self.scaled_shift(&sprite.bounds);
                sprite.bounds.origin.x += shift;
                sprite.transformation = shifted(sprite.transformation, shift);
                sprite.content_mask = mask;
            }
            Primitive::SubpixelSprite(sprite) => {
                let shift = self.scaled_shift(&sprite.bounds);
                sprite.bounds.origin.x += shift;
                sprite.transformation = shifted(sprite.transformation, shift);
                sprite.content_mask = mask;
            }
            Primitive::PolychromeSprite(sprite) => {
                sprite.bounds.origin.x += self.scaled_shift(&sprite.bounds);
                if reflect {
                    sprite.corner_radii = swap_corners(sprite.corner_radii);
                }
                sprite.content_mask = mask;
            }
            Primitive::Surface(surface) => {
                surface.bounds.origin.x += self.scaled_shift(&surface.bounds);
                surface.content_mask = mask;
            }
        }
    }

    pub(crate) fn quad(&self, quad: &mut Quad, mask: ContentMask<ScaledPixels>) {
        quad.bounds.origin.x += self.scaled_shift(&quad.bounds);
        if self.unit.is_none() {
            quad.corner_radii = swap_corners(quad.corner_radii);
            quad.border_widths = swap_edges(quad.border_widths);
            reflect_background(&mut quad.background);
        }
        quad.content_mask = mask;
    }

    pub(crate) fn underline(&self, underline: &mut Underline, mask: ContentMask<ScaledPixels>) {
        underline.bounds.origin.x += self.scaled_shift(&underline.bounds);
        underline.content_mask = mask;
    }

    /// A device-space span with the move of the primitive it belongs to: shifted in a unit, else reflected.
    pub(crate) fn scaled_span(&self, span: Range<ScaledPixels>) -> Range<ScaledPixels> {
        match self.unit {
            Some(unit) => {
                let delta = ScaledPixels((unit.delta.0 * self.scale).round());
                span.start + delta..span.end + delta
            }
            None => {
                let width = ScaledPixels((self.viewport.width.0 * self.scale).round());
                width - span.end..width - span.start
            }
        }
    }

    /// Maps logical bounds handed to the platform (an IME caret) to the physical ones.
    #[cfg(test)]
    pub(crate) fn physical_bounds(
        &self,
        bounds: Bounds<Pixels>,
        units: &[MirrorUnit],
    ) -> Bounds<Pixels> {
        let contains =
            |unit: &&MirrorUnit| !unit.reflected && unit.logical.contains(&bounds.origin);
        match units
            .iter()
            .rev()
            .filter(|unit| unit.region)
            .find(contains)
            .or_else(|| units.iter().rev().find(contains))
        {
            Some(unit) => bounds + point(unit.delta, px(0.)),
            None => bounds + point(self.viewport.width - bounds.left() - bounds.right(), px(0.)),
        }
    }

    /// A horizontal scroll outside an unmirrored region runs the other way, so content follows the fingers.
    pub(crate) fn scroll_delta(delta: ScrollDelta) -> ScrollDelta {
        match delta {
            ScrollDelta::Pixels(delta) => ScrollDelta::Pixels(point(-delta.x, delta.y)),
            ScrollDelta::Lines(delta) => ScrollDelta::Lines(point(-delta.x, delta.y)),
        }
    }
}

fn swap_corners<T: Clone + std::fmt::Debug + Default + PartialEq>(
    corners: Corners<T>,
) -> Corners<T> {
    Corners {
        top_left: corners.top_right,
        top_right: corners.top_left,
        bottom_right: corners.bottom_left,
        bottom_left: corners.bottom_right,
    }
}

fn swap_edges<T: Clone + std::fmt::Debug + Default + PartialEq>(edges: Edges<T>) -> Edges<T> {
    Edges {
        top: edges.top,
        right: edges.left,
        bottom: edges.bottom,
        left: edges.right,
    }
}

/// A linear gradient's angle runs clockwise from the top, so its mirror is the angle's negation.
fn reflect_background(background: &mut Background) {
    if background.tag == BackgroundTag::LinearGradient {
        background.gradient_angle_or_pattern_height =
            (360. - background.gradient_angle_or_pattern_height).rem_euclid(360.);
    }
}

/// Conjugates a sprite transformation by a horizontal shift, so a rotation keeps its moved centre.
fn shifted(matrix: TransformationMatrix, shift: ScaledPixels) -> TransformationMatrix {
    let [[a, _], [c, _]] = matrix.rotation_scale;
    let [x, y] = matrix.translation;
    TransformationMatrix {
        rotation_scale: matrix.rotation_scale,
        translation: [x + shift.0 - a * shift.0, y - c * shift.0],
    }
}
