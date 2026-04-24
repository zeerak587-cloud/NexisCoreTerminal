use eframe::{egui, NativeOptions};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const SPLASH_DURATION: Duration = Duration::from_secs(5);
const BACKDROP_FILE: &str = "OPEN.png";
const BACKDROP_FALLBACK_FILE: &str = "OPEN.png";
const WELCOME_SOUND_FILE: &str = "welcome.wav"

#[derive(Clone)]
struct Config {
    safe_mode: bool,
    confirm_delete: bool,
    symbol_for_root: String,
}

fn load_config() -> Config {
    let path = "config.lst";

    if !std::path::Path::new(path).exists() {
        let default = "safe_mode = true\nconfirm_delete = true\nsymbol_for_root = \"R\"";
        fs::write(path, default).ok();
    }

    let content = fs::read_to_string(path).unwrap_or_default();
    let mut map = HashMap::new();

    for line in content.lines() {
        let parts: Vec<&str> = line.split('=').collect();
        if parts.len() == 2 {
            map.insert(
                parts[0].trim().to_string(),
                parts[1].trim().trim_matches('"').to_string(),
            );
        }
    }

    Config {
        safe_mode: map.get("safe_mode").unwrap_or(&"true".to_string()) == "true",
        confirm_delete: map.get("confirm_delete").unwrap_or(&"true".to_string()) == "true",
        symbol_for_root: map
            .get("symbol_for_root")
            .unwrap_or(&"R".to_string())
            .to_string(),
    }
}

struct AppState {
    config: Config,
    env_dir: PathBuf,
    current_dir: PathBuf,
    entries: Vec<fs::DirEntry>,
    selected: Option<String>,
    file_content: String,
    new_name: String,
    new_file_content: String,
    message: String,
    fonts_loaded: bool,
    started_at: Instant,
    welcome_played: bool,
    splash_load_attempted: bool,
    splash_texture: Option<egui::TextureHandle>,
}

impl AppState {
    fn refresh_entries(&mut self) {
        self.entries.clear();
        if let Ok(read) = fs::read_dir(&self.current_dir) {
            for entry in read.flatten() {
                self.entries.push(entry);
            }
            // sort by name
            self.entries.sort_by_key(|e| e.file_name());
        }
    }

    fn load_splash_texture(&mut self, ctx: &egui::Context) {
        if self.splash_load_attempted {
            return;
        }

        self.splash_load_attempted = true;
        let image_path = find_asset(BACKDROP_FILE).or_else(|| find_asset(BACKDROP_FALLBACK_FILE));
        let image_bytes = image_path.as_ref().map(fs::read);

        let Some(Ok(image_bytes)) = image_bytes else {
            self.message = format!("Splash image missing: assets/{}", BACKDROP_FILE);
            return;
        };

        let Ok(image) = image::load_from_memory(&image_bytes) else {
            self.message = format!("Failed to load splash image: assets/{}", BACKDROP_FILE);
            return;
        };

        let image = image.to_rgba8();
        let size = [image.width() as usize, image.height() as usize];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
        self.splash_texture = Some(ctx.load_texture(
            "startup_backdrop",
            color_image,
            egui::TextureOptions::LINEAR,
        ));
    }

    fn show_splash(&mut self, ctx: &egui::Context) {
        self.load_splash_texture(ctx);
        ctx.request_repaint_after(Duration::from_millis(16));

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::BLACK))
            .show(ctx, |ui| {
                let available = ui.max_rect();

                if let Some(texture) = &self.splash_texture {
                    let texture_size = texture.size_vec2();
                    let scale = (available.width() / texture_size.x)
                        .max(available.height() / texture_size.y);
                    let image_size = texture_size * scale;
                    let image_rect = egui::Rect::from_center_size(available.center(), image_size);
                    let uv = egui::Rect::from_min_max(
                        egui::Pos2::new(0.0, 0.0),
                        egui::Pos2::new(1.0, 1.0),
                    );

                    ui.painter()
                        .image(texture.id(), image_rect, uv, egui::Color32::WHITE);
                }
            });
    }
}

impl eframe::App for AppState {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.welcome_played {
            play_welcome_sound();
            self.welcome_played = true;
        }

        if self.started_at.elapsed() < SPLASH_DURATION {
            self.show_splash(ctx);
            return;
        }

