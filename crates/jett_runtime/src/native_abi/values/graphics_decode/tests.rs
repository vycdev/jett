use super::*;
use std::collections::BTreeMap;

fn scalar(bits: u64) -> NativeField {
    NativeField {
        bits,
        owned: false,
        pending_depth: 0,
    }
}

fn owned(bits: u64) -> NativeField {
    NativeField {
        bits,
        owned: true,
        pending_depth: 0,
    }
}

fn record(values: &mut NativeValues, fields: Vec<NativeField>) -> u64 {
    let value = values.new_struct(fields.len() as u64).unwrap();
    values.structs.get_mut(&value).unwrap().fields = fields.into_iter().map(Some).collect();
    value
}

fn color(values: &mut NativeValues) -> u64 {
    record(values, vec![scalar(1), scalar(2), scalar(3)])
}

struct Fixture {
    config: u64,
    title: u64,
    scene: u64,
    background: u64,
    rectangles: u64,
    rect: u64,
    rect_color: u64,
    texts: u64,
    text: u64,
    text_string: u64,
    text_color: u64,
}

impl Fixture {
    fn new(values: &mut NativeValues) -> Self {
        let title = values.insert_pending("headless".into(), 0).unwrap();
        let config = record(values, vec![owned(title), scalar(16), scalar(16)]);
        let background = color(values);
        let rect_color = color(values);
        let rect = record(
            values,
            vec![
                scalar(0),
                scalar(0),
                scalar(1),
                scalar(1),
                owned(rect_color),
            ],
        );
        let rectangles = values.new_list(true).unwrap();
        values
            .lists
            .get_mut(&rectangles)
            .unwrap()
            .elements
            .push(Some(rect));
        let text_color = color(values);
        let text_string = values.insert_pending("A".into(), 0).unwrap();
        let text = record(
            values,
            vec![
                scalar(0),
                scalar(0),
                owned(text_string),
                scalar(1),
                owned(text_color),
            ],
        );
        let texts = values.new_list(true).unwrap();
        values
            .lists
            .get_mut(&texts)
            .unwrap()
            .elements
            .push(Some(text));
        let scene = record(
            values,
            vec![owned(background), owned(rectangles), owned(texts)],
        );
        Self {
            config,
            title,
            scene,
            background,
            rectangles,
            rect,
            rect_color,
            texts,
            text,
            text_string,
            text_color,
        }
    }

    fn destroy(self, values: &mut NativeValues) {
        values.drop_value(self.config).unwrap();
        values.drop_value(self.scene).unwrap();
        assert!(values.is_empty());
        assert_eq!(values.structs_created, values.structs_destroyed);
        assert_eq!(values.lists_created, values.lists_destroyed);
        assert_eq!(values.sums_created, values.sums_destroyed);
    }

    fn config_order(&self) -> Vec<(Site, &'static str)> {
        vec![
            (
                Site::Record(self.config),
                "graphics: expected graphics.Config",
            ),
            (
                Site::Field(self.config, 0),
                "graphics: graphics.Config.title must be string",
            ),
            (
                Site::String(self.title),
                "graphics: graphics.Config.title must be string",
            ),
            (
                Site::Field(self.config, 1),
                "graphics: graphics.Config.width must be int64",
            ),
            (
                Site::Field(self.config, 2),
                "graphics: graphics.Config.height must be int64",
            ),
        ]
    }

