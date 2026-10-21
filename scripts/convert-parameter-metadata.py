"""Converts Mission Planner's parameter metadata into the catalogue MPCraft reads.

Upstream keeps the metadata of the parameters in two files, and they complement each
other:

* `ParameterFactMetaData.xml` says what a parameter *is* — its type, its unit, the range
  it takes, the values it can be given — for seven hundred and fifty-nine parameters.

* `ParameterMetaDataBackup.xml` says what a parameter *is for* — the name it is shown
  with, what it does, the range it takes — for every parameter of every vehicle family:
  eleven thousand of them.

Together they are the knowledge of the autopilots, and the interface reads it through the
backend. The conversion is made once, into a TOML catalogue the `mpcraft-params` crate
carries, so that the application does not have to carry a parser for a two-megabyte XML
file to draw a list of parameters.

    python scripts/convert-parameter-metadata.py
"""

import io
import os
import sys
import xml.etree.ElementTree as ElementTree

ROOT = "_ref_missionplanner"

FACTS = os.path.join(ROOT, "ParameterFactMetaData.xml")

# The backups are the same file in several places; the biggest one has the most.
BACKUP = os.path.join(ROOT, "ParameterMetaDataBackup.xml")

TARGET = os.path.join("crates", "mpcraft-params", "data", "parameters.toml")

# The types the metadata uses, and the ones the catalogue uses.
KINDS = {
    "INT8": "int8",
    "UINT8": "uint8",
    "INT16": "int16",
    "UINT16": "uint16",
    "INT32": "int32",
    "UINT32": "uint32",
    "FLOAT32": "real32",
    "FLOAT64": "real64",
}

# The fields of the facts file, and the tag they come from.
FIELDS = (
    ("description", "short_desc"),
    ("long_description", "long_desc"),
    ("unit", "unit"),
    ("minimum", "min"),
    ("maximum", "max"),
    ("increment", "increment"),
    ("reboot_required", "reboot_required"),
)


def escape(text):
    """The text of the catalogue, with the characters TOML cares about."""
    if text is None:
        return ""

    return (
        text.replace("\\", "\\\\")
        .replace('"', '\\"')
        .replace("\n", " ")
        .replace("\r", " ")
        .strip()
    )


def find_backup():
    """The backup with the most parameters."""
    best = None
    best_count = 0

    for directory, _directories, files in os.walk(ROOT):
        for name in files:
            if name != "ParameterMetaDataBackup.xml":
                continue

            path = os.path.join(directory, name)

            try:
                root_element = ElementTree.parse(path).getroot()
            except ElementTree.ParseError:
                continue

            count = sum(
                len(list(family))
                for family in root_element
                if family.tag != "version"
            )

            if count > best_count:
                best = path
                best_count = count

    return best


def read_facts():
    """The parameters of the facts file, by name."""
    if not os.path.exists(FACTS):
        return {}

    root = ElementTree.parse(FACTS).getroot()
    found = {}

    for group in root.findall("group"):
        group_name = group.get("name", "")

        for parameter in group.findall("parameter"):
            name = parameter.get("name")
            if not name:
                continue

            entry = {"group": group_name}

            kind = parameter.get("type", "").upper()
            if kind in KINDS:
                entry["kind"] = KINDS[kind]

            default = parameter.get("default")
            if default is not None:
                entry["default"] = escape(default)

            for field, tag in FIELDS:
                element = parameter.find(tag)
                if element is not None and element.text:
                    entry[field] = escape(element.text)

            values = parameter.find("values")
            if values is not None:
                collected = [
                    escape(value.text)
                    for value in values.findall("value")
                    if value.text
                ]

                if collected:
                    entry["values"] = collected

            found[name] = entry

    return found


def read_backup(path):
    """The parameters of the backup, by name.

    The backup holds the documentation of every parameter of every vehicle family, and
    the same parameter is documented once per family.
    """
    found = {}

    if path is None:
        return found

    root = ElementTree.parse(path).getroot()

    for family in root:
        if family.tag == "version":
            continue

        family_name = family.tag

        for parameter in family:
            name = parameter.tag

            entry = found.setdefault(name, {"vehicles": [], "descriptions": []})
            entry["vehicles"].append(family_name)

            for tag, field in (
                ("DisplayName", "display_name"),
                ("Description", "description"),
                ("Range", "range"),
                ("User", "user"),
            ):
                element = parameter.find(tag)
                if element is not None and element.text and field not in entry:
                    entry[field] = escape(element.text)

    return found


def main():
    if not os.path.exists(FACTS):
        print(f"the metadata was not found at {FACTS}", file=sys.stderr)
        return 1

    facts = read_facts()
    backup = read_backup(find_backup())

    names = sorted(set(facts) | set(backup))

    lines = [
        "# The metadata of the parameters of the autopilots.",
        "#",
        "# It is converted from Mission Planner's `ParameterFactMetaData.xml` and",
        "# `ParameterMetaDataBackup.xml`, which are domain specifications: the description",
        "# of a parameter, its unit, the range it takes and the values it can be given are",
        "# the autopilots' knowledge, not the interface's.",
        "#",
        "# The conversion is made by `scripts/convert-parameter-metadata.py`.",
        "",
    ]

    for name in names:
        entry = dict(facts.get(name, {}))
        documentation = backup.get(name, {})

        for field in ("display_name", "description", "range", "user"):
            if field not in entry and field in documentation:
                entry[field] = documentation[field]

        vehicles = documentation.get("vehicles", [])
        if vehicles:
            entry["vehicles"] = sorted(set(vehicles))

        lines.append("[[parameters]]")
        lines.append(f'name = "{escape(name)}"')

        for field in (
            "group",
            "display_name",
            "kind",
            "default",
            "description",
            "long_description",
            "unit",
            "minimum",
            "maximum",
            "increment",
            "range",
            "reboot_required",
            "user",
        ):
            value = entry.get(field)
            if value:
                lines.append(f'{field} = "{escape(value)}"')

        values = entry.get("values", [])
        if values:
            listed = ", ".join(f'"{escape(value)}"' for value in values)
            lines.append(f"values = [{listed}]")

        vehicles = entry.get("vehicles", [])
        if vehicles:
            listed = ", ".join(f'"{escape(value)}"' for value in vehicles)
            lines.append(f"vehicles = [{listed}]")

    os.makedirs(os.path.dirname(TARGET), exist_ok=True)
    io.open(TARGET, "w", encoding="utf-8", newline="\n").write("\n".join(lines) + "\n")

    size = os.path.getsize(TARGET)
    print(
        f"{len(names)} parameters written to {TARGET} ({size // 1024} KB): "
        f"{len(facts)} from the facts, {len(backup)} from the documentation"
    )

    return 0


if __name__ == "__main__":
    sys.exit(main())
