/**
 * The types a parameter can have.
 *
 * The backend reports the type of every parameter as the MAVLink `MAV_PARAM_TYPE`
 * enumeration, and the number it sends is the value of that enumeration. This is the one
 * place in the interface where the enumeration is understood: the number becomes the name
 * on the badge, and it decides whether a value may have a fraction in it.
 *
 * The numbering is the one on the wire, which is the one the canonical MAVLink
 * definitions use: `UINT8` is 1 and `REAL64` is 10. The backend sends the enumeration as
 * it stands, so the interface reads it the same way. Upstream's own enumeration in
 * `Mavlink.cs` has the same numbering.
 */

/** What is known about one type. */
interface TypeInfo {
  /** The name the badge shows. */
  readonly name: string;
  /** Whether a value of this type is a whole number. */
  readonly integral: boolean;
}

/** The types, by the number the backend reports. */
const TYPES: Readonly<Record<number, TypeInfo>> = {
  1: { name: "UINT8", integral: true },
  2: { name: "INT8", integral: true },
  3: { name: "UINT16", integral: true },
  4: { name: "INT16", integral: true },
  5: { name: "UINT32", integral: true },
  6: { name: "INT32", integral: true },
  7: { name: "UINT64", integral: true },
  8: { name: "INT64", integral: true },
  9: { name: "REAL32", integral: false },
  10: { name: "REAL64", integral: false },
};

/** The name of the type the backend reported. */
export function paramTypeName(reportedType: number): string {
  return TYPES[reportedType]?.name ?? `TYPE ${reportedType}`;
}

/**
 * Whether a value of this type has to be a whole number.
 *
 * A type the interface has not heard of is not held to be whole: refusing a value the
 * vehicle would have taken is worse than accepting one it will not.
 */
export function isIntegralType(reportedType: number): boolean {
  return TYPES[reportedType]?.integral ?? false;
}
