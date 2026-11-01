// A mirrored text line moves as one unit, so its glyph order and decorations stay together.
use super::{Paint, Painted, frames};
use crate::{
    AppContext as _, Bounds, ContentMask, DecorationRun, DevicePixels, Font, FontId, FontMetrics,
    FontRun, GlyphId, LineLayout, NoopTextSystem, Pixels, PlatformTextSystem, RenderGlyphParams,
    SharedString, Size, TestAppContext, TestDispatcher, TextAlign, TextRenderingMode, TextRun,
    UnderlineStyle, Window, WrapBoundary, WrappedLine, WrappedLineLayout, black, font, point, px,
    red, size,
};
use anyhow::Result;
use smallvec::SmallVec;
use std::{borrow::Cow, cell::Cell, rc::Rc, sync::Arc};

// The no-op text system's layout, with a raster box per glyph so a painted line leaves sprites.
struct Glyphs;

impl PlatformTextSystem for Glyphs {
    fn add_fonts(&self, _: Vec<Cow<'static, [u8]>>) -> Result<()> {
        Ok(())
    }
    fn all_font_names(&self) -> Vec<String> {
        Vec::new()
    }
    fn font_id(&self, font: &Font) -> Result<FontId> {
        NoopTextSystem.font_id(font)
    }
    fn font_metrics(&self, font_id: FontId) -> FontMetrics {
        NoopTextSystem.font_metrics(font_id)
    }
    fn typographic_bounds(&self, font_id: FontId, glyph_id: GlyphId) -> Result<Bounds<f32>> {
        NoopTextSystem.typographic_bounds(font_id, glyph_id)
    }
    fn advance(&self, font_id: FontId, glyph_id: GlyphId) -> Result<Size<f32>> {
        NoopTextSystem.advance(font_id, glyph_id)
    }
    fn glyph_for_char(&self, font_id: FontId, ch: char) -> Option<GlyphId> {
        NoopTextSystem.glyph_for_char(font_id, ch)
    }
    fn glyph_raster_bounds(&self, _: &RenderGlyphParams) -> Result<Bounds<DevicePixels>> {
        Ok(Bounds {
            origin: point(DevicePixels(0), DevicePixels(-10)),
            size: size(DevicePixels(6), DevicePixels(12)),
        })
    }
    fn rasterize_glyph(
        &self,
        _: &RenderGlyphParams,
        raster_bounds: Bounds<DevicePixels>,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
        let area = raster_bounds.size.width.0 * raster_bounds.size.height.0;
        Ok((raster_bounds.size, vec![0; area as usize]))
    }
    fn layout_line(&self, text: &str, font_size: Pixels, runs: &[FontRun]) -> LineLayout {
        NoopTextSystem.layout_line(text, font_size, runs)
    }
    fn recommended_rendering_mode(&self, font_id: FontId, font_size: Pixels) -> TextRenderingMode {
        NoopTextSystem.recommended_rendering_mode(font_id, font_size)
    }
}

pub(super) fn text_cx() -> TestAppContext {
    TestAppContext::build_with_text_system(TestDispatcher::new(0), None, Arc::new(Glyphs))
}

pub(super) fn line(
    window: &Window,
    text: &str,
    underline: std::ops::Range<usize>,
) -> crate::ShapedLine {
    let style = UnderlineStyle {
        thickness: px(1.),
        color: Some(black()),
        wavy: false,
    };
    let decoration = |len: usize, underline: Option<UnderlineStyle>| DecorationRun {
        len: len as u32,
        color: black(),
        background_color: underline.map(|_| red()),
        underline,
        strikethrough: None,
    };
    let mut line = window.text_system().shape_line(
        SharedString::new(text),
        px(16.),
        &[TextRun {
            len: text.len(),
            font: font(".ZedMono"),
            color: black(),
            ..TextRun::default()
        }],
        None,
    );
    let runs = [
        decoration(underline.start, None),
        decoration(underline.len(), Some(style)),
        decoration(text.len() - underline.end, None),
    ];
    line.decoration_runs = runs.into_iter().filter(|run| run.len > 0).collect();
    line
}