    fn scene_order(&self) -> Vec<(Site, &'static str)> {
        vec![
            (
                Site::Record(self.scene),
                "graphics: expected graphics.Scene",
            ),
            (
                Site::Field(self.scene, 0),
                "graphics: expected graphics.Color",
            ),
            (
                Site::Record(self.background),
                "graphics: expected graphics.Color",
            ),
            (
                Site::Field(self.background, 0),
                "graphics: graphics.Color.red must be int64",
            ),
            (
                Site::Field(self.background, 1),
                "graphics: graphics.Color.green must be int64",
            ),
            (
                Site::Field(self.background, 2),
                "graphics: graphics.Color.blue must be int64",
            ),
            (
                Site::Field(self.scene, 1),
                "graphics: graphics.Scene.rectangles must be a list",
            ),
            (
                Site::List(self.rectangles),
                "graphics: graphics.Scene.rectangles must be a list",
            ),
            (
                Site::Element(self.rectangles, 0),
                "graphics: expected graphics.Rect",
            ),
            (Site::Record(self.rect), "graphics: expected graphics.Rect"),
            (
                Site::Field(self.rect, 0),
                "graphics: graphics.Rect.x must be int64",
            ),
            (
                Site::Field(self.rect, 1),
                "graphics: graphics.Rect.y must be int64",
            ),
            (
                Site::Field(self.rect, 2),
                "graphics: graphics.Rect.width must be int64",
            ),
            (
                Site::Field(self.rect, 3),
                "graphics: graphics.Rect.height must be int64",
            ),
            (
                Site::Field(self.rect, 4),
                "graphics: expected graphics.Color",
            ),
            (
                Site::Record(self.rect_color),
                "graphics: expected graphics.Color",
            ),
            (
                Site::Field(self.rect_color, 0),
                "graphics: graphics.Color.red must be int64",
            ),
            (
                Site::Field(self.rect_color, 1),
                "graphics: graphics.Color.green must be int64",
            ),
            (
                Site::Field(self.rect_color, 2),
                "graphics: graphics.Color.blue must be int64",
            ),
            (
                Site::Field(self.scene, 2),
                "graphics: graphics.Scene.texts must be a list",
            ),
            (
                Site::List(self.texts),
                "graphics: graphics.Scene.texts must be a list",
            ),
            (
                Site::Element(self.texts, 0),
                "graphics: expected graphics.Text",
            ),
            (Site::Record(self.text), "graphics: expected graphics.Text"),
            (
                Site::Field(self.text, 0),
                "graphics: graphics.Text.x must be int64",
            ),
            (
                Site::Field(self.text, 1),
                "graphics: graphics.Text.y must be int64",
            ),
            (
                Site::Field(self.text, 2),
                "graphics: graphics.Text.text must be string",
            ),
            (
                Site::String(self.text_string),
                "graphics: graphics.Text.text must be string",
            ),
            (
                Site::Field(self.text, 3),
                "graphics: graphics.Text.scale must be int64",
            ),
            (
                Site::Field(self.text, 4),
                "graphics: expected graphics.Color",
            ),
            (
                Site::Record(self.text_color),
                "graphics: expected graphics.Color",
            ),
            (
                Site::Field(self.text_color, 0),
                "graphics: graphics.Color.red must be int64",
            ),
            (
                Site::Field(self.text_color, 1),
                "graphics: graphics.Color.green must be int64",
            ),
            (
                Site::Field(self.text_color, 2),
                "graphics: graphics.Color.blue must be int64",
            ),
        ]
    }
}

#[derive(Clone, Copy, Debug)]
enum Site {
    Record(u64),
    Field(u64, usize),
    String(u64),
    List(u64),
    Element(u64, usize),
}

impl Site {
    fn set(self, values: &mut NativeValues, depth: u64) {
        match self {
            Self::Record(id) => values.structs.get_mut(&id).unwrap().pending_depth = depth,
            Self::Field(id, index) => {
                values.structs.get_mut(&id).unwrap().fields[index]
                    .as_mut()
                    .unwrap()
                    .pending_depth = depth
            }
            Self::String(id) => values.strings.get_mut(&id).unwrap().pending_depth = depth,
            Self::List(id) => values.lists.get_mut(&id).unwrap().pending_depth = depth,
            Self::Element(id, index) => {
                let depths = &mut values.lists.get_mut(&id).unwrap().element_pending_depths;
                if depth == 0 {
                    depths.remove(&index);
                } else {
                    depths.insert(index, depth);
                }
            }
        }
    }
}

// Include every graphics carrier's ownership, payload and pending metadata.
fn snapshot(values: &NativeValues) -> String {
    let strings: BTreeMap<_, _> = values
        .strings
        .iter()
        .map(|(id, value)| (*id, (&value.text, value.references, value.pending_depth)))
        .collect();
    let records: BTreeMap<_, _> = values
        .structs
        .iter()
        .map(|(id, value)| {
            (
                *id,
                (
                    value.pending_depth,
                    value
                        .fields
                        .iter()
                        .map(|field| {
                            field.map(|field| (field.bits, field.owned, field.pending_depth))
                        })
                        .collect::<Vec<_>>(),
                ),
            )
        })
        .collect();
    let lists: BTreeMap<_, _> = values
        .lists
        .iter()
        .map(|(id, value)| {
            (
                *id,
                (
                    value.pending_depth,
                    value.owned,
                    &value.elements,
                    value
                        .element_pending_depths
                        .iter()
                        .map(|(index, depth)| (*index, *depth))
                        .collect::<BTreeMap<_, _>>(),
                ),
            )
        })
        .collect();
    format!("{strings:?}/{records:?}/{lists:?}")
}

