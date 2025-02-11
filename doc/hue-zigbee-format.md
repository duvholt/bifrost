# Zigbee format for Philips Hue manufacturer-specific light updates

## Introduction

Philips hue lights support zigbee frames in a manufacturer-specific format, on cluster 0xFC03.

This type of message is necessary to support many of the advanced features in Hue lights, such as:

 - Multiple colors ("gradient") in LED strips
 - Light Effects ("Candle", "Fireplace", etc)
 - Combining effects with color settings

Several attempts have been made to reverse this format before, but none have
managed to get everything decoded, although many attempts and techniques have
been employed. A few examples of the ongoing work:

 - <https://github.com/kjagiello/hue-gradient-command-wizard/blob/main/src/modes/CustomGradient/utils.tsx>
 - <https://github.com/Koenkk/zigbee-herdsman-converters/pull/5192>
 - <https://github.com/zigpy/zha-device-handlers/issues/2517>

The best (and newest) work so far, is probably Krzysztof Jagiełło's "Hue Gradient Command Wizard":

 - <https://kjagiello.github.io/hue-gradient-command-wizard/>

This one gets much right, but is still missing quite a few details.

Another invaluable resource when researching XY-based lights, is Thomas Lochmatter's RGB/XY converter:

 - <https://viereck.ch/hue-xy-rgb/>

## Examples

Here are some examples of the zigbee messages discussed in this document (hex encoded):

 - `50010000135000fffff3620c400f5bf4120d400f5b0cf4f43858`
 - `ab00012e6f2f40100f7f`
 - `51010104000d30040000fa441eb7cb49bff65f1800`
 - `19000132518f530400`
 - `1100000800`
 - `bb0001feb575156904000a80`
 - `51010104001350020000fa441e590834b7cb49ff8857bff65f2800`

At first glance, there's no obvious repeating pattern or fixed header in this
format, but with a combination of careful analysis and applied elbow grease, we
have managed to reverse the format in its entirety.

## Current state of the art (in zigbee-herdsman-converters)

The current state of the art in zigbee-herdsman-converters
(`srd/lib/philips.ts`) has patchy support for a few advanced features, but is
riddled with errors and inaccuracies. It also suffers from being written before a
complete understanding of the format was available, widely using "magic" numbers
that happen to work, although they may not be a good fit in the bigger picture.

There is great potential for improving zigbee-herdsman-converters, using the
information found in this repository (and in particular, this file).

# Frame format

Okay, let's start looking at the actual format now.

Philips hue lights work as simple I/O devices with up to 9 properties.

Each message to cluster 0xFC03 can update any chosen subset of these properties
as desired.

When sending an update, only the included properties will be affected. All other
properties will retain their previous values.

### Header

The first two bytes of the message form a little-endian integer that contains
these flags:

```text
 FEDCBA98 76543210
 xxxxxxxx xxxxxxxx
 |||||||| ||||||||
 |||||||| |||||||'--> ON_OFF
 |||||||| ||||||'---> BRIGHTNESS
 |||||||| |||||'----> COLOR_MIREK
 |||||||| ||||'-----> COLOR_XY
 |||||||| |||'------> FADE_SPEED
 |||||||| ||'-------> EFFECT_TYPE
 |||||||| |'--------> GRADIENT_PARAMS
 |||||||| '---------> EFFECT_SPEED
 ||||||||
 |||||||'-----------> GRADIENT_COLORS
 ||||||'------------> UNUSED_9
 |||||'-------------> UNUSED_A
 ||||'--------------> UNUSED_B
 |||'---------------> UNUSED_C
 ||'----------------> UNUSED_D
 |'-----------------> UNUSED_E
 '------------------> UNUSED_F
```

As an example, let us consider the message:

`530101c00400135000000094031fda1955b98347f00468c426792800`

The first bytes are [0x53, 0x01], which is 0x0153 in little-endian:

```text
   0x01     0x53
  /    \   /    \
 .......1 01010011
        | ||||||||
        | |||||||'--> ON_OFF
        | ||||||'---> BRIGHTNESS
        | |||||'----> -
        | ||||'-----> -
        | |||'------> FADE_SPEED
        | ||'-------> -
        | |'--------> GRADIENT_PARAMS
        | '---------> -
        |
        '-----------> GRADIENT_COLORS
```

Now we can read the properties that have their corresponding flag set.

## Field order

The fields are always read this this order:

| Field             | Size     |
|-------------------|----------|
| `ON_OFF`          | 1 byte   |
| `BRIGHTNESS`      | 1 byte   |
| `COLOR_MIREK`     | 2 bytes  |
| `COLOR_XY`        | 4 bytes  |
| `FADE_SPEED`      | 2 bytes  |
| `EFFECT_TYPE`     | 1 byte   |
| `GRADIENT_COLORS` | variable |
| `EFFECT_SPEED`    | 1 byte   |
| `GRADIENT_PARAMS` | 2 bytes  |

After reading the message in this manner, there shouldn't be any bytes left over.

However, this is seemingly a protocol that has been extended a few times (as can
be observed from the newest flags occupying the highest bits, for instance).

This would theoretically allow older devices to simply ignore all unknown flags,
and the corresponding "tail" of the message, while still reacting to the
properties they understand.

It is unknown if Hue devices actually operate in this way, or if they would
reject messages they do not fully understand.

Certainly, invalid messages with known flags are readily reject (and thus,
completely ignored), if they cannot be parsed 100% successfully.

### Property: `ON_OFF`

Size: 1 byte.

Light is turned off if `0`, or on otherwise.

### Property: `BRIGHTNESS`

Size: 1 byte.

NOTE: Values `0` and `255` are INVALID.

Valid range is `1..254` (dimmest to brightest, respectively)

### Property: `COLOR_MIREK`

Size: 2 bytes (little-endian)

Contains the color temperature in MIREK.

Typically valid range is `153` - `500` (both inclusive).

### Property: `COLOR_XY`

Size: 2 + 2 bytes (X, Y)

The color of the light, in XY format.

These coordinates are encoded as 16-bit little-endian integers, each
representing a fixed-point number in the range `0`..`1`.

Here 0 represents `0.0` and `0xFFFF` represents `1.0`.

### Property: `FADE_SPEED`

Size: 2 bytes (little-endian)

This number sets the transition speed for applying the new properties.

A value of 0 makes all transitions as fast as possible (practically
instant). Typical practical values are in the range `2..8`.

While values above 0x100 are possible, these cause very slow
transitions. However, the animation is running inside the light, so this could
be a good way to enable smooth, lightweight light transitions.

### Property: `EFFECT_TYPE`

Size: 1 byte (specifically, [`EffectType`])

| Name         | Value |
|--------------|-------|
| `NoEffect`   | 0x00  |
| `Candle`     | 0x01  |
| `Fireplace`  | 0x02  |
| `Prism`      | 0x03  |
| `Sunrise`    | 0x09  |
| `Sparkle`    | 0x0a  |
| `Opal`       | 0x0b  |
| `Glisten`    | 0x0c  |
| `Underwater` | 0x0e  |
| `Cosmos`     | 0x0f  |
| `Sunbeam`    | 0x10  |
| `Enchant`    | 0x11  |

This enables one of the specific, known effects in the [`EffectType`] enum. Most
(all?) effects allow setting other properties (such as color xy or color
temperature) while the effect is active.

This is how custom effects like "Purple Fireplace" or "Blue Candle" from the Hue
app are activated.

### Property: `GRADIENT_COLORS`

For gradient light strips, this property allows setting a number of independent
colors at once.

The gradient colors black is the most complicated of the property data blocks
used in this format. It has the following layout:

```text

  ┌───────────┬───┬───┬───┬───┬───┬───┬───┬───┐
  │ Byte  Bit ► 7 │ 6 │ 5 │ 4 │ 3 │ 2 │ 1 │ 0 │
  ├─▼─────────┼───┴───┴───┴───┴───┴───┴───┴───┤
  │ 0         │ .size (excluding this field)  │
  ├───────────┼───────────────┬───────────────┤
  │ 1         │ .color_count  │ MUST be zero! │
  ├───────────┼───────────────┴───────────────┤
  │ 2         │ .gradient_style               │
  ├───────────┼───────────────────────────────┤
  │ 3         │ Reserved (seems unused)       │
  │           │                               │
  │ 4         │                               │
  ├───────────┼───────────────────────────────┤
  │ 5+3*index │ .color_x (low 8 bits)         │\
  │           ├───────────────┐ - - - - - - - │ \
  │ 6+3*index │ (low 4 bits)  │ (high 4 bits) │  Repeated {.color_count} times
  │           │ - - - - - - - └───────────────┤ /
  │ 7+3*index │ .color_y (high 8 bits)        │/
  ├───────────┼───────────────────────────────┤
  :           :                               :
  :           :                               :
```

The gradient style *must* be one of these values:

```rust
pub enum GradientStyle {
    Linear = 0x00,
    Scattered = 0x02,
    Mirrored = 0x04,
}
```

### Property: `GRADIENT_COLORS`: Color encoding

The gradient colors format can specify up to and including 9 colors, even for
light strips with fewer than 9 segments. Any attempt to specify 10 or more
colors will result in the entire message being rejected.

Each color is packed into 3 bytes, representing 12 bits for the `X` and `Y`
color coordinate, respectively. These bytes are packed in an odd way. The
following code snippet demonstrates how to unpack them:

```rust
let x = u16::from(bytes[0]) | u16::from(bytes[1] & 0x0F) << 8;
let y = u16::from(bytes[2]) << 4 | u16::from(bytes[1] >> 4);
```

And packing:

```rust
let bytes: [u8; 3] = [
    (x & 0xFF) as u8,
    (((x >> 8) & 0x0F) | ((y & 0x0F) << 4)) as u8,
    (y >> 4 & 0xFF) as u8,
];
```

These 12-bit values are fractional values, but NOT in the unit range 0..1 as
might be expected.

Instead, the coordinates are scaled so that precision is not wasted on useless
coordinates outside the visible light spectrum, for example.

Other implementations all seem to make this guess about the scaling:

```rust
const maxX = 0.7347;
const maxY = 0.8431;
```

As far as I can tell, these numbers appeared at one point in someone's
implementation (probably as a best guess), and have been mercilessly copy-pasted
since then. If anyone can show a good source for why these numbers would be
correct, please let me know!

Now, the X coordinate makes a lot of sense, and is right as far as I can tell.

The value 0.7347 is the maximum X value inside the visible light spectrum.
For a visual illustration of this, see <https://viereck.ch/hue-xy-rgb/>.

The "Wide" color gamut also has this exact number in its specification, as the X
value of the "Red" coordinate, specifically.

However, the Y value doesn't match any source I can find.

If the scaling matches the Wide Gamut, the Y value (maximum height) should be
`0.8264`. It it matches the top of the visible light area, it should be around
`0.836`. I have no idea where `0.8431` comes from!

From experimentation, I have determined that the most likely candidate is the
outer bounds of the wide gamut, leading to the following values:

```rust
const MAX_X: f64 = 0.7347;
const MAX_Y: f64 = 0.8264;
```

These are then the scaling values used when serializing/deserializing the 24-bit
(X,Y) values in the gradient colors.

In other words, X values from `0` to `0xFFF` represent the X-coordinates `0.0`
to `0.7347`, while Y values in the same range represent Y-coordinates from `0.0`
to `0.8264`.

### Property: `EFFECT_SPEED`

Size: 1 byte

This property controls the animation speed for effects (see `EFFECT_TYPE`).

All values in the range `0`..`255` seem to be allowed, with `0` being the
slowest and `255` the fastest.

