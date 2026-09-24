use crate::file_handler::FileHandler;
use egui::{
    text::{LayoutJob, TextFormat}, Align, Color32, Context, FontData, FontDefinitions, FontFamily,
    FontId, Id, Key, Layout, RichText, ScrollArea, TextEdit, TextStyle, Ui,
};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{mpsc::{channel, Receiver, Sender}, Arc};

type FileResult = Result<FileHandler, String>;

fn fmt_num(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

#[derive(Clone)]
struct SearchStage {
    id: usize,
    keyword: String,
    line_indices: Vec<usize>,
}

struct SearchWindowState {
    stage_index: usize,
    refine_query: String,
    open: bool,
}

pub struct BigTextViewer {
    file_handler: Option<Arc<FileHandler>>,
    status: String,
    search_query: String,
    search_results: Vec<usize>,
    search_result_set: HashSet<usize>,
    current_result: usize,
    scroll_to_line: Option<usize>,
    file_rx: Receiver<FileResult>,
    file_tx: Sender<FileResult>,
    search_rx: Receiver<Vec<usize>>,
    search_tx: Sender<Vec<usize>>,
    is_searching: bool,
    stages: Vec<SearchStage>,
    windows: Vec<SearchWindowState>,
    pending_keyword: String,
    nil_dialog: bool,
    about_open: bool,
}

impl BigTextViewer {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_traditional_chinese_font(&cc.egui_ctx);
        let (file_tx, file_rx) = channel::<FileResult>();
        let (search_tx, search_rx) = channel::<Vec<usize>>();
        Self {
            file_handler: None,
            status: "就緒 — 拖曳檔案或按 Ctrl+O".to_string(),
            search_query: String::new(),
            search_results: Vec::new(),
            search_result_set: HashSet::new(),
            current_result: 0,
            scroll_to_line: None,
            file_rx,
            file_tx,
            search_rx,
            search_tx,
            is_searching: false,
            stages: Vec::new(),
            windows: Vec::new(),
            pending_keyword: String::new(),
            nil_dialog: false,
            about_open: false,
        }
    }

    fn poll_background(&mut self, ctx: &Context) {
        while let Ok(result) = self.file_rx.try_recv() {
            match result {
                Ok(handler) => {
                    let size_mb = handler.file_size as f64 / 1_048_576.0;
                    self.status = format!(
                        "已開啟 {} | {} 行 | {:.1} MB | {}",
                        handler.file_name,
                        fmt_num(handler.total_lines),
                        size_mb,
                        handler.encoding_name
                    );
                    self.file_handler = Some(Arc::new(handler));
                    self.reset_results();
                }
                Err(error) => {
                    self.status = format!("開啟失敗：{}", error);
                    self.file_handler = None;
                }
            }
            ctx.request_repaint();
        }

        while let Ok(indices) = self.search_rx.try_recv() {
            self.is_searching = false;
            self.apply_stage(self.pending_keyword.clone(), indices, true);
            ctx.request_repaint();
        }
    }

    fn reset_results(&mut self) {
        self.search_results.clear();
        self.search_result_set.clear();
        self.current_result = 0;
        self.scroll_to_line = None;
        self.stages.clear();
        self.windows.clear();
        self.pending_keyword.clear();
        self.nil_dialog = false;
        self.is_searching = false;
    }

    fn clear_search(&mut self) {
        self.search_query.clear();
        self.reset_results();
        if let Some(handler) = &self.file_handler {
            self.status = format!(
                "已開啟 {} | {} 行 | {}",
                handler.file_name,
                fmt_num(handler.total_lines),
                handler.encoding_name
            );
        }
    }

    fn request_open_file(&mut self, path: PathBuf) {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "未知檔案".to_string());
        self.status = format!("正在建立 {} 的索引…", name);
        self.file_handler = None;
        self.reset_results();

        let tx = self.file_tx.clone();
        std::thread::spawn(move || {
            let result = FileHandler::open(&path).map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
    }

    fn show_open_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("文字檔", &["txt", "log", "md", "csv", "json"])
            .add_filter("所有檔案", &["*"])
            .pick_file()
        {
            self.request_open_file(path);
        }
    }

    fn start_search(&mut self) {
        let keyword = self.search_query.trim().to_string();
        if keyword.is_empty() || self.file_handler.is_none() || self.is_searching {
            return;
        }

        let handler = Arc::clone(self.file_handler.as_ref().unwrap());
        let total_lines = handler.total_lines;
        let keyword_for_thread = keyword.clone();
        let tx = self.search_tx.clone();

        self.reset_results();
        self.is_searching = true;
        self.pending_keyword = keyword.clone();
        self.status = format!("正在搜尋「{}」…", keyword);

        std::thread::spawn(move || {
            let needle = keyword_for_thread.to_lowercase();
            let results: Vec<usize> = (0..total_lines)
                .filter(|&index| handler.get_line(index).to_lowercase().contains(&needle))
                .collect();
            let _ = tx.send(results);
        });
    }

    fn refine_search_in_stage(&mut self, stage_index: usize, keyword: String) {
        let keyword = keyword.trim().to_string();
        if keyword.is_empty() || stage_index >= self.stages.len() {
            return;
        }

        let Some(handler) = &self.file_handler else {
            return;
        };
        let previous = self.stages[stage_index].line_indices.clone();
        let needle = keyword.to_lowercase();
        let indices: Vec<usize> = previous
            .into_iter()
            .filter(|&index| handler.get_line(index).to_lowercase().contains(&needle))
            .collect();
        self.apply_stage(keyword, indices, false);
    }

    fn apply_stage(&mut self, keyword: String, indices: Vec<usize>, first_search: bool) {
        self.search_results = indices.clone();
        self.search_result_set = indices.iter().copied().collect();
        self.current_result = 0;
        self.scroll_to_line = indices.first().copied();

        if indices.is_empty() {
            self.status = format!("Nil result — 找不到「{}」", keyword);
            self.nil_dialog = true;
            return;
        }

        let stage_id = self.stages.len() + 1;
        self.stages.push(SearchStage {
            id: stage_id,
            keyword: keyword.clone(),
            line_indices: indices,
        });
        self.windows.push(SearchWindowState {
            stage_index: self.stages.len() - 1,
            refine_query: String::new(),
            open: true,
        });
        self.status = if first_search {
            format!("找到 {} 個結果：{}", self.search_results.len(), keyword)
        } else {
            format!("第 {} 階找到 {} 個結果：{}", stage_id, self.search_results.len(), keyword)
        };
    }

    fn go_to_result(&mut self, delta: isize) {
        if self.search_results.is_empty() {
            return;
        }
        let len = self.search_results.len() as isize;
        self.current_result = (self.current_result as isize + delta).rem_euclid(len) as usize;
        self.scroll_to_line = Some(self.search_results[self.current_result]);
    }

    fn handle_shortcuts(&mut self, ctx: &Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.first().and_then(|f| f.path.clone()));
        if let Some(path) = dropped {
            self.request_open_file(path);
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::O)) {
            self.show_open_dialog();
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::F)) {
            ctx.memory_mut(|m| m.request_focus(Id::new("search_box")));
        }
        if ctx.input(|i| i.key_pressed(Key::F3)) {
            if ctx.input(|i| i.modifiers.shift) {
                self.go_to_result(-1);
            } else {
                self.go_to_result(1);
            }
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.clear_search();
        }
    }

    fn render_top_panel(&mut self, ctx: &Context) {
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("開啟檔案").clicked() {
                    self.show_open_dialog();
                }
                ui.separator();
                let response = ui.add(
                    TextEdit::singleline(&mut self.search_query)
                        .id(Id::new("search_box"))
                        .hint_text("輸入搜尋關鍵字…")
                        .desired_width(230.0),
                );
                if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    self.start_search();
                }
                if ui.add_enabled(!self.is_searching, egui::Button::new("搜尋")).clicked() {
                    self.start_search();
                }
                if ui.button("上一個").clicked() {
                    self.go_to_result(-1);
                }
                if ui.button("下一個").clicked() {
                    self.go_to_result(1);
                }
                if ui.button("清除").clicked() {
                    self.clear_search();
                }
                if ui.button("關於").clicked() {
                    self.about_open = true;
                }
                if !self.search_results.is_empty() {
                    ui.label(format!(
                        "{} / {}",
                        self.current_result + 1,
                        self.search_results.len()
                    ));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(&self.status).size(13.0));
                });
            });
        });
    }

    fn render_central_panel(&mut self, ctx: &Context) {
        let scroll_target = self.scroll_to_line.take();
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(handler) = &self.file_handler {
                render_text_viewer(
                    ui,
                    handler.as_ref(),
                    scroll_target,
                    &self.search_results,
                    self.current_result,
                    &self.search_result_set,
                    self.search_query.trim(),
                );
            } else {
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("Big Text Viewer V4");
                        ui.add_space(12.0);
                        ui.label("拖曳文字檔到此處，或按 Ctrl+O 開啟");
                        ui.label("支援 UTF-8、UTF-16、Big5/HKSCS 等編碼，檔案上限 4GB");
                    });
                });
            }
        });
    }

    fn render_result_windows(&mut self, ctx: &Context) {
        let initial_count = self.windows.len();
        let mut refine_requests: Vec<(usize, String)> = Vec::new();

        for window_index in 0..initial_count {
            let stage_index = self.windows[window_index].stage_index;
            if stage_index >= self.stages.len() {
                continue;
            }
            let stage = self.stages[stage_index].clone();
            let mut open = self.windows[window_index].open;
            let mut query = self.windows[window_index].refine_query.clone();

            egui::Window::new(format!("搜尋結果 #{}", stage.id))
                .open(&mut open)
                .default_size([850.0, 520.0])
                .show(ctx, |ui| {
                    let frame = egui::Frame::none()
                        .fill(Color32::from_rgb(250, 250, 245))
                        .inner_margin(10.0);
                    frame.show(ui, |ui| {
                        ui.visuals_mut().override_text_color = Some(Color32::BLACK);
                        ui.label(
                            RichText::new(format!(
                                "第 {} 階｜關鍵字：「{}」｜{} 行",
                                stage.id,
                                stage.keyword,
                                stage.line_indices.len()
                            ))
                            .color(Color32::BLACK)
                            .strong(),
                        );
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("在此結果中再搜尋：").color(Color32::BLACK));
                            let response = ui.add(
                                TextEdit::singleline(&mut query)
                                    .hint_text("下一階關鍵字…")
                                    .desired_width(230.0),
                            );
                            let enter = response.lost_focus()
                                && ui.input(|i| i.key_pressed(Key::Enter));
                            if ui.button("進一步篩選").clicked() || enter {
                                refine_requests.push((stage_index, query.clone()));
                            }
                        });
                        ui.separator();
                        ScrollArea::both().auto_shrink([false, false]).show_rows(
                            ui,
                            21.0,
                            stage.line_indices.len(),
                            |ui, range| {
                                if let Some(handler) = &self.file_handler {
                                    for result_index in range {
                                        let line_index = stage.line_indices[result_index];
                                        let text = handler.get_line(line_index);
                                        ui.label(
                                            RichText::new(format!("{:>8}: {}", line_index + 1, text))
                                                .font(FontId::new(14.0, FontFamily::Monospace))
                                                .color(Color32::BLACK)
                                                .background_color(Color32::from_rgb(245, 245, 232)),
                                        );
                                    }
                                }
                            },
                        );
                    });
                });

            self.windows[window_index].open = open;
            self.windows[window_index].refine_query = query;
        }

        for (stage_index, keyword) in refine_requests {
            self.refine_search_in_stage(stage_index, keyword);
        }
    }

    fn render_dialogs(&mut self, ctx: &Context) {
        if self.nil_dialog {
            let mut open = self.nil_dialog;
            egui::Window::new("Nil result")
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.heading("Nil result");
                    ui.label("找不到符合目前搜尋條件的文字。");
                    if ui.button("確定").clicked() {
                        self.nil_dialog = false;
                    }
                });
            self.nil_dialog = self.nil_dialog && open;
        }

        if self.about_open {
            let mut open = self.about_open;
            egui::Window::new("關於 Big Text Viewer")
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.heading("Big Text Viewer");
                    ui.label("產品版本：V4");
                    ui.label("著作權：ChunChun");
                    ui.label("最大檔案大小：4GB");
                });
            self.about_open = open;
        }
    }
}

