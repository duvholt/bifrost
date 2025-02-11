# The Color of Magic: Reversing the Hue Zigbee Clusters

This document, which builds on the [initial work](hue-zigbee-format.md), aims to
compile all available information about the custom Zigbee messages used by
Philips Hue devices, and in particular, lights.

The following text refers to commands and attributes on Hue devices. This has
been researched using the following units:

## "Hue Bulb"

 - "Hue white and color ambiance E27 1100lm"
 - Model `LCA006`
 - Firmware 1.122.2 (20240902)

## "Hue Gradient strip"

 - "Hue Play gradient lightstrip for PC"
 - Model `LCX005`
 - Firmware 1.122.2 (20240902)

# Cluster 0xFC00: Hue Button events

Used by hue buttons to report button events and other state changes.

## Cluster-specific commands

### Command 0: Button Event

These are mostly documented elsewhere, and because they are button events, they
are not the main focus of this document.

# Cluster 0xFC01: Entertainment

This cluster is used to control "Entertainment Zones", a defining feature of the
Hue ecosystem.

## Cluster-specific commands

### Command 1: Update entertainment zone

This is the major command used to send a "frame" of Hue Entertainment data.

Sending it to a Hue bulb will cause that bulb to repeat it in broadcast mode,
for other devices to pick up.

```text
  ┌───────────┬───┬───┬───┬───┬───┬───┬───┬───┐
  │ Byte  Bit ► 7 │ 6 │ 5 │ 4 │ 3 │ 2 │ 1 │ 0 │
  ├─▼─────────┼───┴───┴───┴───┴───┴───┴───┴───┤
  │ 0         │ .counter                      │
  │           │                               │
  │ 1         │                               │
  │           │                               │
  │ 2         │                               │
  │           │                               │
  │ 3         │                               │
  ├───────────┼───────────────────────────────┤
  │ 4         │ .x0 (unknown)                 │
  │           │                               │
  │ 5         │ Always observed as 0x0400     │
  ├───────────┼───────────────────────────────┤
  │ 6         │ Light data block 0            │\
  │           │                               │ \
  │ ..        │                               │  Repeated for each light
  │           │                               │ /
  │ 12        │                               │/
  ├───────────┼───────────────────────────────┤
  : 13        : Light data block 1            :
  :           :                               :
```

Each "light data block" is a 7-byte packed structure describing the desired
state for a light (a bulb, or single segment of a multi-segment light source).



```text
 ┌───────────┬───┬───┬───┬───┬───┬───┬───┬───┐
 │ Byte  Bit ► 7 │ 6 │ 5 │ 4 │ 3 │ 2 │ 1 │ 0 │
 ├─▼─────────┼───┴───┴───┴───┴───┴───┴───┴───┤
 │ 0         │ .addr                         │
 │           │ Zigbee address (or alias)     │
 │ 1         │ for the target light          │
 ├───────────┼───────────────────────────────┤
 │ 2         │ .brightness (11 bits)         │
 │           │ - - - - - ┌───────────────────┤
 │ 3         │           │ X   X   X   X   X │ <-- 5 reserved bits MUST be zero
 ├───────────┼───────────┴───────────────────┤
 │ 4         │ .color_x (low 8 bits)         │\
 │           ├───────────────┐ - - - - - - - │ \
 │ 5         │ (low 4 bits)  │ (high 4 bits) │  same format as for composite updates
 │           │ - - - - - - - └───────────────┤ /
 │ 6         │ .color_y (high 8 bits)        │/
 └───────────┴───────────────────────────────┘
```


### Command 3: Synchronize entertainment zone

This command is used to synchronize the sequence number in an entertainment
group. The first two bytes are unknown.

```c
struct {
    x0: u8, // only seen as 0
    x1: u8, // seen as 0 or 1. unknown function
    counter: u32, // frame counter for entertainment group
}
```

### Command 4: Retrieve segment mapping

This command is used to retrieve the segment mapping for a hue multi-segment
light.

#### Request

A single byte is sent. Only observed as `00` (might be an index for highly
addressable devices?).

#### Response

```c
struct Response {
  x0: u8, // unknown
  x1: u8, // unknown
  count: u8, // number of segments
  segments: [Segment], // segment descriptors
}

struct Segment {
  start: u8, // start index for segment
  length: u8, // segment length
}
```

As an example, the following is a real response from a Hue Gradient light strip:

```
           ┌───┬───First segment descriptor
           │   │
  00 00 07 00 01 01 01 02 01 03 01 04 01 05 01 06 01
  │      │ │                                       │
  └header┘ └───────Seven segment descriptors───────┘
```

This tells us the segments are arranged thus:

 - Start at `00`, length `01`
 - Start at `01`, length `01`
 - Start at `02`, length `01`
 - ...

These are all length 1. In other words, the layout is:

 `0, 1, 2, 3, 4, 5, 6`

### Command 7: Configure segments for entertainment mode (req/rsp)

Hue Entertainment frames consists of brightness and color data for up to 10
lights, all in a single frame.

Each light is identified by 2 bytes containing its zigbee network (short)
address.

For Hue devices that contain multiple lights (such as gradient strips), this
presents a problem, since the entire strip only has a single zigbee address!

