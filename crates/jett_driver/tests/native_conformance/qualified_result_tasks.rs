use super::*;

const JOIN_CONTROLS: &str = r#"namespace app
type OutcomeAlias = result[list[string], string]
type HiddenAlias = secret[result[list[string], string]]
type RefinedOutcome = result[list[string], string] where true
function public_outcome(valid: bool) returns result[list[string], string]:
    if valid:
        return ok(list("Ada-λ🙂", "kept"))
    return fail("missing-λ🙂")
function qualify[T](candidate: result[T, string]) returns secret[result[T, string]]:
    return candidate
function join_hidden[T](candidate: secret[result[T, string]]) returns secret[result[T, string]]:
    secret[result[T, string]] ready = join candidate handle error:
        result[T, string] failed = fail("outer:{error}")
        return qualify[T](failed)
    return ready
function describe_public(view candidate: result[list[string], string]) returns string:
    list[string] values = clone candidate handle error:
        return "inner:{error}"
    string first = list.get[string](view values, 0) handle:
        return "missing"
    return "ok:{list.length(view values)}:{first}"
function describe(view candidate: secret[result[list[string], string]]) returns string:
    result[list[string], string] public = declassify clone candidate
    return describe_public(view public)
function ready_report(valid: bool) returns string:
    result[list[string], string] public = public_outcome(valid)
    secret[result[list[string], string]] hidden = qualify[list[string]](public)
    secret[result[list[string], string]] ready = join_hidden[list[string]](hidden)
    return describe(view ready)
function once_report(valid: bool) returns string:
    result[list[string], string] public = public_outcome(valid)
    HiddenAlias pending = run qualify[list[string]](public)
    HiddenAlias ready = join pending handle error:
        return "outer:{error}"
    return describe(view ready)
function twice_report(valid: bool) returns string:
    result[list[string], string] public = public_outcome(valid)
    secret[result[list[string], string]] pending = run run qualify[list[string]](public)
    secret[result[list[string], string]] once = join_hidden[list[string]](pending)
    secret[result[list[string], string]] ready = join_hidden[list[string]](once)
    return describe(view ready)
function exact_report(valid: bool) returns string:
    OutcomeAlias pending = run run public_outcome(valid)
    OutcomeAlias once = join pending
    OutcomeAlias ready = join once
    return describe_public(view ready)
function nested_report(valid: bool) returns string:
    result[list[string], string] public = public_outcome(valid)
    secret[secret[result[list[string], string]]] hidden = public
    secret[secret[result[list[string], string]]] pending = run run hidden
    secret[secret[result[list[string], string]]] once = join pending handle error:
        return "outer:{error}"
    secret[secret[result[list[string], string]]] ready = join once handle error:
        return "outer:{error}"
    secret[result[list[string], string]] inner = declassify ready
    return describe(view inner)
function refined_report(valid: bool) returns string:
    result[list[string], string] public = public_outcome(valid)
    RefinedOutcome checked = public handle error:
        return "refinement:{error}"
    RefinedOutcome pending = run run checked
    RefinedOutcome once = join pending handle error:
        return "outer:{error}"
    RefinedOutcome ready = join once handle error:
        return "outer:{error}"
    result[list[string], string] base = coarsen ready
    return describe_public(view base)
function scoped_report() returns string:
    comptime type Bound = type.info[secret[result[list[string], string]]]():
        result[list[string], string] public = public_outcome(true)
        Bound pending = run run qualify[list[string]](public)
        Bound once = join pending handle error:
            return "outer:{error}"
        Bound ready = join once handle error:
            return "outer:{error}"
        return describe(view ready)
function text_report(valid: bool) returns string:
    mutable result[string, string] public = fail("text-error-λ🙂")
    if valid:
        public = ok("text-λ🙂")
    secret[result[string, string]] pending = run run qualify[string](public)
    secret[result[string, string]] once = join_hidden[string](pending)
    secret[result[string, string]] ready = join_hidden[string](once)
    result[string, string] exposed = declassify ready
    string text = exposed handle error:
        return "inner:{error}"
    return "ok:{text}"