impl eframe::App for BigTextViewer {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.poll_background(ctx);
        self.handle_shortcuts(ctx);
        self.render_top_panel(ctx);
        self.render_central_panel(ctx);
        self.render_result_windows(ctx);
        self.render_dialogs(ctx);
    }
}

fn setup_traditional_chinese_font(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "noto_tc".to_owned(),
        FontData::from_static(include_bytes!("../assets/NotoSansCJKtc-Regular.otf")),
    );
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "noto_tc".to_owned());
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, "noto_tc".to_owned());
    ctx.set_fonts(fonts);
}

fn render_text_viewer(
    ui: &mut Ui,
    handler: &FileHandler,
    scroll_to_line: Option<usize>,
    search_results: &[usize],
    current_result: usize,
    search_result_set: &HashSet<usize>,
    keyword: &str,
) {
    let row_height = 21.0;
    let digits = handler.total_lines.to_string().len();
    let line_num_width = (digits.max(4) as f32 * 9.0 + 18.0).clamp(58.0, 130.0);
    let mut scroll = ScrollArea::both().auto_shrink([false, false]);
    if let Some(target) = scroll_to_line {
        scroll = scroll.vertical_scroll_offset(target as f32 * row_height);
    }

    scroll.show_rows(ui, row_height, handler.total_lines, |ui, range| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width(line_num_width);
                for row in range.clone() {
                    let selected = search_results.get(current_result) == Some(&row);
                    let matched = search_result_set.contains(&row);
                    let color = if selected {
                        Color32::YELLOW
                    } else if matched {
                        Color32::from_rgb(255, 190, 80)
                    } else {
                        Color32::GRAY
                    };
                    ui.label(
                        RichText::new((row + 1).to_string())
                            .font(FontId::new(14.0, FontFamily::Monospace))
                            .color(color),
                    );
                }
            });
            ui.separator();
            ui.vertical(|ui| {
                ui.set_min_width(ui.available_width());
                for row in range {
                    let line = handler.get_line(row);
                    let selected = search_results.get(current_result) == Some(&row);
                    let matched = search_result_set.contains(&row);
                    if matched && !keyword.is_empty() {
                        ui.label(highlight_keyword(&line, keyword, selected));
                    } else {
                        ui.label(
                            RichText::new(line)
                                .font(FontId::new(14.0, FontFamily::Monospace)),
                        );
                    }
                }
            });
        });
    });
}

