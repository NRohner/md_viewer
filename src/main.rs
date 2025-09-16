use std::{fs, path::PathBuf, time::SystemTime};

use anyhow::Result;
use eframe::{egui, NativeOptions};
use egui::{SelectableLabel, Vec2};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use rfd::FileDialog;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1080.0, 720.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Markdown Viewer",
        native_options,
        Box::new(|cc| {
            // create and return your App wrapped in Ok(...)
            Ok(Box::new(App::new(cc)) as Box<dyn eframe::App>)
        }),
    )?;

    Ok(())
}

struct DocTab {
    title: String,
    path: PathBuf,
    content: String,
    last_read: SystemTime,
}

impl DocTab {
    fn from_path(path: PathBuf) -> Result<Self> {
        let content = fs::read_to_string(&path)?;
        let title = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled.md".to_string());
        Ok(Self {
            title,
            path,
            content,
            last_read: SystemTime::now(),
        })
    }
}

struct App {
    tabs: Vec<DocTab>,
    active: usize,
    cm_cache: CommonMarkCache,
    status: String,
    md_text_scale: f32,
}

impl App {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            tabs: Vec::new(),
            active: 0,
            cm_cache: CommonMarkCache::default(),
            status: "Ready".into(),
            md_text_scale: 1.0,
        }
    }

    fn open_files(&mut self) {
        if let Some(files) = FileDialog::new()
            .add_filter("Markdown", &["md", "markdown"])
            .set_title("Open Markdown file(s)")
            .pick_files()
        {
            for path in files {
                let is_md = path
                    .extension()
                    .map(|e| matches!(e.to_string_lossy().to_lowercase().as_str(), "md" | "markdown"))
                    .unwrap_or(false);

                if !is_md {
                    self.status = format!(
                        "Skipped non-markdown file: {}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    );
                    continue;
                }

                match DocTab::from_path(path) {
                    Ok(tab) => {
                        self.tabs.push(tab);
                        self.active = self.tabs.len().saturating_sub(1);
                        self.status = "Opened file".into();
                    }
                    Err(e) => {
                        self.status = format!("Failed to open: {e}");
                    }
                }
            }
        }
    }

    fn close_tab(&mut self, idx: usize) {
        if idx < self.tabs.len() {
            self.tabs.remove(idx);
            if self.active >= self.tabs.len() {
                self.active = self.tabs.len().saturating_sub(1);
            }
        }
    }

    fn reload_active(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            match fs::read_to_string(&tab.path) {
                Ok(new_content) => {
                    tab.content = new_content;
                    tab.last_read = SystemTime::now();
                    self.status = "Reloaded from disk".into();
                }
                Err(e) => {
                    self.status = format!("Reload failed: {e}");
                }
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply Font Scaling
        ctx.set_pixels_per_point(1.25);
        
        // Show full URLs on hover (suggested in egui_commonmark docs)
        ctx.style_mut(|s| s.url_in_tooltip = true);

        // Top menu
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open…").clicked() {
                        ui.close();
                        self.open_files();
                    }
                    if ui.button("Reload").clicked() {
                        ui.close();
                        self.reload_active();
                    }
                    if ui.button("Close Tab").clicked() {
                        ui.close();
                        let idx = self.active;
                        self.close_tab(idx);
                    }
                    if ui.button("Quit").clicked() {
                        ui.close();
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.separator();

                // Text size controls
                if ui.button("A–").clicked() {
                    self.md_text_scale = (self.md_text_scale * 0.9).max(0.5);
                }
                if ui.button("A+").clicked() {
                    self.md_text_scale = (self.md_text_scale * 1.1).min(3.0);
                }

                ui.separator();

                ui.menu_button("Help", |ui| {
                    ui.label("Markdown Viewer");
                    ui.label("View-only .md files with tabs and code highlighting.");
                });
            });
        });

        // Status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.label(&self.status);
        });

        // Tabs header
        egui::TopBottomPanel::top("tab_strip").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                for idx in 0..self.tabs.len() {
                    let selected = idx == self.active;
                    if ui
                        .add(SelectableLabel::new(selected, &self.tabs[idx].title))
                        .clicked()
                    {
                        self.active = idx;
                    }
                    ui.scope(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        if ui.button("×").on_hover_text("Close tab").clicked() {
                            self.close_tab(idx);
                        }
                    });
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("+ Open").clicked() {
                        self.open_files();
                    }
                });
            });
        });

        // Main viewer
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.tabs.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.heading("Welcome to Markdown Viewer");
                    ui.label("Use File → Open… or the + Open button to load one or more .md files.");
                });
                return;
            }

            let tab = &self.tabs[self.active];

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.scope(|ui| {
                        // Temporarily scale ONLY the markdown area's text styles
                        let style = ui.style_mut();
                        for font_id in style.text_styles.values_mut() {
                            font_id.size *= self.md_text_scale;
                        }

                        egui_commonmark::CommonMarkViewer::new()
                            .show(ui, &mut self.cm_cache, &tab.content);
                    });
                });

        });


    }
}
