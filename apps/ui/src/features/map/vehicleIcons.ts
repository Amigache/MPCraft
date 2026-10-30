/**
 * The mark the map draws for a vehicle, by the family it belongs to.
 *
 * The map used to draw one arrow for every board, which says where a vehicle is pointing and
 * nothing about what it is: a rover has no heading to speak of, and a plane and a quadcopter
 * do not fly the same way. Each family is drawn as it is seen from above, nose up, so that the
 * same shape turns with the heading the vehicle reports.
 *
 * The drawings are built from a 24-unit box and are filled with `currentColor`, so the colour
 * belongs to the sheet and not to the file. Every shape survives being drawn at 28 px, which is
 * the size the map uses: at that size nothing here is thinner than about 1.5 px.
 */

/** The family a vehicle belongs to, as the link names it. */
export type VehicleKindName =
  | "ArduCopter"
  | "ArduPlane"
  | "ArduRover"
  | "ArduSub"
  | "ArduTracker"
  | "PX4"
  | "Other";

const COPTER = `
  <g fill="none" stroke="currentColor" stroke-width="1.7">
    <path d="M6.5 6.5 L17.5 17.5" />
    <path d="M17.5 6.5 L6.5 17.5" />
  </g>
  <g fill="currentColor">
    <circle cx="5.2" cy="5.2" r="3.1" />
    <circle cx="18.8" cy="5.2" r="3.1" />
    <circle cx="5.2" cy="18.8" r="3.1" />
    <circle cx="18.8" cy="18.8" r="3.1" />
  </g>
  <rect x="9.6" y="9.6" width="4.8" height="4.8" rx="1.2" fill="currentColor" />
`;

const PLANE = `
  <path fill="currentColor"
        d="M12 1.6 L13.4 10 L21 14.6 V16.4 L13.4 14.2 L13.4 20 L15.8 21.6 V22.8 L12 22
           L8.2 22.8 V21.6 L10.6 20 L10.6 14.2 L3 16.4 V14.6 L10.6 10 Z" />
`;

const ROVER = `
  <path fill="currentColor" fill-rule="evenodd"
        d="M8.8 3.6 H15.2 A2.6 2.6 0 0 1 17.8 6.2 V17.8 A2.6 2.6 0 0 1 15.2 20.4 H8.8
           A2.6 2.6 0 0 1 6.2 17.8 V6.2 A2.6 2.6 0 0 1 8.8 3.6 Z
           M9.6 7 H14.4 L12 10 Z" />
  <g fill="currentColor">
    <rect x="3.2" y="6.6" width="2.6" height="4.4" rx="1.1" />
    <rect x="18.2" y="6.6" width="2.6" height="4.4" rx="1.1" />
    <rect x="3.2" y="13.4" width="2.6" height="4.4" rx="1.1" />
    <rect x="18.2" y="13.4" width="2.6" height="4.4" rx="1.1" />
  </g>
`;

const SUB = `
  <path fill="currentColor" fill-rule="evenodd"
        d="M12 1.8 C15 4.4 16.2 8.4 16.2 12.4 C16.2 17 14.6 20.4 12 22.4
           C9.4 20.4 7.8 17 7.8 12.4 C7.8 8.4 9 4.4 12 1.8 Z
           M10.4 9.2 H13.6 V15.2 H10.4 Z" />
`;

const TRACKER = `
  <g fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
    <path d="M6 8.4 A7.4 7.4 0 0 1 18 8.4" />
    <path d="M12 11.6 V19" />
    <path d="M8.4 21.4 L12 19 L15.6 21.4" stroke-linejoin="round" />
  </g>
  <circle cx="12" cy="11.6" r="1.8" fill="currentColor" />
`;

const OTHER = `
  <path fill="currentColor" d="M12 1.8 L22.2 21.4 L12 17 L1.8 21.4 Z" />
`;

/** The drawing of a family, by the name the link gives it. */
const ICONS: Readonly<Record<VehicleKindName, string>> = {
  ArduCopter: COPTER,
  ArduPlane: PLANE,
  ArduRover: ROVER,
  ArduSub: SUB,
  ArduTracker: TRACKER,
  PX4: OTHER,
  Other: OTHER,
};

/**
 * The markup the map puts inside the marker of a vehicle.
 *
 * A family the interface has never heard of is drawn as a delta, which is what a vehicle whose
 * kind is not known looks like here: the map says "a craft" rather than drawing nothing.
 */
export function vehicleIcon(kind: string | null | undefined): string {
  const drawing = ICONS[kind as VehicleKindName] ?? OTHER;
  return `<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">${drawing}</svg>`;
}
