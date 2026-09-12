use std::collections::VecDeque;

use jett_runtime::graphics::{self, Color, Config, Key, Rect, Scene, Text};

use super::{Interpreter, Value};

/// Input supplied by a deterministic graphics provider. It never opens a window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphicsTestEvent {
    Key(Key),
    Close,
    HostError(String),
}

/// Observable graphics operations recorded by a deterministic provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphicsTestObservation {
    Opened(Config),
    Presented(Scene),
    Key(Key),
    Closed,
}

pub(super) enum GraphicsProvider {
    Native,
    Scripted(GraphicsScript),
}

pub(super) struct GraphicsScript {
    events: VecDeque<GraphicsTestEvent>,
    observations: Vec<GraphicsTestObservation>,
}

enum GraphicsSession<'a> {
    Native(graphics::Session),
    Scripted {
        script: &'a mut GraphicsScript,
        width: i64,
        height: i64,
    },
}

impl GraphicsProvider {
    fn open(&mut self, config: Config) -> Result<GraphicsSession<'_>, String> {
        match self {
            Self::Native => graphics::Session::new(config).map(GraphicsSession::Native),
            Self::Scripted(script) => {
                let width = config.width;
                let height = config.height;
                script
                    .observations
                    .push(GraphicsTestObservation::Opened(config));
                Ok(GraphicsSession::Scripted {
                    script,
                    width,
                    height,
                })
            }
        }
    }
}

impl GraphicsSession<'_> {
    fn present(&mut self, scene: &Scene) -> Result<(), String> {
        match self {
            Self::Native(session) => session.present(scene),
            Self::Scripted {
                script,
                width,
                height,
            } => {
                graphics::render_scene(*width, *height, scene)?;
                script
                    .observations
                    .push(GraphicsTestObservation::Presented(scene.clone()));
                Ok(())
            }
        }
    }

    fn next_key(&mut self) -> Result<Option<Key>, String> {
        match self {
            Self::Native(session) => session.next_key(),
            Self::Scripted { script, .. } => match script.events.pop_front() {
                Some(GraphicsTestEvent::Key(key)) => {
                    script.observations.push(GraphicsTestObservation::Key(key));
                    Ok(Some(key))
                }
                Some(GraphicsTestEvent::Close) => Ok(None),
                Some(GraphicsTestEvent::HostError(message)) => Err(message),
                None => Err("Graphics: test provider exhausted".to_string()),
            },
        }
    }
}

impl Drop for GraphicsSession<'_> {
    fn drop(&mut self) {
        if let Self::Scripted { script, .. } = self {
            script.observations.push(GraphicsTestObservation::Closed);
        }
    }
}

enum GraphicsFailure {
    Domain(String),
    Callback(String),
}

impl Interpreter {
    /// Authorize graphics sessions for a runtime whose main requests Graphics.
    pub fn initialize_graphics_provider(&mut self) {
        self.graphics_provider = Some(GraphicsProvider::Native);
    }

    /// Inject deterministic input and record graphics output without a display.
    pub fn set_graphics_test_events(&mut self, events: Vec<GraphicsTestEvent>) {
        self.graphics_provider = Some(GraphicsProvider::Scripted(GraphicsScript {
            events: events.into(),
            observations: Vec::new(),
        }));
    }

    pub fn take_graphics_test_observations(&mut self) -> Vec<GraphicsTestObservation> {
        match self.graphics_provider.as_mut() {
            Some(GraphicsProvider::Scripted(script)) => std::mem::take(&mut script.observations),
            _ => Vec::new(),
        }
    }

    pub fn graphics_test_events_remaining(&self) -> Option<usize> {
        match self.graphics_provider.as_ref() {
            Some(GraphicsProvider::Scripted(script)) => Some(script.events.len()),
            _ => None,
        }
    }