function ownership_report() returns string:
    result[list[string], string] public = public_outcome(true)
    secret[result[list[string], string]] pending = run qualify[list[string]](public)
    secret[result[list[string], string]] first = join_hidden[list[string]](clone pending)
    secret[result[list[string], string]] second = join_hidden[list[string]](clone pending)
    result[list[string], string] exposed = declassify clone first
    mutable list[string] independent = exposed handle error:
        return error
    independent = list.append[string](independent, "independent")
    return "{list.length(view independent)}|{describe(view first)}|{describe(view second)}"
function baked_report() returns string:
    string success = comptime twice_report(true)
    string failure = comptime twice_report(false)
    string nested = comptime nested_report(true)
    return "{success}|{failure}|{nested}"
function matrix(valid: bool) returns string:
    return "{ready_report(valid)}|{once_report(valid)}|{twice_report(valid)}|{exact_report(valid)}|{nested_report(valid)}|{refined_report(valid)}"
function main(stdout: Stdout) returns nothing:
    Stdout.write(view stdout, "success:{matrix(true)}\n")
    Stdout.write(view stdout, "failure:{matrix(false)}\n")
    Stdout.write(view stdout, "text:{text_report(true)}|{text_report(false)}\n")
    Stdout.write(view stdout, "scoped:{scoped_report()}\n")
    Stdout.write(view stdout, "ownership:{ownership_report()}\n")
    Stdout.write(view stdout, "baked:{baked_report()}\n")
    result[list[string], string] public = public_outcome(true)
    secret[result[list[string], string]] pending = run run qualify[list[string]](public)
    secret[result[list[string], string]] once = join_hidden[list[string]](pending)
    result[list[string], string] partial = declassify clone once
    trace partial
    secret[result[list[string], string]] ready = join_hidden[list[string]](once)
    Stdout.write(view stdout, "partial-then-ready:{describe(view ready)}\n")
verify qualified_result_joins:
    assert matrix(true) == "ok:2:Ada-λ🙂|ok:2:Ada-λ🙂|ok:2:Ada-λ🙂|ok:2:Ada-λ🙂|ok:2:Ada-λ🙂|ok:2:Ada-λ🙂"
    assert matrix(false) == "inner:missing-λ🙂|inner:missing-λ🙂|inner:missing-λ🙂|inner:missing-λ🙂|inner:missing-λ🙂|inner:missing-λ🙂"
    assert text_report(true) == "ok:text-λ🙂"
    assert text_report(false) == "inner:text-error-λ🙂"
    assert scoped_report() == "ok:2:Ada-λ🙂"
    assert ownership_report() == "3|ok:2:Ada-λ🙂|ok:2:Ada-λ🙂"
    assert baked_report() == "ok:2:Ada-λ🙂|inner:missing-λ🙂|ok:2:Ada-λ🙂"
property qualified_result_trials:
    given valid: bool
    mutable string expected = "inner:missing-λ🙂"
    if valid:
        expected = "ok:2:Ada-λ🙂"
    assert ready_report(valid) == expected
    assert once_report(valid) == expected
    assert twice_report(valid) == expected
    assert exact_report(valid) == expected
    assert nested_report(valid) == expected
    assert refined_report(valid) == expected
    assert ownership_report() == "3|ok:2:Ada-λ🙂|ok:2:Ada-λ🙂"
"#;

