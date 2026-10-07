//! Font-picker samples use the document shaper, including collection faces and variable instances.
//! They are UI textures only: browsing never changes the document or its undo history.

use std::collections::HashMap;

use egui::{Color32, Response, Sense, TextureHandle, Ui, vec2};
use vectorcraft_doc::{CharStyle, NodeKind, TextObject};
use vectorcraft_geom::{Affine, Point, Shape};
use vectorcraft_text::{FontDb, layout};

use crate::VectorcraftApp;
use crate::theme::Tokens;

const SAMPLE: &str = "AaBbCc 123";
const SAMPLE_WIDTH: f32 = 220.0;
const NAME_WIDTH: f32 = 180.0;
const MAX_CACHED: usize = 128;

#[derive(Clone, Default)]
struct Previews(HashMap<PreviewKey, Option<TextureHandle>>);

type PreviewKey = (String, String, String, u32, u32);

pub(crate) struct FontSample {
    pub text: String,
    pub style: String,
    pub size: f32,
    pub enabled: bool,
}

impl Default for FontSample {
    fn default() -> Self {
        Self { text: SAMPLE.into(), style: "Regular".into(), size: 17.0, enabled: true }
    }
}

/// Selected characters, otherwise the selected text object's text, otherwise a neutral sample.
pub(crate) fn selection_sample(app: &VectorcraftApp) -> FontSample {
    let options = FontSample {
        enabled: app.session.prefs.font_preview,
        size: match app.session.prefs.font_preview_size.as_str() {
            "small" => 13.0,
            "large" => 23.0,
            _ => 17.0,
        },
        ..Default::default()
    };
    let editing = crate::panels::character::text_editing(app);
    let Some(st) = app.session.active() else { return options };
    let node = editing
        .and_then(|(id, _, _)| st.doc.node(id))
        .or_else(|| st.selection.objects.iter().filter_map(|id| st.doc.node(*id)).find(|n| matches!(n.kind, NodeKind::Text(_))));
    let Some(NodeKind::Text(text)) = node.map(|n| &n.kind) else { return options };
    let plain = text.plain_text();
    let selected = editing.filter(|(_, a, b)| b > a).and_then(|(_, a, b)| plain.get(a..b)).unwrap_or(&plain);
    let style = editing.map_or_else(|| text.first_style(), |(_, a, b)| vectorcraft_text::edit::insertion_style(&text.runs, a, b));
    FontSample { text: sample(selected), style: style.font_style, ..options }
}

fn sample(text: &str) -> String {
    let text: String = text.chars().take(48).map(|c| if c.is_whitespace() || c.is_control() { ' ' } else { c }).collect();
    if text.trim().is_empty() { SAMPLE.into() } else { text }
}

/// White coverage mask, tinted at paint time so cached previews work in every brightness theme.
fn image(family: &str, sample: &str, style: &str, size: f32, scale: f32) -> Option<egui::ColorImage> {
    let db = FontDb::global();
    let style = CharStyle { font_family: family.into(), font_style: style.into(), size: f64::from(size), ..Default::default() };
    let text = TextObject::point(Point::ZERO, sample, style);
    let mut path = layout(db, &text).to_bezpath();
    let bounds = path.bounding_box();
    if !bounds.width().is_finite() || !bounds.height().is_finite() || bounds.height() <= 0.0 {
        return None;
    }
    let height = size + 9.0;
    let fit = ((height as f64 - 4.0) / bounds.height()).min(1.0);
    let scale = f64::from(scale);
    path.apply_affine(
        Affine::scale(scale)
            * Affine::translate((2.0, (height as f64 - bounds.height() * fit) * 0.5))
            * Affine::scale(fit)
            * Affine::translate((-bounds.x0, -bounds.y0)),
    );
    let (w, h) = ((SAMPLE_WIDTH as f64 * scale).ceil() as usize, (height as f64 * scale).ceil() as usize);
    let mask = crate::panels::glyphs::rasterize(&path, w, h);
    Some(egui::ColorImage::new([w, h], mask.into_iter().map(|a| Color32::from_rgba_premultiplied(a, a, a, a)).collect()))
}

fn texture(ui: &Ui, family: &str, sample: &FontSample) -> Option<TextureHandle> {
    let scale = ui.ctx().pixels_per_point().clamp(1.0, 3.0);
    let key = (family.to_string(), sample.style.clone(), sample.text.clone(), sample.size.to_bits(), scale.to_bits());
    let id = egui::Id::new("font-preview-textures");
    if let Some(cached) = ui.data_mut(|d| d.get_temp_mut_or_default::<Previews>(id).0.get(&key).cloned()) {
        return cached;
    }
    let texture =
        image(family, &key.2, &key.1, sample.size, scale).map(|image| ui.ctx().load_texture("font-preview", image, egui::TextureOptions::LINEAR));
    ui.data_mut(|d| {
        let cache = d.get_temp_mut_or_default::<Previews>(id);
        if cache.0.len() >= MAX_CACHED {
            cache.0.clear();
        }
        cache.0.insert(key, texture.clone());
    });
    texture
}

