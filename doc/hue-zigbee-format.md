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

## State of the art (zigbee-herdsman-converters)

The current state of the art in zigbee-herdsman-converters
(`srd/lib/philips.ts`) has patchy support for a few advanced features, but is
riddled with errors and inacuracies. It also suffers from being written before a
complete understanding of the format was availeble, widely using "magic" numbers
that happen to work, although they may not be a good fit in the bigger picture.

There is great potential for improving zigbee-herdsman-converters, using the
information found in this repository (and in particular, this file).

## Frame format

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

  - `ON_OFF`
  - `BRIGHTNESS`
  - `COLOR_MIREK`
  - `COLOR_XY`
  - `FADE_SPEED`
  - `EFFECT_TYPE`
  - `GRADIENT_COLORS`
  - `EFFECT_SPEED`
  - `GRADIENT_PARAMS`

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

Here 0 represents `0.0` and `0xFFF` represents `1.0`.

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

This enables one of the specific, known effects in the [`EffectType`] enum. Most
(all?) effects allow setting other properties (such as color xy or color
temperature) while the effect is active.

This is how combinations like "Purple Fireplace" or "Blue Candle" are made from
the Hue app.

### Property: `GRADIENT_COLORS`

For gradient light strips, this property allows setting a number of independent colors

### Property: `EFFECT_SPEED`

Size: 1 byte

This property controls the animation speed for effects (see `EFFECT_TYPE`).

All values in the range `0`..`255` seem to be allowed, with `0` being the
slowest and `255` the fastest.

Curiously, the Hue app does not use the full rnage for all effects, and at high
values, some animations are rendered so quickly that they start to break down.

A good starting point seems to be 128 (representing 0.5).

### Property: `GRADIENT_PARAMS`

Size: 2 x 1 byte