fn assert_error<T: std::fmt::Debug>(result: LeafResult<T>, message: &str) {
    let (status, actual) = result.unwrap_err();
    assert_eq!(status, JettRuntimeStatusV1::INVALID_ARGUMENT);
    assert_eq!(std::str::from_utf8(actual).unwrap(), message);
}

#[test]
fn graphics_decode_rejects_every_pending_shape_without_changing_sources() {
    for depth in [1, 2] {
        let mut values = NativeValues::default();
        let fixture = Fixture::new(&mut values);
        for (config, cases) in [
            (true, fixture.config_order()),
            (false, fixture.scene_order()),
        ] {
            for (site, message) in cases {
                site.set(&mut values, depth);
                let before = snapshot(&values);
                if config {
                    assert_error(values.graphics_config(fixture.config), message);
                } else {
                    assert_error(values.graphics_scene(fixture.scene), message);
                }
                assert_eq!(snapshot(&values), before, "{site:?}");
                site.set(&mut values, 0);
            }
        }
        fixture.destroy(&mut values);
    }
}

#[test]
fn graphics_decode_reports_first_fault_in_reference_field_order() {
    for depth in [1, 2] {
        let mut values = NativeValues::default();
        let fixture = Fixture::new(&mut values);
        for (config, cases) in [
            (true, fixture.config_order()),
            (false, fixture.scene_order()),
        ] {
            for (site, _) in &cases {
                site.set(&mut values, depth);
            }
            for (site, message) in cases {
                if config {
                    assert_error(values.graphics_config(fixture.config), message);
                } else {
                    assert_error(values.graphics_scene(fixture.scene), message);
                }
                site.set(&mut values, 0);
            }
        }
        assert_eq!(values.graphics_config(fixture.config).unwrap().width, 16);
        assert_eq!(
            values.graphics_scene(fixture.scene).unwrap().texts[0].text,
            "A"
        );
        fixture.destroy(&mut values);
    }
}

#[test]
fn graphics_decode_does_not_observe_payloads_below_pending_metadata() {
    for depth in [1, 2] {
        let mut values = NativeValues::default();
        let fixture = Fixture::new(&mut values);
        // A pending owner cannot expose even a missing first field.
        let saved = std::mem::take(&mut values.structs.get_mut(&fixture.config).unwrap().fields);
        Site::Record(fixture.config).set(&mut values, depth);
        assert_error(
            values.graphics_config(fixture.config),
            "graphics: expected graphics.Config",
        );
        Site::Record(fixture.config).set(&mut values, 0);
        assert_eq!(values.graphics_config(fixture.config), Err(INVALID_STRUCT));
        values.structs.get_mut(&fixture.config).unwrap().fields = saved;

        // Field metadata precedes both an invalid handle and its ownership flag.
        let saved = values.structs.get_mut(&fixture.config).unwrap().fields[0].replace(scalar(0));
        Site::Field(fixture.config, 0).set(&mut values, depth);
        assert_error(
            values.graphics_config(fixture.config),
            "graphics: graphics.Config.title must be string",
        );
        Site::Field(fixture.config, 0).set(&mut values, 0);
        assert_eq!(values.graphics_config(fixture.config), Err(INVALID_STRUCT));
        values.structs.get_mut(&fixture.config).unwrap().fields[0]
            .as_mut()
            .unwrap()
            .owned = true;
        assert_eq!(values.graphics_config(fixture.config), Err(INVALID_HANDLE));
        values.structs.get_mut(&fixture.config).unwrap().fields[0] = saved;

        let saved = values.lists.get_mut(&fixture.rectangles).unwrap().elements[0].take();
        values.lists.get_mut(&fixture.rectangles).unwrap().owned = false;
        Site::List(fixture.rectangles).set(&mut values, depth);
        assert_error(
            values.graphics_scene(fixture.scene),
            "graphics: graphics.Scene.rectangles must be a list",
        );
        Site::List(fixture.rectangles).set(&mut values, 0);
        assert_eq!(values.graphics_scene(fixture.scene), Err(INVALID_LIST));
        values.lists.get_mut(&fixture.rectangles).unwrap().owned = true;
        Site::Element(fixture.rectangles, 0).set(&mut values, depth);
        assert_error(
            values.graphics_scene(fixture.scene),
            "graphics: expected graphics.Rect",
        );
        Site::Element(fixture.rectangles, 0).set(&mut values, 0);
        assert_eq!(values.graphics_scene(fixture.scene), Err(INVALID_LIST));
        values.lists.get_mut(&fixture.rectangles).unwrap().elements[0] = saved;

        // Ready scalar fields reject ownership metadata instead of treating a
        // live handle as an integer. Restore it before normal source cleanup.
        values.structs.get_mut(&fixture.config).unwrap().fields[1]
            .as_mut()
            .unwrap()
            .owned = true;
        assert_eq!(values.graphics_config(fixture.config), Err(INVALID_STRUCT));
        values.structs.get_mut(&fixture.config).unwrap().fields[1]
            .as_mut()
            .unwrap()
            .owned = false;
        fixture.destroy(&mut values);
    }
}

