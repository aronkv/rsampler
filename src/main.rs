mod audio;

use std::f32::consts::PI;
use std::fs;
use std::path::{Path, PathBuf};

use eframe::egui::{
    self, Align2, Color32, FontId, Key, Painter, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, vec2,
};

/// Panel size in design units; everything is drawn in these and scaled to the window.
const W: f32 = 1000.0;
const H: f32 = 730.0;

const BACKDROP: Color32 = Color32::from_rgb(0x1c, 0x1d, 0x20);
const BODY: Color32 = Color32::from_rgb(0xdc, 0xd9, 0xcf);
const BODY_EDGE: Color32 = Color32::from_rgb(0xb4, 0xb0, 0xa5);
const INK: Color32 = Color32::from_rgb(0x3a, 0x3b, 0x3d);
const SHADOW: Color32 = Color32::from_rgb(0x9e, 0x9b, 0x92);
const PANEL: Color32 = Color32::from_rgb(0xa6, 0xad, 0xae);
const PANEL_INSET: Color32 = Color32::from_rgb(0xd9, 0xda, 0xd5);
const HOUSING: Color32 = Color32::from_rgb(0x2e, 0x30, 0x32);
const GLASS: Color32 = Color32::from_rgb(0x12, 0x20, 0x3a);
const LCD_BG: Color32 = Color32::from_rgb(0x3f, 0xb4, 0xc4);
const LCD_INK: Color32 = Color32::from_rgb(0x0f, 0x2f, 0x5a);
const LOGO: Color32 = Color32::from_rgb(0xc8, 0x23, 0x2c);
const DARK: Color32 = Color32::from_rgb(0x2a, 0x2b, 0x2d);
const RED_ON: Color32 = Color32::from_rgb(0xff, 0x3b, 0x2f);
const RED_OFF: Color32 = Color32::from_rgb(0x6b, 0x2a, 0x26);
const GREEN_ON: Color32 = Color32::from_rgb(0x8e, 0xf0, 0x6a);
const GREEN_OFF: Color32 = Color32::from_rgb(0x2f, 0x4a, 0x2a);
const PAD: Color32 = Color32::from_rgb(0x85, 0x88, 0x87);
const PAD_EDGE: Color32 = Color32::from_rgb(0x5f, 0x62, 0x61);
const PAD_HIT: Color32 = Color32::from_rgb(0x6c, 0x6f, 0x6e);

struct KeyStyle {
    face: Color32,
    pressed: Color32,
    edge: Color32,
}

const WHITE: KeyStyle = KeyStyle {
    face: Color32::from_rgb(0xf1, 0xef, 0xe8),
    pressed: Color32::from_rgb(0xd6, 0xd3, 0xca),
    edge: Color32::from_rgb(0x9d, 0x9a, 0x92),
};
const BLUE: KeyStyle = KeyStyle {
    face: Color32::from_rgb(0x4d, 0x7f, 0xb8),
    pressed: Color32::from_rgb(0x3d, 0x6a, 0x9c),
    edge: Color32::from_rgb(0x2c, 0x4d, 0x72),
};
const RED: KeyStyle = KeyStyle {
    face: Color32::from_rgb(0xe8, 0x6a, 0x5e),
    pressed: Color32::from_rgb(0xc9, 0x55, 0x4a),
    edge: Color32::from_rgb(0x8f, 0x3a, 0x32),
};
const ORANGE: KeyStyle = KeyStyle {
    face: Color32::from_rgb(0xf0, 0x8a, 0x2c),
    pressed: Color32::from_rgb(0xd0, 0x72, 0x1e),
    edge: Color32::from_rgb(0x9a, 0x55, 0x18),
};

/// Keys for pads 1–16, bottom row first: pad 1 is bottom-left, like on the hardware.
const PAD_KEYS: [Key; 16] = [
    Key::Z,
    Key::X,
    Key::C,
    Key::V,
    Key::A,
    Key::S,
    Key::D,
    Key::F,
    Key::Q,
    Key::W,
    Key::E,
    Key::R,
    Key::Num1,
    Key::Num2,
    Key::Num3,
    Key::Num4,
];

