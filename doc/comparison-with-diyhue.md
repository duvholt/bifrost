## Comparison with diyHue

You might already be familiar with [diyHue](https://github.com/diyhue/diyHue),
an existing project that aims to emulate a Philips Hue Bridge.

diyHue is a well-established project, that integrates with countless
servers/services/light systems, and emulates many Hue Bridge features.

However, I have been frustrated with diyHue's MQTT integration, and its fairly
poor performance when operating more than a handful of lights at a time. Since
diyHue always sends individual messages to each light in a group, large rooms
can get quite slow (multiple seconds for every adjustment, no matter how minor).

Currently, diyHue does not support Zigbee groups (or MQTT groups) at all,
whereas Bifrost is written specifically to present Zigbee2MQTT groups as Hue
Bridge "rooms". For zigbee/mqtt use cases, this massively increases performance
and reliability.

Another thing about diyHue that frustrates me to no end, is the lack of
(working) support for push notifications. If you use the Hue App to control a
diyHue bridge, you will notice that it does not react to any changes from other
phones, home automation, etc. Also, the reported light states (on/off, color,
temperature, etc) are sometimes just wrong.

Overall, diyHue can do an impressive number of things, but it seems to have some
pretty rough edges.

Just to clarify, I've enjoyed using diyHue, and I wish them all the best. It's
also very useful, both as a home automation service, and a reverse engineering
resource.

However, if you're also using one or more Zigbee2MQTT servers to control Zigbee
devices, feel free to give Bifrost a try. It might be a better fit for your use
case.

In any case, feedback always welcome.


| Feature                              | diyHue                                  | Bifrost                                   |
|--------------------------------------|-----------------------------------------|-------------------------------------------|
| Language                             | Python                                  | Rust                                      |
| Project scope                        | Broad (supports countless integrations) | Narrow (specifically targets Zigbee2MQTT) |
| Use Hue Bridge as backend            | ✅                                      | ❌                                        |
| Usable from Homeassistant            | ✅ (as a Hue Bridge)                    | ✅ (as a Hue Bridge)                      |
| Control individual lights            | ✅                                      | ✅                                        |
| Good performance for groups of light | ❌ (sends a message per light)          | ✅ (uses zigbee groups)                   |
| Connect to Zigbee2MQTT               | (✅) (but only one server)              | ✅ (multiple servers supported)           |
| Auto-detection of color features     | ❌ (needs manual configuration)         | ✅                                        |
| Create Zigbee2MQTT scenes            | ❌                                      | ✅                                        |
| Recall Zigbee2MQTT scenes            | ❌                                      | ✅                                        |
| Learn Zigbee2MQTT scenes             | ❌                                      | ✅                                        |
| Delete Zigbee2MQTT scenes            | ❌                                      | ✅                                        |
| Join new zigbee lights               | ✅                                      | ❌                                        |
| Add/remove lights to rooms           | ❌                                      | ✅                                        |
| Live state of lights in Hue app      | ❌ [^1]                                 | ✅                                        |
| Multiple type of backends            | ✅                                      | ❌ (only Zigbee2MQTT)                     |
| Entertainment zones                  | ✅                                      | ✅                                        |
| Zigbee Entertainment mode support    | ❌                                      | ✅                                        |
| Hue effects (fireplace, candle, etc) | (✅) (partial)                          | ✅                                        |
| Routines / Wake up / Go to sleep     | ✅                                      | ❌ (planned)                              |
| Remote services                      | (✅) (only with Hue essentials)         | ❌                                        |
| Add custom lights and switches       | ✅                                      | ❌                                        |

[^1]: Light state synchronization (i.e. consistency between hue emulator, hue
    app and reality) seems to be, unfortunately, somewhat brittle in diyHue. See
    for example:
    * https://github.com/diyhue/diyHue/issues/883
    * https://github.com/diyhue/diyHue/issues/835
    * https://github.com/diyhue/diyHue/issues/795
