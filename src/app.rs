use crate::math::{norm, Point, Recurrence, Root};
use crate::simulation::{orbit, Field};
use eframe::egui::{
    self, pos2, vec2, Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, Vec2,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const INK: Color32 = Color32::from_rgb(7, 12, 20);
const PANEL: Color32 = Color32::from_rgb(16, 23, 34);
const LINE: Color32 = Color32::from_rgb(38, 51, 66);
const MUTED: Color32 = Color32::from_rgb(132, 151, 169);
const TEXT: Color32 = Color32::from_rgb(226, 236, 243);
const CYAN: Color32 = Color32::from_rgb(83, 226, 220);
const PINK: Color32 = Color32::from_rgb(241, 115, 192);
const GOLD: Color32 = Color32::from_rgb(255, 201, 108);

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    recurrence: Recurrence,
    count: usize,
    trail: usize,
    speed: f64,
    spread: f64,
    brightness: f32,
    zoom: f32,
    yaw: f64,
    pitch: f64,
    initial: Point,
    orbit_steps: usize,
    show_orbit: bool,
    show_field: bool,
    grid: bool,
    glow: bool,
    palette: usize,
    auto_rotate: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            recurrence: Recurrence {
                matrix: None,
                order: 2,
                c: [1.36, -0.96, 0.0],
            },
            count: 750,
            trail: 32,
            speed: 9.0,
            spread: 4.0,
            brightness: 0.65,
            zoom: 1.0,
            yaw: -0.42,
            pitch: 0.38,
            initial: [2.0, 0.2, -1.0],
            orbit_steps: 64,
            show_orbit: false,
            show_field: true,
            grid: false,
            glow: true,
            palette: 0,
            auto_rotate: false,
        }
    }
}

impl Settings {
    fn validate(&self) -> Result<(), &'static str> {
        let r = self.recurrence;
        if ![2, 3].contains(&r.order)
            || !r.c.iter().all(|v| v.is_finite())
            || (r.matrix.is_none() && r.c.iter().any(|v| v.abs() > 8.0))
        {
            return Err("漸化式の次数・係数が範囲外です");
        }
        if let Some(matrix) = r.matrix {
            if r.order != 2
                || !matrix
                    .iter()
                    .flatten()
                    .all(|v| v.is_finite() && v.abs() <= 3.0)
            {
                return Err("連立漸化式の行列が範囲外です");
            }
        }
        if !(50..=1800).contains(&self.count)
            || !(4..=80).contains(&self.trail)
            || !(1..=300).contains(&self.orbit_steps)
            || self.palette > 2
        {
            return Err("粒子数・軌跡長・配色が範囲外です");
        }
        for (value, lo, hi) in [
            (self.speed, 0.2, 30.0),
            (self.spread, 0.5, 12.0),
            (self.brightness as f64, 0.1, 1.5),
            (self.zoom as f64, 0.2, 8.0),
            (self.yaw, -100.0, 100.0),
            (self.pitch, -1.5, 1.5),
        ] {
            if !value.is_finite() || value < lo || value > hi {
                return Err("表示設定が範囲外です");
            }
        }
        if !self
            .initial
            .iter()
            .all(|v| v.is_finite() && v.abs() <= 12.0)
        {
            return Err("初期値が範囲外です");
        }
        Ok(())
    }
}

const PRESETS: [(&str, &str, usize, [f64; 3]); 7] = [
    ("01", "渦巻き / Spiral", 2, [1.36, -0.96, 0.0]),
    ("02", "収束 / Sink", 2, [1.0, -0.61, 0.0]),
    ("03", "鞍点 / Saddle", 2, [1.0, 1.0, 0.0]),
    ("04", "周期 / Orbit", 2, [1.0, -1.0, 0.0]),
    ("05", "銀河 / Galaxy", 3, [1.88, -1.4996, 0.345744]),
    ("06", "三つの実根 / Real roots", 3, [1.8, -0.99, 0.162]),
    ("07", "放射 / Outward", 2, [2.08, -1.0816, 0.0]),
];

pub struct RecurApp {
    settings: Settings,
    field: Field,
    playing: bool,
    previous: Instant,
    fps: f32,
    pan: Vec2,
    preset: Option<usize>,
    notice: Option<(String, Instant)>,
    help: bool,
    fullscreen: bool,
    clean: bool,
    selected_root: Option<usize>,
    screenshot_pending: bool,
}

