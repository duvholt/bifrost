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

## Cluster-specific commands

### Command 1: Update entertainment zone

This is the major command used to send a "frame" of Hue Entertainment data.

Sending it to a Hue lamp will cause that lamp to repeat it in broadcast mode,
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
state for a light (a lamp, or single segment of a multi-segment light source).



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


### Command 3: Synchronize entertainment mode

This command is used to synchronize the sequence number in an entertainment
group. The first two bytes are unknown.

```c
struct {
    x0: u8, // always 0
    x1: u8, // seen as 0 or 1. unknown
    counter: u32, // frame counter for entertainment group
}
```

### Command 4: Retrieve segment mapping

This command is used to retrieve the segment mapping for a hue lamp.

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
00 00 07 0001 0101 0201 0301 0401 0501 0601
|      | |                                |
\      / \                               /
 header   \                             /
           \                           /
             seven segment descriptors
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

// example: 0007 98d2 99d2 9ad2 9bd2 9cd2 9dd2 9ed2
```

#### Response

```c
struct {
  x0: u16,
}

// example: 0000
```

The only observed response is `0000`, which indicates success.

Running this command on a Hue device that does not have multiple segments (i.e,
a regular Hue lamp) gets a "Command Not Supported" standard Zigbee response, so
returning `0000` seems to be a safe way to detect success.

## Attributes

| Attr   | Type | Desc                 | Sample value |
|--------|------|----------------------|--------------|
| 0x0005 | u8   | Light balance factor | `0xFE`       |

So far the only attribute known on this cluster is `0x005`, which sets the light
level balancing for entertainment mode.

This is a feature where lights can be dimmed relatively, so certain lights
aren't blindingly bright. Just like regular brightness updates, the valid range
is `0x01` to `0xFE`.

This should always be set to `0xFE`, unless you want to dim the light in
entertainment mode.

# Cluster 0xFC02

Never seen. Maybe they skipped a number?

# Cluster 0xFC03: Gradients, Effects, Animations



## Cluster-specific commands

### Command 0: Write combined state

This is perhaps the single most complicated Hue command. It is used to
simultaneously set all supported properties of a Hue lamp.

It has been extensively [documented in a separate document](hue-zigbee-format.md).

## Attributes

| Attr   | Type        | Desc                    | Sample value from Hue device |
|--------|-------------|-------------------------|------------------------------|
| 0x0001 | u32 bitmap  | ?                       | `00 00 00 0F`                |
| 0x0002 | octets      | u8: len, combined state | `08 0B 00 01 1F 67 98 26 4F` |
| 0x0010 | u16         | ?                       | `00 01`                      |
| 0x0011 | u64         | ?                       | `00 00 00 00 00 03 FE 0E`    |
| 0x0012 | u32         | ?                       | `00 00 00 03`                |
| 0x0013 | u16         | maybe `segment_count`   | `00 07`                      |
| 0x0030 | unsupported | ?                       | -                            |
| 0x0032 | u8          | ?                       | `00`                         |
| 0x0033 | u8          | ?                       | `00`                         |
| 0x0034 | u8          | ?                       | `03`                         |
| 0x0034 | u8          | ?                       | `03`                         |
| 0x0035 | u8          | ?                       | `FE`                         |
| 0x0036 | u8          | ?                       | `4F`                         |
| 0x0037 | unsupported | ?                       | -                            |
| 0x0038 | u16         | maybe `segment_count`   | `00 07`                      |

# Cluster 0xFC04

Very rarely observed. Only seen with ZCL: Read Attributes.

## Attributes

| Attr   | Type       | Desc | Sample |
|--------|------------|------|--------|
| 0x0000 | u16 bitmap | ?    | 0x1007 |
|        |            |      | 0x0007 |