const BANKS: [&str; 4] = ["A", "B", "C", "D"];
const MAIN_SOFT_KEYS: [&str; 6] = ["STEP", "EDIT", "TR.MUTE", "SOLO", "TR -", "TR +"];
const LOAD_SOFT_KEYS: [&str; 6] = ["UP DIR", "", "", "", "", "DO IT"];
/// Velocity for pads hit from the keyboard, which has no pressure.
const KEY_VELOCITY: f32 = 0.8;
/// Numeric keypad, top row first, with the mode each key opens.
const KEYPAD: [(&str, &str); 10] = [
    ("7", "SETUP"),
    ("8", "MIXER"),
    ("9", "OTHER"),
    ("4", "SAMPLE"),
    ("5", "TRIM"),
    ("6", "PROGRAM"),
    ("1", "SONG"),
    ("2", "MISC"),
    ("3", "LOAD"),
    ("0", ""),
];

/// Maps design units to screen coordinates.
struct Panel {
    origin: Pos2,
    scale: f32,
}

impl Panel {
    fn pos(&self, x: f32, y: f32) -> Pos2 {
        self.origin + vec2(x, y) * self.scale
    }

    fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(self.pos(x, y), vec2(w, h) * self.scale)
    }

    fn mono(&self, size: f32) -> FontId {
        FontId::monospace(size * self.scale)
    }

    fn prop(&self, size: f32) -> FontId {
        FontId::proportional(size * self.scale)
    }

    /// Printed panel legend, centered on (x, y).
    fn label(&self, painter: &Painter, x: f32, y: f32, text: &str, size: f32) {
        painter.text(self.pos(x, y), Align2::CENTER_CENTER, text, self.prop(size), INK);
    }

    /// Section title with a rule on each side, like "LOCATE" or "PAD BANK".
    fn section(&self, painter: &Painter, x0: f32, x1: f32, y: f32, text: &str) {
        let mid = (x0 + x1) / 2.0;
        let gap = text.len() as f32 * 3.6 + 8.0;
        let stroke = Stroke::new(self.scale, INK);
        painter.hline(self.pos(x0, y).x..=self.pos(mid - gap, y).x, self.pos(0.0, y).y, stroke);
        painter.hline(self.pos(mid + gap, y).x..=self.pos(x1, y).x, self.pos(0.0, y).y, stroke);
        self.label(painter, mid, y, text, 8.0);
    }
}

struct Slot {
    name: String,
    data: audio::Sample,
}

/// LOAD screen: a directory listing of subfolders and WAV files.
struct Browser {
    dir: PathBuf,
    entries: Vec<PathBuf>,
    cursor: usize,
}

impl Browser {
    fn open(dir: PathBuf) -> Self {
        let visible = |p: &Path| !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'));
        let mut entries: Vec<PathBuf> = fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| visible(p) && (p.is_dir() || is_wav(p)))
            .collect();
        entries.sort_by_key(|p| (!p.is_dir(), p.file_name().map(|n| n.to_string_lossy().to_lowercase())));
        Self { dir, entries, cursor: 0 }
    }
}

fn is_wav(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("wav"))
}

enum Screen {
    Main,
    Load(Browser),
}

struct App {
    engine: Option<audio::Engine>,
    /// Four banks of 16 pads.
    slots: Vec<Option<Slot>>,
    screen: Screen,
    load_dir: PathBuf,
    /// Last error or confirmation, shown on the LCD until the screen changes.
    message: String,
    held: [bool; 16],
    key_held: [bool; 16],
    wheel_drag: f32,
    wheel_angle: f32,
    volume: f32,
    velocity: f32,
    bank: usize,
    last_pad: Option<usize>,
    playing: bool,
    rec: bool,
    overdub: bool,
    full_level: bool,
    sixteen_levels: bool,
}

impl App {
    fn new() -> Self {
        let (engine, message) = match audio::Engine::new() {
            Ok(engine) => (Some(engine), String::new()),
            Err(e) => (None, format!("Audio: {e}")),
        };
        let mut slots: Vec<Option<Slot>> = (0..64).map(|_| None).collect();
        if let Some(engine) = &engine {
            for (slot, (name, data)) in slots.iter_mut().zip(audio::builtin_kit(engine.rate)) {
                *slot = Some(Slot { name: name.to_owned(), data });
            }
        }
        let home = std::env::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        let music = home.join("Music");
        Self {
            engine,
            slots,
            screen: Screen::Main,
            load_dir: if music.is_dir() { music } else { home },
            message,
            held: [false; 16],
            key_held: [false; 16],
            wheel_drag: 0.0,
            wheel_angle: 0.0,
            volume: 0.8,
            velocity: 0.0,
            bank: 0,
            last_pad: None,
            playing: false,
            rec: false,
            overdub: false,
            full_level: false,
            sixteen_levels: false,
        }
    }