#[test]
fn graphics_decode_completes_shape_checks_before_domain_validation() {
    let mut values = NativeValues::default();
    let fixture = Fixture::new(&mut values);
    values.structs.get_mut(&fixture.config).unwrap().fields[1]
        .as_mut()
        .unwrap()
        .bits = 0;
    Site::Field(fixture.config, 2).set(&mut values, 2);
    assert_error(
        values.graphics_validate_config(fixture.config),
        "graphics: graphics.Config.height must be int64",
    );
    Site::Field(fixture.config, 2).set(&mut values, 0);
    let outcome = values.graphics_validate_config(fixture.config).unwrap();
    assert_eq!(values.sums[&outcome].tag, SUM_FAILURE);
    assert_eq!(
        values.text(values.sums[&outcome].bits).unwrap(),
        graphics::validate_config(&values.graphics_config(fixture.config).unwrap()).unwrap_err()
    );
    values.drop_value(outcome).unwrap();

    values.structs.get_mut(&fixture.background).unwrap().fields[0]
        .as_mut()
        .unwrap()
        .bits = 256;
    Site::Field(fixture.text, 3).set(&mut values, 2);
    assert_error(
        values.graphics_validate_scene(16, 16, fixture.scene),
        "graphics: graphics.Text.scale must be int64",
    );
    Site::Field(fixture.text, 3).set(&mut values, 0);
    let outcome = values
        .graphics_validate_scene(16, 16, fixture.scene)
        .unwrap();
    assert_eq!(values.sums[&outcome].tag, SUM_FAILURE);
    assert_eq!(
        values.text(values.sums[&outcome].bits).unwrap(),
        graphics::render_scene(16, 16, &values.graphics_scene(fixture.scene).unwrap()).unwrap_err()
    );
    values.drop_value(outcome).unwrap();
    fixture.destroy(&mut values);
}

#[test]
fn graphics_decode_accepts_joined_record_roots_without_consuming_originals() {
    for depth in [1, 2] {
        let mut values = NativeValues::default();
        let fixture = Fixture::new(&mut values);
        for (source, kind) in [
            (fixture.config, GraphicsRecord::Config),
            (fixture.scene, GraphicsRecord::Scene),
            (fixture.background, GraphicsRecord::Color),
            (fixture.rect, GraphicsRecord::Rect),
            (fixture.text, GraphicsRecord::Text),
        ] {
            let original = snapshot(&values);
            let mut joined = values.clone_with_pending_depth(source, depth).unwrap();
            for remaining in (0..depth).rev() {
                assert_eq!(
                    values.graphics_record(joined, kind).err(),
                    Some(kind.pending())
                );
                let previous = joined;
                joined = values.join_record(previous).unwrap();
                assert_eq!(values.structs[&previous].pending_depth, remaining + 1);
                assert_eq!(values.structs[&joined].pending_depth, remaining);
                values.drop_value(previous).unwrap();
            }
            match kind {
                GraphicsRecord::Config => assert_eq!(
                    values.graphics_config(joined),
                    values.graphics_config(source)
                ),
                GraphicsRecord::Scene => {
                    assert_eq!(values.graphics_scene(joined), values.graphics_scene(source))
                }
                GraphicsRecord::Color => {
                    assert_eq!(values.graphics_color(joined), values.graphics_color(source))
                }
                GraphicsRecord::Rect => {
                    assert_eq!(values.graphics_rect(joined), values.graphics_rect(source))
                }
                GraphicsRecord::Text => {
                    assert_eq!(values.graphics_text(joined), values.graphics_text(source))
                }
            }
            values.drop_value(joined).unwrap();
            assert_eq!(snapshot(&values), original);
        }
        fixture.destroy(&mut values);
    }
}