impl RecurApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = &cc.egui_ctx;
        ctx.set_theme(egui::Theme::Dark);
        let mut fonts = egui::FontDefinitions::default();
        // Use a locally installed font; no OS font is redistributed.
        for path in [
            "C:/Windows/Fonts/meiryo.ttc",
            "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        ] {
            if let Ok(bytes) = fs::read(path) {
                fonts
                    .font_data
                    .insert("japanese".into(), egui::FontData::from_owned(bytes).into());
                fonts
                    .families
                    .get_mut(&egui::FontFamily::Proportional)
                    .unwrap()
                    .push("japanese".into());
                fonts
                    .families
                    .get_mut(&egui::FontFamily::Monospace)
                    .unwrap()
                    .push("japanese".into());
                break;
            }
        }
        ctx.set_fonts(fonts);
        let mut style = (*ctx.style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.panel_fill = PANEL;
        style.visuals.window_fill = PANEL;
        style.visuals.extreme_bg_color = INK;
        style.visuals.override_text_color = Some(TEXT);
        style.visuals.selection.bg_fill = Color32::from_rgb(31, 88, 89);
        style.visuals.selection.stroke = Stroke::new(1.0, CYAN);
        style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(27, 39, 53);
        style.visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(27, 39, 53);
        style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, LINE);
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(39, 65, 74);
        style.visuals.widgets.active.bg_fill = Color32::from_rgb(39, 105, 104);
        style.spacing.item_spacing = vec2(10.0, 9.0);
        style.spacing.button_padding = vec2(12.0, 7.0);
        style.spacing.slider_width = 159.0;
        style
            .text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(13.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(13.0));
        style
            .text_styles
            .insert(egui::TextStyle::Small, FontId::proportional(11.0));
        ctx.set_style(style);
        let settings = Settings::default();
        let mut field = Field::default();
        field.reset(
            settings.recurrence,
            settings.count,
            settings.spread,
            settings.trail,
        );
        Self {
            settings,
            field,
            playing: true,
            previous: Instant::now(),
            fps: 60.0,
            pan: Vec2::ZERO,
            preset: Some(0),
            notice: None,
            help: false,
            fullscreen: false,
            clean: false,
            selected_root: None,
            screenshot_pending: false,
        }
    }

    fn reset_field(&mut self) {
        let s = &self.settings;
        self.field.reset(s.recurrence, s.count, s.spread, s.trail);
    }

    fn notify(&mut self, text: String) {
        self.notice = Some((text, Instant::now()));
    }

    fn apply_preset(&mut self, index: usize) {
        let (_, _, order, c) = PRESETS[index];
        self.settings.recurrence = if index == 6 {
            Recurrence::linear([[1.08, 0.0], [0.0, 1.08]])
        } else {
            Recurrence {
                order,
                c,
                matrix: None,
            }
        };
        self.settings.zoom = 1.0;
        self.settings.yaw = -0.42;
        self.settings.pitch = 0.38;
        self.settings.palette = if index >= 4 { 1 } else { 0 };
        self.pan = Vec2::ZERO;
        self.preset = Some(index);
        self.reset_field();
    }

    fn header(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("header")
            .exact_height(70.0)
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(egui::Margin::symmetric(24, 16)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("R / ").size(25.0).strong().color(CYAN));
                    ui.label(RichText::new("RECUR").size(24.0).strong());
                    ui.add_space(14.0);
                    ui.label(RichText::new("漸化式ラボ").size(14.0).color(MUTED));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("使い方  ?").clicked() {
                            self.help = true;
                        }
                        if ui.button("全画面  F11").clicked() {
                            self.toggle_fullscreen(ctx);
                        }
                        if ui
                            .button("PNG 保存")
                            .on_hover_text("画面全体を実行ファイル横の output フォルダーへ保存")
                            .clicked()
                        {
                            self.screenshot_pending = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(
                                egui::UserData::default(),
                            ));
                        }
                    });
                });
            });
    }

    fn sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("controls")
            .exact_width(304.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(PANEL).inner_margin(20))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        section(ui, "01", "漸化式");
                        let before = self.settings.recurrence;
                        ui.horizontal(|ui| {
                            if ui
                                .selectable_label(
                                    before.order == 2 && before.matrix.is_none(),
                                    "三項",
                                )
                                .clicked()
                            {
                                self.settings.recurrence.order = 2;
                                self.settings.recurrence.matrix = None;
                            }
                            if ui
                                .selectable_label(before.order == 3, "四項 / 3D")
                                .clicked()
                            {
                                self.settings.recurrence.order = 3;
                                self.settings.recurrence.matrix = None;
                            }
                            if ui
                                .selectable_label(before.matrix.is_some(), "連立")
                                .clicked()
                            {
                                self.settings.recurrence =
                                    Recurrence::linear([[1.08, 0.0], [0.0, 1.08]]);
                            }
                        });
                        ui.add_space(4.0);
                        let preset_name = self.preset.map(|i| PRESETS[i].1).unwrap_or("カスタム");
                        let mut chosen = None;
                        egui::ComboBox::from_id_salt("presets")
                            .selected_text(preset_name)
                            .width(250.0)
                            .show_ui(ui, |ui| {
                                for (i, (number, name, _, _)) in PRESETS.iter().enumerate() {
                                    if ui
                                        .selectable_label(
                                            self.preset == Some(i),
                                            format!("{number}   {name}"),
                                        )
                                        .clicked()
                                    {
                                        chosen = Some(i);
                                    }
                                }
                            });
                        if let Some(i) = chosen {
                            self.apply_preset(i);
                        }
                        ui.add_space(9.0);
                        if let Some(mut matrix) = self.settings.recurrence.matrix {
                            for (i, row) in matrix.iter_mut().enumerate() {
                                for (j, value) in row.iter_mut().enumerate() {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(format!("m{}{}", i + 1, j + 1))
                                                .monospace()
                                                .color(CYAN),
                                        );
                                        ui.add(
                                            egui::Slider::new(value, -3.0..=3.0).fixed_decimals(3),
                                        );
                                    });
                                }
                            }
                            self.settings.recurrence = Recurrence::linear(matrix);
                        } else {
                            for i in 0..self.settings.recurrence.order {
                                let name = ["p", "q", "r"][i];
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(name)
                                            .monospace()
                                            .size(17.0)
                                            .color([CYAN, PINK, GOLD][i]),
                                    );
                                    ui.add(
                                        egui::Slider::new(
                                            &mut self.settings.recurrence.c[i],
                                            -3.0..=3.0,
                                        )
                                        .clamping(egui::SliderClamping::Never)
                                        .fixed_decimals(3),
                                    );
                                });
                            }
                            // A type-in value may extend the visible slider range up to the validated bound.
                            for c in &mut self.settings.recurrence.c {
                                if !c.is_finite() {
                                    *c = 0.0;
                                }
                                *c = c.clamp(-8.0, 8.0);
                            }
                        }
                        if before != self.settings.recurrence && chosen.is_none() {
                            self.preset = None;
                            self.reset_field();
                        }
                        ui.label(
                            RichText::new("数値をクリックして直接入力")
                                .small()
                                .color(MUTED),
                        );
                        ui.add_space(11.0);
                        ui.separator();
                        section(ui, "02", "粒子と軌跡");
                        let mut rebuild = false;
                        rebuild |= slider_row(
                            ui,
                            "粒子数",
                            egui::Slider::new(&mut self.settings.count, 50..=1800),
                        )
                        .changed();
                        rebuild |= slider_row(
                            ui,
                            "軌跡の長さ",
                            egui::Slider::new(&mut self.settings.trail, 4..=80).suffix(" 項"),
                        )
                        .changed();
                        slider_row(
                            ui,
                            "速度",
                            egui::Slider::new(&mut self.settings.speed, 0.2..=30.0)
                                .fixed_decimals(1)
                                .suffix(" 項/秒"),
                        );
                        rebuild |= slider_row(
                            ui,
                            "初期分布 ±",
                            egui::Slider::new(&mut self.settings.spread, 0.5..=12.0)
                                .fixed_decimals(1),
                        )
                        .changed();
                        slider_row(
                            ui,
                            "明るさ",
                            egui::Slider::new(&mut self.settings.brightness, 0.1..=1.5)
                                .fixed_decimals(2),
                        );
                        if rebuild {
                            self.reset_field();
                        }
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("配色").color(MUTED));
                            for (i, name) in ["海", "光", "虹"].iter().enumerate() {
                                ui.selectable_value(&mut self.settings.palette, i, *name);
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut self.settings.glow, "発光");
                            ui.checkbox(&mut self.settings.grid, "座標軸");
                            ui.checkbox(&mut self.settings.show_field, "粒子");
                        });
                        ui.add_space(8.0);
                        ui.separator();
                        section(ui, "03", "初期値から追跡");
                        ui.checkbox(
                            &mut self.settings.show_orbit,
                            RichText::new("1本の軌道を強調表示").color(GOLD),
                        );
                        if self.settings.show_orbit {
                            ui.horizontal(|ui| {
                                for i in 0..self.settings.recurrence.order {
                                    ui.vertical(|ui| {
                                        ui.label(
                                            RichText::new(
                                                if self.settings.recurrence.matrix.is_some() {
                                                    ["x0", "y0", "z0"][i].into()
                                                } else {
                                                    format!("a{i}")
                                                },
                                            )
                                            .small()
                                            .color(MUTED),
                                        );
                                        ui.add(
                                            egui::DragValue::new(&mut self.settings.initial[i])
                                                .range(-12.0..=12.0)
                                                .speed(0.02)
                                                .fixed_decimals(2),
                                        );
                                    });
                                }
                            });
                            slider_row(
                                ui,
                                "反復回数",
                                egui::Slider::new(&mut self.settings.orbit_steps, 1..=300),
                            );
                            ui.label(
                                RichText::new("金色の点は整数ステップの値です")
                                    .small()
                                    .color(MUTED),
                            );
                        }
                        ui.add_space(8.0);
                        ui.separator();
                        section(ui, "04", "ビュー");
                        slider_row(
                            ui,
                            "ズーム",
                            egui::Slider::new(&mut self.settings.zoom, 0.2..=8.0)
                                .logarithmic(true)
                                .fixed_decimals(2),
                        );
                        if self.settings.recurrence.order == 3 {
                            ui.checkbox(&mut self.settings.auto_rotate, "カメラを自動回転");
                        }
                        if ui.button("視点をリセット").clicked() {
                            self.pan = Vec2::ZERO;
                            self.settings.zoom = 1.0;
                            self.settings.yaw = -0.42;
                            self.settings.pitch = 0.38;
                        }
                        ui.horizontal(|ui| {
                            if ui.button("設定を保存").clicked() {
                                self.save_settings();
                            }
                            if ui.button("読み込み").clicked() {
                                self.load_settings();
                            }
                        });
                        if ui.button("軌道データを書き出す  CSV").clicked() {
                            self.export_csv();
                        }
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new("LOCAL / RUST / OFFLINE")
                                .monospace()
                                .size(10.0)
                                .color(MUTED),
                        );
                    });
            });
    }

    fn footer(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("footer")
            .exact_height(58.0)
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(egui::Margin::symmetric(22, 10)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let label = if self.playing {
                        "一時停止"
                    } else {
                        "再生"
                    };
                    if ui
                        .add(
                            egui::Button::new(RichText::new(label).color(INK).strong())
                                .fill(CYAN)
                                .min_size(vec2(100.0, 32.0)),
                        )
                        .clicked()
                    {
                        self.playing = !self.playing;
                    }
                    if ui
                        .button("1ステップ")
                        .on_hover_text("右矢印キーでも操作できます")
                        .clicked()
                    {
                        self.single_step();
                    }
                    if ui.button("リスタート").clicked() {
                        self.reset_field();
                    }
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new(format!("n = {:05}", self.field.steps))
                            .monospace()
                            .color(MUTED),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{:.0} FPS", self.fps))
                                .monospace()
                                .color(MUTED),
                        );
                        ui.label(
                            RichText::new(if self.settings.recurrence.order == 3 {
                                "ドラッグ: 回転  /  Shift: 移動  /  ホイール: 拡大"
                            } else {
                                "ドラッグ: 移動  /  ホイール: 拡大"
                            })
                            .small()
                            .color(MUTED),
                        );
                    });
                });
            });
    }

    fn single_step(&mut self) {
        self.playing = false;
        self.field.phase = 0.0;
        let s = &self.settings;
        self.field
            .advance(s.recurrence, 0.1, 10.0, s.spread, s.trail);
    }

    fn toggle_fullscreen(&mut self, ctx: &egui::Context) {
        self.fullscreen = !self.fullscreen;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
    }

    fn project(&self, point: Point, rect: Rect) -> Pos2 {
        let [mut x, mut y, z] = point;
        if self.settings.recurrence.order == 3 {
            let (sy, cy) = self.settings.yaw.sin_cos();
            let (sp, cp) = self.settings.pitch.sin_cos();
            let xx = x * cy + z * sy;
            let zz = -x * sy + z * cy;
            x = xx;
            y = y * cp - zz * sp;
        }
        let scale = rect.height().min(rect.width()) as f64 / (self.settings.spread * 2.9)
            * self.settings.zoom as f64;
        rect.center() + self.pan + vec2((x * scale) as f32, (-y * scale) as f32)
    }

    fn canvas(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(INK))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let response = ui.allocate_rect(rect, Sense::click_and_drag());
                // Keep the complex-plane overlay out of camera gestures.
                let root_rect = Rect::from_min_size(
                    pos2(rect.right() - 225.0, rect.top() + 24.0),
                    vec2(201.0, 254.0),
                );
                let over_roots = ctx
                    .input(|i| i.pointer.hover_pos().is_some_and(|p| root_rect.contains(p)))
                    && !self.clean;
                if response.dragged() && !over_roots {
                    let delta = ctx.input(|i| i.pointer.delta());
                    if self.settings.recurrence.order == 3 && !ctx.input(|i| i.modifiers.shift) {
                        self.settings.yaw =
                            (self.settings.yaw + delta.x as f64 * 0.008) % std::f64::consts::TAU;
                        self.settings.pitch =
                            (self.settings.pitch + delta.y as f64 * 0.008).clamp(-1.5, 1.5);
                    } else {
                        self.pan += delta;
                    }
                }
                if response.hovered() && !over_roots {
                    let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
                    self.settings.zoom =
                        (self.settings.zoom * (scroll * 0.002).exp()).clamp(0.2, 8.0);
                }
                if response.double_clicked() && !over_roots {
                    self.pan = Vec2::ZERO;
                    self.settings.zoom = 1.0;
                }
                let painter = ui.painter().with_clip_rect(rect);
                if self.settings.grid {
                    self.draw_grid(&painter, rect);
                }
                if self.settings.show_field {
                    self.draw_field(&painter, rect);
                }
                if self.settings.show_orbit {
                    self.draw_orbit(&painter, rect);
                }
                if !self.clean {
                    let title = if self.settings.recurrence.order == 2 {
                        "PHASE SPACE  /  2D"
                    } else {
                        "PHASE SPACE  /  3D"
                    };
                    painter.text(
                        rect.left_top() + vec2(26.0, 27.0),
                        Align2::LEFT_TOP,
                        title,
                        FontId::monospace(12.0),
                        CYAN,
                    );
                    painter.text(
                        rect.left_top() + vec2(26.0, 52.0),
                        Align2::LEFT_TOP,
                        if self.settings.recurrence.matrix.is_some() {
                            "(x[n], y[n])"
                        } else if self.settings.recurrence.order == 2 {
                            "(a[n], a[n+1])"
                        } else {
                            "(a[n], a[n+1], a[n+2])"
                        },
                        FontId::proportional(15.0),
                        MUTED,
                    );
                    self.draw_equation(&painter, rect);
                    self.root_panel(ui, root_rect);
                } else {
                    painter.text(
                        rect.left_bottom() + vec2(20.0, -20.0),
                        Align2::LEFT_BOTTOM,
                        "H : コントロールを表示",
                        FontId::proportional(12.0),
                        MUTED,
                    );
                }
                if let Some((message, since)) = &self.notice {
                    if since.elapsed() < Duration::from_secs(8) {
                        let r = Rect::from_center_size(
                            pos2(rect.center().x, rect.top() + 35.0),
                            vec2((rect.width() - 50.0).min(650.0), 42.0),
                        );
                        painter.rect_filled(r, 8.0, PANEL);
                        painter.rect_stroke(
                            r,
                            8.0,
                            Stroke::new(1.0, CYAN),
                            egui::StrokeKind::Inside,
                        );
                        painter.text(
                            r.center(),
                            Align2::CENTER_CENTER,
                            message,
                            FontId::proportional(12.0),
                            TEXT,
                        );
                    }
                }
            });
    }

    fn draw_grid(&self, painter: &egui::Painter, rect: Rect) {
        let span = self.settings.spread * 2.0;
        for i in -10..=10 {
            let k = i as f64 * self.settings.spread / 4.0;
            let stroke = Stroke::new(
                if i == 0 { 1.0 } else { 0.5 },
                if i == 0 {
                    LINE
                } else {
                    Color32::from_rgb(17, 28, 40)
                },
            );
            draw_segment(
                painter,
                self.project([-span, k, 0.0], rect),
                self.project([span, k, 0.0], rect),
                stroke,
            );
            draw_segment(
                painter,
                self.project([k, -span, 0.0], rect),
                self.project([k, span, 0.0], rect),
                stroke,
            );
        }
        let system = self.settings.recurrence.matrix.is_some();
        for (p, label) in [
            (
                [span * 0.65, 0.0, 0.0],
                if system { "x[n]" } else { "a[n]" },
            ),
            (
                [0.0, span * 0.65, 0.0],
                if system { "y[n]" } else { "a[n+1]" },
            ),
        ] {
            painter.text(
                self.project(p, rect),
                Align2::LEFT_BOTTOM,
                label,
                FontId::proportional(12.0),
                MUTED,
            );
        }
        if self.settings.recurrence.order == 3 {
            draw_segment(
                painter,
                self.project([0.0, 0.0, -span], rect),
                self.project([0.0, 0.0, span], rect),
                Stroke::new(1.0, LINE),
            );
            painter.text(
                self.project([0.0, 0.0, span * 0.65], rect),
                Align2::LEFT_BOTTOM,
                "a[n+2]",
                FontId::proportional(12.0),
                MUTED,
            );
        }
    }

    fn draw_field(&self, painter: &egui::Painter, rect: Rect) {
        // Mesh batches keep thousands of translucent segments inexpensive.
        let mut mesh = egui::Mesh::default();
        let mut halos = egui::Mesh::default();
        let rec = self.settings.recurrence;
        for particle in &self.field.particles {
            let len = particle.history.len();
            let color = palette(self.settings.palette, particle.hue);
            let alpha_base = self.settings.brightness * 0.48;
            let mut last = None;
            for (i, point) in particle.history.iter().enumerate() {
                let pos = self.project(*point, rect);
                if let Some(prev) = last {
                    let fade = (i as f32 / len as f32).powf(1.5);
                    let alpha = alpha_base * (0.10 + fade * 0.90);
                    mesh_segment(&mut mesh, prev, pos, 0.72, tint(color, alpha), rect);
                    if self.settings.glow && i + 7 >= len {
                        mesh_segment(&mut halos, prev, pos, 3.3, tint(color, alpha * 0.10), rect);
                    }
                }
                last = Some(pos);
            }
            if let Some(prev) = last {
                let point = self.field.head(particle, rec);
                if norm(point) < self.settings.spread * 40.0 {
                    let head = self.project(point, rect);
                    mesh_segment(
                        &mut mesh,
                        prev,
                        head,
                        1.0,
                        tint(color, alpha_base * 1.4),
                        rect,
                    );
                    if rect.contains(head) {
                        mesh.add_colored_rect(
                            Rect::from_center_size(head, vec2(1.5, 1.5)),
                            tint(color, alpha_base * 1.5),
                        );
                    }
                }
            }
        }
        painter.add(egui::Shape::mesh(halos));
        painter.add(egui::Shape::mesh(mesh));
    }

    fn draw_orbit(&self, painter: &egui::Painter, rect: Rect) {
        let points = orbit(
            self.settings.recurrence,
            self.settings.initial,
            self.settings.orbit_steps,
        );
        for pair in points.windows(2) {
            draw_segment(
                painter,
                self.project(pair[0], rect),
                self.project(pair[1], rect),
                Stroke::new(1.8, GOLD.gamma_multiply(0.8)),
            );
        }
        for (i, p) in points.iter().enumerate() {
            let point = self.project(*p, rect);
            if rect.contains(point) {
                painter.circle_filled(point, if i == 0 { 5.0 } else { 2.1 }, GOLD);
                if i == 0 {
                    painter.text(
                        point + vec2(10.0, -10.0),
                        Align2::LEFT_BOTTOM,
                        "n = 0",
                        FontId::monospace(12.0),
                        GOLD,
                    );
                }
            }
        }
    }

    fn draw_equation(&self, painter: &egui::Painter, rect: Rect) {
        let r = Rect::from_min_max(
            pos2(rect.left() + 24.0, rect.bottom() - 119.0),
            pos2(rect.right() - 24.0, rect.bottom() - 22.0),
        );
        painter.rect_filled(r, 10.0, Color32::from_rgba_unmultiplied(15, 24, 35, 236));
        painter.rect_stroke(r, 10.0, Stroke::new(1.0, LINE), egui::StrokeKind::Inside);
        let rec = self.settings.recurrence;
        painter.text(
            r.left_top() + vec2(18.0, 13.0),
            Align2::LEFT_TOP,
            "RECURRENCE",
            FontId::monospace(10.0),
            MUTED,
        );
        let [p, q, s] = rec.c;
        let equation = if let Some([[a, b], [c, d]]) = rec.matrix {
            format!("x[n+1] = {a:.3} x[n] {b:+.3} y[n]   /   y[n+1] = {c:.3} x[n] {d:+.3} y[n]")
        } else if rec.order == 2 {
            format!("a[n+2] = {p:.3} a[n+1] {q:+.3} a[n]")
        } else {
            format!("a[n+3] = {p:.3} a[n+2] {q:+.3} a[n+1] {s:+.3} a[n]")
        };
        let size = if rec.matrix.is_some() {
            if r.width() < 760.0 {
                13.0
            } else {
                19.0
            }
        } else if r.width() < 700.0 {
            18.0
        } else {
            25.0
        };
        painter.text(
            r.left_top() + vec2(18.0, 35.0),
            Align2::LEFT_TOP,
            equation,
            FontId::proportional(size),
            TEXT,
        );
        painter.text(
            r.left_bottom() + vec2(18.0, -10.0),
            Align2::LEFT_BOTTOM,
            "点は離散値、線はその補間  ·  粒子は画面外・原点付近・寿命で再配置",
            FontId::proportional(10.0),
            MUTED,
        );
    }

    fn root_panel(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let painter = ui.painter().with_clip_rect(rect);
        painter.rect_filled(rect, 10.0, Color32::from_rgba_unmultiplied(14, 22, 32, 242));
        painter.rect_stroke(rect, 10.0, Stroke::new(1.0, LINE), egui::StrokeKind::Inside);
        painter.text(
            rect.left_top() + vec2(14.0, 13.0),
            Align2::LEFT_TOP,
            "特性根 / COMPLEX PLANE",
            FontId::proportional(11.0),
            MUTED,
        );
        let plot = Rect::from_min_size(rect.left_top() + vec2(18.0, 39.0), vec2(165.0, 165.0));
        let center = plot.center();
        let roots = self.settings.recurrence.roots();
        let extent = roots.iter().map(|r| r.abs()).fold(1.55, f64::max).max(1.55) as f32;
        let scale = 76.0 / extent;
        painter.line_segment(
            [pos2(plot.left(), center.y), pos2(plot.right(), center.y)],
            Stroke::new(0.8, LINE),
        );
        painter.line_segment(
            [pos2(center.x, plot.top()), pos2(center.x, plot.bottom())],
            Stroke::new(0.8, LINE),
        );
        for i in 0..40 {
            let a = i as f32 / 40.0 * std::f32::consts::TAU;
            let b = a + 0.085;
            painter.line_segment(
                [
                    center + vec2(a.cos(), a.sin()) * scale,
                    center + vec2(b.cos(), b.sin()) * scale,
                ],
                Stroke::new(0.8, MUTED.gamma_multiply(0.6)),
            );
        }
        painter.text(
            plot.right_center() + vec2(-1.0, 5.0),
            Align2::RIGHT_TOP,
            "Re",
            FontId::monospace(9.0),
            MUTED,
        );
        painter.text(
            plot.center_top(),
            Align2::CENTER_TOP,
            "Im",
            FontId::monospace(9.0),
            MUTED,
        );
        painter.text(
            center + vec2(scale, 5.0),
            Align2::CENTER_TOP,
            "1",
            FontId::monospace(9.0),
            MUTED,
        );
        for (i, root) in roots.iter().enumerate() {
            let p = center + vec2(root.re as f32, -root.im as f32) * scale;
            let color = [CYAN, PINK, GOLD][i];
            painter.circle_filled(p, 10.0, color.gamma_multiply(0.10));
            painter.circle_filled(p, 4.6, color);
        }
        let system = self.settings.recurrence.matrix.is_some();
        let response = ui.interact(
            plot,
            ui.id().with("root_drag"),
            if system {
                Sense::hover()
            } else {
                Sense::click_and_drag()
            },
        );
        if response.drag_started() || response.clicked() {
            if let Some(pointer) = response.interact_pointer_pos() {
                self.selected_root = roots
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        (
                            i,
                            (center + vec2(r.re as f32, -r.im as f32) * scale).distance(pointer),
                        )
                    })
                    .filter(|(_, d)| *d < 20.0)
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|(i, _)| i);
            }
        }
        if response.dragged() {
            if let (Some(index), Some(pointer)) =
                (self.selected_root, response.interact_pointer_pos())
            {
                let mut updated = roots.clone();
                let re = ((pointer.x - center.x) / scale).clamp(-1.5, 1.5) as f64;
                let im = ((center.y - pointer.y) / scale).clamp(-1.5, 1.5) as f64;
                let old = roots[index];
                let pair = if old.im.abs() > 1e-7 {
                    roots
                        .iter()
                        .enumerate()
                        .find(|(j, r)| {
                            *j != index
                                && (r.re - old.re).abs() < 1e-6
                                && (r.im + old.im).abs() < 1e-6
                        })
                        .map(|(j, _)| j)
                } else if roots.iter().all(|r| r.im.abs() < 1e-7) && im.abs() > 0.06 {
                    (0..roots.len()).find(|j| *j != index)
                } else {
                    None
                };
                if let Some(pair) = pair {
                    updated[index] = Root {
                        re,
                        im: if im.abs() < 0.045 { 0.0 } else { im },
                    };
                    updated[pair] = Root {
                        re,
                        im: -updated[index].im,
                    };
                } else {
                    updated[index] = Root { re, im: 0.0 };
                }
                let rec = Recurrence::from_roots(&updated);
                if rec.c.iter().all(|v| v.abs() <= 8.0) {
                    self.settings.recurrence = rec;
                    self.selected_root = rec
                        .roots()
                        .iter()
                        .enumerate()
                        .min_by(|(_, a), (_, b)| {
                            ((a.re - re).powi(2) + (a.im - im).powi(2))
                                .total_cmp(&((b.re - re).powi(2) + (b.im - im).powi(2)))
                        })
                        .map(|(i, _)| i);
                    self.preset = None;
                    self.reset_field();
                }
            }
        }
        if response.drag_stopped() {
            self.selected_root = None;
        }
        response.on_hover_text(
            "点をドラッグして特性根を変更。実係数を保つため複素根は共役で移動します。",
        );
        let rec = self.settings.recurrence;
        painter.text(
            pos2(rect.center().x, rect.bottom() - 40.0),
            Align2::CENTER_CENTER,
            format!("ρ = {:.3}  ·  {}", rec.spectral_radius(), rec.behavior()),
            FontId::proportional(10.0),
            CYAN,
        );
        painter.text(
            pos2(rect.center().x, rect.bottom() - 18.0),
            Align2::CENTER_CENTER,
            if system {
                "行列の係数で変更"
            } else {
                "点をドラッグして変更"
            },
            FontId::proportional(10.0),
            MUTED,
        );
    }

    fn save_settings(&mut self) {
        let result = (|| -> Result<(), String> {
            self.settings.validate().map_err(str::to_owned)?;
            let dir = output_dir()?;
            let json = serde_json::to_string_pretty(&self.settings).map_err(|e| e.to_string())?;
            fs::write(dir.join("settings.json"), json).map_err(|e| e.to_string())
        })();
        self.notify(match result {
            Ok(()) => "設定を output/settings.json に保存しました".into(),
            Err(e) => format!("保存できませんでした: {e}"),
        });
    }

    fn load_settings(&mut self) {
        let result = (|| -> Result<Settings, String> {
            let path = output_dir()?.join("settings.json");
            let metadata = fs::metadata(&path)
                .map_err(|_| "保存済みの output/settings.json がありません".to_owned())?;
            if metadata.len() > 64 * 1024 {
                return Err("設定ファイルが大きすぎます".into());
            }
            let file = fs::File::open(path).map_err(|e| e.to_string())?;
            let settings: Settings = serde_json::from_reader(std::io::Read::take(file, 64 * 1024))
                .map_err(|_| "設定ファイルの形式が不正です".to_owned())?;
            settings.validate().map_err(str::to_owned)?;
            Ok(settings)
        })();
        match result {
            Ok(settings) => {
                self.settings = settings;
                self.preset = None;
                self.pan = Vec2::ZERO;
                self.reset_field();
                self.notify("設定を読み込みました".into());
            }
            Err(e) => self.notify(e),
        }
    }

    fn export_csv(&mut self) {
        use std::fmt::Write;
        let result = (|| -> Result<String, String> {
            let dir = output_dir()?;
            let name = format!("orbit-{}.csv", timestamp());
            let system = self.settings.recurrence.matrix.is_some();
            let mut csv = String::from(if system {
                "n,x_n,y_n\n"
            } else {
                "n,a_n,a_n_plus_1,a_n_plus_2\n"
            });
            for (n, p) in orbit(
                self.settings.recurrence,
                self.settings.initial,
                self.settings.orbit_steps,
            )
            .iter()
            .enumerate()
            {
                if system {
                    let _ = writeln!(csv, "{n},{:.15e},{:.15e}", p[0], p[1]);
                } else if self.settings.recurrence.order == 2 {
                    let _ = writeln!(csv, "{n},{:.15e},{:.15e},", p[0], p[1]);
                } else {
                    let _ = writeln!(csv, "{n},{:.15e},{:.15e},{:.15e}", p[0], p[1], p[2]);
                }
            }
            write_new(&dir.join(&name), csv.as_bytes())?;
            Ok(name)
        })();
        self.notify(match result {
            Ok(name) => format!("初期値からの軌道を output/{name} に保存しました"),
            Err(e) => format!("CSVを保存できませんでした: {e}"),
        });
    }

    fn receive_screenshot(&mut self, ctx: &egui::Context) {
        let captured = ctx.input(|i| {
            i.events.iter().find_map(|event| {
                if let egui::Event::Screenshot { image, .. } = event {
                    Some(image.clone())
                } else {
                    None
                }
            })
        });
        if self.screenshot_pending {
            if let Some(image) = captured {
                self.screenshot_pending = false;
                let result = (|| -> Result<String, String> {
                    let dir = output_dir()?;
                    let name = format!("recur-{}.png", timestamp());
                    let mut bytes = Vec::with_capacity(image.pixels.len() * 4);
                    for p in &image.pixels {
                        bytes.extend_from_slice(&p.to_array());
                    }
                    let file = fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(dir.join(&name))
                        .map_err(|e| e.to_string())?;
                    image::ImageEncoder::write_image(
                        image::codecs::png::PngEncoder::new(file),
                        &bytes,
                        image.width() as u32,
                        image.height() as u32,
                        image::ExtendedColorType::Rgba8,
                    )
                    .map_err(|e| e.to_string())?;
                    Ok(name)
                })();
                self.notify(match result {
                    Ok(name) => format!("output/{name} に保存しました"),
                    Err(e) => format!("PNGを保存できませんでした: {e}"),
                });
            }
        }
    }

    fn help_window(&mut self, ctx: &egui::Context) {
        egui::Window::new("漸化式を、動きで理解する").open(&mut self.help).default_width(550.0).resizable(false).show(ctx, |ui| {
            ui.label("三項漸化式は (a[n], a[n+1])、四項漸化式は (a[n], a[n+1], a[n+2]) を座標として描きます。光の1本1本が、異なる初期値から始まる数列です。");
            ui.label("連立モードでは (x[n+1], y[n+1]) = M (x[n], y[n]) を表示します。2×2行列の4係数を編集できます。特性根は表示のみです。");
            ui.add_space(8.0);
            ui.label("1. プリセットを選び、p・q・r を変えて軌道の変化を観察します。\n2. 右上の特性根をドラッグして、単位円の内外の変化を試せます。\n3.「1本の軌道を強調表示」で初期値と反復回数を指定できます。");
            ui.separator();
            ui.label(RichText::new("特性根と安定性").color(CYAN));
            ui.label("すべての根の絶対値が1未満なら原点に収束します。1を超える根がある場合は発散するモードがあります。1に等しい根では、重根の有無や初期値によって周期・定常・多項式的な発散が起こります。");
            ui.separator();
            ui.label(RichText::new("表示上の約束").color(CYAN));
            ui.label("粒子表示は継続して観察するため、原点近傍・遠方・180項経過で初期分布へ再配置します。金色の単独軌道とCSVは再配置しません。線と動く先端は離散的な数列の線形補間であり、連続時間の解ではありません。");
            ui.separator();
            egui::Grid::new("shortcuts").num_columns(2).spacing(vec2(30.0, 8.0)).show(ui, |ui| {
                for (key, action) in [("Space", "再生 / 一時停止"), ("→", "1ステップ"), ("R", "粒子をリスタート"), ("H", "鑑賞モード / コントロールを表示"), ("F11 / Esc", "全画面 / 全画面と鑑賞モードを解除"), ("ドラッグ", "2D: 移動 / 3D: 回転"), ("Shift + ドラッグ", "3D: 移動"), ("ホイール / ダブルクリック", "ズーム / 移動とズームをリセット")] { ui.monospace(key); ui.label(action); ui.end_row(); }
            });
            ui.separator();
            ui.label("保存先は実行ファイルと同じ場所の output フォルダーです。設定は settings.json に保存され、読み込み時に値を検証します。外部通信は行いません。");
        });
    }
}

