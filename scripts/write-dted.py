"""Writes a DTED cell, for testing and for demonstrating the ground.

    python scripts/write-dted.py --output ground.dt1 --lat -35.3 --lng 149.1

The cell is a level 1 one: a square of the ground whose heights are written post by
post. The ground of this one is a slope that rises towards the north, with a hill in
the middle, so that a survey that follows it can be seen to follow it.

The format is the one of the standard: an eighty byte header that says where the cell
is and how fine it is, the record of the data set, the record of the accuracy, and then
one record per column of the grid, each of them with its own checksum.
"""

import argparse
import math
import struct

# The sizes of the blocks of the file.
UHL_SIZE = 80
DSI_SIZE = 648
ACC_SIZE = 2700
DATA_OFFSET = UHL_SIZE + DSI_SIZE + ACC_SIZE
RECORD_HEADER_SIZE = 8
CHECKSUM_SIZE = 4

# The tenths of an arc-second, which is the unit the spacings are written in.
TENTH_OF_ARC_SECOND = 1.0 / 3600.0 / 10.0


def write_number(data, offset, width, value):
    """Writes a number into a field of the header."""
    data[offset : offset + width] = f"{value:0{width}d}".encode("ascii")


def write_dms(data, offset, value, hemisphere):
    """Writes a place in degrees, minutes and seconds."""
    absolute = abs(value)
    degrees = int(math.floor(absolute))
    minutes_exact = (absolute - degrees) * 60.0
    minutes = int(math.floor(minutes_exact))
    seconds = int(round((minutes_exact - minutes) * 60.0))

    write_number(data, offset, 3, degrees)
    write_number(data, offset + 3, 2, minutes)
    write_number(data, offset + 5, 2, seconds)

    # The hemisphere is the one of the place: a place south of the equator is in the
    # south, and one west of the meridian is in the west.
    data[offset + 7] = ord(hemisphere)


def checksum_of(record):
    """The sum of the bytes of a record, without the four of its checksum."""
    return sum(record[: len(record) - CHECKSUM_SIZE])


def build_cell(origin_lat, origin_lng, columns, rows):
    """Builds a level 1 cell whose ground is a slope with a hill in it."""
    size = DATA_OFFSET + columns * (RECORD_HEADER_SIZE + 2 * rows + CHECKSUM_SIZE)
    data = bytearray(size)

    # The user header label.
    data[0:3] = b"UHL"
    data[3] = ord("1")
    write_dms(data, 4, origin_lng, "E" if origin_lng >= 0 else "W")
    write_dms(data, 12, origin_lat, "N" if origin_lat >= 0 else "S")
    write_number(data, 20, 4, 30)
    write_number(data, 24, 4, 30)
    write_number(data, 47, 4, columns)
    write_number(data, 51, 4, rows)

    # The record of the data set and the record of the accuracy.
    data[UHL_SIZE : UHL_SIZE + 3] = b"DSI"
    data[UHL_SIZE + DSI_SIZE : UHL_SIZE + DSI_SIZE + 3] = b"ACC"

    def record_of(column):
        return DATA_OFFSET + column * (RECORD_HEADER_SIZE + 2 * rows + CHECKSUM_SIZE)

    for column in range(columns):
        record = record_of(column)
        size_of = RECORD_HEADER_SIZE + 2 * rows + CHECKSUM_SIZE

        data[record] = 0xAA
        data[record + 1] = 0
        data[record + 2 : record + 4] = struct.pack(">H", column)
        data[record + 4 : record + 6] = struct.pack(">H", column)
        data[record + 6] = 0
        data[record + 7] = 0

        for row in range(rows):
            # The ground: a slope that rises towards the north, and a hill in the
            # middle of the cell.
            height = 400 + row * 5 + column * 3
            distance = (column - columns / 2) ** 2 + (row - rows / 2) ** 2
            hill = int(120 * math.exp(-distance / 8.0))

            magnitude = abs(height)
            offset = record + RECORD_HEADER_SIZE + row * 2
            data[offset] = (magnitude >> 8) & 0xFF
            data[offset + 1] = magnitude & 0xFF

        checksum = checksum_of(data[record : record + size_of])
        data[record + size_of - 4 : record + size_of] = struct.pack(">I", checksum)

    return bytes(data)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, help="the file that is written")
    parser.add_argument("--lat", type=float, default=-35.3, help="the south-west corner")
    parser.add_argument("--lng", type=float, default=149.1, help="the south-west corner")
    parser.add_argument("--columns", type=int, default=120, help="the columns of the cell")
    parser.add_argument("--rows", type=int, default=120, help="the rows of the cell")
    args = parser.parse_args()

    data = build_cell(args.lat, args.lng, args.columns, args.rows)

    with open(args.output, "wb") as handle:
        handle.write(data)

    print(f"the cell was written to {args.output} ({len(data)} bytes)")


if __name__ == "__main__":
    main()
