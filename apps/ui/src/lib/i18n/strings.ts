/**
 * The strings the interface says, and what they mean in another language.
 *
 * A string is looked up by what it says in English, because that is the language the code is
 * written in. A string that is not in a bundle is said as it is written: a part of the interface
 * that has not been translated yet still works, and a gap can be seen rather than being a hole in
 * the window.
 *
 * Upstream keeps its strings in `Strings.resx` and translates them through Crowdin. This is the
 * same idea with fewer moving parts: the bundles are files in the repository, so a missing string
 * is a change that can be reviewed, and a test says which names have no entry.
 */

/** A language the interface can be read in. */
export type Locale = "en" | "es";

/** The language the interface is read in when nothing was chosen. */
export const DEFAULT_LOCALE: Locale = "en";

/** The languages the interface offers, in the order the menu shows them. */
export const LOCALES: readonly {
  readonly id: Locale;
  readonly label: string;
  /** How a number is written in that language, which is not the same in both. */
  readonly tag: string;
}[] = [
  { id: "en", label: "English", tag: "en-US" },
  { id: "es", label: "Español", tag: "es-ES" },
];

/** How a number is written in a language. */
export function tagFor(locale: Locale): string {
  const known = LOCALES.find((entry) => entry.id === locale);

  return known ? known.tag : "en-US";
}

/**
 * What each string means in each language.
 *
 * English has no entries because English is what the code says. A bundle is kept in the order
 * the parts of the interface are in, so that a change to a menu and a change to its translation
 * are found next to each other.
 */
const bundles: Record<Locale, Record<string, string>> = {
  en: {},
  es: {
    // The bar of the menus.
    File: "Archivo",
    Flight: "Vuelo",
    View: "Ver",
    Setup: "Configuración",
    Help: "Ayuda",

    "Connect…": "Conectar…",
    Disconnect: "Desconectar",
    "Serial passthrough?": "¿Puerto serie directo?",
    "The files of the vehicle…": "Los archivos del vehículo…",
    Exit: "Salir",

    Arm: "Armar",
    Disarm: "Desarmar",
    "Set mode…": "Elegir el modo…",
    "Download plan": "Bajar el plan",
    "Upload plan": "Subir el plan",
    "Clear plan": "Quitar el plan",

    "The map alone": "El mapa solo",
    Logs: "Registros",
    "The theme": "El tema",
    "Serve the flight on the network": "Servir el vuelo en la red",
    "Units in feet and miles": "Medidas en pies y millas",
    "Side panel": "Panel lateral",
    Graphs: "Gráficos",
    "Follow the vehicle": "Seguir al vehículo",
    "Map controls": "Controles del mapa",
    "The language": "El idioma",

    // The themes, which the View menu lists under its own name.
    Dark: "Oscuro",
    Light: "Claro",
    "High contrast": "Contraste alto",

    "Flight modes": "Modos de vuelo",
    Accelerometer: "Acelerómetro",
    Compass: "Brújula",
    "Frame type": "Tipo de bastidor",
    GPS: "GPS",
    "Serial ports": "Puertos serie",
    Radio: "Radio",
    "Fail safe": "Seguridad",
    Mount: "Montura",
    DroneCAN: "DroneCAN",
    "The folder of tiles…": "La carpeta de las teselas…",
    Terrain: "El terreno",
    "On-screen display…": "La pantalla escrita…",
    "The corrections of a base station…": "Las correcciones de una base…",
    "The transponder…": "El transpondedor…",
    "The traffic of the air…": "El tráfico del aire…",
    "Video streams…": "Los canales de vídeo…",
    "The camera and the gimbal…": "La cámara y la montura móvil…",
    "The pictures and where they were taken…": "Las fotos y dónde se hicieron…",
    Parameters: "Los parámetros",
    "Mission planner": "Planificador de misión",
    Firmware: "El firmware",

    "Write a diagnostics bundle…": "Escribir un paquete de diagnóstico…",
    "About MPCraft": "Sobre MPCraft",

    // The switcher of the screens.
    "Flight data": "Datos del vuelo",
    Plan: "Plan",

    // The tabs of the left column.
    Quick: "Vista rápida",
    Actions: "Acciones",
    Messages: "Mensajes",
    "Pre-flight": "Antes del vuelo",
    Gauges: "Diales",
    Status: "Estado",
    Servos: "Servos",
    Aux: "Auxiliares",

    // The bar of the state, along the bottom.
    Mode: "Modo",
    Armed: "Armado",
    "To home": "A casa",
    Received: "Recibidos",
    Lost: "Perdidos",
    yes: "sí",
    no: "no",

    // The bar of the connection.
    Serial: "Serie",
    UDP: "UDP",
    TCP: "TCP",
    Connect: "Conectar",
    "no ports": "sin puertos",
    "The way of connecting": "La forma de conectar",
    "The port the vehicle is on": "El puerto donde está el vehículo",
    "The speed of the port": "La velocidad del puerto",
    "The address of the vehicle": "La dirección del vehículo",
    "Open the link": "Abrir el enlace",
    "Close the link": "Cerrar el enlace",

    // What a menu says when an entry cannot be used.
    "no link": "sin enlace",
  },
};

/** What a string means in a language, or nothing when the language has no entry for it. */
export function entry(locale: Locale, source: string): string | undefined {
  return bundles[locale][source];
}

/**
 * What a string means in a language, with the values put where the sentence has a gap.
 *
 * A sentence that says a number is kept with a gap — `{count}` — because another language puts the
 * number somewhere else in the sentence and changes the words around it. A gap with no value is
 * left as it was written, so that a mistake can be seen rather than becoming a missing word.
 */
export function translate(
  locale: Locale,
  source: string,
  values?: Record<string, string | number>,
): string {
  const text = entry(locale, source) ?? source;

  if (!values) {
    return text;
  }

  return text.replace(/\{(\w+)\}/g, (gap: string, name: string) =>
    values[name] === undefined ? gap : String(values[name]),
  );
}