impl eframe::App for RecurApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = Instant::now();
        let dt = now.duration_since(self.previous).as_secs_f64();
        self.previous = now;
        if dt > 0.0 && dt < 0.5 {
            self.fps = self.fps * 0.95 + (1.0 / dt) as f32 * 0.05;
        }
        if !ctx.wants_keyboard_input() {
            if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
                self.playing = !self.playing;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                self.single_step();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::R)) {
                self.reset_field();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::H)) {
                self.clean = !self.clean;
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::F11)) {
            self.toggle_fullscreen(ctx);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.fullscreen = false;
            self.clean = false;
            self.help = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        }
        self.receive_screenshot(ctx);
        if !self.clean {
            self.header(ctx);
            self.footer(ctx);
            self.sidebar(ctx);
        }
        if self.playing {
            let s = &self.settings;
            self.field
                .advance(s.recurrence, dt, s.speed, s.spread, s.trail);
            if s.auto_rotate && s.recurrence.order == 3 {
                self.settings.yaw =
                    (self.settings.yaw + dt.min(0.1) * 0.12) % std::f64::consts::TAU;
            }
        }
        self.canvas(ctx);
        self.help_window(ctx);
        if self.playing
            || self.screenshot_pending
            || self
                .notice
                .as_ref()
                .is_some_and(|(_, at)| at.elapsed() < Duration::from_secs(8))
        {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }
}

