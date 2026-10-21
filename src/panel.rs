//! Drawing the front panel: colours, the design-unit grid and the hardware controls.

use std::f32::consts::PI;

use eframe::egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, vec2};

/// Panel size in design units; everything is drawn in these and scaled to the window.
pub const W: f32 = 1000.0;
pub const H: f32 = 730.0;

pub const BACKDROP: Color32 = Color32::from_rgb(0x1c, 0x1d, 0x20);
pub const BODY: Color32 = Color32::from_rgb(0xdc, 0xd9, 0xcf);
pub const BODY_EDGE: Color32 = Color32::from_rgb(0xb4, 0xb0, 0xa5);
pub const INK: Color32 = Color32::from_rgb(0x3a, 0x3b, 0x3d);
pub const SHADOW: Color32 = Color32::from_rgb(0x9e, 0x9b, 0x92);
pub const PANEL: Color32 = Color32::from_rgb(0xa6, 0xad, 0xae);
pub const PANEL_INSET: Color32 = Color32::from_rgb(0xd9, 0xda, 0xd5);
pub const HOUSING: Color32 = Color32::from_rgb(0x2e, 0x30, 0x32);
pub const GLASS: Color32 = Color32::from_rgb(0x12, 0x20, 0x3a);
pub const LCD_BG: Color32 = Color32::from_rgb(0x3f, 0xb4, 0xc4);
pub const LCD_INK: Color32 = Color32::from_rgb(0x0f, 0x2f, 0x5a);
pub const LOGO: Color32 = Color32::from_rgb(0xc8, 0x23, 0x2c);
pub const DARK: Color32 = Color32::from_rgb(0x2a, 0x2b, 0x2d);
pub const RED_ON: Color32 = Color32::from_rgb(0xff, 0x3b, 0x2f);
pub const RED_OFF: Color32 = Color32::from_rgb(0x6b, 0x2a, 0x26);
pub const GREEN_ON: Color32 = Color32::from_rgb(0x8e, 0xf0, 0x6a);
pub const GREEN_OFF: Color32 = Color32::from_rgb(0x2f, 0x4a, 0x2a);
const PAD: Color32 = Color32::from_rgb(0x85, 0x88, 0x87);
const PAD_EDGE: Color32 = Color32::from_rgb(0x5f, 0x62, 0x61);
const PAD_HIT: Color32 = Color32::from_rgb(0x6c, 0x6f, 0x6e);

pub struct KeyStyle {
    pub face: Color32,
    pressed: Color32,
    edge: Color32,
}

pub const WHITE: KeyStyle = KeyStyle {
    face: Color32::from_rgb(0xf1, 0xef, 0xe8),
    pressed: Color32::from_rgb(0xd6, 0xd3, 0xca),
    edge: Color32::from_rgb(0x9d, 0x9a, 0x92),
};
pub const BLUE: KeyStyle = KeyStyle {
    face: Color32::from_rgb(0x4d, 0x7f, 0xb8),
    pressed: Color32::from_rgb(0x3d, 0x6a, 0x9c),
    edge: Color32::from_rgb(0x2c, 0x4d, 0x72),
};
pub const RED: KeyStyle = KeyStyle {
    face: Color32::from_rgb(0xe8, 0x6a, 0x5e),
    pressed: Color32::from_rgb(0xc9, 0x55, 0x4a),
    edge: Color32::from_rgb(0x8f, 0x3a, 0x32),
};
pub const ORANGE: KeyStyle = KeyStyle {
    face: Color32::from_rgb(0xf0, 0x8a, 0x2c),
    pressed: Color32::from_rgb(0xd0, 0x72, 0x1e),
    edge: Color32::from_rgb(0x9a, 0x55, 0x18),
};

/// Maps design units to screen coordinates.
pub struct Panel {
    pub origin: Pos2,
    pub scale: f32,
}

impl Panel {
    pub fn pos(&self, x: f32, y: f32) -> Pos2 {
        self.origin + vec2(x, y) * self.scale
    }