Curiously, the Hue app does not use the full range for all effects, and at high
values, some animations are rendered so quickly that they start to break down.

A good starting point seems to be 128 (representing 0.5).

### Property: `GRADIENT_PARAMS`

Size: 2 bytes (`scale`, `offset`)

The gradient parameters block contain two bytes, describing the `scale` (first
byte) and `offset` (second byte).

Both bytes are in fixed-point format, with the upper 5 bits representing the
integer portion, and the lower 3 bits the fractional part:

```text
| 7 | 6 | 5 | 4 | 3 | 2 | 1 | 0 |
\                  / \          /
 \________________/   \________/
       integer         fraction

```

Here are some examples of translating to/from this fixed-point format:

| Encoded value | Quotient | Numeric value |
|---------------|----------|---------------|
| 0x00          | 0/8      | 0.0           |
| 0x01          | 1/8      | 0.125         |
| 0x04          | 4/8      | 0.5           |
| 0x08          | 8/8      | 1.0           |
| 0x38          | 56/8     | 7.0           |
| 0x39          | 57/8     | 7,125         |
| 0x3a          | 58/8     | 7.25          |

#### Property: `GRADIENT_PARAMS`: `scale`

For a gradient light strip, the `scale` value determines how "wide" the gradient
colors are rendered. Specifically, a gradient light strip will scale the
gradient colors to fit "scale" colors on the strip.

As an example, the Hue Play gradient lightstrip for PC (model `LCX005`) has 42
LEDs, but only 7 independent sections. Each group of 6 LEDs can thus be thought
of as one "pixel".

With such a low pixel count, `scale` greatly affects the resulting colors when
updating the gradient strip.

For sharp, clear colors, the scale should match the number of segments in the
light strip. Again using the `LCX005` as an example, a good value for `Linear`
gradient mode is `0x38` (since this represents `7.0` in the fixed-point
notation).

This allows each of the colors to fit exactly in a segment on the strip, but
other values are possible too, of course. For example, `0x08` (= `1.0`) will
show one color on the entire strip, while `0x10` (= `2.0`) will show a smooth
transition between the first and second colors.

The above example is for the `Linear` gradient style only. The `Mirrored` mode
uses the middle segment as the base, and thus has 3 available (mirrored)
segments on either side, on a 7-segment light strip.

The `Scattered` mode always shows colors aligned with segments. As a result,
`scale` is ignored in this mode.

| Gradient style:                   | Linear | Mirrored | Scattered |
|-----------------------------------|--------|----------|-----------|
| Scale to show single color        | 0x08   | 0x08     | N/A       |
| Scale to fade between 2 colors    | 0x10   | 0x10     | N/A       |
| Scale to fit colors to 7 segments | 0x38   | 0x20     | N/A       |

NOTE: The `scale` value MUST BE at least `0x08` (= `1.0`)

NOTE: The `scale` value `0x00` is a special condition. It stretches the gradient
colors to exactly fit the gradient strip. Kind of a "zoom to fit" option.

#### Property: `GRADIENT_PARAMS`: `offset`

This property is simpler than `scale`. When rendering the gradient colors to the
light strip, the first `offset` lights are skipped.

For example, assume we have these abstract values for gradient light colors:

```text
|-------------------|
| A | B | C | D | E |
|-------------------|
```

With `offset` set to `0`, the colors will be rendered starting with `A`, so
[`A`, `B`, `C`, ...].

With `offset` set to `0x08` (= `1.0` in the fixed-point format), the first color
shown will be `B`, so [`B`, `C`, `D`, ...], and so forth.

For *fractional* offset values, proportional blending is used to emulate the
sub-pixel offset. With an offset of `0x04` (= `0.5`), the rendered colors will be:

 - `50% A + 50% B`
 - `50% B + 50% C`
 - `50% C + 50% D`
 - ...

If unsure, a good value for `offset` is `0x00`.