#[test]
fn a_mirrored_text_line_moves_as_one_unit_with_its_decorations() {
    let cx = &mut text_cx();
    for scale in [1., 1.25, 2.] {
        let measured = Rc::new(Cell::new(0.));
        let line_width = measured.clone();
        let (plain, mirrored, width) = frames(cx, scale, move |window, cx| {
            let line = line(window, "abcdef", 1..3);
            line_width.set(line.width.0);
            let origin = point(px(30.25), px(10.));
            line.paint_background(origin, px(20.), TextAlign::Left, None, window, cx)
                .unwrap();
            line.paint(origin, px(20.), TextAlign::Left, None, window, cx)
                .unwrap();
        });
        let glyphs = |scene: &Painted| -> Vec<f32> {
            let mut xs: Vec<f32> = scene
                .monochrome_sprites
                .iter()
                .map(|sprite| sprite.bounds.origin.x.0)
                .chain(
                    scene
                        .subpixel_sprites
                        .iter()
                        .map(|sprite| sprite.bounds.origin.x.0),
                )
                .collect();
            xs.sort_by(f32::total_cmp);
            xs
        };
        let (before, after) = (glyphs(&plain), glyphs(&mirrored));
        assert_eq!(before.len(), 6, "scale {scale}");
        let shift = after[0] - before[0];
        assert!(shift > 0.);
        assert_eq!(shift.fract(), 0., "a unit moves by whole device pixels");
        // Glyph order is kept: every glyph moves by the same amount.
        for (before, after) in before.iter().zip(&after) {
            assert_eq!(after - before, shift, "scale {scale}");
        }
        assert_eq!(
            mirrored.underlines[0].bounds.origin.x.0 - plain.underlines[0].bounds.origin.x.0,
            shift
        );
        assert_eq!(
            mirrored.quads[0].bounds.origin.x.0 - plain.quads[0].bounds.origin.x.0,
            shift
        );
        // The line's left edge lands where its right edge reflects to.
        let line_right = (30.25 + measured.get()) * scale;
        assert!((before[0] + shift - (width - line_right)).abs() <= 1.5 * scale);
    }
}

#[test]
fn mirrored_wrapped_lines_move_one_visual_line_at_a_time() {
    let cx = &mut text_cx();
    let measured = Rc::new(Cell::new((0., 0.)));
    let widths = measured.clone();
    let (plain, mirrored, width) = frames(cx, 1., move |window, cx| {
        let line = line(window, "abcde", 0..0);
        let wrap_x = line.layout.runs[0].glyphs[3].position.x.0;
        widths.set((wrap_x, line.width.0 - wrap_x));
        let wrapped = WrappedLine {
            layout: Arc::new(WrappedLineLayout {
                unwrapped_layout: line.layout.clone(),
                wrap_boundaries: SmallVec::from_buf([WrapBoundary {
                    run_ix: 0,
                    glyph_ix: 3,
                }]),
                wrap_width: Some(px(100.)),
            }),
            text: line.text.clone(),
            decoration_runs: line.decoration_runs.to_vec(),
        };
        wrapped
            .paint(
                point(px(20.), px(0.)),
                px(20.),
                TextAlign::Left,
                None,
                window,
                cx,
            )
            .unwrap();
    });
    let rows = |scene: &Painted| -> Vec<Vec<f32>> {
        let mut glyphs: Vec<(f32, f32)> = scene
            .monochrome_sprites
            .iter()
            .map(|sprite| (sprite.bounds.origin.y.0, sprite.bounds.origin.x.0))
            .chain(
                scene
                    .subpixel_sprites
                    .iter()
                    .map(|sprite| (sprite.bounds.origin.y.0, sprite.bounds.origin.x.0)),
            )
            .collect();
        glyphs.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        let mut rows: Vec<Vec<f32>> = Vec::new();
        let mut last_y = None;
        for (y, x) in glyphs {
            if last_y.is_none_or(|last: f32| (y - last).abs() > 10.) {
                rows.push(Vec::new());
                last_y = Some(y);
            }
            rows.last_mut().unwrap().push(x);
        }
        rows
    };
    let (before, after) = (rows(&plain), rows(&mirrored));
    assert_eq!(before.iter().map(Vec::len).collect::<Vec<_>>(), [3, 2]);
    assert_eq!(after.iter().map(Vec::len).collect::<Vec<_>>(), [3, 2]);
    // Both left-aligned lines start at x 20; mirrored, each ends at its reflected right edge.
    let first_shift = after[0][0] - before[0][0];
    let second_shift = after[1][0] - before[1][0];
    let (first, second) = measured.get();
    assert_eq!(first_shift, (width - 20. - 20. - first).round());
    assert_eq!(second_shift, (width - 20. - 20. - second).round());
    for (row, (before, after)) in before.iter().zip(&after).enumerate() {
        let shift = after[0] - before[0];
        assert!(
            before.iter().zip(after).all(|(b, a)| a - b == shift),
            "line {row}"
        );
    }
}

