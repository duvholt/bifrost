# Bifrost crates

```
 ┌─────────────────────┐ ┌─────────────────────┐
 │       Bifrost       │ │   bifrost-frontend  │
 └──┬────┬───────┬─┬─┬─┘ └┬────────────┬───────┘
    │    │       │ │ └────┼────────────┼─────┐
    │    │       │ └──────┼──────┐     │     │
    │    ▼       ▼        │      ▼     ▼     │
    │ ┌─────┐ ┌─────┐     │ ┌──────────────┐ │
    │ │ z2m │ │ zcl │     │ │  bifrost-api │ │
    │ └──┬──┘ └──┬──┘     │ └──┬───────┬───┘ │
    │    │       │        │    │       │     │
    ▼    ▼       ▼        ▼    ▼       ▼     ▼
 ┌─────────────────────────────────┐ ┌─────────┐
 │               hue               │ │   svc   │
 └─────────────────────────────────┘ └─────────┘
```


## `hue`: Philips Hue data models

Low-level data models for Philips Hue API requests, entertainment mode
streams, and Hue-specific Zigbee encodings.

## `zcl`: Zigbee Cluster Library

Serializing and deserializing support for ZCL (Zigbee Cluster Library)
frames. Rudimentary support for standard frames, and cutting-edge support for
Hue-specific frames.

## `z2m`: Zigbee2MQTT interface

Rust code for interfacing with Zigbee2MQTT. Serializing and deserializing
support for z2m messages.

## `svc`: Service management

Crate to manage, control and communicate with running "services".

These services (rust functions) are managed through service manager;
a minimal systemd-inspired service "daemon".

Since all "services" controlled by `svc` are rust functions, these are
*not* system services in the classical sense.

## `bifrost-api`: Bifrost API

Data structures and types that define the Bifrost-specific API supported by the
Bifrost server. This is used by `bifrost` and `bifrost-frontend` to have a
well-defined interface between them, but can be used by any program that wants
to communicate with a Bifrost server.

## `bifrost-frontend`: Dioxus web frontend for Bifrost

The Bifrost web frontend, made with Dioxus.

Compiles to a static site (html/css/js/wasm) that communicates with a Bifrost
server.