    pub(super) fn run_graphics_builtin(&mut self, args: Vec<Value>) -> Result<Value, String> {
        let [capability, config, initial, update, render]: [Value; 5] = args
            .try_into()
            .map_err(|_| "graphics.__run expects 5 arguments".to_string())?;
        if !matches!(capability, Value::Capability(name) if name == "Graphics") {
            return Err("graphics.__run expects a Graphics capability".to_string());
        }
        if self.graphics_session_active {
            return Ok(domain_failure(
                "graphics.run: nested sessions are unsupported",
            ));
        }
        let config = decode_config(&config)?;
        if let Err(message) = graphics::validate_config(&config) {
            return Ok(domain_failure(message));
        }
        let Some(mut provider) = self.graphics_provider.take() else {
            return Err("Graphics: runtime provider unavailable".to_string());
        };
        self.graphics_session_active = true;
        let result = self.run_graphics_session(&mut provider, config, initial, update, render);
        self.graphics_session_active = false;
        self.graphics_provider = Some(provider);
        match result {
            Ok(()) => Ok(Value::ResultOk(Box::new(Value::Nothing))),
            Err(GraphicsFailure::Domain(message)) => Ok(domain_failure(message)),
            Err(GraphicsFailure::Callback(message)) => Err(message),
        }
    }

    fn run_graphics_session(
        &mut self,
        provider: &mut GraphicsProvider,
        config: Config,
        initial: Value,
        update: Value,
        render: Value,
    ) -> Result<(), GraphicsFailure> {
        let mut state = initial;
        let initial_scene = self.graphics_scene(&render, &state)?;
        graphics::render_scene(config.width, config.height, &initial_scene)
            .map_err(GraphicsFailure::Domain)?;
        let mut session = provider.open(config).map_err(GraphicsFailure::Domain)?;
        session
            .present(&initial_scene)
            .map_err(GraphicsFailure::Domain)?;
        while let Some(key) = session.next_key().map_err(GraphicsFailure::Domain)? {
            state = self
                .call_fn_value(update.clone(), vec![state, key_value(key)])
                .map_err(GraphicsFailure::Callback)?;
            let scene = self.graphics_scene(&render, &state)?;
            session.present(&scene).map_err(GraphicsFailure::Domain)?;
        }
        Ok(())
    }

    fn graphics_scene(&mut self, render: &Value, state: &Value) -> Result<Scene, GraphicsFailure> {
        let value = self
            .call_fn_value(render.clone(), vec![state.clone()])
            .map_err(GraphicsFailure::Callback)?;
        decode_scene(&value).map_err(GraphicsFailure::Callback)
    }
}

fn domain_failure(message: impl Into<String>) -> Value {
    Value::ResultFail(Box::new(Value::String(message.into())))
}

fn key_value(key: Key) -> Value {
    let variant = match key {
        Key::Up => "up",
        Key::Down => "down",
        Key::Left => "left",
        Key::Right => "right",
        Key::W => "w",
        Key::A => "a",
        Key::S => "s",
        Key::D => "d",
        Key::U => "u",
        Key::Z => "z",
        Key::R => "r",
        Key::N => "n",
        Key::P => "p",
        Key::Enter => "enter",
        Key::Space => "space",
    };
    Value::Enum {
        type_name: "graphics.Key".to_string(),
        variant: variant.to_string(),
        fields: Vec::new(),
    }
}

fn field<'a>(value: &'a Value, expected_type: &str, name: &str) -> Result<&'a Value, String> {
    let Value::Struct { type_name, fields } = value else {
        return Err(format!("graphics: expected {expected_type}"));
    };
    if type_name != expected_type {
        return Err(format!(
            "graphics: expected {expected_type}, got {type_name}"
        ));
    }
    fields
        .iter()
        .find_map(|(field_name, value)| (field_name == name).then_some(value))
        .ok_or_else(|| format!("graphics: {expected_type} is missing {name}"))
}

