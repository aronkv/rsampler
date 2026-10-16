use eframe::egui::{
    self, Align2, Color32, FontId, Key, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2,
};

/// Panel size in design units; everything is drawn in these and scaled to the window.
const W: f32 = 980.0;
const H: f32 = 620.0;

const BACKDROP: Color32 = Color32::from_rgb(0x1c, 0x1d, 0x20);
const BODY: Color32 = Color32::from_rgb(0xcf, 0xcc, 0xc4);
const INK: Color32 = Color32::from_rgb(0x2a, 0x2b, 0x2e);
const INK_SOFT: Color32 = Color32::from_rgb(0x6e, 0x6c, 0x66);
const SHADOW: Color32 = Color32::from_rgb(0x9c, 0x99, 0x92);
const BEZEL: Color32 = Color32::from_rgb(0x2b, 0x2d, 0x30);
const LCD_BG: Color32 = Color32::from_rgb(0x9d, 0xb4, 0xc0);
const LCD_INK: Color32 = Color32::from_rgb(0x1e, 0x2b, 0x33);
const LED_ON: Color32 = Color32::from_rgb(0xff, 0x3b, 0x2f);
const LED_OFF: Color32 = Color32::from_rgb(0x6b, 0x2a, 0x26);
const PAD: Color32 = Color32::from_rgb(0x36, 0x37, 0x3a);
const PAD_TOP: Color32 = Color32::from_rgb(0x41, 0x42, 0x46);
const PAD_HIT: Color32 = Color32::from_rgb(0x5a, 0x45, 0x40);

struct ButtonStyle {
    face: Color32,
    pressed: Color32,
    edge: Color32,
    text: Color32,
}

const LIGHT: ButtonStyle = ButtonStyle {
    face: Color32::from_rgb(0xe2, 0xdf, 0xd8),
    pressed: Color32::from_rgb(0xc6, 0xc2, 0xba),
    edge: Color32::from_rgb(0xa9, 0xa5, 0x9c),
    text: INK,
};

const DARK: ButtonStyle = ButtonStyle {
    face: Color32::from_rgb(0x4a, 0x4c, 0x50),
    pressed: Color32::from_rgb(0x3a, 0x3c, 0x40),
    edge: Color32::from_rgb(0x2a, 0x2b, 0x2e),
    text: Color32::from_rgb(0xe8, 0xe6, 0xe0),
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
const SOFT_KEYS: [&str; 6] = ["T/C", "TR.ON", "SOLO", "TR-", "TR+", "OPEN"];
const MODES: [&str; 12] =
    ["SONG", "MISC", "LOAD", "SAMPLE", "TRIM", "PROGRAM", "SETUP", "MIXER", "OTHER", "MAIN", "WINDOW", "SHIFT"];

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

    fn font(&self, size: f32) -> FontId {
        FontId::monospace(size * self.scale)
    }
}

#[derive(Default)]
struct App {
    bank: usize,
    last_pad: Option<usize>,
    playing: bool,
    rec: bool,
    overdub: bool,
    full_level: bool,
    sixteen_levels: bool,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(BACKDROP)).show(ui, |ui| self.panel(ui));
    }
}