#[test]
fn graphics_decode_accepts_joined_string_and_list_fields_without_consuming_originals() {
    for depth in [1, 2] {
        let mut values = NativeValues::default();
        let fixture = Fixture::new(&mut values);
        for (owner, index, source, string, config, message) in [
            (
                fixture.config,
                0,
                fixture.title,
                true,
                true,
                "graphics: graphics.Config.title must be string",
            ),
            (
                fixture.text,
                2,
                fixture.text_string,
                true,
                false,
                "graphics: graphics.Text.text must be string",
            ),
            (
                fixture.scene,
                1,
                fixture.rectangles,
                false,
                false,
                "graphics: graphics.Scene.rectangles must be a list",
            ),
            (
                fixture.scene,
                2,
                fixture.texts,
                false,
                false,
                "graphics: graphics.Scene.texts must be a list",
            ),
        ] {
            let original = snapshot(&values);
            let mut joined = values.clone_with_pending_depth(source, depth).unwrap();
            for remaining in (0..depth).rev() {
                values.structs.get_mut(&owner).unwrap().fields[index]
                    .as_mut()
                    .unwrap()
                    .bits = joined;
                if config {
                    assert_error(values.graphics_config(fixture.config), message);
                } else {
                    assert_error(values.graphics_scene(fixture.scene), message);
                }
                values.structs.get_mut(&owner).unwrap().fields[index]
                    .as_mut()
                    .unwrap()
                    .bits = source;
                let previous = joined;
                joined = if string {
                    values.join_string(previous).unwrap()
                } else {
                    values.join_list(previous).unwrap()
                };
                assert_eq!(values.owned_pending_depth(previous), Ok(remaining + 1));
                assert_eq!(values.owned_pending_depth(joined), Ok(remaining));
                values.drop_value(previous).unwrap();
            }
            values.structs.get_mut(&owner).unwrap().fields[index]
                .as_mut()
                .unwrap()
                .bits = joined;
            assert!(values.graphics_config(fixture.config).is_ok());
            assert!(values.graphics_scene(fixture.scene).is_ok());
            values.structs.get_mut(&owner).unwrap().fields[index]
                .as_mut()
                .unwrap()
                .bits = source;
            values.drop_value(joined).unwrap();
            assert_eq!(snapshot(&values), original);
        }
        fixture.destroy(&mut values);
    }
}

#[test]
fn graphics_decode_failures_precede_provider_use_and_preserve_session_cleanup() {
    let mut values = NativeValues::default();
    let fixture = Fixture::new(&mut values);
    let authority = next_identity().unwrap();
    values.graphics = Some(authority);
    let events = graphics::decode_test_script(&graphics::encode_test_script(&[
        graphics::TestEvent::Key(graphics::Key::Right),
        graphics::TestEvent::Close,
    ]))
    .unwrap();
    values.graphics_script = Some(events.clone());
    Site::Field(fixture.config, 1).set(&mut values, 2);
    assert_error(
        values.graphics_open(authority, fixture.config),
        "graphics: graphics.Config.width must be int64",
    );
    assert!(values.graphics_session.is_none());
    assert!(GRAPHICS_SESSION.with(|slot| slot.borrow().is_none()));
    assert_eq!(values.graphics_script.as_ref(), Some(&events));
    Site::Field(fixture.config, 1).set(&mut values, 0);

    let opened = values.graphics_open(authority, fixture.config).unwrap();
    assert_eq!(values.sums[&opened].tag, SUM_SUCCESS);
    let session = values.sums[&opened].bits;
    values.drop_value(opened).unwrap();
    Site::Field(fixture.text, 0).set(&mut values, 1);
    let before = snapshot(&values);
    assert_error(
        values.graphics_present(session, fixture.scene),
        "graphics: graphics.Text.x must be int64",
    );
    assert_eq!(snapshot(&values), before);
    assert_eq!(values.graphics_script.as_ref(), Some(&events));
    assert_eq!(values.graphics_session, Some(session));
    // This is the same cleanup path used by native context destruction after
    // a terminal leaf failure; it must still work with pending owned sources.
    values.release_graphics_session().unwrap();
    assert!(GRAPHICS_SESSION.with(|slot| slot.borrow().is_none()));
    assert!(values.graphics_session.is_none());
    fixture.destroy(&mut values);
}
