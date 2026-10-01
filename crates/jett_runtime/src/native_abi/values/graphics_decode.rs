//! Strict decoding of the existing, checked graphics record layouts.
//!
//! Decode in reference field order before running domain validation. Pending
//! values are terminal shape errors, including metadata on otherwise ready
//! handles; decoding must never observe a pending value's payload.
use super::*;

#[derive(Clone, Copy)]
enum GraphicsRecord {
    Config,
    Scene,
    Color,
    Rect,
    Text,
}

impl GraphicsRecord {
    fn pending(self) -> Failure {
        let message: &'static [u8] = match self {
            Self::Config => b"graphics: expected graphics.Config",
            Self::Scene => b"graphics: expected graphics.Scene",
            Self::Color => b"graphics: expected graphics.Color",
            Self::Rect => b"graphics: expected graphics.Rect",
            Self::Text => b"graphics: expected graphics.Text",
        };
        graphics_type_error(message)
    }
}

fn graphics_type_error(message: &'static [u8]) -> Failure {
    (JettRuntimeStatusV1::INVALID_ARGUMENT, message)
}

fn field(record: &NativeStruct, index: usize) -> LeafResult<NativeField> {
    record
        .fields
        .get(index)
        .copied()
        .flatten()
        .ok_or(INVALID_STRUCT)
}

fn integer(record: &NativeStruct, index: usize, pending: &'static [u8]) -> LeafResult<i64> {
    let value = field(record, index)?;
    if value.pending_depth != 0 {
        return Err(graphics_type_error(pending));
    }
    if value.owned {
        return Err(INVALID_STRUCT);
    }
    Ok(value.bits as i64)
}

fn record_field(record: &NativeStruct, index: usize, kind: GraphicsRecord) -> LeafResult<u64> {
    let value = field(record, index)?;
    if value.pending_depth != 0 {
        return Err(kind.pending());
    }
    if !value.owned {
        return Err(INVALID_STRUCT);
    }
    Ok(value.bits)
}

fn record_element(list: &NativeList, index: usize, kind: GraphicsRecord) -> LeafResult<u64> {
    if list
        .element_pending_depths
        .get(&index)
        .copied()
        .unwrap_or(0)
        != 0
    {
        return Err(kind.pending());
    }
    list.elements
        .get(index)
        .copied()
        .flatten()
        .ok_or(INVALID_LIST)
}

impl NativeValues {
    fn graphics_record(&self, value: u64, kind: GraphicsRecord) -> LeafResult<&NativeStruct> {
        let record = self.structs.get(&value).ok_or(INVALID_STRUCT)?;
        if record.pending_depth != 0 {
            return Err(kind.pending());
        }
        Ok(record)
    }

    fn graphics_string(
        &self,
        record: &NativeStruct,
        index: usize,
        pending: &'static [u8],
    ) -> LeafResult<String> {
        let value = field(record, index)?;
        if value.pending_depth != 0 {
            return Err(graphics_type_error(pending));
        }
        if !value.owned {
            return Err(INVALID_STRUCT);
        }
        let string = self.strings.get(&value.bits).ok_or(INVALID_HANDLE)?;
        if string.pending_depth != 0 {
            return Err(graphics_type_error(pending));
        }
        Ok(string.text.clone())
    }

    fn graphics_list(
        &self,
        record: &NativeStruct,
        index: usize,
        pending: &'static [u8],
    ) -> LeafResult<&NativeList> {
        let value = field(record, index)?;
        if value.pending_depth != 0 {
            return Err(graphics_type_error(pending));
        }
        if !value.owned {
            return Err(INVALID_STRUCT);
        }
        let list = self.lists.get(&value.bits).ok_or(INVALID_LIST)?;
        if list.pending_depth != 0 {
            return Err(graphics_type_error(pending));
        }
        if !list.owned {
            return Err(INVALID_LIST);
        }
        Ok(list)
    }

    pub(super) fn graphics_config(&self, value: u64) -> LeafResult<graphics::Config> {
        let record = self.graphics_record(value, GraphicsRecord::Config)?;
        Ok(graphics::Config {
            title: self.graphics_string(
                record,
                0,
                b"graphics: graphics.Config.title must be string",
            )?,
            width: integer(record, 1, b"graphics: graphics.Config.width must be int64")?,
            height: integer(record, 2, b"graphics: graphics.Config.height must be int64")?,
        })
    }

    fn graphics_color(&self, value: u64) -> LeafResult<graphics::Color> {
        let record = self.graphics_record(value, GraphicsRecord::Color)?;
        Ok(graphics::Color {
            red: integer(record, 0, b"graphics: graphics.Color.red must be int64")?,
            green: integer(record, 1, b"graphics: graphics.Color.green must be int64")?,
            blue: integer(record, 2, b"graphics: graphics.Color.blue must be int64")?,
        })
    }

    fn graphics_rect(&self, value: u64) -> LeafResult<graphics::Rect> {
        let record = self.graphics_record(value, GraphicsRecord::Rect)?;
        Ok(graphics::Rect {
            x: integer(record, 0, b"graphics: graphics.Rect.x must be int64")?,
            y: integer(record, 1, b"graphics: graphics.Rect.y must be int64")?,
            width: integer(record, 2, b"graphics: graphics.Rect.width must be int64")?,
            height: integer(record, 3, b"graphics: graphics.Rect.height must be int64")?,
            color: self.graphics_color(record_field(record, 4, GraphicsRecord::Color)?)?,
        })
    }

    fn graphics_text(&self, value: u64) -> LeafResult<graphics::Text> {
        let record = self.graphics_record(value, GraphicsRecord::Text)?;
        Ok(graphics::Text {
            x: integer(record, 0, b"graphics: graphics.Text.x must be int64")?,
            y: integer(record, 1, b"graphics: graphics.Text.y must be int64")?,
            text: self.graphics_string(
                record,
                2,
                b"graphics: graphics.Text.text must be string",
            )?,
            scale: integer(record, 3, b"graphics: graphics.Text.scale must be int64")?,
            color: self.graphics_color(record_field(record, 4, GraphicsRecord::Color)?)?,
        })
    }

    pub(super) fn graphics_scene(&self, value: u64) -> LeafResult<graphics::Scene> {
        let record = self.graphics_record(value, GraphicsRecord::Scene)?;
        let background = self.graphics_color(record_field(record, 0, GraphicsRecord::Color)?)?;
        let rectangles = self.graphics_list(
            record,
            1,
            b"graphics: graphics.Scene.rectangles must be a list",
        )?;
        let rectangles = (0..rectangles.elements.len())
            .map(|index| {
                self.graphics_rect(record_element(rectangles, index, GraphicsRecord::Rect)?)
            })
            .collect::<LeafResult<Vec<_>>>()?;
        let texts =
            self.graphics_list(record, 2, b"graphics: graphics.Scene.texts must be a list")?;
        let texts = (0..texts.elements.len())
            .map(|index| self.graphics_text(record_element(texts, index, GraphicsRecord::Text)?))
            .collect::<LeafResult<Vec<_>>>()?;
        Ok(graphics::Scene {
            background,
            rectangles,
            texts,
        })
    }
}

#[cfg(test)]
mod tests;