fn section(ui: &mut egui::Ui, number: &str, title: &str) {
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(number).monospace().size(10.0).color(CYAN));
        ui.label(RichText::new(title).strong().size(14.0));
    });
    ui.add_space(4.0);
}

fn slider_row(ui: &mut egui::Ui, name: &str, slider: egui::Slider<'_>) -> egui::Response {
    ui.label(RichText::new(name).size(11.0).color(MUTED));
    ui.add(slider)
}

fn palette(index: usize, t: f32) -> Color32 {
    match index {
        1 => Color32::from_rgb((155.0 + t * 90.0) as u8, (190.0 + t * 60.0) as u8, 255),
        2 => egui::ecolor::Hsva::new(t * 0.85, 0.65, 1.0, 1.0).into(),
        _ => {
            let t = t * t;
            Color32::from_rgb(
                (45.0 + t * 80.0) as u8,
                (125.0 + t * 125.0) as u8,
                (255.0 - t * 40.0) as u8,
            )
        }
    }
}

fn tint(color: Color32, alpha: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(
        color.r(),
        color.g(),
        color.b(),
        (alpha.clamp(0.0, 1.0) * 255.0) as u8,
    )
}

/// Liang–Barsky clips before tessellation, avoiding huge or non-finite vertices.
fn clip_segment(a: Pos2, b: Pos2, rect: Rect) -> Option<(Pos2, Pos2)> {
    if !a.is_finite() || !b.is_finite() {
        return None;
    }
    let d = b - a;
    if !d.is_finite() {
        return None;
    }
    let mut lo: f32 = 0.0;
    let mut hi: f32 = 1.0;
    for (p, q) in [
        (-d.x, a.x - rect.left()),
        (d.x, rect.right() - a.x),
        (-d.y, a.y - rect.top()),
        (d.y, rect.bottom() - a.y),
    ] {
        if p.abs() < 1e-12 {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                lo = lo.max(r);
            } else {
                hi = hi.min(r);
            }
        }
    }
    if lo > hi {
        None
    } else {
        Some((a + lo * d, a + hi * d))
    }
}

