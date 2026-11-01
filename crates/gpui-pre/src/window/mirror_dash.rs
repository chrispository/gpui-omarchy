use super::*;

const DASH_LENGTH_PER_WIDTH: f32 = 2.;
const DASH_GAP_PER_WIDTH: f32 = 1.;
const HALF_PIXEL: f32 = 0.5;

// Keep dash phase and antialiasing identical to fs_quad's border coverage.
pub(crate) fn alpha(
    quad: &Quad,
    p: Point<f32>,
    center: Point<f32>,
    corner: Point<f32>,
    radius: f32,
) -> f32 {
    let period_per_width = DASH_LENGTH_PER_WIDTH + DASH_GAP_PER_WIDTH;
    let numerator = 1. / period_per_width;
    let radii = quad.corner_radii;
    let widths = quad.border_widths;
    let unrounded = [
        radii.top_left,
        radii.top_right,
        radii.bottom_left,
        radii.bottom_right,
    ]
    .iter()
    .all(|radius| radius.0 == 0.);
    let (t, mut max_t, velocity) = if unrounded {
        let horizontal = corner.x < corner.y;
        let width = if horizontal {
            widths.top.0.max(widths.bottom.0)
        } else {
            widths.left.0.max(widths.right.0)
        };
        if width == 0. {
            return 0.;
        }
        let velocity = numerator / width;
        if horizontal {
            (
                p.x * velocity,
                quad.bounds.size.width.0 * velocity,
                velocity,
            )
        } else {
            (
                p.y * velocity,
                quad.bounds.size.height.0 * velocity,
                velocity,
            )
        }
    } else {
        let velocity = |width: ScaledPixels| {
            if width.0 > 0. {
                numerator / width.0
            } else {
                0.
            }
        };
        let top = velocity(widths.top);
        let right = velocity(widths.right);
        let bottom = velocity(widths.bottom);
        let left = velocity(widths.left);
        let tr = corner_velocity(top, right);
        let br = corner_velocity(bottom, right);
        let bl = corner_velocity(bottom, left);
        let tl = corner_velocity(top, left);
        let side_top = (quad.bounds.size.width.0 - radii.top_left.0 - radii.top_right.0) * top;
        let side_right =
            (quad.bounds.size.height.0 - radii.top_right.0 - radii.bottom_right.0) * right;
        let side_bottom =
            (quad.bounds.size.width.0 - radii.bottom_left.0 - radii.bottom_right.0) * bottom;
        let side_left = (quad.bounds.size.height.0 - radii.top_left.0 - radii.bottom_left.0) * left;
        let upto_tr = side_top;
        let upto_right = upto_tr + radii.top_right.0 * std::f32::consts::FRAC_PI_2 * tr;
        let upto_br = upto_right + side_right;
        let upto_bottom = upto_br + radii.bottom_right.0 * std::f32::consts::FRAC_PI_2 * br;
        let upto_bl = upto_bottom + side_bottom;
        let upto_left = upto_bl + radii.bottom_left.0 * std::f32::consts::FRAC_PI_2 * bl;
        let upto_tl = upto_left + side_left;
        let max_t = upto_tl + radii.top_left.0 * std::f32::consts::FRAC_PI_2 * tl;
        let (t, velocity) = if corner.x >= 0. && corner.y >= 0. {
            let distance = corner.y.atan2(corner.x) * radius;
            if center.x >= 0. {
                if center.y < 0. {
                    (upto_right - distance * tr, tr)
                } else {
                    (upto_br + distance * br, br)
                }
            } else if center.y >= 0. {
                (upto_left - distance * bl, bl)
            } else {
                (upto_tl + distance * tl, tl)
            }
        } else if corner.x < corner.y {
            if center.y < 0. {
                ((p.x - radii.top_left.0) * top, top)
            } else {
                (upto_bl - (p.x - radii.bottom_left.0) * bottom, bottom)
            }
        } else if center.x < 0. {
            (upto_tl - (p.y - radii.top_left.0) * left, left)
        } else {
            (upto_right + (p.y - radii.top_right.0) * right, right)
        };
        (t, max_t, velocity)
    };
    let length = DASH_LENGTH_PER_WIDTH / period_per_width;
    if unrounded {
        max_t -= length;
    }
    let period = if max_t >= 1. {
        max_t / max_t.floor()
    } else if unrounded && max_t - length > 0. {
        max_t
    } else {
        return 1.;
    };
    if velocity == 0. {
        return 0.;
    }
    let centered = (t + period / 2. - length / 2.) % period - period / 2.;
    let distance = centered.abs() - length / 2.;
    (HALF_PIXEL - distance / velocity).clamp(0., 1.)
}

fn corner_velocity(first: f32, second: f32) -> f32 {
    if first == 0. {
        second
    } else if second == 0. {
        first
    } else {
        first.min(second)
    }
}