fn highlight_keyword(text: &str, keyword: &str, selected: bool) -> LayoutJob {
    let mut job = LayoutJob::default();
    let lower_text = text.to_lowercase();
    let lower_keyword = keyword.to_lowercase();
    let base = TextFormat {
        font_id: FontId::new(14.0, FontFamily::Monospace),
        color: Color32::WHITE,
        background: if selected {
            Color32::from_rgb(70, 70, 35)
        } else {
            Color32::TRANSPARENT
        },
        ..Default::default()
    };
    let hit = TextFormat {
        font_id: FontId::new(14.0, FontFamily::Monospace),
        color: Color32::BLACK,
        background: Color32::from_rgb(255, 235, 70),
        ..Default::default()
    };

    let mut cursor = 0;
    let exact_matches: Vec<(usize, &str)> = text.match_indices(keyword).collect();
    if !exact_matches.is_empty() {
        for (start, matched) in exact_matches {
            let end = start + matched.len();
            job.append(&text[cursor..start], 0.0, base.clone());
            job.append(&text[start..end], 0.0, hit.clone());
            cursor = end;
        }
        job.append(&text[cursor..], 0.0, base);
    } else if lower_text.contains(&lower_keyword) {
        job.append(text, 0.0, hit);
    } else {
        job.append(text, 0.0, base);
    }
    job
}