    fn slot(&self, pad: usize) -> &Option<Slot> {
        &self.slots[self.bank * 16 + pad]
    }

    fn hit_pad(&mut self, pad: usize, velocity: f32) {
        self.last_pad = Some(pad);
        self.velocity = if self.full_level { 1.0 } else { velocity };
        if let (Some(engine), Some(slot)) = (&self.engine, self.slot(pad)) {
            engine.play(&slot.data, self.velocity);
        }
    }

    fn set_screen(&mut self, screen: Screen) {
        if let Screen::Load(b) = &self.screen {
            self.load_dir = b.dir.clone();
        }
        self.screen = screen;
        self.message.clear();
    }

    fn move_cursor(&mut self, steps: isize) {
        if let Screen::Load(b) = &mut self.screen {
            let last = b.entries.len().saturating_sub(1);
            b.cursor = b.cursor.saturating_add_signed(steps).min(last);
        }
    }

    fn parent_dir(&mut self) {
        if let Screen::Load(b) = &mut self.screen
            && let Some(parent) = b.dir.parent()
        {
            let from = b.dir.clone();
            *b = Browser::open(parent.to_owned());
            b.cursor = b.entries.iter().position(|e| *e == from).unwrap_or(0);
        }
    }

    /// ENTER on the LOAD screen: opens a folder, or loads a WAV into the selected pad.
    fn enter(&mut self) {
        let Screen::Load(b) = &mut self.screen else { return };
        let Some(path) = b.entries.get(b.cursor).cloned() else { return };
        if path.is_dir() {
            *b = Browser::open(path);
            return;
        }
        let Some(engine) = &self.engine else {
            self.message = "No audio device".to_owned();
            return;
        };
        let pad = self.last_pad.unwrap_or(0);
        match audio::load_wav(&path, engine.rate) {
            Ok(data) => {
                engine.play(&data, KEY_VELOCITY);
                let name = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
                self.slots[self.bank * 16 + pad] = Some(Slot { name, data });
                self.message = "Loaded".to_owned();
            }
            Err(e) => self.message = e.to_string(),
        }
    }

    fn keyboard(&mut self, ui: &Ui) {
        let (up, down, enter, back, esc) = ui.input(|i| {
            (
                i.key_pressed(Key::ArrowUp),
                i.key_pressed(Key::ArrowDown),
                i.key_pressed(Key::Enter) || i.key_pressed(Key::ArrowRight),
                i.key_pressed(Key::Backspace) || i.key_pressed(Key::ArrowLeft),
                i.key_pressed(Key::Escape),
            )
        });
        if up {
            self.move_cursor(-1);
        }
        if down {
            self.move_cursor(1);
        }
        if enter {
            self.enter();
        }
        if back {
            self.parent_dir();
        }
        if esc {
            self.set_screen(Screen::Main);
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(BACKDROP)).show(ui, |ui| self.panel(ui));
    }
}

impl App {
    fn panel(&mut self, ui: &mut Ui) {
        self.keyboard(ui);
        let area = ui.max_rect();
        let scale = (area.width() / W).min(area.height() / H);
        let p = Panel { origin: area.center() - vec2(W, H) * scale / 2.0, scale };
        let s = scale;
        let painter = ui.painter().clone();

        painter.rect_filled(p.rect(15.0, 15.0, 970.0, 700.0), 6.0 * s, BODY_EDGE);
        painter.rect_filled(p.rect(15.0, 15.0, 970.0, 694.0), 6.0 * s, BODY);

        self.lcd(&painter, &p);
        for i in 0..6 {
            let rect = p.rect(167.0 + i as f32 * 41.0, 212.0, 28.0, 14.0);
            if key(ui, &p, rect, &WHITE, &format!("F{}", i + 1)).clicked() && matches!(self.screen, Screen::Load(_)) {
                match i {
                    0 => self.parent_dir(),
                    5 => self.enter(),
                    _ => {}
                }
            }
        }

        self.left_controls(ui, &painter, &p);
        self.transport(ui, &painter, &p);
        self.upper_panel(ui, &painter, &p);
        self.pads(ui, &painter, &p);
    }