    pub fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(self.pos(x, y), vec2(w, h) * self.scale)
    }

    pub fn prop(&self, size: f32) -> FontId {
        FontId::proportional(size * self.scale)
    }

    /// Printed panel legend, centered on (x, y).
    pub fn label(&self, painter: &Painter, x: f32, y: f32, text: &str, size: f32) {
        painter.text(self.pos(x, y), Align2::CENTER_CENTER, text, self.prop(size), INK);
    }

    /// Section title with a rule on each side, like "LOCATE" or "PAD BANK".
    pub fn section(&self, painter: &Painter, x0: f32, x1: f32, y: f32, text: &str) {
        let mid = (x0 + x1) / 2.0;
        let gap = text.len() as f32 * 3.6 + 8.0;
        let stroke = Stroke::new(self.scale, INK);
        painter.hline(self.pos(x0, y).x..=self.pos(mid - gap, y).x, self.pos(0.0, y).y, stroke);
        painter.hline(self.pos(mid + gap, y).x..=self.pos(x1, y).x, self.pos(0.0, y).y, stroke);
        self.label(painter, mid, y, text, 8.0);
    }
}

/// A hardware button; `held` keeps it drawn pressed while its keyboard shortcut is down.
pub fn key(ui: &mut Ui, p: &Panel, rect: Rect, style: &KeyStyle, glyph: &str, held: bool) -> Response {
    let s = p.scale;
    let id = ui.scope_id().with(((rect.min.x / s) as i32, (rect.min.y / s) as i32));
    let resp = ui.interact(rect, id, Sense::click());
    let down = held || resp.is_pointer_button_down_on();
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

pub fn led(painter: &Painter, p: &Panel, x: f32, y: f32, on: bool, lit: Color32, unlit: Color32) {
    let c = p.pos(x, y);
    if on {
        painter.circle_filled(c, 6.0 * p.scale, lit.gamma_multiply(0.3));
    }
    painter.circle_filled(c, 2.6 * p.scale, if on { lit } else { unlit });
}

pub fn pad(painter: &Painter, p: &Panel, rect: Rect, hit: bool) {
    let s = p.scale;
    painter.rect_filled(rect.translate(vec2(0.0, 2.0 * s)), 3.0 * s, Color32::from_rgb(0x4e, 0x51, 0x50));
    let face = if hit { rect.translate(vec2(0.0, 1.5 * s)) } else { rect };
    painter.rect_filled(face, 3.0 * s, PAD_EDGE);
    painter.rect_filled(face.shrink(2.5 * s), 2.0 * s, if hit { PAD_HIT } else { PAD });
}

/// Rotary knob; `value` is 0..1 from MIN to MAX.
pub fn knob(painter: &Painter, p: &Panel, center: Pos2, radius: f32, cap: Color32, value: f32) {
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
pub fn wheel(painter: &Painter, p: &Panel, center: Pos2, radius: f32, angle: f32) {
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

/// The NOTE VARIATION fader; `value` is 0..1 from bottom to top.
pub fn fader(painter: &Painter, p: &Panel, value: f32) {
    let s = p.scale;
    let tick = Stroke::new(s, INK);
    for k in 0..11 {
        let y = p.pos(0.0, 520.0 + k as f32 * 14.0).y;
        painter.hline(p.pos(58.0, 0.0).x..=p.pos(74.0, 0.0).x, y, tick);
        painter.hline(p.pos(96.0, 0.0).x..=p.pos(112.0, 0.0).x, y, tick);
    }
    painter.rect_filled(p.rect(82.0, 516.0, 6.0, 148.0), 2.0 * s, DARK);
    let y = 640.0 - value * 130.0;
    painter.rect_filled(p.rect(58.0, y + 2.0, 54.0, 20.0), 2.0 * s, SHADOW);
    painter.rect_filled(p.rect(58.0, y, 54.0, 20.0), 2.0 * s, WHITE.face);
    painter.rect_filled(p.rect(58.0, y + 8.0, 54.0, 4.0), 0.0, BLUE.face);
}
