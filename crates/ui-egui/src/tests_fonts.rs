//! The font menus list the installed fonts without asking for them (#36), and the font dropdown
//! searches them.

use serde_json::json;
use vectorcraft_engine::Session;
use vectorcraft_text::{FontDb, system_font_dirs};

use crate::tests_labels::shapes_text;
use crate::{VectorcraftApp, menus, widgets};

#[test]
fn type_font_menu_lists_every_available_family() {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({})).unwrap();
    let fonts = |app: &VectorcraftApp| -> Vec<String> {
        menus::menu_entries(app).into_iter().filter(|e| e.path == ["Type", "Font"]).map(|e| e.label).collect()
    };
    let labels = fonts(&app);
    assert_eq!(labels, *FontDb::global().menu_family_list());
    let installed = FontDb::with_font_dirs(system_font_dirs());
    installed.load_system_fonts();
    let missing: Vec<String> = installed.menu_family_list().iter().filter(|f| !labels.contains(f)).cloned().collect();
    assert!(missing.is_empty(), "installed but not in Type › Font: {missing:?}");
    // The menu is built every frame, from a list built once.
    assert_eq!(fonts(&app), labels);
}

#[test]
fn the_font_dropdown_searches_the_families_and_enter_picks_the_first_match() {
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    // Each frame: what it painted, where the dropdown is and what it picked.
    let frame = |events: Vec<egui::Event>| {
        let input =
            egui::RawInput { events, screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 800.0))), ..Default::default() };
        let (mut at, mut picked) = (egui::Pos2::ZERO, None);
        let mut out = ctx.run_ui(input, |ui| {
            at = ui.next_widget_position();
            picked = widgets::font_dropdown(ui, "test-font", "Inter", 220.0);
        });
        out.textures_delta.clear();
        (shapes_text(&out), at, picked)
    };
    let (closed, at, _) = frame(vec![]);
    assert_eq!(closed.trim(), "Inter");
    let click = at + egui::vec2(40.0, 10.0);
    let button = |pressed| egui::Event::PointerButton { pos: click, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    frame(vec![egui::Event::PointerMoved(click), button(true)]);
    frame(vec![button(false)]);
    // The popup sizes itself invisibly in its first frame, then the list scrolls (animated).
    for _ in 0..30 {
        frame(vec![]);
    }
    // The families painted in the list (the button shows the current one, Inter).
    let families = FontDb::global().families();
    let listed = |text: &str| -> Vec<String> { text.lines().skip(1).filter(|l| families.iter().any(|f| f == l)).map(str::to_string).collect() };
    let (open, ..) = frame(vec![]);
    assert!(listed(&open).len() >= families.len().min(5), "{open}");
    assert!(listed(&open).iter().any(|f| f == "Inter"), "the current font in view: {open}");
    // The search field has the focus: typing filters the list.
    frame(vec![egui::Event::Text("source serif".into())]);
    let (filtered, ..) = frame(vec![]);
    let shown = listed(&filtered);
    assert!(shown.iter().any(|f| f == "Source Serif 4"), "{filtered}");
    assert!(shown.iter().all(|f| f.to_lowercase().contains("source serif")), "{shown:?}");
    let enter = |pressed| egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed, repeat: false, modifiers: Default::default() };
    let (_, _, picked) = frame(vec![enter(true), enter(false)]);
    assert_eq!(picked.as_deref(), Some("Source Serif 4"));
    let (closed, ..) = frame(vec![]);
    assert_eq!(closed.trim(), "Inter", "the list closes");
}

#[test]
fn the_character_panel_menu_refreshes_the_font_list() {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({})).unwrap();
    // The installed fonts are always listed: the menu offers to look for new ones instead.
    let text = crate::tests_labels::painted_text(&mut app, crate::panels::character::menu);
    assert!(text.contains("Refresh Font List") && !text.contains("System Fonts"), "{text}");
}

#[test]
fn browsing_previews_keeps_the_document_unchanged_and_clicking_the_sample_applies_the_font() {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({})).unwrap();
    app.run("text.create", json!({"x": 10, "y": 40, "text": "Holiday Blend", "font": "Inter"})).unwrap();
    app.session.doc_mut().unwrap().mark_saved();
    let journal = app.session.journal.len();
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    let frame = |app: &mut VectorcraftApp, events| {
        let input =
            egui::RawInput { events, screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(700.0, 800.0))), ..Default::default() };
        let mut out = ctx.run_ui(input, |ui| {
            ui.set_width(260.0);
            crate::panels::character::show(app, ui);
        });
        out.textures_delta.clear();
        out
    };
    let button = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    let open = |app: &mut VectorcraftApp| {
        let pos = egui::pos2(45.0, 20.0);
        frame(app, vec![egui::Event::PointerMoved(pos), button(pos, true)]);
        frame(app, vec![button(pos, false)]);
        for _ in 0..30 {
            frame(app, vec![]);
        }
        frame(app, vec![egui::Event::Text("source serif".into())]);
        for _ in 0..30 {
            frame(app, vec![]);
        }
        frame(app, vec![])
    };
    frame(&mut app, vec![]);
    let out = open(&mut app);
    let sample_pos = out
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) if t.galley.text() == "Source Serif 4" => Some(t.pos + egui::vec2(220.0, 5.0)),
            _ => None,
        })
        .expect("the filtered font row is visible");
    assert!(
        out.shapes.iter().any(|s| matches!(&s.shape, egui::epaint::Shape::Mesh(m) if m.texture_id != egui::TextureId::default())),
        "preview image painted beside the name"
    );
    frame(&mut app, vec![egui::Event::PointerMoved(sample_pos)]);
    assert_eq!(crate::panels::character::text_style(&app).unwrap().0.font_family, "Inter");
    assert!(!app.session.doc().unwrap().is_dirty());
    assert_eq!(app.session.journal.len(), journal);
    let escape = egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() };
    frame(&mut app, vec![escape]);
    assert!(!egui::Popup::is_any_open(&ctx));
    assert!(!app.session.doc().unwrap().is_dirty());
    frame(
        &mut app,
        vec![egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: false, repeat: false, modifiers: Default::default() }],
    );
    let out = open(&mut app);
    let sample_pos = out
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) if t.galley.text() == "Source Serif 4" => Some(t.pos + egui::vec2(220.0, 5.0)),
            _ => None,
        })
        .expect("the reopened font row is visible");
    frame(&mut app, vec![egui::Event::PointerMoved(sample_pos), button(sample_pos, true)]);
    frame(&mut app, vec![button(sample_pos, false)]);
    assert_eq!(crate::panels::character::text_style(&app).unwrap().0.font_family, "Source Serif 4");
    assert_eq!(app.session.journal.len(), journal + 1);
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(crate::panels::character::text_style(&app).unwrap().0.font_family, "Inter");
    assert!(!app.session.doc().unwrap().is_dirty());
}