    fn lcd(&self, painter: &Painter, p: &Panel) {
        let s = p.scale;
        painter.rect_filled(p.rect(78.0, 28.0, 412.0, 152.0), 6.0 * s, HOUSING);
        painter.rect_filled(p.rect(112.0, 52.0, 345.0, 96.0), 3.0 * s, GLASS);
        let header = "16 BIT DRUM SAMPLER / MIDI SEQUENCER";
        painter.text(p.pos(283.0, 63.0), Align2::CENTER_CENTER, header, p.prop(6.0), Color32::WHITE);
        painter.rect_filled(p.rect(163.0, 70.0, 241.0, 66.0), s, LCD_BG);

        let status = match (self.playing, self.rec || self.overdub) {
            (true, true) => "REC",
            (true, false) => "PLAY",
            _ => "STOP",
        };
        let pad = self.last_pad.unwrap_or(0);
        let pad_line = format!(
            "Pad:{}{:02} {}",
            BANKS[self.bank],
            pad + 1,
            self.slot(pad).as_ref().map_or("(empty)", |s| &s.name).chars().take(16).collect::<String>()
        );
        // Each row is (left text, right text, highlighted).
        let (rows, soft_keys) = match &self.screen {
            Screen::Main => {
                let note = if self.message.is_empty() { "Count:ON       Loop:ON" } else { &self.message };
                let rows = vec![
                    ("Sq:01-Sequence01".to_owned(), "Now:001.01.00".to_owned(), false),
                    (" J:120.0(SEQ)  Timing:1/16".to_owned(), "Tsig: 4/4".to_owned(), false),
                    (note.to_owned(), "Bars:  2".to_owned(), false),
                    ("Tr:01-(Unused) ON:YES".to_owned(), format!("Vel:{:3}", (self.velocity * 127.0) as u8), false),
                    (pad_line, status.to_owned(), false),
                ];
                (rows, MAIN_SOFT_KEYS)
            }
            Screen::Load(b) => {
                let dir = b.dir.to_string_lossy();
                let tail = dir.chars().rev().take(34).collect::<Vec<_>>().into_iter().rev().collect::<String>();
                let mut rows = vec![(format!("LOAD {tail}"), String::new(), false)];
                let start = b.cursor.saturating_sub(2);
                for (i, entry) in b.entries.iter().enumerate().skip(start).take(3) {
                    let name = entry.file_name().unwrap_or_default().to_string_lossy();
                    let name = if entry.is_dir() { format!("{name}/") } else { name.into_owned() };
                    rows.push((format!(" {name}"), String::new(), i == b.cursor));
                }
                if b.entries.is_empty() {
                    rows.push((" (no folders or .wav files)".to_owned(), String::new(), false));
                }
                rows.resize(4, (String::new(), String::new(), false));
                let hint = if self.message.is_empty() { "ENTER: load" } else { &self.message };
                rows.push((pad_line, hint.to_owned(), false));
                (rows, LOAD_SOFT_KEYS)
            }
        };
        for (i, (left, right, highlight)) in rows.iter().enumerate() {
            let y = 73.0 + i as f32 * 10.0;
            let ink = if *highlight {
                painter.rect_filled(p.rect(164.0, y - 0.5, 239.0, 9.5), 0.0, LCD_INK);
                LCD_BG
            } else {
                LCD_INK
            };
            painter.text(p.pos(166.0, y), Align2::LEFT_TOP, left, p.mono(7.0), ink);
            painter.text(p.pos(401.0, y), Align2::RIGHT_TOP, right, p.mono(7.0), ink);
        }

        for (i, label) in soft_keys.iter().enumerate() {
            let tab = p.rect(163.0 + i as f32 * 41.0, 126.0, 36.0, 9.0);
            if !label.is_empty() {
                painter.rect_filled(tab, 0.0, LCD_INK);
                painter.text(tab.center(), Align2::CENTER_CENTER, *label, p.mono(6.0), LCD_BG);
            }
            let legend = format!("F{}", i + 1);
            let at = p.pos(181.0 + i as f32 * 41.0, 163.0);
            painter.text(at, Align2::CENTER_CENTER, legend, p.prop(7.0), Color32::from_gray(0xc8));
        }
    }