impl App {
    fn panel(&mut self, ui: &mut Ui) {
        let area = ui.max_rect();
        let scale = (area.width() / W).min(area.height() / H);
        let p = Panel { origin: area.center() - vec2(W, H) * scale / 2.0, scale };
        let s = scale;

        let painter = ui.painter().clone();
        painter.rect_filled(p.rect(0.0, 0.0, W, H), 14.0 * s, BODY);
        painter.text(p.pos(40.0, 36.0), Align2::LEFT_CENTER, "rsampler", p.font(22.0), INK);
        painter.text(p.pos(940.0, 36.0), Align2::RIGHT_CENTER, "SAMPLER · SEQUENCER", p.font(11.0), INK_SOFT);
        painter.hline(p.pos(40.0, 0.0).x..=p.pos(940.0, 0.0).x, p.pos(0.0, 60.0).y, Stroke::new(s, SHADOW));

        self.lcd(&painter, &p);

        for i in 0..SOFT_KEYS.len() {
            let rect = p.rect(62.0 + i as f32 * 78.0, 240.0, 66.0, 26.0);
            button(ui, &p, rect, &format!("F{}", i + 1), &LIGHT, None);
        }

        for (i, label) in MODES.iter().enumerate() {
            let (col, row) = ((i % 3) as f32, (i / 3) as f32);
            button(ui, &p, p.rect(56.0 + col * 96.0, 300.0 + row * 46.0, 84.0, 34.0), label, &LIGHT, None);
        }

        wheel(&painter, &p, p.pos(440.0, 386.0), 64.0);

        let transport = [("REC", Some(self.rec)), ("OVERDUB", Some(self.overdub)), ("STOP", None)];
        for (i, (label, led)) in transport.into_iter().enumerate() {
            if button(ui, &p, p.rect(56.0 + i as f32 * 94.0, 520.0, 84.0, 40.0), label, &DARK, led).clicked() {
                match i {
                    0 => self.rec = !self.rec,
                    1 => self.overdub = !self.overdub,
                    _ => (self.playing, self.rec, self.overdub) = (false, false, false),
                }
            }
        }
        for (i, label) in ["PLAY", "PLAY START"].into_iter().enumerate() {
            let led = (i == 0).then_some(self.playing);
            if button(ui, &p, p.rect(338.0 + i as f32 * 94.0, 520.0, 84.0, 40.0), label, &DARK, led).clicked() {
                self.playing = true;
            }
        }

        for (i, name) in BANKS.iter().enumerate() {
            let rect = p.rect(584.0 + i as f32 * 92.0, 100.0, 80.0, 30.0);
            if button(ui, &p, rect, name, &LIGHT, Some(self.bank == i)).clicked() {
                self.bank = i;
            }
        }

        for (n, key) in PAD_KEYS.iter().enumerate() {
            let (col, row) = ((n % 4) as f32, (3 - n / 4) as f32);
            let rect = p.rect(584.0 + col * 92.0, 150.0 + row * 92.0, 80.0, 80.0);
            let resp = ui.interact(rect, ui.scope_id().with(("pad", n)), Sense::click());
            let hit = resp.is_pointer_button_down_on() || ui.input(|i| i.key_down(*key));
            if hit {
                self.last_pad = Some(n);
            }
            pad(&painter, &p, rect, &format!("{}{:02}", BANKS[self.bank], n + 1), key.name(), hit);
        }

        let levels = p.rect(584.0, 520.0, 172.0, 40.0);
        if button(ui, &p, levels, "FULL LEVEL", &LIGHT, Some(self.full_level)).clicked() {
            self.full_level = !self.full_level;
        }
        let levels = levels.translate(vec2(184.0 * s, 0.0));
        if button(ui, &p, levels, "16 LEVELS", &LIGHT, Some(self.sixteen_levels)).clicked() {
            self.sixteen_levels = !self.sixteen_levels;
        }
    }