fn integer(value: &Value, expected_type: &str, name: &str) -> Result<i64, String> {
    match field(value, expected_type, name)? {
        Value::Int64(value) => Ok(*value),
        _ => Err(format!("graphics: {expected_type}.{name} must be int64")),
    }
}

fn string(value: &Value, expected_type: &str, name: &str) -> Result<String, String> {
    match field(value, expected_type, name)? {
        Value::String(value) => Ok(value.clone()),
        _ => Err(format!("graphics: {expected_type}.{name} must be string")),
    }
}

fn list<'a>(value: &'a Value, expected_type: &str, name: &str) -> Result<&'a [Value], String> {
    match field(value, expected_type, name)? {
        Value::List(values) => Ok(values),
        _ => Err(format!("graphics: {expected_type}.{name} must be a list")),
    }
}

fn decode_config(value: &Value) -> Result<Config, String> {
    Ok(Config {
        title: string(value, "graphics.Config", "title")?,
        width: integer(value, "graphics.Config", "width")?,
        height: integer(value, "graphics.Config", "height")?,
    })
}

fn decode_color(value: &Value) -> Result<Color, String> {
    Ok(Color {
        red: integer(value, "graphics.Color", "red")?,
        green: integer(value, "graphics.Color", "green")?,
        blue: integer(value, "graphics.Color", "blue")?,
    })
}

fn decode_rect(value: &Value) -> Result<Rect, String> {
    Ok(Rect {
        x: integer(value, "graphics.Rect", "x")?,
        y: integer(value, "graphics.Rect", "y")?,
        width: integer(value, "graphics.Rect", "width")?,
        height: integer(value, "graphics.Rect", "height")?,
        color: decode_color(field(value, "graphics.Rect", "color")?)?,
    })
}

fn decode_text(value: &Value) -> Result<Text, String> {
    Ok(Text {
        x: integer(value, "graphics.Text", "x")?,
        y: integer(value, "graphics.Text", "y")?,
        text: string(value, "graphics.Text", "text")?,
        scale: integer(value, "graphics.Text", "scale")?,
        color: decode_color(field(value, "graphics.Text", "color")?)?,
    })
}

fn decode_scene(value: &Value) -> Result<Scene, String> {
    Ok(Scene {
        background: decode_color(field(value, "graphics.Scene", "background")?)?,
        rectangles: list(value, "graphics.Scene", "rectangles")?
            .iter()
            .map(decode_rect)
            .collect::<Result<_, _>>()?,
        texts: list(value, "graphics.Scene", "texts")?
            .iter()
            .map(decode_text)
            .collect::<Result<_, _>>()?,
    })
}

#[cfg(test)]
mod tests {
    use jett_common::{FileId, STDLIB_FILE_ID_START};
    use jett_parser::parse;

    use super::*;

    const PROGRAM: &str = r#"namespace game
function increment(value: int64) returns int64:
    return value + 1
function update(state: int64, key: graphics.Key) returns int64:
    match key:
        right:
            return increment(state)
        left:
            return state - 1
        other:
            return state
function render(view state: int64) returns graphics.Scene:
    graphics.Color background = graphics.Color(red: state, green: 0, blue: 0)
    list[graphics.Rect] rectangles = list.new[graphics.Rect]()
    list[graphics.Text] texts = list.new[graphics.Text]()
    return graphics.Scene(background: background, rectangles: rectangles, texts: texts)
function play(view display: Graphics) returns result[nothing, string]:
    graphics.Config config = graphics.Config(title: "scripted", width: 16, height: 16)
    return graphics.run[int64](view display, config, 0, update, render)
"#;