    fn left_controls(&mut self, ui: &mut Ui, painter: &Painter, p: &Panel) {
        let s = p.scale;

        for (i, (num, mode)) in KEYPAD.iter().enumerate() {
            let (x, y) =
                if i == 9 { (141.0, 402.0) } else { (77.0 + (i % 3) as f32 * 64.0, 286.0 + (i / 3) as f32 * 36.0) };
            p.label(painter, x + 12.0, y - 7.0, num, 8.0);
            if key(ui, p, p.rect(x, y, 24.0, 10.0), &BLUE, "").clicked() && *mode == "LOAD" {
                self.set_screen(Screen::Load(Browser::open(self.load_dir.clone())));
            }
            p.label(painter, x + 12.0, y + 16.0, mode, 6.0);
        }
        painter.rect_filled(p.rect(66.0, 386.0, 40.0, 30.0), 2.0 * s, PANEL);
        p.label(painter, 86.0, 393.0, "SHIFT", 7.0);
        key(ui, p, p.rect(72.0, 401.0, 28.0, 12.0), &WHITE, "");
        p.label(painter, 217.0, 393.0, "ENTER", 7.0);
        if key(ui, p, p.rect(203.0, 401.0, 28.0, 12.0), &WHITE, "").clicked() {
            self.enter();
        }

        p.label(painter, 304.0, 262.0, "MAIN SCREEN", 7.0);
        if key(ui, p, p.rect(288.0, 271.0, 32.0, 15.0), &WHITE, "").clicked() {
            self.set_screen(Screen::Main);
        }
        p.label(painter, 398.0, 262.0, "OPEN WINDOW", 7.0);
        key(ui, p, p.rect(382.0, 271.0, 32.0, 15.0), &ORANGE, "");

        p.label(painter, 350.0, 304.0, "DATA", 9.0);
        let center = p.pos(350.0, 378.0);
        let resp = ui.interact(
            Rect::from_center_size(center, vec2(124.0, 124.0) * s),
            ui.scope_id().with("data"),
            Sense::drag(),
        );
        let scroll = if resp.hovered() { ui.input(|i| i.smooth_scroll_delta.y) } else { 0.0 };
        // Dragging up or scrolling up turns the wheel clockwise, which moves the cursor down.
        let turn = (-resp.drag_delta().y - scroll) / s;
        self.wheel_angle += turn * 0.03;
        self.wheel_drag += turn;
        let steps = (self.wheel_drag / 12.0).trunc();
        self.wheel_drag -= steps * 12.0;
        self.move_cursor(steps as isize);
        wheel(painter, p, center, 62.0, self.wheel_angle);

        p.label(painter, 350.0, 467.0, "CURSOR", 9.0);
        painter.rect_filled(p.rect(308.0, 476.0, 86.0, 66.0), 4.0 * s, BODY_EDGE);
        if key(ui, p, p.rect(331.0, 479.0, 40.0, 29.0), &WHITE, "︿").clicked() {
            self.move_cursor(-1);
        }
        if key(ui, p, p.rect(331.0, 510.0, 40.0, 29.0), &WHITE, "﹀").clicked() {
            self.move_cursor(1);
        }
        if key(ui, p, p.rect(311.0, 479.0, 18.0, 60.0), &WHITE, "‹").clicked() {
            self.parent_dir();
        }
        if key(ui, p, p.rect(373.0, 479.0, 18.0, 60.0), &WHITE, "›").clicked() {
            self.enter();
        }
        p.label(painter, 350.0, 551.0, "DIGIT", 6.0);

        painter.rect_stroke(p.rect(44.0, 430.0, 84.0, 244.0), 0.0, Stroke::new(s, INK), StrokeKind::Inside);
        p.label(painter, 86.0, 440.0, "NOTE", 7.0);
        p.label(painter, 86.0, 449.0, "VARIATION", 7.0);
        p.label(painter, 86.0, 462.0, "AFTER", 7.0);
        led(painter, p, 86.0, 470.0, false, RED_ON, RED_OFF);
        key(ui, p, p.rect(70.0, 477.0, 32.0, 16.0), &WHITE, "");
        let tick = Stroke::new(s, INK);
        for k in 0..11 {
            let y = p.pos(0.0, 520.0 + k as f32 * 14.0).y;
            painter.hline(p.pos(58.0, 0.0).x..=p.pos(74.0, 0.0).x, y, tick);
            painter.hline(p.pos(96.0, 0.0).x..=p.pos(112.0, 0.0).x, y, tick);
        }
        painter.rect_filled(p.rect(82.0, 516.0, 6.0, 148.0), 2.0 * s, DARK);
        painter.rect_filled(p.rect(58.0, 576.0, 54.0, 20.0), 2.0 * s, SHADOW);
        painter.rect_filled(p.rect(58.0, 574.0, 54.0, 20.0), 2.0 * s, WHITE.face);
        painter.rect_filled(p.rect(58.0, 582.0, 54.0, 4.0), 0.0, BLUE.face);

        p.label(painter, 205.0, 436.0, "TAP TEMPO", 7.0);
        p.label(painter, 205.0, 445.0, "NOTE REPEAT", 7.0);
        key(ui, p, p.rect(185.0, 454.0, 40.0, 30.0), &WHITE, "");

        p.label(painter, 176.0, 509.0, "UNDO SEQ", 7.0);
        led(painter, p, 176.0, 518.0, false, RED_ON, RED_OFF);
        key(ui, p, p.rect(160.0, 525.0, 32.0, 16.0), &WHITE, "");
        p.label(painter, 234.0, 509.0, "ERASE", 7.0);
        key(ui, p, p.rect(218.0, 525.0, 32.0, 16.0), &WHITE, "");

        p.section(painter, 160.0, 425.0, 563.0, "LOCATE");
        let legends = [
            (176.0, "<"),
            (205.0, "STEP"),
            (234.0, ">"),
            (293.0, "GO TO"),
            (351.0, "<<"),
            (380.0, "BAR"),
            (409.0, ">>"),
        ];
        for (x, text) in legends {
            p.label(painter, x, 574.0, text, 7.0);
        }
        for x in [160.0, 218.0, 277.0, 335.0, 393.0] {
            key(ui, p, p.rect(x, 582.0, 32.0, 16.0), &WHITE, "");
        }
    }

