//! A synchronous, host-owned graphics session. Native windows never enter Jett
//! values; dropping the session also drops its window on the owning thread.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use font8x8::UnicodeFonts;
use minifb::{InputCallback, Scale, Window, WindowOptions};

pub const MAX_DIMENSION: i64 = 2048;
pub const MAX_RECTANGLES: usize = 10_000;
pub const MAX_TEXT_ITEMS: usize = 1_024;
pub const MAX_TEXT_BYTES: usize = 16_384;
pub const MAX_TITLE_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub title: String,
    pub width: i64,
    pub height: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub red: i64,
    pub green: i64,
    pub blue: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rect {
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
    pub color: Color,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text {
    pub x: i64,
    pub y: i64,
    pub text: String,
    pub scale: i64,
    pub color: Color,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scene {
    pub background: Color,
    pub rectangles: Vec<Rect>,
    pub texts: Vec<Text>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    W,
    A,
    S,
    D,
    U,
    Z,
    R,
    N,
    P,
    Enter,
    Space,
}

pub fn validate_config(config: &Config) -> Result<(), String> {
    dimensions(config.width, config.height)?;
    if config.title.len() > MAX_TITLE_BYTES || config.title.contains('\0') {
        return Err(format!(
            "graphics.run: window title must have at most {MAX_TITLE_BYTES} bytes and no NUL"
        ));
    }
    Ok(())
}

fn dimensions(width: i64, height: i64) -> Result<(usize, usize), String> {
    if !(1..=MAX_DIMENSION).contains(&width) || !(1..=MAX_DIMENSION).contains(&height) {
        return Err(format!(
            "graphics.run: window dimensions must be in 1..{MAX_DIMENSION}"
        ));
    }
    let width = usize::try_from(width).map_err(|_| "graphics.run: invalid width".to_string())?;
    let height = usize::try_from(height).map_err(|_| "graphics.run: invalid height".to_string())?;
    Ok((width, height))
}

fn pixel(color: Color) -> Result<u32, String> {
    fn channel(value: i64) -> Result<u32, String> {
        u8::try_from(value)
            .map(u32::from)
            .map_err(|_| "graphics.run: color channels must be in 0..255".to_string())
    }
    Ok((channel(color.red)? << 16) | (channel(color.green)? << 8) | channel(color.blue)?)
}

fn validate_rect(rect: &Rect) -> Result<(), String> {
    if rect.width < 0 || rect.height < 0 {
        return Err("graphics.run: rectangle dimensions must be nonnegative".to_string());
    }
    rect.x
        .checked_add(rect.width)
        .ok_or_else(|| "graphics.run: rectangle coordinate overflow".to_string())?;
    rect.y
        .checked_add(rect.height)
        .ok_or_else(|| "graphics.run: rectangle coordinate overflow".to_string())?;
    pixel(rect.color)?;
    Ok(())
}

fn validate_text(text: &Text) -> Result<(), String> {
    if !(1..=8).contains(&text.scale) {
        return Err("graphics.run: text scale must be in 1..8".to_string());
    }
    if text.text.len() > MAX_TEXT_BYTES {
        return Err(format!(
            "graphics.run: each text item must have at most {MAX_TEXT_BYTES} bytes"
        ));
    }
    // Byte length is a conservative upper bound for both line count and cells
    // in a row, so every later text coordinate is proven to fit here.
    let length = i64::try_from(text.text.len())
        .map_err(|_| "graphics.run: text length overflow".to_string())?;
    let extent = (length + 1) * 8 * text.scale;
    text.x
        .checked_add(extent)
        .ok_or_else(|| "graphics.run: text coordinate overflow".to_string())?;
    text.y
        .checked_add(extent)
        .ok_or_else(|| "graphics.run: text coordinate overflow".to_string())?;
    pixel(text.color)?;
    Ok(())
}

/// Validate and rasterize without creating a window, also used by scripted
/// interpreter providers and rendering tests.
pub fn render_scene(width: i64, height: i64, scene: &Scene) -> Result<Vec<u32>, String> {
    let (width, height) = dimensions(width, height)?;
    if scene.rectangles.len() > MAX_RECTANGLES || scene.texts.len() > MAX_TEXT_ITEMS {
        return Err(format!(
            "graphics.run: scenes allow at most {MAX_RECTANGLES} rectangles and {MAX_TEXT_ITEMS} text items"
        ));
    }
    let background = pixel(scene.background)?;
    for rect in &scene.rectangles {
        validate_rect(rect)?;
    }
    for text in &scene.texts {
        validate_text(text)?;
    }
    let mut canvas = Canvas {
        pixels: vec![background; width * height],
        width,
        height,
    };
    for rect in &scene.rectangles {
        canvas.rect(rect, pixel(rect.color)?);
    }
    for text in &scene.texts {
        canvas.text(text, pixel(text.color)?);
    }
    Ok(canvas.pixels)
}

struct Canvas {
    pixels: Vec<u32>,
    width: usize,
    height: usize,
}

impl Canvas {
    fn rect(&mut self, rect: &Rect, color: u32) {
        // All coordinates and extents were validated before rasterization.
        let left = clip(rect.x, self.width);
        let top = clip(rect.y, self.height);
        let right = clip(rect.x + rect.width, self.width);
        let bottom = clip(rect.y + rect.height, self.height);
        for row in top..bottom {
            self.pixels[row * self.width + left..row * self.width + right].fill(color);
        }
    }

    fn text(&mut self, text: &Text, color: u32) {
        let mut x = text.x;
        let mut y = text.y;
        for character in text.text.chars() {
            if character == '\n' {
                x = text.x;
                y += 8 * text.scale;
                continue;
            }
            let glyph = font8x8::BASIC_FONTS
                .get(character)
                .or_else(|| font8x8::BASIC_FONTS.get('?'))
                .unwrap_or([0; 8]);
            self.glyph(x, y, text.scale, &glyph, color);
            x += 8 * text.scale;
        }
    }

    fn glyph(&mut self, x: i64, y: i64, scale: i64, glyph: &[u8; 8], color: u32) {
        for row in 0..8_u8 {
            let bits = glyph[usize::from(row)];
            for column in 0..8_u8 {
                if bits & (1_u8 << column) == 0 {
                    continue;
                }
                self.rect(
                    &Rect {
                        x: x + i64::from(column) * scale,
                        y: y + i64::from(row) * scale,
                        width: scale,
                        height: scale,
                        color: Color {
                            red: 0,
                            green: 0,
                            blue: 0,
                        },
                    },
                    color,
                );
            }
        }
    }
}

fn clip(coordinate: i64, limit: usize) -> usize {
    if coordinate <= 0 {
        return 0;
    }
    usize::try_from(coordinate).unwrap_or(limit).min(limit)
}

/// Native window owner. It stays on the thread which creates it. There is no
/// clone or exposed native window handle, and every error exit uses Rust Drop.
pub struct Session {
    window: Window,
    width: usize,
    height: usize,
    pixels: Vec<u32>,
    input: Rc<RefCell<InputState>>,
    closed: bool,
}

#[derive(Default)]
struct InputState {
    held: Vec<minifb::Key>,
    pending: VecDeque<Key>,
    escape: bool,
}

impl InputState {
    fn key_changed(&mut self, key: minifb::Key, pressed: bool) {
        if !pressed {
            self.held.retain(|held| *held != key);
            return;
        }
        if key == minifb::Key::Escape {
            self.escape = true;
            return;
        }
        if let Some(portable) = portable_key(key)
            && !self.held.contains(&key)
        {
            self.held.push(key);
            self.pending.push_back(portable);
        }
    }
}

struct InputReceiver(Rc<RefCell<InputState>>);

impl InputCallback for InputReceiver {
    fn add_char(&mut self, _character: u32) {}

    fn set_key_state(&mut self, key: minifb::Key, pressed: bool) {
        self.0.borrow_mut().key_changed(key, pressed);
    }
}

impl Session {
    pub fn new(config: Config) -> Result<Self, String> {
        validate_config(&config)?;
        let (width, height) = dimensions(config.width, config.height)?;
        let mut window = Window::new(
            &config.title,
            width,
            height,
            WindowOptions {
                resize: false,
                scale: Scale::X1,
                ..WindowOptions::default()
            },
        )
        .map_err(|error| format!("graphics.run: could not open window: {error}"))?;
        window.set_target_fps(60);
        let input = Rc::new(RefCell::new(InputState::default()));
        window.set_input_callback(Box::new(InputReceiver(Rc::clone(&input))));
        Ok(Self {
            window,
            width,
            height,
            pixels: vec![0; width * height],
            input,
            closed: false,
        })
    }

    pub fn present(&mut self, scene: &Scene) -> Result<(), String> {
        let width = i64::try_from(self.width)
            .map_err(|_| "graphics.run: invalid viewport width".to_string())?;
        let height = i64::try_from(self.height)
            .map_err(|_| "graphics.run: invalid viewport height".to_string())?;
        self.pixels = render_scene(width, height, scene)?;
        self.pump()
    }

    pub fn next_key(&mut self) -> Result<Option<Key>, String> {
        loop {
            if self.closed {
                return Ok(None);
            }
            if let Some(key) = self.input.borrow_mut().pending.pop_front() {
                return Ok(Some(key));
            }
            self.pump()?;
        }
    }

    fn pump(&mut self) -> Result<(), String> {
        if self.closed || !self.window.is_open() {
            self.closed = true;
            return Ok(());
        }
        self.window
            .update_with_buffer(&self.pixels, self.width, self.height)
            .map_err(|error| format!("graphics.run: could not present window: {error}"))?;
        let mut input = self.input.borrow_mut();
        if !self.window.is_open() || input.escape {
            self.closed = true;
            input.pending.clear();
            return Ok(());
        }
        // A key may be released while another app has focus. Do not leave that
        // key permanently marked as held when this window receives focus again.
        if !self.window.is_active() {
            input.held.clear();
        }
        Ok(())
    }
}

fn portable_key(key: minifb::Key) -> Option<Key> {
    Some(match key {
        minifb::Key::Up => Key::Up,
        minifb::Key::Down => Key::Down,
        minifb::Key::Left => Key::Left,
        minifb::Key::Right => Key::Right,
        minifb::Key::W => Key::W,
        minifb::Key::A => Key::A,
        minifb::Key::S => Key::S,
        minifb::Key::D => Key::D,
        minifb::Key::U => Key::U,
        minifb::Key::Z => Key::Z,
        minifb::Key::R => Key::R,
        minifb::Key::N => Key::N,
        minifb::Key::P => Key::P,
        minifb::Key::Enter => Key::Enter,
        minifb::Key::Space => Key::Space,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLACK: Color = Color {
        red: 0,
        green: 0,
        blue: 0,
    };
    const WHITE: Color = Color {
        red: 255,
        green: 255,
        blue: 255,
    };

    fn scene(rectangles: Vec<Rect>, texts: Vec<Text>) -> Scene {
        Scene {
            background: BLACK,
            rectangles,
            texts,
        }
    }

    #[test]
    fn graphics_rejects_invalid_windows_before_host_creation() {
        for (width, height) in [(0, 100), (100, -1), (2049, 100), (i64::MAX, i64::MAX)] {
            assert!(
                validate_config(&Config {
                    title: "test".into(),
                    width,
                    height
                })
                .is_err()
            );
        }
        assert!(
            validate_config(&Config {
                title: "bad\0title".into(),
                width: 1,
                height: 1
            })
            .is_err()
        );
    }

    #[test]
    fn graphics_clips_rectangles_and_preserves_draw_order() {
        let red = Color {
            red: 255,
            green: 0,
            blue: 0,
        };
        let image = render_scene(
            3,
            2,
            &scene(
                vec![
                    Rect {
                        x: -1,
                        y: -1,
                        width: 3,
                        height: 3,
                        color: WHITE,
                    },
                    Rect {
                        x: 1,
                        y: 1,
                        width: 50,
                        height: 50,
                        color: red,
                    },
                    Rect {
                        x: i64::MIN,
                        y: 0,
                        width: 1,
                        height: 1,
                        color: WHITE,
                    },
                ],
                vec![],
            ),
        )
        .unwrap();
        assert_eq!(
            image,
            vec![0xffffff, 0xffffff, 0, 0xffffff, 0xff0000, 0xff0000]
        );
    }

    #[test]
    fn graphics_rejects_invalid_scene_before_rasterizing() {
        let invalid_rects = [
            Rect {
                x: 0,
                y: 0,
                width: -1,
                height: 1,
                color: WHITE,
            },
            Rect {
                x: i64::MAX,
                y: 0,
                width: 1,
                height: 1,
                color: WHITE,
            },
            Rect {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
                color: Color {
                    red: 256,
                    green: 0,
                    blue: 0,
                },
            },
        ];
        for rect in invalid_rects {
            assert!(render_scene(1, 1, &scene(vec![rect], vec![])).is_err());
        }
        for scale in [0, 9] {
            assert!(
                render_scene(
                    1,
                    1,
                    &scene(
                        vec![],
                        vec![Text {
                            x: 0,
                            y: 0,
                            text: "A".into(),
                            scale,
                            color: WHITE,
                        }]
                    )
                )
                .is_err()
            );
        }
        assert!(
            render_scene(
                1,
                1,
                &scene(
                    vec![],
                    vec![Text {
                        x: i64::MAX,
                        y: 0,
                        text: "A".into(),
                        scale: 1,
                        color: WHITE,
                    }]
                )
            )
            .is_err()
        );
    }

    #[test]
    fn graphics_bitmap_text_scales_and_replaces_unsupported_glyphs() {
        let render = |text: &str, scale| {
            render_scene(
                32,
                32,
                &scene(
                    vec![],
                    vec![Text {
                        x: 0,
                        y: 0,
                        text: text.into(),
                        scale,
                        color: WHITE,
                    }],
                ),
            )
            .unwrap()
        };
        assert_eq!(render("?", 1), render("\u{1f4e6}", 1));
        let small = render("A", 1);
        let large = render("A", 2);
        assert!(small.contains(&0xffffff));
        for y in 0..8 {
            for x in 0..8 {
                for dy in 0..2 {
                    for dx in 0..2 {
                        assert_eq!(small[y * 32 + x], large[(y * 2 + dy) * 32 + x * 2 + dx]);
                    }
                }
            }
        }
    }

    #[test]
    fn graphics_retains_short_presses_between_frames_in_arrival_order() {
        let mut input = InputState::default();
        input.key_changed(minifb::Key::Right, true);
        input.key_changed(minifb::Key::Right, false);
        input.key_changed(minifb::Key::Up, true);
        input.key_changed(minifb::Key::Up, false);
        assert_eq!(input.pending, VecDeque::from([Key::Right, Key::Up]));
        assert!(input.held.is_empty());
    }

    #[test]
    fn graphics_suppresses_held_key_repeat_and_remembers_escape() {
        let mut input = InputState::default();
        for _ in 0..10 {
            input.key_changed(minifb::Key::R, true);
        }
        assert_eq!(input.pending, VecDeque::from([Key::R]));
        input.key_changed(minifb::Key::R, false);
        input.key_changed(minifb::Key::R, true);
        assert_eq!(input.pending, VecDeque::from([Key::R, Key::R]));
        input.key_changed(minifb::Key::Escape, true);
        input.key_changed(minifb::Key::Escape, false);
        assert!(input.escape);
    }
}
