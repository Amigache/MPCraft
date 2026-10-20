/**
 * The tone of a message the vehicle sent.
 *
 * The backend sends both the number of the severity and its name. The name is what is
 * shown; the number only decides how the line is coloured, which is a matter of
 * presentation rather than of protocol.
 */
export type MessageTone = "critical" | "warning" | "info" | "debug";

/** The tone a severity is drawn in. */
export function messageTone(severity: number): MessageTone {
  if (severity <= 3) {
    return "critical";
  }
  if (severity === 4) {
    return "warning";
  }
  if (severity <= 6) {
    return "info";
  }
  return "debug";
}