    fn transport(&mut self, ui: &mut Ui, painter: &Painter, p: &Panel) {
        let buttons: [(&str, &str, &KeyStyle, Option<bool>); 5] = [
            ("REC", "", &RED, Some(self.rec)),
            ("OVER", "DUB", &RED, Some(self.overdub)),
            ("STOP", "", &WHITE, None),
            ("PLAY", "", &WHITE, Some(self.playing)),
            ("PLAY", "START", &WHITE, None),
        ];
        for (i, (top, bottom, style, lit)) in buttons.into_iter().enumerate() {
            let x = [155.0, 213.0, 272.0, 331.0, 389.0][i];
            if bottom.is_empty() {
                p.label(painter, x + 21.0, 630.0, top, 7.0);
            } else {
                p.label(painter, x + 23.0, 624.0, top, 7.0);
                p.label(painter, x + 23.0, 632.0, bottom, 7.0);
            }
            if let Some(on) = lit {
                led(painter, p, x + 6.0, 627.0, on, RED_ON, RED_OFF);
            }
            if key(ui, p, p.rect(x, 640.0, 42.0, 32.0), style, "").clicked() {
                match i {
                    0 => self.rec = !self.rec,
                    1 => self.overdub = !self.overdub,
                    2 => (self.playing, self.rec, self.overdub) = (false, false, false),
                    _ => self.playing = true,
                }
            }
        }
    }