    fn interpreter(program: &str) -> Interpreter {
        let mut interpreter = Interpreter::new();
        for (index, source) in [
            include_str!("../../../../stdlib/list.jett"),
            include_str!("../../../../stdlib/graphics.jett"),
        ]
        .into_iter()
        .enumerate()
        {
            let file = FileId::new(STDLIB_FILE_ID_START + u32::try_from(index).unwrap());
            let parsed = parse(source, file);
            assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
            interpreter.register_module(&parsed.module);
        }
        let parsed = parse(program, FileId::new(1));
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        interpreter.register_module(&parsed.module);
        interpreter
    }

    fn play(interpreter: &mut Interpreter) -> Result<Value, String> {
        interpreter.call_function("game.play", vec![Value::Capability("Graphics".to_string())])
    }

    fn presented_red_channels(observations: &[GraphicsTestObservation]) -> Vec<i64> {
        observations
            .iter()
            .filter_map(|observation| match observation {
                GraphicsTestObservation::Presented(scene) => Some(scene.background.red),
                _ => None,
            })
            .collect()
    }

    fn assert_closed_once(observations: &[GraphicsTestObservation]) {
        assert_eq!(
            observations
                .iter()
                .filter(|observation| matches!(observation, GraphicsTestObservation::Closed))
                .count(),
            1
        );
        assert!(matches!(
            observations.last(),
            Some(GraphicsTestObservation::Closed)
        ));
    }

    #[test]
    fn graphics_script_renders_initial_and_each_updated_state_in_provider_order() {
        let mut interpreter = interpreter(PROGRAM);
        interpreter.set_graphics_test_events(vec![
            GraphicsTestEvent::Key(Key::Right),
            GraphicsTestEvent::Key(Key::Right),
            GraphicsTestEvent::Key(Key::Left),
            GraphicsTestEvent::Close,
        ]);

        assert_eq!(
            play(&mut interpreter),
            Ok(Value::ResultOk(Box::new(Value::Nothing)))
        );
        assert_eq!(interpreter.graphics_test_events_remaining(), Some(0));
        let observations = interpreter.take_graphics_test_observations();
        assert!(matches!(
            observations.first(),
            Some(GraphicsTestObservation::Opened(_))
        ));
        assert_eq!(presented_red_channels(&observations), vec![0, 1, 2, 1]);
        assert_closed_once(&observations);
        assert_eq!(interpreter.scopes.len(), 1);
    }

    #[test]
    fn graphics_inline_callbacks_keep_their_lexical_namespace() {
        let program = PROGRAM.replace(
            "0, update, render)",
            "0, function(state: int64, key: graphics.Key) returns int64: return update(state, key), function(view state: int64) returns graphics.Scene: return render(view state))",
        );
        let mut interpreter = interpreter(&program);
        interpreter.set_graphics_test_events(vec![
            GraphicsTestEvent::Key(Key::Right),
            GraphicsTestEvent::Close,
        ]);

        assert_eq!(
            play(&mut interpreter),
            Ok(Value::ResultOk(Box::new(Value::Nothing)))
        );
        assert_eq!(
            presented_red_channels(&interpreter.take_graphics_test_observations()),
            vec![0, 1]
        );
    }

    #[test]
    fn graphics_qualified_callbacks_preserve_their_declaration_namespace() {
        let program = PROGRAM
            .replace("function play(", "namespace launcher\nfunction play(")
            .replace("0, update, render)", "0, game.update, game.render)");
        let mut interpreter = interpreter(&program);
        interpreter.set_graphics_test_events(vec![
            GraphicsTestEvent::Key(Key::Right),
            GraphicsTestEvent::Close,
        ]);

        let result = interpreter.call_function(
            "launcher.play",
            vec![Value::Capability("Graphics".to_string())],
        );
        assert_eq!(result, Ok(Value::ResultOk(Box::new(Value::Nothing))));
        assert_eq!(
            presented_red_channels(&interpreter.take_graphics_test_observations()),
            vec![0, 1]
        );
    }