#[test]
fn native_qualified_result_tasks_preserve_inner_results_and_pending_depth() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::write(&source, JOIN_CONTROLS).unwrap();
    let expected = jett_driver::run_file_capture_output(&source)
        .expect("qualified join returns an outer result around the qualified inner result");
    assert_eq!(
        expected.stdout,
        concat!(
            "success:ok:2:Ada-λ🙂|ok:2:Ada-λ🙂|ok:2:Ada-λ🙂|ok:2:Ada-λ🙂|ok:2:Ada-λ🙂|ok:2:Ada-λ🙂\n",
            "failure:inner:missing-λ🙂|inner:missing-λ🙂|inner:missing-λ🙂|inner:missing-λ🙂|inner:missing-λ🙂|inner:missing-λ🙂\n",
            "text:ok:text-λ🙂|inner:text-error-λ🙂\n",
            "scoped:ok:2:Ada-λ🙂\n",
            "ownership:3|ok:2:Ada-λ🙂|ok:2:Ada-λ🙂\n",
            "baked:ok:2:Ada-λ🙂|inner:missing-λ🙂|ok:2:Ada-λ🙂\n",
            "partial-then-ready:ok:2:Ada-λ🙂\n",
        )
    );
    assert_eq!(
        debug_trace_lines(&expected.debug_events),
        ["trace partial: result[list[string], string] = pending(ok(list(Ada-λ🙂, kept)))"]
    );
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory
            .path()
            .join(format!("qualified_result_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native qualified result tasks");
        binaries.push((binary, release));
    }
    let verify_binary = directory.path().join("qualified_result_verify.exe");
    build_host_verify_suite_executable(&source, launcher(), &verify_binary)
        .expect("compiled qualified result verify suite");
    let property_binary = directory.path().join("qualified_result_property.exe");
    build_host_property_suite_executable(&source, launcher(), &property_binary)
        .expect("compiled qualified result property suite");
    fs::remove_file(&source).unwrap();
    for (binary, release) in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(actual.status.code(), Some(0), "{actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes());
        let debug = if release {
            String::new()
        } else {
            jett_driver::render_debug_events(&expected.debug_events)
        };
        assert_eq!(actual.stderr, debug.as_bytes(), "{actual:?}");
    }
    for binary in [verify_binary, property_binary] {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(actual.status.code(), Some(0), "{actual:?}");
        assert!(actual.stdout.is_empty(), "{actual:?}");
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
}

const HANDLER_CONTROLS: &str = r#"namespace app
function public_failure(view stdout: Stdout) returns result[list[string], string]:
    Stdout.write(view stdout, "operand\n")
    return fail("inner-error")
function qualified_failure(view stdout: Stdout) returns secret[result[list[string], string]]:
    result[list[string], string] public = public_failure(view stdout)
    return public
function later_failure(view stdout: Stdout) returns list[string]:
    Stdout.write(view stdout, "handler\n")
    list[string] owned = list("staged", "owner")
    return list.remove_at[string](owned, -1)
function main(stdout: Stdout) returns nothing:
    bytes earlier = bytes.from_string("retained")
    list[string] retained = list("earlier", "owner")
    Stdout.write(view stdout, "before\n")
    secret[result[list[string], string]] pending = run qualified_failure(view stdout)
    secret[result[list[string], string]] ready = join pending handle error:
        Stdout.write(view stdout, "must not handle outer\n")
        return nothing
    result[list[string], string] public = declassify ready
    list[string] values = public handle error:
        Stdout.write(view stdout, "inner:{error}\n")
        default later_failure(view stdout)
    Stdout.write(view stdout, "must not run:{list.length(view values)}:{bytes.to_hex(view earlier)}:{list.length(view retained)}\n")
"#;

#[test]
fn native_qualified_result_join_keeps_inner_failure_for_its_handler_and_cleanup() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::write(&source, HANDLER_CONTROLS).unwrap();
    let expected = jett_driver::run_file_capture_outcome(&source)
        .expect_err("only the inner handler runs and its later failure remains terminal");
    assert_eq!(
        expected.message,
        "runtime error: list.__remove_at: index -1 out of bounds"
    );
    assert_eq!(
        expected.output.stdout,
        "before\noperand\ninner:inner-error\nhandler\n"
    );
    assert!(expected.output.debug_events.is_empty(), "{expected:?}");
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory
            .path()
            .join(format!("qualified_handler_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native qualified result inner handler");
        binaries.push(binary);
    }
    fs::remove_file(&source).unwrap();
    for binary in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(actual.status.code(), Some(71), "{actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes());
        assert_eq!(actual.stderr, format!("{}\n", expected.message).as_bytes());
    }
}