    fn upper_panel(&mut self, ui: &mut Ui, painter: &Painter, p: &Panel) {
        let s = p.scale;
        painter.text(p.pos(553.0, 86.0), Align2::LEFT_CENTER, "rsampler", p.prop(38.0), LOGO);
        painter.text(p.pos(965.0, 72.0), Align2::RIGHT_CENTER, "DRUM SAMPLER", p.prop(20.0), INK);
        painter.hline(p.pos(760.0, 0.0).x..=p.pos(965.0, 0.0).x, p.pos(0.0, 88.0).y, Stroke::new(2.0 * s, INK));
        painter.text(p.pos(965.0, 100.0), Align2::RIGHT_CENTER, "MIDI SEQUENCER", p.prop(11.0), INK);

        painter.rect_filled(p.rect(545.0, 123.0, 430.0, 140.0), 3.0 * s, PANEL);
        painter.rect_filled(p.rect(568.0, 141.0, 135.0, 54.0), 2.0 * s, PANEL_INSET);
        painter.rect_filled(p.rect(568.0, 203.0, 135.0, 54.0), 2.0 * s, PANEL_INSET);

        let toggles = [
            (602.0, 151.0, "FULL LEVEL", Some(&mut self.full_level)),
            (664.0, 151.0, "16 LEVELS", Some(&mut self.sixteen_levels)),
            (602.0, 213.0, "NEXT SEQ", None),
            (664.0, 213.0, "TRACK MUTE", None),
        ];
        for (x, y, text, state) in toggles {
            p.label(painter, x, y, text, 7.0);
            let on = state.as_deref().copied().unwrap_or(false);
            led(painter, p, x, y + 10.0, on, RED_ON, RED_OFF);
            if key(ui, p, p.rect(x - 14.0, y + 18.0, 28.0, 14.0), &WHITE, "").clicked()
                && let Some(state) = state
            {
                *state = !*state;
            }
        }

        p.label(painter, 790.0, 135.0, "REC GAIN", 8.0);
        knob(painter, p, p.pos(790.0, 170.0), 20.0, Color32::from_rgb(0xe9, 0xa0, 0xa6), 0.35);
        p.label(painter, 885.0, 135.0, "MAIN VOLUME", 8.0);
        let center = p.pos(885.0, 170.0);
        let resp = ui.interact(
            Rect::from_center_size(center, vec2(52.0, 52.0) * s),
            ui.scope_id().with("volume"),
            Sense::drag(),
        );
        let scroll = if resp.hovered() { ui.input(|i| i.smooth_scroll_delta.y) } else { 0.0 };
        let delta = (-resp.drag_delta().y + scroll) / s / 120.0;
        if delta != 0.0 {
            self.volume = (self.volume + delta).clamp(0.0, 1.0);
            if let Some(engine) = &self.engine {
                engine.set_volume(self.volume);
            }
        }
        knob(painter, p, center, 20.0, WHITE.face, self.volume);
        for x in [790.0, 885.0] {
            p.label(painter, x - 22.0, 197.0, "MIN", 5.5);
            p.label(painter, x + 22.0, 197.0, "MAX", 5.5);
        }

        p.section(painter, 712.0, 965.0, 212.0, "PAD BANK");
        for (i, name) in BANKS.iter().enumerate() {
            let x = 714.0 + i as f32 * 63.0;
            p.label(painter, x + 14.0, 221.0, name, 7.0);
            led(painter, p, x + 14.0, 229.0, self.bank == i, GREEN_ON, GREEN_OFF);
            if key(ui, p, p.rect(x, 237.0, 28.0, 14.0), &WHITE, "").clicked() {
                self.bank = i;
            }
        }
    }

    fn pads(&mut self, ui: &mut Ui, painter: &Painter, p: &Panel) {
        painter.rect_filled(p.rect(545.0, 272.0, 430.0, 400.0), 3.0 * p.scale, PANEL);
        p.section(painter, 575.0, 945.0, 284.0, "DRUMS");

        // Pads follow physical key positions, so the bottom-left key is pad 1 on QWERTZ and
        // AZERTY too. Events rather than key_down, so a tap released within one frame still counts.
        let mut tapped = [false; 16];
        ui.input(|i| {
            for e in &i.events {
                if let egui::Event::Key { key, physical_key, pressed, repeat, .. } = e
                    && let Some(n) = PAD_KEYS.iter().position(|k| *k == physical_key.unwrap_or(*key))
                {
                    tapped[n] |= *pressed && !*repeat;
                    self.key_held[n] = *pressed;
                }
            }
        });

        for (n, key) in PAD_KEYS.iter().enumerate() {
            let (x, y) = (583.0 + (n % 4) as f32 * 92.0, 302.0 + (3 - n / 4) as f32 * 92.0);
            painter.text(p.pos(x, y - 7.0), Align2::LEFT_CENTER, format!("PAD {}", n + 1), p.prop(7.0), INK);
            let legend = if *key == Key::Z { "Z/Y" } else { key.name() };
            painter.text(p.pos(x + 76.0, y - 7.0), Align2::RIGHT_CENTER, legend, p.prop(7.0), INK);
            let rect = p.rect(x, y, 76.0, 74.0);
            let resp = ui.interact(rect, ui.scope_id().with(("pad", n)), Sense::click());
            let clicked = resp.is_pointer_button_down_on();
            let tapped = tapped[n];
            if clicked && !self.held[n] {
                // With the mouse, hitting higher on the pad plays louder.
                let velocity = resp
                    .interact_pointer_pos()
                    .map_or(KEY_VELOCITY, |pos| 1.0 - (pos.y - rect.top()) / rect.height() * 0.8);
                self.hit_pad(n, velocity);
            } else if tapped {
                self.hit_pad(n, KEY_VELOCITY);
            }
            self.held[n] = clicked;
            pad(painter, p, rect, clicked || tapped || self.key_held[n]);
        }
    }
}

