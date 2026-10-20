---
name: Add a Parameter to the Metadata
description: Use when adding or updating parameter metadata in MPCraft — descriptions, ranges, bitmasks, enumerated options, and units — and when wiring a new hardware configuration page.
metadata:
  opencode/autoinvoke: true
---

# Adding parameter metadata in MPCraft

Parameter metadata is what turns a raw `PARAM_VALUE` into a usable editor with the right widget,
validation, and explanation.

## Where metadata lives

`crates/mpcraft-params/src/`

| File | Contents |
| --- | --- |
| `metadata.rs` | The `ParameterMetadata` model |
| `repository.rs` | Load, index, and query the metadata store |
| `xml.rs` | Parser for `ParameterMetaData.xml` |
| `types.rs` | `ParamType`, ranges, bitmasks, enumerated options |

The source of truth is upstream's `ParameterMetaData.xml` — see
`_ref_missionplanner/ParameterMetaDataBackup.xml` (4.1 MB) and
`_ref_missionplanner/ParameterFactMetaData.xml` (238 KB).

This is a **domain specification**, not code. Port it faithfully.

## The metadata model

```rust
pub struct ParameterMetadata {
    pub name: String,
    pub human_name: String,
    pub description: String,
    pub units: Option<String>,
    pub values: Vec<ParamOption>,   // enumerated options
    pub bitmask: Option<u32>,        // bit flags
    pub range: Option<(f64, f64)>,   // min/max
    pub increment: Option<f64>,
    pub reboot_required: bool,
    pub vehicle_types: Vec<VehicleType>,
}
```

## What the frontend does with it

The Rust side resolves metadata and hands the frontend a **resolved widget descriptor**. The
frontend never parses XML or reasons about bitmasks.

```rust
pub enum ParamWidget {
    Number { min: f64, max: f64, step: f64, unit: Option<String> },
    Enum { options: Vec<(String, f64)> },
    Bitmask { bits: Vec<(String, u32)> },
    Text,
}
```

## Adding a single parameter

1. Add the entry to the XML under the right vehicle group.
2. Confirm the parser indexes it: `repository.get("ARMING_CHECK")` returns it.
3. Add a unit test asserting name, human name, range, and type.
4. If it has enumerated values, the frontend gets a dropdown automatically — verify it renders.

## Adding a bitmask parameter

Bitmask parameters need a bit→label mapping. The XML encodes this in the `Bitmask` attribute.
Confirm the parser splits it into individual bits with their labels — a bitmask that renders as
one opaque number is a bug.

## Adding a new hardware configuration page

Hardware pages are **generated from metadata**, not hand-written per parameter. A new page is:

1. A metadata group in the XML.
2. A page definition in `crates/mpcraft-params/src/pages.rs` naming the group, the title, and the
   visibility conditions (vehicle type, connected state).
3. A frontend page that renders the group's parameters using the generic widget renderer.

Do **not** write a bespoke React form per parameter. The whole point of the metadata pipeline is
that adding a parameter requires no UI work.

## Parameter types

Upstream uses eight `MAV_PARAM_TYPE` wire types. MPCraft normalises to `f64` internally but must
preserve the reported type, because ArduPilot's wire type determines how the value is encoded on
write. Getting this wrong silently corrupts integer parameters.

- If the vehicle advertises `MAV_PROTOCOL_CAPABILITY::PARAM_FLOAT`, send `REAL32`.
- Otherwise, send the reported type.

## Testing

- Parser test against a small XML fixture.
- Index test: every parameter in the fixture is retrievable by name.
- Round-trip test: a downloaded parameter set resolves to metadata for the known parameters.
- A test that an unknown parameter name returns `None` rather than a default — the UI must be able
  to say "no metadata for this one".

## Checklist

- [ ] Metadata entry added to the XML
- [ ] Parser indexes it and the widget descriptor resolves correctly
- [ ] Unit test added
- [ ] Bitmask parameters render individual bits
- [ ] Wire type preserved for writes
- [ ] `docs/PARITY.md` updated if a new config page shipped
