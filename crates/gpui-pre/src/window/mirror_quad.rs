use super::*;
use crate::color::BackgroundTag;

const HALF_PIXEL: f32 = 0.5;
const PACKED_PATTERN_PERIOD: f32 = 65535.;
const PACKED_PATTERN_COMPONENT: f32 = 255.;
const FULL_TURN_DEGREES: f32 = 360.;
const OUTSIDE_INNER_EDGE: f32 = -1.;

#[derive(Clone, Copy)]
pub(crate) struct MirrorQuad {
    pub(crate) quad: Quad,
    pub(crate) scale: f32,
}

impl MirrorQuad {
    // Match fs_quad's coverage at the recorded physical scale, not its shading-work masks.
    pub(crate) fn contains(&self, position: Point<Pixels>) -> bool {
        let quad = &self.quad;
        let position = point(
            ScaledPixels(position.x.0 * self.scale),
            ScaledPixels(position.y.0 * self.scale),
        );
        if !quad.bounds.contains(&position) || !quad.content_mask.bounds.contains(&position) {
            return false;
        }
        let p = point(
            position.x.0 - quad.bounds.left().0,
            position.y.0 - quad.bounds.top().0,
        );
        let half = point(
            quad.bounds.size.width.0 / 2.,
            quad.bounds.size.height.0 / 2.,
        );
        let center = p - half;
        let radii = quad.corner_radii;
        let radius = if center.x < 0. {
            if center.y < 0. {
                radii.top_left.0
            } else {
                radii.bottom_left.0
            }
        } else if center.y < 0. {
            radii.top_right.0
        } else {
            radii.bottom_right.0
        };
        let corner = point(center.x.abs() - half.x, center.y.abs() - half.y);
        let rounded = corner + point(radius, radius);
        let outer = if radius == 0. {
            rounded.x.max(rounded.y)
        } else {
            rounded.x.max(0.).hypot(rounded.y.max(0.)) + rounded.x.max(rounded.y).min(0.) - radius
        };
        if outer >= HALF_PIXEL {
            return false;
        }
        if background_alpha(quad.background, p, quad.bounds.size) > 0. {
            return true;
        }
        if quad.border_color.a == 0. {
            return false;
        }
        let border = point(
            if center.x < 0. {
                quad.border_widths.left.0
            } else {
                quad.border_widths.right.0
            },
            if center.y < 0. {
                quad.border_widths.top.0
            } else {
                quad.border_widths.bottom.0
            },
        );
        let reduced = point(
            if border.x == 0. {
                -HALF_PIXEL
            } else {
                border.x
            },
            if border.y == 0. {
                -HALF_PIXEL
            } else {
                border.y
            },
        );
        let straight = corner + reduced;
        let inner = if rounded.x <= 0. || rounded.y <= 0. {
            -straight.x.max(straight.y)
        } else if straight.x > 0. || straight.y > 0. {
            OUTSIDE_INNER_EDGE
        } else if reduced.x == reduced.y {
            -(outer + reduced.x)
        } else {
            let ellipse = point((radius - reduced.x).max(0.), (radius - reduced.y).max(0.));
            ((rounded.x / ellipse.x).hypot(rounded.y / ellipse.y) - 1.)
                * (ellipse.x + ellipse.y)
                * -HALF_PIXEL
        };
        inner < HALF_PIXEL
            && (quad.border_style != BorderStyle::Dashed
                || super::mirror_dash::alpha(quad, p, center, rounded, radius) > 0.)
    }
}

// Alpha interpolation is independent of the shader's RGB color-space conversion.
fn background_alpha(background: Background, p: Point<f32>, size: Size<ScaledPixels>) -> f32 {
    match background.tag {
        BackgroundTag::Solid => background.solid.a,
        BackgroundTag::LinearGradient => {
            let angle = (background.gradient_angle_or_pattern_height % FULL_TURN_DEGREES)
                .to_radians()
                - std::f32::consts::FRAC_PI_2;
            let mut direction = point(angle.cos(), angle.sin());
            let size = point(size.width.0, size.height.0);
            if size.x > size.y {
                direction.y *= size.y / size.x;
            } else {
                direction.x *= size.x / size.y;
            }
            let center = p - size / 2.;
            let mut t =
                (center.x * direction.x + center.y * direction.y) / direction.x.hypot(direction.y);
            t = if direction.x.abs() > direction.y.abs() {
                (t + size.x / 2.) / size.x
            } else {
                (t + size.y / 2.) / size.y
            };
            let [first, last] = background.colors;
            t = ((t - first.percentage) / (last.percentage - first.percentage)).clamp(0., 1.);
            first.color.a + (last.color.a - first.color.a) * t
        }
        BackgroundTag::PatternSlash => {
            let packed = background.gradient_angle_or_pattern_height;
            let width = (packed / PACKED_PATTERN_PERIOD) / PACKED_PATTERN_COMPONENT;
            let interval = (packed % PACKED_PATTERN_PERIOD) / PACKED_PATTERN_COMPONENT;
            let height = width + interval;
            let diagonal = std::f32::consts::FRAC_PI_4;
            let period = height * diagonal.sin();
            let pattern = (diagonal.cos() * p.x + diagonal.sin() * p.y) % period;
            let distance = pattern.min(period - pattern) - period * (width / height) / 2.;
            background.solid.a * (HALF_PIXEL - distance).clamp(0., 1.)
        }
        BackgroundTag::Checkerboard => {
            let size = background.gradient_angle_or_pattern_height;
            background.solid.a * (((p.x / size).floor() + (p.y / size).floor()) % 2.).clamp(0., 1.)
        }
    }
}
