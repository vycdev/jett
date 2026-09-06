use jett_common::breakpoint::{
    BREAKPOINT_PROTOCOL, BreakpointSession, FailureKind, Operation, ProtocolFailure, Request,
    RequestArguments, RequestDisposition, SessionState, SourceEntry, SourceKind, SourceManifest,
    constant_time_token_eq,
};

fn request(
    session_id: &str,
    pause_id: Option<u64>,
    request_id: u64,
    operation: Operation,
    arguments: RequestArguments,
) -> Request {
    Request {
        protocol: BREAKPOINT_PROTOCOL.to_string(),
        session_id: session_id.to_string(),
        pause_id,
        request_id,
        operation,
        arguments,
    }
}

#[test]
fn lifecycle_allocates_monotonic_pause_ids_and_invalidates_resumed_pauses() {
    let mut session = BreakpointSession::new("session-a");
    assert_eq!(session.state(), SessionState::Starting);

    session.start().unwrap();
    session.begin_pause().unwrap();
    let first = session.finish_pause().unwrap();
    assert_eq!(first, 1);
    assert_eq!(session.state(), SessionState::Paused { pause_id: first });

    session.begin_resume(first).unwrap();
    assert_eq!(session.state(), SessionState::Resuming);
    session.finish_resume().unwrap();
    assert_eq!(session.state(), SessionState::Running);
    assert_eq!(
        session.begin_resume(first).unwrap_err().kind,
        FailureKind::StalePause
    );

    session.begin_pause().unwrap();
    let second = session.finish_pause().unwrap();
    assert_eq!(second, 2);
}

#[test]
fn paused_operations_require_the_current_pause_and_matching_arguments() {
    let mut session = BreakpointSession::new("session-a");
    session.start().unwrap();
    session.begin_pause().unwrap();
    let pause_id = session.finish_pause().unwrap();

    let bindings = request(
        "session-a",
        Some(pause_id),
        1,
        Operation::Bindings,
        RequestArguments::Frame {
            frame_id: "frame-0".into(),
        },
    );
    assert_eq!(
        session.admit_request(bindings).unwrap(),
        RequestDisposition::New
    );

    let missing_pause = request(
        "session-a",
        None,
        2,
        Operation::Bindings,
        RequestArguments::Frame {
            frame_id: "frame-0".into(),
        },
    );
    assert_eq!(
        session.admit_request(missing_pause).unwrap_err().kind,
        FailureKind::InvalidRequest
    );

    let stale_pause = request(
        "session-a",
        Some(pause_id + 1),
        3,
        Operation::Bindings,
        RequestArguments::Frame {
            frame_id: "frame-0".into(),
        },
    );
    assert_eq!(
        session.admit_request(stale_pause).unwrap_err().kind,
        FailureKind::StalePause
    );

    let wrong_arguments = request(
        "session-a",
        Some(pause_id),
        4,
        Operation::Value,
        RequestArguments::Frame {
            frame_id: "frame-0".into(),
        },
    );
    assert_eq!(
        session.admit_request(wrong_arguments).unwrap_err().kind,
        FailureKind::InvalidRequest
    );
}

#[test]
fn wait_and_disconnect_apply_state_dependent_pause_rules() {
    let mut session = BreakpointSession::new("session-a");
    session.start().unwrap();

    let wait = request(
        "session-a",
        None,
        1,
        Operation::Wait,
        RequestArguments::None,
    );
    assert_eq!(
        session.admit_request(wait).unwrap(),
        RequestDisposition::New
    );

    let running_disconnect = request(
        "session-a",
        None,
        2,
        Operation::Disconnect,
        RequestArguments::None,
    );
    assert_eq!(
        session.admit_request(running_disconnect).unwrap(),
        RequestDisposition::New
    );
    session.complete_request(2).unwrap();

    session.begin_pause().unwrap();
    let pause_id = session.finish_pause().unwrap();
    let paused_wait_with_id = request(
        "session-a",
        Some(pause_id),
        3,
        Operation::Wait,
        RequestArguments::None,
    );
    assert_eq!(
        session.admit_request(paused_wait_with_id).unwrap_err().kind,
        FailureKind::InvalidRequest
    );

    let paused_disconnect = request(
        "session-a",
        Some(pause_id),
        4,
        Operation::Disconnect,
        RequestArguments::None,
    );
    assert_eq!(
        session.admit_request(paused_disconnect).unwrap(),
        RequestDisposition::New
    );
}