    #[test]
    fn graphics_callback_failure_closes_window_and_restores_interpreter_context() {
        let program = PROGRAM.replace(
            "return increment(state)",
            "assert false\n            return state",
        );
        let mut interpreter = interpreter(&program);
        interpreter.set_graphics_test_events(vec![GraphicsTestEvent::Key(Key::Right)]);

        assert_eq!(play(&mut interpreter), Err("assertion failed".to_string()));
        let observations = interpreter.take_graphics_test_observations();
        assert_eq!(presented_red_channels(&observations), vec![0]);
        assert_closed_once(&observations);
        assert!(!interpreter.graphics_session_active);
        assert_eq!(interpreter.scopes.len(), 1);
        assert_eq!(interpreter.current_namespace, None);

        interpreter.set_graphics_test_events(vec![GraphicsTestEvent::Close]);
        assert_eq!(
            play(&mut interpreter),
            Ok(Value::ResultOk(Box::new(Value::Nothing)))
        );
        assert_closed_once(&interpreter.take_graphics_test_observations());
    }

    #[test]
    fn graphics_invalid_replacement_scene_is_a_handled_failure_and_closes() {
        let program = PROGRAM.replace("return increment(state)", "return 256");
        let mut interpreter = interpreter(&program);
        interpreter.set_graphics_test_events(vec![GraphicsTestEvent::Key(Key::Right)]);

        assert_eq!(
            play(&mut interpreter),
            Ok(domain_failure(
                "graphics.run: color channels must be in 0..255"
            ))
        );
        let observations = interpreter.take_graphics_test_observations();
        assert_eq!(presented_red_channels(&observations), vec![0]);
        assert_closed_once(&observations);
    }

    #[test]
    fn graphics_invalid_config_or_initial_scene_never_opens_a_window() {
        for program in [
            PROGRAM.replace("width: 16", "width: 0"),
            PROGRAM.replace("config, 0, update", "config, 256, update"),
        ] {
            let mut interpreter = interpreter(&program);
            interpreter.set_graphics_test_events(vec![GraphicsTestEvent::Close]);
            assert!(matches!(play(&mut interpreter), Ok(Value::ResultFail(_))));
            assert!(interpreter.take_graphics_test_observations().is_empty());
            assert_eq!(interpreter.graphics_test_events_remaining(), Some(1));
        }
    }

    #[test]
    fn graphics_host_failure_closes_window_and_preserves_domain_error() {
        let mut interpreter = interpreter(PROGRAM);
        interpreter.set_graphics_test_events(vec![GraphicsTestEvent::HostError(
            "scripted presentation failed".to_string(),
        )]);
        assert_eq!(
            play(&mut interpreter),
            Ok(domain_failure("scripted presentation failed"))
        );
        assert_closed_once(&interpreter.take_graphics_test_observations());
    }

    #[test]
    fn graphics_requires_trusted_dispatch_exact_capability_and_injected_provider() {
        let mut interpreter = interpreter(PROGRAM);
        assert!(
            interpreter
                .call_higher_order_builtin("graphics.__run", &[])
                .is_none()
        );
        assert_eq!(
            play(&mut interpreter),
            Err("Graphics: runtime provider unavailable".to_string())
        );
        interpreter.set_graphics_test_events(vec![GraphicsTestEvent::Close]);
        assert_eq!(
            interpreter.call_function("game.play", vec![Value::Nothing]),
            Err("graphics.__run expects a Graphics capability".to_string())
        );
        assert!(interpreter.take_graphics_test_observations().is_empty());
    }

    #[test]
    fn graphics_nested_session_is_rejected_before_any_window_opens() {
        let mut interpreter = interpreter(PROGRAM);
        interpreter.set_graphics_test_events(vec![GraphicsTestEvent::Close]);
        interpreter.graphics_session_active = true;
        assert_eq!(
            play(&mut interpreter),
            Ok(domain_failure(
                "graphics.run: nested sessions are unsupported"
            ))
        );
        assert!(interpreter.take_graphics_test_observations().is_empty());
    }
}
