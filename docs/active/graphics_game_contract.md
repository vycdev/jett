# Initial synchronous 2D graphics contract

Status: implemented on `codex/first-game` for the first external Jett game.
This is a bounded initial graphics surface, not a general game engine.

## Ownership and effects

`Graphics` is a runtime-injected capability authorizing a synchronous graphics
session. It contains no source-visible window identity and is not a window
resource. `graphics.run` creates and owns one host-local window for the duration
of the call. Returning normally, a host error, or a callback runtime error drops
that window. No window handle, pointer, or native resource enters a Jett value.

This scope deliberately avoids exposing a `Window` before the interpreter has
implemented the accepted opaque-resource transfer and scope-cleanup contract.
Any later source-visible window must use `resource Window` with that complete
contract; a struct, capability, or primitive ID cannot replace it.

The pure callbacks receive only state and key data. They acquire no graphics
authority and cannot open windows or perform other effects. Required comptime
evaluation and verify/property blocks cannot run a graphics session. Source-owned
wrappers and private stdlib hooks follow the existing stdlib trust boundary;
untrusted code cannot invoke or impersonate the native kernel.

The initial API accepts directly named or inline callbacks whose parameter modes
can be checked from their declarations. Passing callbacks through local function
variables or higher-order parameters is rejected until ordinary function types
retain sufficient borrowing and effect information. The update callback owns
both arguments; render borrows its sole argument. Both must be pure. The display
argument must explicitly borrow a declared `Graphics` parameter. These checks
are conservative API gates, not exemptions from normal function checking.

`State` is concrete data: primitive values, structs, enums, and supported
collections containing only concrete data. Function values, capabilities,
resources, actors, and interfaces are rejected recursively, including behind
aliases. This prevents state from carrying a closure with hidden authority
across the pure callback boundary while function types still erase effects.
The scoped callback audit also follows named helpers and nested inline
functions to reject hidden effects in the callback call graph.

## Source API

All data declarations and the public wrapper live in namespace `graphics` in
`stdlib/graphics.jett`:

- `Config`: `title: string`, `width: int64`, `height: int64`.
- `Color`: `red: int64`, `green: int64`, `blue: int64`.
- `Rect`: `x: int64`, `y: int64`, `width: int64`, `height: int64`, `color: Color`.
- `Text`: `x: int64`, `y: int64`, `text: string`, `scale: int64`, `color: Color`.
- `Scene`: `background: Color`, `rectangles: list[Rect]`, `texts: list[Text]`.
- `Key`: `up`, `down`, `left`, `right`, `w`, `a`, `s`, `d`, `u`, `z`, `r`, `n`,
  `p`, `enter`, and `space`.

```jett
export function run[State](view display: Graphics, config: Config, initial: State, update: function(State, Key) returns State, render: function(view State) returns Scene) returns result[nothing, string]:
    return graphics.__run[State](view display, config, initial, update, render)
```

The window initially displays `render(view initial)`. Each supported key press
transfers state to `update`, obtains its replacement, and renders that state.
Held keys do not repeat. Escape and the OS close action end the session without
calling update. Unsupported keys do nothing. Concurrent keys have provider order;
applications must not rely on a particular order for simultaneous presses.

The backend continues pumping window messages while idle and retains the current
scene. Callbacks run synchronously on the interpreter's thread. A slow callback
delays input and drawing. This first API has no animation tick, frame-clock
promise, audio, mouse input, texture loading, asynchronous callbacks, or native
executable output. Nested sessions are rejected.

## Rendering and validation

The initial backend uses minifb for a native fixed-size window and framebuffer,
and a built-in bitmap font for text. Application geometry, colors, and text are
ordinary Jett values. A scene clears its background, draws rectangles in list
order, then text in list order. Coordinates are top-left pixel coordinates.
Rectangles clip at the viewport. Colors are opaque RGB channels in 0..255.
Text uses 8 by 8 bitmap cells scaled by an integer in 1..8; unsupported glyphs
use a question mark. Newlines start a new text row.

Window dimensions must be in 1..2048, rectangle dimensions nonnegative, and
scenes contain at most 10,000 rectangles and 1,024 text items. Each text item
contains at most 16,384 UTF-8 bytes. Window titles contain at most 256 UTF-8 bytes
and no NUL. Validate before opening a window or presenting a scene, and use checked
coordinate arithmetic. Invalid input and host creation/presentation failures
return `fail(string)`. Callback runtime errors remain interpreter errors; both
error routes still close the host window.

## Verification

Check invalid dimensions/colors/geometry and clipped rendering without a display.
Use a scripted host provider to exercise callback order, initial rendering, state
transfer, and close/error cleanup. Check capability, private-kernel, generic
callback, and comptime restrictions with compiler fixtures. Finally run the
external game in a real native window and verify movement, undo, restart,
progression, and close.
