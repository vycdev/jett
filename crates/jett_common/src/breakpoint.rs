use std::collections::BTreeMap;

pub const BREAKPOINT_PROTOCOL: &str = "jett.breakpoint.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Starting,
    Running,
    Pausing,
    Paused { pause_id: u64 },
    Resuming,
    Failed,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Wait,
    Bindings,
    Value,
    Evaluate,
    Stack,
    Continue,
    Disconnect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestArguments {
    None,
    Frame {
        frame_id: String,
    },
    Binding {
        frame_id: String,
        name: String,
    },
    Evaluate {
        frame_id: String,
        expression: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub protocol: String,
    pub session_id: String,
    pub pause_id: Option<u64>,
    pub request_id: u64,
    pub operation: Operation,
    pub arguments: RequestArguments,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestDisposition {
    New,
    Duplicate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    InvalidRequest,
    Unauthorized,
    StalePause,
    UnknownFrame,
    UnknownBinding,
    UnavailableBinding,
    ForbiddenQuery,
    InvalidExpression,
    EvaluationLimit,
    TargetFailed,
    Internal,
}

impl FailureKind {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidRequest => "BP1001",
            Self::Unauthorized => "BP1002",
            Self::StalePause => "BP1003",
            Self::UnavailableBinding => "BP1004",
            Self::UnknownFrame => "BP1005",
            Self::UnknownBinding => "BP1006",
            Self::ForbiddenQuery => "BP1007",
            Self::InvalidExpression => "BP1008",
            Self::EvaluationLimit => "BP1009",
            Self::TargetFailed => "BP1010",
            Self::Internal => "BP1099",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::Unauthorized => "unauthorized",
            Self::StalePause => "stale_pause",
            Self::UnknownFrame => "unknown_frame",
            Self::UnknownBinding => "unknown_binding",
            Self::UnavailableBinding => "unavailable_binding",
            Self::ForbiddenQuery => "forbidden_query",
            Self::InvalidExpression => "invalid_expression",
            Self::EvaluationLimit => "evaluation_limit",
            Self::TargetFailed => "target_failed",
            Self::Internal => "internal",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolFailure {
    pub kind: FailureKind,
    pub message: String,
}

impl ProtocolFailure {
    pub fn new(kind: FailureKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn render_toon(&self, session_id: &str, pause_id: Option<u64>, request_id: u64) -> String {
        let mut output = format!(
            "protocol: {BREAKPOINT_PROTOCOL}\nsession_id: {}\n",
            escape_toon_scalar(session_id)
        );
        if let Some(pause_id) = pause_id {
            output.push_str(&format!("pause_id: {pause_id}\n"));
        }
        output.push_str(&format!(
            "request_id: {request_id}\nstatus: error\nfailure:\n  code: {}\n  kind: {}\n  message: {}\n",
            self.kind.code(),
            self.kind.name(),
            escape_toon_scalar(&self.message)
        ));
        output
    }
}

#[derive(Debug)]
pub struct BreakpointSession {
    session_id: String,
    state: SessionState,
    next_pause_id: u64,
    requests: BTreeMap<u64, Request>,
}

impl BreakpointSession {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            state: SessionState::Starting,
            next_pause_id: 1,
            requests: BTreeMap::new(),
        }
    }

    pub const fn state(&self) -> SessionState {
        self.state
    }

    pub fn start(&mut self) -> Result<(), ProtocolFailure> {
        self.transition(SessionState::Starting, SessionState::Running)
    }

    pub fn begin_pause(&mut self) -> Result<(), ProtocolFailure> {
        self.transition(SessionState::Running, SessionState::Pausing)
    }

    pub fn finish_pause(&mut self) -> Result<u64, ProtocolFailure> {
        if self.state != SessionState::Pausing {
            return Err(invalid_transition(self.state, "finish pause"));
        }
        let pause_id = self.next_pause_id;
        self.next_pause_id = self
            .next_pause_id
            .checked_add(1)
            .ok_or_else(|| ProtocolFailure::new(FailureKind::Internal, "pause id exhausted"))?;
        self.state = SessionState::Paused { pause_id };
        Ok(pause_id)
    }

    pub fn begin_resume(&mut self, pause_id: u64) -> Result<(), ProtocolFailure> {
        match self.state {
            SessionState::Paused { pause_id: current } if current == pause_id => {
                self.state = SessionState::Resuming;
                Ok(())
            }
            _ if pause_id < self.next_pause_id => Err(stale_pause(pause_id)),
            _ => Err(invalid_transition(self.state, "resume")),
        }
    }

    pub fn finish_resume(&mut self) -> Result<(), ProtocolFailure> {
        self.transition(SessionState::Resuming, SessionState::Running)
    }

    pub fn fail(&mut self) {
        self.state = SessionState::Failed;
    }

    pub fn close(&mut self) {
        self.state = SessionState::Closed;
    }

    pub fn admit_request(
        &mut self,
        request: Request,
    ) -> Result<RequestDisposition, ProtocolFailure> {
        if request.protocol != BREAKPOINT_PROTOCOL {
            return Err(ProtocolFailure::new(
                FailureKind::InvalidRequest,
                "unsupported breakpoint protocol",
            ));
        }
        if request.session_id != self.session_id {
            return Err(ProtocolFailure::new(
                FailureKind::Unauthorized,
                "request does not belong to this breakpoint session",
            ));
        }
        if let Some(previous) = self.requests.get(&request.request_id) {
            return if previous == &request {
                Ok(RequestDisposition::Duplicate)
            } else {
                Err(ProtocolFailure::new(
                    FailureKind::InvalidRequest,
                    "request id was reused with different content",
                ))
            };
        }

        validate_arguments(&request)?;
        self.validate_request_state(&request)?;
        self.requests.insert(request.request_id, request);
        Ok(RequestDisposition::New)
    }

    fn validate_request_state(&self, request: &Request) -> Result<(), ProtocolFailure> {
        match request.operation {
            Operation::Wait => {
                if request.pause_id.is_some() {
                    return Err(ProtocolFailure::new(
                        FailureKind::InvalidRequest,
                        "wait requests must omit pause_id",
                    ));
                }
                if matches!(
                    self.state,
                    SessionState::Running | SessionState::Paused { .. }
                ) {
                    Ok(())
                } else {
                    Err(invalid_state(self.state, request.operation))
                }
            }
            Operation::Bindings
            | Operation::Value
            | Operation::Evaluate
            | Operation::Stack
            | Operation::Continue => self.require_current_pause(request.pause_id),
            Operation::Disconnect => match self.state {
                SessionState::Running if request.pause_id.is_none() => Ok(()),
                SessionState::Paused { .. } => self.require_current_pause(request.pause_id),
                SessionState::Running => Err(ProtocolFailure::new(
                    FailureKind::InvalidRequest,
                    "disconnect must omit pause_id while running",
                )),
                _ => Err(invalid_state(self.state, request.operation)),
            },
        }
    }

    fn require_current_pause(&self, requested: Option<u64>) -> Result<(), ProtocolFailure> {
        let Some(requested) = requested else {
            return Err(ProtocolFailure::new(
                FailureKind::InvalidRequest,
                "operation requires the current pause_id",
            ));
        };
        match self.state {
            SessionState::Paused { pause_id } if pause_id == requested => Ok(()),
            SessionState::Paused { .. } => Err(stale_pause(requested)),
            _ if requested < self.next_pause_id => Err(stale_pause(requested)),
            _ => Err(ProtocolFailure::new(
                FailureKind::InvalidRequest,
                "operation requires a paused session",
            )),
        }
    }

    fn transition(
        &mut self,
        expected: SessionState,
        next: SessionState,
    ) -> Result<(), ProtocolFailure> {
        if self.state != expected {
            return Err(invalid_transition(self.state, "transition"));
        }
        self.state = next;
        Ok(())
    }
}

fn validate_arguments(request: &Request) -> Result<(), ProtocolFailure> {
    let valid = matches!(
        (&request.operation, &request.arguments),
        (Operation::Wait, RequestArguments::None)
            | (Operation::Bindings, RequestArguments::Frame { .. })
            | (Operation::Value, RequestArguments::Binding { .. })
            | (Operation::Evaluate, RequestArguments::Evaluate { .. })
            | (Operation::Stack, RequestArguments::None)
            | (Operation::Continue, RequestArguments::None)
            | (Operation::Disconnect, RequestArguments::None)
    );
    if valid {
        Ok(())
    } else {
        Err(ProtocolFailure::new(
            FailureKind::InvalidRequest,
            "operation arguments do not match the request operation",
        ))
    }
}

fn invalid_transition(state: SessionState, action: &str) -> ProtocolFailure {
    ProtocolFailure::new(
        FailureKind::InvalidRequest,
        format!("cannot {action} while session is {state:?}"),
    )
}

fn invalid_state(state: SessionState, operation: Operation) -> ProtocolFailure {
    ProtocolFailure::new(
        FailureKind::InvalidRequest,
        format!("operation {operation:?} is invalid while session is {state:?}"),
    )
}

fn stale_pause(pause_id: u64) -> ProtocolFailure {
    ProtocolFailure::new(
        FailureKind::StalePause,
        format!("pause_id {pause_id} is not the active pause"),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Project,
    Dependency,
    StandardLibrary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceEntry {
    pub source_id: String,
    pub path: String,
    pub kind: SourceKind,
}

impl SourceEntry {
    pub fn new(source_id: impl Into<String>, path: impl Into<String>, kind: SourceKind) -> Self {
        Self {
            source_id: source_id.into(),
            path: path.into(),
            kind,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceManifest {
    entries: BTreeMap<String, SourceEntry>,
}

impl SourceManifest {
    pub fn new(entries: impl IntoIterator<Item = SourceEntry>) -> Result<Self, ProtocolFailure> {
        let mut manifest = BTreeMap::new();
        for entry in entries {
            validate_source_entry(&entry)?;
            if manifest.insert(entry.source_id.clone(), entry).is_some() {
                return Err(ProtocolFailure::new(
                    FailureKind::InvalidRequest,
                    "source manifest contains a duplicate source id",
                ));
            }
        }
        if manifest.is_empty() {
            return Err(ProtocolFailure::new(
                FailureKind::InvalidRequest,
                "source manifest must not be empty",
            ));
        }
        Ok(Self { entries: manifest })
    }

    pub fn get(&self, source_id: &str) -> Option<&SourceEntry> {
        self.entries.get(source_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &SourceEntry> {
        self.entries.values()
    }
}

fn validate_source_entry(entry: &SourceEntry) -> Result<(), ProtocolFailure> {
    if entry.source_id.is_empty() {
        return Err(ProtocolFailure::new(
            FailureKind::InvalidRequest,
            "source id must not be empty",
        ));
    }
    let path = entry.path.as_str();
    let invalid_segment = path.split('/').any(|segment| {
        segment.is_empty() || segment == "." || segment == ".." || segment.ends_with(':')
    });
    if path.starts_with('/') || path.contains('\\') || invalid_segment {
        return Err(ProtocolFailure::new(
            FailureKind::InvalidRequest,
            "source path must be a normalized manifest-relative path",
        ));
    }
    Ok(())
}

pub fn constant_time_token_eq(expected: &[u8], provided: &[u8]) -> bool {
    let max_len = expected.len().max(provided.len());
    let mut difference = expected.len() ^ provided.len();
    for index in 0..max_len {
        let expected_byte = expected.get(index).copied().unwrap_or(0);
        let provided_byte = provided.get(index).copied().unwrap_or(0);
        difference |= usize::from(expected_byte ^ provided_byte);
    }
    difference == 0
}

fn escape_toon_scalar(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
        .replace(',', "\\,")
}