#[test]
fn a_click_on_a_mirrored_text_line_maps_to_its_own_glyph() {
    let cx = &mut text_cx();
    let handle = cx.add_window(|_, _| {
        Paint(Rc::new(|window, cx| {
            let line = line(window, "abcdef", 0..0);
            line.paint(
                point(px(30.), px(10.)),
                px(20.),
                TextAlign::Left,
                None,
                window,
                cx,
            )
            .unwrap();
        }))
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.set_layout_mirrored(true);
        window.draw(cx).clear(cx);
        let unit = *window
            .rendered_frame
            .mirror_units
            .iter()
            .find(|unit| !unit.region && !unit.reflected)
            .expect("painted text line");
        assert!(!unit.region);
        assert_eq!(unit.logical.origin, point(px(30.), px(10.)));
        // The first glyph paints at the line's mirrored left edge and maps back to the line's start.
        let first = unit.physical().origin + point(px(1.), px(5.));
        assert_eq!(window.mirror_point_in(first), point(px(31.), px(15.)));
        let caret = Bounds::new(point(px(31.), px(15.)), size(px(1.), px(10.)));
        assert_eq!(
            window.mirror_bounds_out(caret),
            caret + point(unit.delta, px(0.))
        );
    })
    .unwrap();
}

#[test]
fn a_clipped_mirrored_text_line_takes_the_pointer_only_where_it_shows() {
    let cx = &mut text_cx();
    let handle = cx.add_window(|_, _| {
        Paint(Rc::new(|window, cx| {
            let mask = ContentMask {
                bounds: Bounds::new(point(px(30.), px(0.)), size(px(10.), px(40.))),
            };
            window.with_content_mask(Some(mask), |window| {
                let line = line(window, "abcdefghijkl", 0..0);
                assert!(line.width > px(30.));
                line.paint(
                    point(px(30.), px(10.)),
                    px(20.),
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                )
                .unwrap();
            });
        }))
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.set_layout_mirrored(true);
        window.draw(cx).clear(cx);
        let width = window.viewport_size().width;
        let unit = *window
            .rendered_frame
            .mirror_units
            .iter()
            .find(|unit| !unit.region && !unit.reflected)
            .expect("painted text line");
        let shown = unit.physical();
        assert!(shown.size.width > px(0.) && shown.size.height > px(0.));
        assert!(shown.left() >= width - px(40.) && shown.right() <= width - px(29.5));
        let visible = shown.center();
        assert_eq!(
            window.mirror_point_in(visible),
            visible - point(unit.delta, px(0.))
        );
        // The masked-off part of the line paints nowhere, so a click there reflects like any other.
        let hidden = point(width - px(50.), px(15.));
        assert_eq!(window.mirror_point_in(hidden), point(px(50.), px(15.)));
    })
    .unwrap();
}