        // Load custom font once
        if !self.fonts_loaded {
            let mut fonts = egui::FontDefinitions::default();
            // Embed the font at compile time. Ensure assets/font.ttf exists.
            fonts.font_data.insert(
                "my_font".to_owned(),
                egui::FontData::from_static(include_bytes!("../assets/font.ttf")),
            );

            // Put it first in the Proportional family so default labels use it
            fonts
                .families
                .get_mut(&egui::FontFamily::Proportional)
                .unwrap()
                .insert(0, "my_font".to_owned());

            // Also register a named family so we can explicitly use it
            fonts
                .families
                .entry(egui::FontFamily::Name("MY_FONT_FAMILY".into()))
                .or_default()
                .push("my_font".to_owned());

            ctx.set_fonts(fonts);
            self.fonts_loaded = true;
        }

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("NEXISCORE TERMINAL GUI").heading());
                ui.separator();
                ui.label(format!("[ {} ]", self.config.symbol_for_root));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Exit").clicked() {
                        _frame.close();
                    }
                });
            });
        });

        egui::SidePanel::left("left_panel")
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("Folders");
                if ui.button("Refresh").clicked() {
                    self.refresh_entries();
                }
                ui.separator();

                // Show current path
                ui.label(self.current_dir.to_string_lossy().to_string());

                ui.separator();
                // List directories and files
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for entry in &self.entries {
                        let name = entry.file_name().to_string_lossy().to_string();
                        let is_dir = entry.path().is_dir();
                        let label = if is_dir {
                            format!("[DIR] {}", name)
                        } else {
                            name.clone()
                        };
                        if ui
                            .selectable_label(self.selected.as_deref() == Some(&name), label)
                            .clicked()
                        {
                            self.selected = Some(name.clone());
                            // load file content if file
                            if !is_dir {
                                match fs::read_to_string(entry.path()) {
                                    Ok(c) => self.file_content = c,
                                    Err(_) => {
                                        self.file_content = "[Failed to read file]".to_string()
                                    }
                                }
                            } else {
                                self.file_content.clear();
                            }
                        }
                    }
                });

                ui.separator();
                ui.label("New name:");
                ui.text_edit_singleline(&mut self.new_name);

                ui.horizontal(|ui| {
                    if ui.button("Make Directory").clicked() {
                        if !self.new_name.trim().is_empty() {
                            let path = self.current_dir.join(self.new_name.trim());
                            match fs::create_dir_all(&path) {
                                Ok(_) => {
                                    self.message = "[ OK ] Directory created".to_string();
                                    self.new_name.clear();
                                    self.refresh_entries();
                                }
                                Err(e) => self.message = format!("Failed: {}", e),
                            }
                        }
                    }

                    if ui.button("Write File").clicked() {
                        if !self.new_name.trim().is_empty() {
                            let path = self.current_dir.join(self.new_name.trim());
                            match fs::write(&path, &self.new_file_content) {
                                Ok(_) => {
                                    self.message = "[ OK ] File written".to_string();
                                    self.new_name.clear();
                                    self.new_file_content.clear();
                                    self.refresh_entries();
                                }
                                Err(e) => self.message = format!("Failed: {}", e),
                            }
                        }
                    }
                });

                ui.label("New file content:");
                ui.text_edit_multiline(&mut self.new_file_content);
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Details");

            ui.horizontal(|ui| {
                if ui.button("Open Directory").clicked() {
                    if let Some(sel) = &self.selected {
                        let path = self.current_dir.join(sel);
                        if path.exists() && path.is_dir() {
                            self.current_dir = path;
                            self.selected = None;
                            self.refresh_entries();
                        } else {
                            self.message = "Directory not found".to_string();
                        }
                    }
                }

                if ui.button("Back").clicked() {
                    if let Some(parent) = self.current_dir.parent() {
                        if parent.starts_with(&self.env_dir) {
                            self.current_dir = parent.to_path_buf();
                            self.selected = None;
                            self.refresh_entries();
                        } else {
                            self.message = "Already at environment root".to_string();
                        }
                    }
                }

                if ui.button("Open File").clicked() {
                    if let Some(sel) = &self.selected {
                        let path = self.current_dir.join(sel);
                        match fs::read_to_string(&path) {
                            Ok(c) => self.file_content = c,
                            Err(_) => self.file_content = "[Failed to open file]".to_string(),
                        }
                    }
                }

                if ui.button("Delete File").clicked() {
                    if let Some(sel) = &self.selected {
                        let path = self.current_dir.join(sel);
                        if !path.exists() {
                            self.message = "File not found".to_string();
                        } else {
                            if self.config.safe_mode || self.config.confirm_delete {
                                // simple confirmation dialog via message field
                                self.message =
                                    format!("Confirm delete {} by pressing Confirm Delete", sel);
                            } else {
                                match fs::remove_file(&path) {
                                    Ok(_) => {
                                        self.message = "[ OK ] File deleted".to_string();
                                        self.selected = None;
                                        self.refresh_entries();
                                    }
                                    Err(e) => self.message = format!("Failed: {}", e),
                                }
                            }
                        }
                    }
                }

                if ui.button("Delete Directory").clicked() {
                    if let Some(sel) = &self.selected {
                        let path = self.current_dir.join(sel);
                        if !path.exists() {
                            self.message = "Folder not found".to_string();
                        } else {
                            if self.config.safe_mode || self.config.confirm_delete {
                                self.message = format!(
                                    "Confirm delete folder {} by pressing Confirm Delete",
                                    sel
                                );
                            } else {
                                match fs::remove_dir_all(&path) {
                                    Ok(_) => {
                                        self.message = "[ OK ] Folder deleted".to_string();
                                        self.selected = None;
                                        self.refresh_entries();
                                    }
                                    Err(e) => self.message = format!("Failed: {}", e),
                                }
                            }
                        }
                    }
                }

                if ui.button("Confirm Delete").clicked() {
                    if let Some(sel) = &self.selected {
                        let path = self.current_dir.join(sel);
                        if path.is_dir() {
                            match fs::remove_dir_all(&path) {
                                Ok(_) => {
                                    self.message = "[ OK ] Folder deleted".to_string();
                                    self.selected = None;
                                    self.refresh_entries();
                                }
                                Err(e) => self.message = format!("Failed: {}", e),
                            }
                        } else {
                            match fs::remove_file(&path) {
                                Ok(_) => {
                                    self.message = "[ OK ] File deleted".to_string();
                                    self.selected = None;
                                    self.refresh_entries();
                                }
                                Err(e) => self.message = format!("Failed: {}", e),
                            }
                        }
                    }
                }
            });

            ui.separator();
            ui.label("File content / Preview:");
            ui.add(egui::TextEdit::multiline(&mut self.file_content).desired_rows(20));

            ui.separator();
            ui.label(egui::RichText::new(&self.message).strong());
        });

        // ensure entries are loaded at least once
        if self.entries.is_empty() {
            self.refresh_entries();
        }
    }
}