fn key(ui: &mut Ui, p: &Panel, rect: Rect, style: &KeyStyle, glyph: &str) -> Response {
    let s = p.scale;
    let id = ui.scope_id().with(((rect.min.x / s) as i32, (rect.min.y / s) as i32));
    let resp = ui.interact(rect, id, Sense::click());
    let down = resp.is_pointer_button_down_on();
    let face = if down { rect.translate(vec2(0.0, s)) } else { rect };
    let painter = ui.painter();

    painter.rect_filled(rect.translate(vec2(0.0, 2.0 * s)), 2.5 * s, SHADOW);
    painter.rect_filled(face, 2.5 * s, if down { style.pressed } else { style.face });
    painter.rect_stroke(face, 2.5 * s, Stroke::new(s, style.edge), StrokeKind::Inside);
    if !glyph.is_empty() {
        painter.text(face.center(), Align2::CENTER_CENTER, glyph, p.prop(8.0), INK);
    }
    resp
}

fn led(painter: &Painter, p: &Panel, x: f32, y: f32, on: bool, lit: Color32, unlit: Color32) {
    let c = p.pos(x, y);
    if on {
        painter.circle_filled(c, 6.0 * p.scale, lit.gamma_multiply(0.3));
    }
    painter.circle_filled(c, 2.6 * p.scale, if on { lit } else { unlit });
}

fn pad(painter: &Painter, p: &Panel, rect: Rect, hit: bool) {
    let s = p.scale;
    painter.rect_filled(rect.translate(vec2(0.0, 2.0 * s)), 3.0 * s, Color32::from_rgb(0x4e, 0x51, 0x50));
    let face = if hit { rect.translate(vec2(0.0, 1.5 * s)) } else { rect };
    painter.rect_filled(face, 3.0 * s, PAD_EDGE);
    painter.rect_filled(face.shrink(2.5 * s), 2.0 * s, if hit { PAD_HIT } else { PAD });
}

/// Rotary knob; `value` is 0..1 from MIN to MAX.
fn knob(painter: &Painter, p: &Panel, center: Pos2, radius: f32, cap: Color32, value: f32) {
    let s = p.scale;
    let tick = Stroke::new(s, INK);
    for k in 0..11 {
        let a = PI * 0.75 + k as f32 * PI * 0.15;
        let dir = vec2(a.cos(), a.sin());
        painter.line_segment([center + dir * (radius + 3.0) * s, center + dir * (radius + 6.0) * s], tick);
    }
    painter.circle_filled(center + vec2(0.0, 2.0 * s), radius * s, SHADOW);
    painter.circle_filled(center, radius * s, DARK);
    painter.circle_filled(center, radius * 0.72 * s, cap);
    let a = PI * 0.75 + value * PI * 1.5;
    let dir = vec2(a.cos(), a.sin());
    painter
        .line_segment([center + dir * radius * 0.2 * s, center + dir * radius * 0.7 * s], Stroke::new(2.0 * s, DARK));
}

/// The DATA wheel, turned by `angle` radians.
fn wheel(painter: &Painter, p: &Panel, center: Pos2, radius: f32, angle: f32) {
    let s = p.scale;
    painter.circle_filled(center + vec2(0.0, 3.0 * s), radius * s, SHADOW);
    painter.circle_filled(center, radius * s, DARK);
    for k in 0..16 {
        let a = angle + k as f32 * PI / 8.0;
        let dir = vec2(a.cos(), a.sin());
        let stroke = Stroke::new(1.5 * s, Color32::from_gray(0x70));
        painter.line_segment([center + dir * (radius - 12.0) * s, center + dir * (radius - 5.0) * s], stroke);
    }
    painter.circle_filled(center, (radius - 18.0) * s, Color32::from_rgb(0x3b, 0x3c, 0x3f));
    painter.circle_stroke(center, (radius - 18.0) * s, Stroke::new(s, Color32::from_gray(0x55)));
    let a = angle + 0.9;
    let dimple = center + vec2(a.cos(), a.sin()) * 23.0 * s;
    painter.circle_filled(dimple, 9.0 * s, DARK);
    painter.circle_stroke(dimple, 9.0 * s, Stroke::new(s, Color32::from_gray(0x5a)));
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1000.0, 730.0]).with_min_inner_size([650.0, 475.0]),
        ..Default::default()
    };
    eframe::run_native("rsampler", options, Box::new(|_| Ok(Box::new(App::new()))))
}