To solve that problem, this command can be used on multi-segment devices to
configure each segment with a virtual address.

#### Request

```c
struct {
  count: u16,
  addresses: [count x u16],
}
```

Here is an example of a command that sets seven virtual addresses for a gradient
light strip with 7 segments:

```
        ┌───┬───Segment index 0
        │   │
  00 07 97 d2 98 d2 99 d2 9a d2 9b d2 9c d2 9d d2
  │   │ │                                       │
  └cnt┘ └───────Seven segment indices───────────┘

```

After this, the segments will respond the these addresses:

 - `0xD297`
 - `0xD298`
 - `0xD299`
 - `0xD29A`
 - `0xD29B`
 - `0xD29C`
 - `0xD29D`

#### Response

```c
struct {
  x0: u16,
}
```

The only observed response is `0000`, which indicates success.

Running this command on a Hue device that does not have multiple segments (i.e,
a regular Hue bulb) gets a "Command Not Supported" standard Zigbee response, so
returning `0000` seems to be a safe way to detect success.

## Attributes

Gradient strip:

| Attr   | Type | Value | Desc                       |
|--------|------|-------|----------------------------|
| `0000` | `b8` | `0F`  | ?                          |
| `0001` | `e8` | `00`  | ?                          |
| `0002` | `u8` | `0A`  | Probably max segment count |
| `0003` | `u8` | `04`  | Probably gradient-related  |
| `0004` | `u8` | `07`  | Probably segment count     |
| `0005` | `u8` | `FE`  | Light balance factor       |


Hue bulb:

| Attr   | Type | Value | Desc |
|--------|------|-------|------|
| `0000` | `b8` | `0B`  | ?    |
| `0001` | `e8` | `00`  | ?    |
| `0005` | `u8` | `FE`  | ?    |

Notice that attributes `0002`, `0003` and `0004` are not present on the hue
bulb. This supports the idea that these attributes are gradient-related.

So far the only attribute known on this cluster is `0x005`, which sets the light
level balancing for entertainment mode.

This is a feature where lights can be dimmed relatively, so certain lights
aren't blindingly bright. Just like regular brightness updates, the valid range
is `0x01` to `0xFE`. This should always be set to `0xFE`, unless you want to dim
the light in entertainment mode.

# Cluster 0xFC02

Never seen. Maybe they skipped a number?

# Cluster 0xFC03: Gradients, Effects, Animations

## Cluster-specific commands

### Command 0: Write combined state

This is perhaps the single most complicated Hue command. It is used to
simultaneously set all supported properties of a Hue bulb.

It has been extensively [documented in a separate document](hue-zigbee-format.md).

After setting the state with this command, it can be read back as property
`0x0002` (see below).

## Attributes

Sample data from gradient strip:

| Attr   | Type  | Value              | Desc            |
|--------|-------|--------------------|-----------------|
| `0001` | `b32` | `0000000F`         | ?               |
| `0002` | `hex` | `0700010a6e01`     | Composite state |
| `0010` | `b16` | `0001`             | ?               |
| `0011` | `b64` | `000000000003FE0E` | ?               |
| `0012` | `b32` | `00000003`         | ?               |
| `0013` | `b16` | `0007`             | ?               |
| `0031` | `u16` | `04E2`             | ?               |
| `0032` | `u8`  | `00`               | ?               |
| `0033` | `u8`  | `00`               | ?               |
| `0034` | `u8`  | `03`               | ?               |
| `0035` | `u8`  | `FE`               | ?               |
| `0036` | `u8`  | `4F`               | ?               |
| `0038` | `u16` | `0007`             | ?               |

From hue bulb:

| Attr   | Type  | Value              | Desc            |
|--------|-------|--------------------|-----------------|
| `0001` | `b32` | `00000007`         | ?               |
| `0002` | `hex` | `070001176f01`     | Composite state |
| `0010` | `b16` | `0001`             | ?               |
| `0011` | `b64` | `000000000003FE0E` | ?               |
| `0012` | `b32` | `00000000`         | ?               |

The bulb supports noticably fewer properties, which makes it likely that the
missing ones are related to gradient handling.

# Cluster 0xFC04

Very rarely observed. Only seen with ZCL: Read Attributes.

## Attributes

Gradient strip:

| Attr   | Type  | Value      | Desc |
|--------|-------|------------|------|
| `0000` | `b16` | `1007`     | ?    |
| `0001` | `b16` | `0000`     | ?    |
| `0002` | `b16` | `0000`     | ?    |
| `0010` | `u32` | `00000000` | ?    |
| `0011` | `u32` | `00000000` | ?    |
| `0012` | `u32` | `00000000` | ?    |
| `0013` | `u32` | `00000000` | ?    |

Hue Bulb:

| Attr   | Type  | Value      | Desc |
|--------|-------|------------|------|
| `0000` | `b16` | `1007`     |      |
| `0001` | `b16` | `0000`     |      |
| `0002` | `b16` | `0000`     |      |
| `0010` | `u32` | `00000000` |      |
| `0011` | `u32` | `00000000` |      |
| `0012` | `u32` | `00000000` |      |
| `0013` | `u32` | `00000000` |      |