fn mesh_segment(mesh: &mut egui::Mesh, a: Pos2, b: Pos2, width: f32, color: Color32, rect: Rect) {
    let Some((a, b)) = clip_segment(a, b, rect.expand(4.0)) else {
        return;
    };
    let d = b - a;
    let len = d.length();
    if len < 0.05 {
        return;
    }
    let normal = vec2(-d.y, d.x) * (width * 0.5 / len);
    let base = mesh.vertices.len() as u32;
    for pos in [a - normal, a + normal, b + normal, b - normal] {
        mesh.colored_vertex(pos, color);
    }
    mesh.add_triangle(base, base + 1, base + 2);
    mesh.add_triangle(base, base + 2, base + 3);
}

fn draw_segment(painter: &egui::Painter, a: Pos2, b: Pos2, stroke: Stroke) {
    if let Some((a, b)) = clip_segment(a, b, painter.clip_rect()) {
        painter.line_segment([a, b], stroke);
    }
}

fn output_dir() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let path = exe
        .parent()
        .ok_or("実行場所を取得できません")?
        .join("output");
    fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    Ok(path)
}
fn timestamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
fn write_new(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut file| file.write_all(bytes))
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_reject_nonfinite_out_of_range_and_unrecognized_input() {
        let mut s = Settings::default();
        assert!(s.validate().is_ok());
        s.recurrence.order = 4;
        assert!(s.validate().is_err());
        s = Settings::default();
        s.count = usize::MAX;
        assert!(s.validate().is_err());
        s = Settings::default();
        s.initial[0] = f64::NAN;
        assert!(s.validate().is_err());
        s = Settings::default();
        s.speed = f64::INFINITY;
        assert!(s.validate().is_err());
        s = Settings::default();
        s.palette = 9;
        assert!(s.validate().is_err());
        s = Settings::default();
        s.recurrence = Recurrence::linear([[f64::NAN, 0.0], [0.0, 1.0]]);
        assert!(s.validate().is_err());
        s.recurrence = Recurrence::linear([[3.1, 0.0], [0.0, 1.0]]);
        assert!(s.validate().is_err());
        s.recurrence = Recurrence::linear([[1.0, 0.0], [0.0, 1.0]]);
        s.recurrence.order = 3;
        assert!(s.validate().is_err());
        assert!(serde_json::from_str::<Settings>("{}").is_err());
        let mut json = serde_json::to_value(Settings::default()).unwrap();
        json["command"] = "untrusted".into();
        assert!(serde_json::from_value::<Settings>(json).is_err());
    }
    #[test]
    fn clipping_rejects_invalid_and_clips_crossing_lines() {
        let rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(10.0, 10.0));
        assert!(clip_segment(pos2(f32::NAN, 0.0), pos2(1.0, 1.0), rect).is_none());
        assert!(clip_segment(pos2(-5.0, -5.0), pos2(-1.0, -1.0), rect).is_none());
        assert_eq!(
            clip_segment(pos2(-5.0, 5.0), pos2(15.0, 5.0), rect),
            Some((pos2(0.0, 5.0), pos2(10.0, 5.0)))
        );
    }
}