fn main() {
    // Initialize config and directories
    let config = load_config();

    let base_dir = std::env::current_dir().unwrap();
    let env_dir = base_dir.join("environment");
    fs::create_dir_all(&env_dir).ok();

    let mut app = AppState {
        config,
        env_dir: env_dir.clone(),
        current_dir: env_dir.clone(),
        entries: Vec::new(),
        selected: None,
        file_content: String::new(),
        new_name: String::new(),
        new_file_content: String::new(),
        message: String::from("[ OK ] Environment Ready"),
        fonts_loaded: false,
        started_at: Instant::now(),
        welcome_played: false,
        splash_load_attempted: false,
        splash_texture: None,
    };

    // initial refresh
    app.refresh_entries();

    let native_options = NativeOptions::default();
    eframe::run_native(
        "NEXISCORE GUI",
        native_options,
        Box::new(|_cc| Box::new(app)),
    )
    .expect("failed to start NEXISCORE GUI");
}

fn find_asset(file_name: &str) -> Option<PathBuf> {
    let current_dir_asset = std::env::current_dir()
        .ok()
        .map(|path| path.join("assets").join(file_name));

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(PathBuf::from));

    let exe_dir_asset = exe_dir
        .as_ref()
        .map(|path| path.join("assets").join(file_name));

    let project_dir_asset = exe_dir
        .as_ref()
        .and_then(|path| path.parent())
        .and_then(|path| path.parent())
        .map(|path| path.join("assets").join(file_name));

    [current_dir_asset, exe_dir_asset, project_dir_asset]
        .into_iter()
        .flatten()
        .find(|path| path.exists())
}

fn play_welcome_sound() {
    use rodio::{Decoder, OutputStream, Sink};
    use std::fs::File;
    use std::io::BufReader;

    let Some(path) = find_asset(WELCOME_SOUND_FILE) else {
        return;
    };

    let Ok((_stream, handle)) = OutputStream::try_default() else {
        return;
    };

    let Ok(file) = File::open(path) else {
        return;
    };

    let Ok(source) = Decoder::new(BufReader::new(file)) else {
        return;
    };

    let sink = Sink::try_new(&handle).unwrap();
    sink.append(source);
    sink.detach();
}