const VIEW_AND_SCALAR_CONTROLS: &str = r#"namespace app
function ordinary() returns result[int8, int64]:
    return ok(7)
function scalar_outcome(valid: bool) returns result[int8, int64]:
    int8 number = 7
    int64 problem = 9
    if valid:
        return ok(run run number)
    return fail(run run problem)
function promote(candidate: result[int8, int64]) returns secret[result[int8, int64]]:
    return candidate
function join_view(view candidate: secret[result[int8, int64]]) returns secret[result[int8, int64]]:
    secret[result[int8, int64]] joined = join view candidate handle error:
        result[int8, int64] failure = fail(-1)
        return promote(failure)
    trace candidate
    return joined
function error_report(problem: int64) returns string:
    int64 once = join problem handle error:
        return "join-error:{error}"
    int64 ready = join once handle error:
        return "join-error:{error}"
    return "fail:{ready}"
function scalar_report(candidate: result[int8, int64]) returns string:
    int8 value = candidate handle error:
        return error_report(error)
    int8 once = join value handle error:
        return "join-error:{error}"
    int8 ready = join once handle error:
        return "join-error:{error}"
    return "ok:{ready}"
function qualified_report(valid: bool) returns string:
    secret[result[int8, int64]] original = run run promote(scalar_outcome(valid))
    secret[result[int8, int64]] once = join original handle error:
        return "outer:{error}"
    secret[result[int8, int64]] ready = join once handle error:
        return "outer:{error}"
    result[int8, int64] exposed = declassify ready
    return scalar_report(exposed)
function exact_report(valid: bool) returns string:
    result[int8, int64] original = run run scalar_outcome(valid)
    result[int8, int64] once = join original
    result[int8, int64] ready = join once
    return scalar_report(ready)
function ready_report(valid: bool) returns string:
    secret[result[int8, int64]] original = promote(scalar_outcome(valid))
    secret[result[int8, int64]] ready = join original handle error:
        return "outer:{error}"
    result[int8, int64] exposed = declassify ready
    return scalar_report(exposed)
function observe_error(view stdout: Stdout, problem: int64) returns nothing:
    trace problem
    int64 error_once = join problem handle error:
        return nothing
    trace error_once
    int64 error_ready = join error_once handle error:
        return nothing
    trace error_ready
    Stdout.write(view stdout, "payload-failure:{error_ready}\n")
function main(stdout: Stdout) returns nothing:
    result[int8, int64] original = run run ordinary()
    result[int8, int64] once = join view original
    trace original
    trace once
    result[int8, int64] joined = join once
    int8 explicit_value = joined handle error:
        return nothing
    Stdout.write(view stdout, "explicit:{explicit_value}\n")
    trace original
    secret[result[int8, int64]] hidden = run run promote(ordinary())
    secret[result[int8, int64]] hidden_once = join_view(view hidden)
    secret[result[int8, int64]] hidden_ready = join_view(view hidden_once)
    result[int8, int64] exposed = declassify hidden_ready
    int8 borrowed_value = exposed handle error:
        return nothing
    Stdout.write(view stdout, "view-param:{borrowed_value}\n")
    result[int8, int64] original_copy = declassify clone hidden
    result[int8, int64] once_copy = declassify clone hidden_once
    trace original_copy
    trace once_copy
    Stdout.write(view stdout, "qualified:{qualified_report(true)}|{qualified_report(false)}\n")
    Stdout.write(view stdout, "exact:{exact_report(true)}|{exact_report(false)}\n")
    Stdout.write(view stdout, "ready-qualified:{ready_report(true)}|{ready_report(false)}\n")
    result[int8, int64] pending_payload = scalar_outcome(true)
    int8 value = pending_payload handle error:
        return nothing
    trace value
    int8 value_once = join value handle error:
        return nothing
    trace value_once
    int8 value_ready = join value_once handle error:
        return nothing
    trace value_ready
    Stdout.write(view stdout, "payload-success:{value_ready}\n")
    result[int8, int64] failed_payload = scalar_outcome(false)
    int8 unused = failed_payload handle error:
        observe_error(view stdout, error)
        default 0