    fn lcd(&self, painter: &egui::Painter, p: &Panel) {
        let s = p.scale;
        painter.rect_filled(p.rect(40.0, 76.0, 500.0, 152.0), 8.0 * s, BEZEL);
        let screen = p.rect(56.0, 88.0, 468.0, 128.0);
        painter.rect_filled(screen, 2.0 * s, LCD_BG);

        let status = match (self.playing, self.rec || self.overdub) {
            (true, true) => "REC",
            (true, false) => "PLAY",
            _ => "STOP",
        };
        let pad = self.last_pad.map_or("---".to_owned(), |n| format!("{}{:02}", BANKS[self.bank], n + 1));
        let lines = [
            format!("Sq:01-Sequence01        {status:>4}"),
            "Tr:01-Track-01        BPM:120.0".to_owned(),
            format!("Pad:{pad} (empty)       Bank:{}", BANKS[self.bank]),
            "Now:001.01.00          Loop:ON".to_owned(),
        ];
        for (i, line) in lines.iter().enumerate() {
            painter.text(p.pos(66.0, 96.0 + i as f32 * 21.0), Align2::LEFT_TOP, line, p.font(14.0), LCD_INK);
        }

        for (i, label) in SOFT_KEYS.iter().enumerate() {
            let tab = p.rect(62.0 + i as f32 * 78.0, 196.0, 66.0, 16.0);
            painter.rect_filled(tab, 0.0, LCD_INK);
            painter.text(tab.center(), Align2::CENTER_CENTER, *label, p.font(10.0), LCD_BG);
        }
    }
}

fn button(ui: &mut Ui, p: &Panel, rect: Rect, label: &str, style: &ButtonStyle, led: Option<bool>) -> Response {
    let s = p.scale;
    let resp = ui.interact(rect, ui.scope_id().with(label), Sense::click());
    let down = resp.is_pointer_button_down_on();
    let face = if down { rect.translate(vec2(0.0, s)) } else { rect };
    let painter = ui.painter();

    if !down {
        painter.rect_filled(rect.translate(vec2(0.0, 2.0 * s)), 5.0 * s, SHADOW);
    }
    painter.rect_filled(face, 5.0 * s, if down { style.pressed } else { style.face });
    painter.rect_stroke(face, 5.0 * s, Stroke::new(s, style.edge), StrokeKind::Inside);

    let mut text_at = face.center();
    if let Some(on) = led {
        let c = pos2(face.left() + 10.0 * s, face.top() + 9.0 * s);
        if on {
            painter.circle_filled(c, 7.0 * s, LED_ON.gamma_multiply(0.25));
        }
        painter.circle_filled(c, 3.5 * s, if on { LED_ON } else { LED_OFF });
        text_at.y += 3.0 * s;
    }
    painter.text(text_at, Align2::CENTER_CENTER, label, p.font(11.0), style.text);
    resp
}

fn pad(painter: &egui::Painter, p: &Panel, rect: Rect, name: &str, key: &str, hit: bool) {
    let s = p.scale;
    painter.rect_filled(rect.translate(vec2(0.0, 3.0 * s)), 8.0 * s, Color32::from_rgb(0x22, 0x23, 0x25));
    let face = if hit { rect.translate(vec2(0.0, 2.0 * s)) } else { rect };
    painter.rect_filled(face, 8.0 * s, PAD);
    painter.rect_filled(face.shrink(5.0 * s), 6.0 * s, if hit { PAD_HIT } else { PAD_TOP });
    painter.text(face.left_bottom() + vec2(9.0, -8.0) * s, Align2::LEFT_BOTTOM, name, p.font(10.0), INK_SOFT);
    painter.text(face.right_top() + vec2(-9.0, 8.0) * s, Align2::RIGHT_TOP, key, p.font(9.0), INK_SOFT);
}

fn wheel(painter: &egui::Painter, p: &Panel, center: Pos2, radius: f32) {
    let s = p.scale;
    painter.circle_filled(center + vec2(0.0, 3.0 * s), radius * s, SHADOW);
    painter.circle_filled(center, radius * s, BEZEL);
    painter.circle_filled(center, (radius - 6.0) * s, Color32::from_rgb(0x3a, 0x3c, 0x40));
    painter.circle_filled(center + vec2(22.0, -22.0) * s, 9.0 * s, BEZEL);
    painter.text(center + vec2(0.0, radius + 14.0) * s, Align2::CENTER_CENTER, "DATA", p.font(10.0), INK_SOFT);
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([980.0, 620.0]).with_min_inner_size([640.0, 405.0]),
        ..Default::default()
    };
    eframe::run_native("rsampler", options, Box::new(|_| Ok(Box::<App>::default())))
}