#[test]
fn duplicate_request_ids_are_idempotent_but_conflicting_reuse_is_rejected() {
    let mut session = BreakpointSession::new("session-a");
    session.start().unwrap();
    let original = request(
        "session-a",
        None,
        9,
        Operation::Wait,
        RequestArguments::None,
    );

    assert_eq!(
        session.admit_request(original.clone()).unwrap(),
        RequestDisposition::New
    );
    assert_eq!(
        session.admit_request(original).unwrap(),
        RequestDisposition::Duplicate
    );

    let conflicting = request(
        "session-a",
        None,
        9,
        Operation::Disconnect,
        RequestArguments::None,
    );
    assert_eq!(
        session.admit_request(conflicting).unwrap_err().kind,
        FailureKind::InvalidRequest
    );
}

#[test]
fn authentication_compares_complete_tokens() {
    assert!(constant_time_token_eq(
        b"0123456789abcdef",
        b"0123456789abcdef"
    ));
    assert!(!constant_time_token_eq(
        b"0123456789abcdef",
        b"0123456789abcdeg"
    ));
    assert!(!constant_time_token_eq(b"short", b"shorter"));
}

#[test]
fn source_manifest_accepts_only_normalized_manifest_relative_paths() {
    let manifest = SourceManifest::new([
        SourceEntry::new(
            "project:src/main.jett",
            "src/main.jett",
            SourceKind::Project,
        ),
        SourceEntry::new(
            "stdlib:string.jett",
            "stdlib/string.jett",
            SourceKind::StandardLibrary,
        ),
    ])
    .unwrap();
    assert_eq!(
        manifest.get("project:src/main.jett").unwrap().path,
        "src/main.jett"
    );
    assert!(manifest.get("project:missing.jett").is_none());

    for path in [
        "",
        "/etc/passwd",
        "../secret.jett",
        "src/../secret.jett",
        "src\\main.jett",
        "C:secret.jett",
        "src/secret\0.jett",
        "src/secret\n.jett",
    ] {
        let error =
            SourceManifest::new([SourceEntry::new("bad", path, SourceKind::Project)]).unwrap_err();
        assert_eq!(error.kind, FailureKind::InvalidRequest, "path: {path}");
    }
}

#[test]
fn event_wait_does_not_block_commands_but_each_lane_is_serialized() {
    let mut session = BreakpointSession::new("session-a");
    session.start().unwrap();
    session.begin_pause().unwrap();
    let pause = session.finish_pause().unwrap();
    let wait = request(
        "session-a",
        None,
        1,
        Operation::Wait,
        RequestArguments::None,
    );
    session.admit_request(wait.clone()).unwrap();
    assert_eq!(
        session.admit_request(wait).unwrap(),
        RequestDisposition::Duplicate
    );
    let second_wait = request(
        "session-a",
        None,
        2,
        Operation::Wait,
        RequestArguments::None,
    );
    assert_eq!(
        session.admit_request(second_wait.clone()).unwrap_err().kind,
        FailureKind::InvalidRequest
    );
    session
        .admit_request(request(
            "session-a",
            Some(pause),
            3,
            Operation::Continue,
            RequestArguments::None,
        ))
        .unwrap();
    let competing = request(
        "session-a",
        Some(pause),
        4,
        Operation::Stack,
        RequestArguments::None,
    );
    assert_eq!(
        session.admit_request(competing.clone()).unwrap_err().kind,
        FailureKind::InvalidRequest
    );
    session.complete_request(3).unwrap();
    session.admit_request(competing).unwrap();
    session.complete_request(1).unwrap();
    session.admit_request(second_wait).unwrap();
}

#[test]
fn terminal_sessions_invalidate_duplicate_requests_and_do_not_reopen() {
    for failed in [false, true] {
        let mut session = BreakpointSession::new("session-a");
        session.start().unwrap();
        let wait = request(
            "session-a",
            None,
            1,
            Operation::Wait,
            RequestArguments::None,
        );
        session.admit_request(wait.clone()).unwrap();
        if failed {
            session.fail();
        } else {
            session.close();
        }
        assert!(session.admit_request(wait).is_err());
        assert!(session.complete_request(1).is_err());
        session.close();
        session.fail();
        assert_eq!(session.state(), SessionState::Closed);
    }
}

#[test]
fn failure_renderer_is_deterministic_and_escapes_multiline_messages() {
    let failure = ProtocolFailure::new(
        FailureKind::UnavailableBinding,
        "binding `order` was consumed,\nretry after pause",
    );
    assert_eq!(
        failure.render_toon("session-a", Some(3), 9),
        "protocol: jett.breakpoint.v1\nsession_id: session-a\npause_id: 3\nrequest_id: 9\nstatus: error\nfailure:\n  code: BP1004\n  kind: unavailable_binding\n  message: binding `order` was consumed\\,\\nretry after pause\n"
    );
}