verify qualified_view_and_scalar_joins:
    assert qualified_report(true) == "ok:7"
    assert qualified_report(false) == "fail:9"
    assert exact_report(true) == "ok:7"
    assert exact_report(false) == "fail:9"
    assert ready_report(true) == "ok:7"
    assert ready_report(false) == "fail:9"
property qualified_scalar_join_trials:
    given valid: bool
    mutable string expected = "fail:9"
    if valid:
        expected = "ok:7"
    assert qualified_report(valid) == expected
    assert exact_report(valid) == expected
    assert ready_report(valid) == expected
"#;

#[test]
fn native_qualified_result_joins_preserve_views_and_scalar_payload_depth() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::write(&source, VIEW_AND_SCALAR_CONTROLS).unwrap();
    let expected = jett_driver::run_file_capture_output(&source)
        .expect("explicit view joins preserve their owners and inner scalar metadata");
    let stdout = concat!(
        "explicit:7\n",
        "view-param:7\n",
        "qualified:ok:7|fail:9\n",
        "exact:ok:7|fail:9\n",
        "ready-qualified:ok:7|fail:9\n",
        "payload-success:7\n",
        "payload-failure:9\n",
    );
    let debug = [
        "trace original: result[int8, int64] = pending(pending(ok(7)))",
        "trace once: result[int8, int64] = pending(ok(7))",
        "trace original: result[int8, int64] = pending(pending(ok(7)))",
        "trace candidate: secret[result[int8, int64]] = [redacted]",
        "trace candidate: secret[result[int8, int64]] = [redacted]",
        "trace original_copy: result[int8, int64] = pending(pending(ok(7)))",
        "trace once_copy: result[int8, int64] = pending(ok(7))",
        "trace value: int8 = pending(pending(7))",
        "trace value_once: int8 = pending(7)",
        "trace value_ready: int8 = 7",
        "trace problem: int64 = pending(pending(9))",
        "trace error_once: int64 = pending(9)",
        "trace error_ready: int64 = 9",
    ];
    assert_eq!(expected.stdout, stdout);
    assert_eq!(debug_trace_lines(&expected.debug_events), debug);
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory
            .path()
            .join(format!("qualified_views_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native qualified result view and scalar controls");
        binaries.push((binary, release));
    }
    let verify_binary = directory.path().join("qualified_views_verify.exe");
    build_host_verify_suite_executable(&source, launcher(), &verify_binary)
        .expect("compiled scalar join verify suite");
    let property_binary = directory.path().join("qualified_views_property.exe");
    build_host_property_suite_executable(&source, launcher(), &property_binary)
        .expect("compiled scalar join property suite");
    fs::remove_file(&source).unwrap();
    for (binary, release) in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(actual.status.code(), Some(0), "{actual:?}");
        assert_eq!(actual.stdout, stdout.as_bytes());
        let stderr = if release {
            String::new()
        } else {
            jett_driver::render_debug_events(&expected.debug_events)
        };
        assert_eq!(actual.stderr, stderr.as_bytes(), "{actual:?}");
    }
    for binary in [verify_binary, property_binary] {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(actual.status.code(), Some(0), "{actual:?}");
        assert!(actual.stdout.is_empty(), "{actual:?}");
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
}
