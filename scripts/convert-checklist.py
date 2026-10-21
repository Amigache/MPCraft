"""Converts Mission Planner's pre-flight checklist into the catalogue MPCraft reads.

Upstream keeps the checklist in an XML file: a dozen items, each one watching a value of
the vehicle or a parameter and comparing it with a threshold, and half of them being
questions the pilot answers by hand.

The conditions are the vehicle's knowledge, not the interface's, so the catalogue lives in
the application crate and the interface only shows what the rules say.

    python scripts/convert-checklist.py
"""

import io
import os
import re
import sys
import xml.etree.ElementTree as ElementTree

SOURCE = os.path.join("_ref_missionplanner", "checklistDefault.xml")

TARGET = os.path.join("crates", "mpcraft-app", "data", "checklist.toml")

# The values of the vehicle that the checklist watches, and the names MPCraft gives them.
SOURCES = {
    "gpsstatus": "gpsStatus",
    "satcount": "satCount",
    "linkqualitygcs": "linkQuality",
    "battery_voltage": "batteryVoltage",
    "mode": "mode",
    "alt": "altitude",
}

# The conditions, as they are named upstream.
CONDITIONS = {
    "NONE": "none",
    "LT": "lt",
    "LTEQ": "lte",
    "EQ": "eq",
    "GT": "gt",
    "GTEQ": "gte",
    "NEQ": "neq",
}


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


def main():
    if not os.path.exists(SOURCE):
        print(f"the checklist was not found at {SOURCE}", file=sys.stderr)
        return 1

    root = ElementTree.parse(SOURCE).getroot()

    lines = [
        "# The pre-flight checklist.",
        "#",
        "# It is converted from Mission Planner's `checklistDefault.xml`, which is a domain",
        "# specification: what has to be true before the vehicle flies is the vehicle's",
        "# knowledge, and the interface only shows what the rules say.",
        "#",
        "# The conversion is made by `scripts/convert-checklist.py`.",
        "",
    ]

    total = 0
    manual = 0

    for item in root.findall("CheckListItem"):
        name = (item.findtext("Name") or "").strip()
        description = (item.findtext("Description") or "").strip()
        text = (item.findtext("Text") or "").strip()
        condition = (item.findtext("ConditionType") or "NONE").strip()
        trigger = (item.findtext("TriggerValue") or "0").strip()

        if not description:
            continue

        # A question the pilot answers is not watched: it has no value.
        if name not in SOURCES:
            manual += 1
            lines.append("[[items]]")
            lines.append(f'description = "{escape(description)}"')
            lines.append('source = "manual"')
            continue

        lines.append("[[items]]")
        lines.append(f'description = "{escape(description)}"')
        lines.append(f'source = "{SOURCES[name]}"')
        lines.append(f'condition = "{CONDITIONS.get(condition.upper(), "none")}"')
        lines.append(f"trigger = {trigger}")
        lines.append(f'text = "{escape(text)}"')

        total += 1

    os.makedirs(os.path.dirname(TARGET), exist_ok=True)
    io.open(TARGET, "w", encoding="utf-8", newline="\n").write("\n".join(lines) + "\n")

    print(
        f"{total} automatic checks and {manual} questions written to {TARGET} "
        f"({os.path.getsize(TARGET) // 1024} KB)"
    )

    return 0


if __name__ == "__main__":
    sys.exit(main())