/// One selectable row: a readable name on the left and the sample in that font on the right.
pub(crate) fn row(ui: &mut Ui, family: &str, selected: bool, sample: &FontSample) -> Response {
    if !sample.enabled {
        return ui.add(egui::Button::selectable(selected, family));
    }
    let row_height = sample.size + 13.0;
    let (rect, response) = ui.allocate_exact_size(vec2(NAME_WIDTH + SAMPLE_WIDTH + 20.0, row_height), Sense::click());
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, ui.is_enabled(), selected, family));
    // Do not load fonts or build textures for rows outside the scroll area's viewport.
    if ui.is_rect_visible(rect) {
        let t = Tokens::get(ui.ctx());
        let color = if ui.is_enabled() { t.text } else { t.text_disabled };
        if response.hovered() || response.has_focus() || selected {
            ui.painter().rect_filled(rect, 2, if selected { t.row_selected } else { t.hover });
        }
        let name_rect = egui::Rect::from_min_size(rect.min + vec2(6.0, 0.0), vec2(NAME_WIDTH, row_height));
        let mut name = egui::text::LayoutJob::simple(family.into(), egui::FontId::proportional(12.5), color, NAME_WIDTH);
        name.wrap.max_rows = 1;
        let name = ui.painter().layout_job(name);
        let at = name_rect.left_center() - vec2(0.0, name.size().y * 0.5);
        ui.painter().with_clip_rect(ui.clip_rect().intersect(name_rect)).galley(at, name, color);
        if let Some(texture) = texture(ui, family, sample) {
            let at = rect.min + vec2(NAME_WIDTH + 14.0, 2.0);
            ui.painter().image(
                texture.id(),
                egui::Rect::from_min_size(at, vec2(SAMPLE_WIDTH, row_height - 4.0)),
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                color,
            );
        }
    }
    response.on_hover_text(family)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previews_use_the_actual_font_outlines() {
        let serif = image("Source Serif 4", "Holiday Blend", "Regular", 17.0, 1.0).unwrap();
        let mono = image("JetBrains Mono", "Holiday Blend", "Regular", 17.0, 1.0).unwrap();
        assert!(serif.pixels.iter().any(|p| p.a() > 0));
        assert!(mono.pixels.iter().any(|p| p.a() > 0));
        assert_ne!(serif.pixels, mono.pixels);
        assert_eq!(serif.size, [220, 26]);
        assert_eq!(image("Source Serif 4", "Holiday Blend", "Regular", 17.0, 2.0).unwrap().size, [440, 52]);
        assert_ne!(
            image("Source Sans 3", "Holiday Blend", "Regular", 17.0, 1.0).unwrap().pixels,
            image("Source Sans 3", "Holiday Blend", "Bold", 17.0, 1.0).unwrap().pixels
        );
    }

    #[test]
    fn samples_are_bounded_single_line_and_have_an_empty_text_fallback() {
        assert_eq!(sample("Hello\nworld\t!"), "Hello world !");
        assert_eq!(sample("Hello\u{2028}world"), "Hello world");
        assert_eq!(sample(" \n\t"), SAMPLE);
        assert_eq!(sample(&"日".repeat(100)).chars().count(), 48);
    }

    #[test]
    fn samples_follow_selected_text_and_character_ranges() {
        use serde_json::json;
        use vectorcraft_engine::ViewInfo;
        use vectorcraft_tools::{PointerEvent, PointerKind};

        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        assert_eq!(selection_sample(&app).text, SAMPLE);
        app.run("file.new", json!({})).unwrap();
        app.run("text.create", json!({"x": 100, "y": 100, "text": "Holiday Blend", "size": 20, "font": "Inter"})).unwrap();
        app.run("text.setStyle", json!({"style": "Bold"})).unwrap();
        let selected = selection_sample(&app);
        assert_eq!((selected.text.as_str(), selected.style.as_str()), ("Holiday Blend", "Bold"));
        app.run("tool.select", json!({"tool": "type"})).unwrap();
        for kind in [PointerKind::Down, PointerKind::Up] {
            app.session.pointer(&PointerEvent::new(kind, 101.0, 95.0), ViewInfo::default()).unwrap();
        }
        app.session.set_tool_option("select", &json!({"start": 8, "end": 13}));
        assert_eq!(selection_sample(&app).text, "Blend");
        app.session.set_tool_option("stopEditing", &json!(true));
        app.run("select.none", json!({})).unwrap();
        assert_eq!(selection_sample(&app).text, SAMPLE);
    }

    #[test]
    fn only_visible_rows_allocate_textures_and_repeated_frames_reuse_them() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let cache = egui::Id::new("font-preview-textures");
        let sample = FontSample::default();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.set_clip_rect(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(500.0, 50.0)));
            ui.add_space(100.0);
            row(ui, "Source Sans 3", false, &sample);
        })
        .textures_delta
        .clear();
        assert!(ctx.data(|d| d.get_temp::<Previews>(cache)).is_none());
        let mut first = None;
        for _ in 0..3 {
            ctx.run_ui(egui::RawInput::default(), |ui| {
                row(ui, "Source Sans 3", false, &sample);
            })
            .textures_delta
            .clear();
            let textures = ctx.data(|d| d.get_temp::<Previews>(cache)).unwrap();
            assert_eq!(textures.0.len(), 1);
            let id = textures.0.values().next().unwrap().as_ref().unwrap().id();
            assert_eq!(*first.get_or_insert(id), id);
        }
    }

    #[test]
    fn type_preferences_disable_previews_and_change_their_size() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.session.prefs.font_preview = false;
        app.session.prefs.font_preview_size = "large".into();
        let sample = selection_sample(&app);
        assert!(!sample.enabled);
        assert_eq!(sample.size, 23.0);
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        ctx.run_ui(egui::RawInput::default(), |ui| {
            row(ui, "Source Sans 3", false, &sample);
        })
        .textures_delta
        .clear();
        assert!(ctx.data(|d| d.get_temp::<Previews>(egui::Id::new("font-preview-textures"))).is_none());
        app.session.prefs.font_preview_size = "small".into();
        assert_eq!(selection_sample(&app).size, 13.0);
        assert_eq!(image("Source Sans 3", "Holiday", "Regular", 13.0, 1.0).unwrap().size, [220, 22]);
        assert_eq!(image("Source Sans 3", "Holiday", "Regular", 23.0, 1.0).unwrap().size, [220, 32]);
    }
}
