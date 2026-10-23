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
    "The plugins…": "Los complementos…",
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
    "no vehicle": "sin vehículo",
    "The state of the link": "El estado del enlace",
    "The link the application is opened on":
      "El enlace en el que la aplicación está abierta",

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

    // What a sheet says when nothing is on the wire.
    "There is no vehicle on the link.": "No hay vehículo en el enlace.",

    // The sheet of the connection.
    Connection: "Conexión",
    Target: "Destino",
    Port: "Puerto",
    Baud: "Baudios",
    "Connection target": "El destino de la conexión",
    "Serial port": "Puerto serie",
    "Baud rate": "Velocidad en baudios",
    Automatic: "Automática",
    "No serial ports": "Sin puertos serie",
    "Not a serial port": "No es un puerto serie",
    "Looking for the serial ports…": "Buscando los puertos serie…",
    "This machine has no serial ports. Write the target by hand — a UDP or a TCP address works too.":
      "Esta máquina no tiene puertos serie. Escribe el destino a mano — una dirección UDP o TCP también vale.",
    connected: "conectado",
    disconnected: "desconectado",
    connecting: "conectando",
    reconnecting: "reconectando",
    failed: "fallido",
    "Saved links": "Enlaces guardados",
    Name: "Nombre",
    Save: "Guardar",
    "The name of the link": "El nombre del enlace",
    "Save the target under this name": "Guardar el destino con este nombre",
    "No link has been saved. The ones that are saved are here the next time, and the ones marked are opened when the application starts.":
      "No se ha guardado ningún enlace. Los que se guardan están aquí la próxima vez, y los marcados se abren cuando la aplicación arranca.",
    "Open {name} when the application starts":
      "Abrir {name} cuando la aplicación arranca",
    "Use this link": "Usar este enlace",
    "Forget this link": "Quitar este enlace",
    "Forget {name}": "Quitar {name}",

    // The sheet of the calibrations.
    Calibration: "Calibración",
    "Full calibration": "Calibración completa",
    "Start the six positions": "Empezar las seis posiciones",
    "Level calibration": "Calibración del nivel",
    "Start the level calibration": "Empezar la calibración del nivel",
    "Start the calibration": "Empezar la calibración",
    "Start the compass calibration": "Empezar la calibración de la brújula",
    "The compensation of the motors": "La compensación de los motores",
    ESC: "ESC",
    "Calibrate the ESC": "Calibrar el ESC",
    "Start the calibration of the ESC": "Empezar la calibración del ESC",
    "The pulse of the motors": "El pulso de los motores",
    "The vehicle measures the accelerometer in six positions and works the offsets out. Put it on a level table, on its side, on its nose, on its back and on each of the other two corners, and tell it when it is in each one.":
      "El vehículo mide el acelerómetro en seis posiciones y trabaja los desplazamientos. Ponlo sobre una mesa nivelada, sobre su lado, sobre su morro, sobre su espalda y sobre cada uno de los otros dos ángulos, y dile cuándo está en cada una.",
    "The compass has to be away from the metal of the vehicle and of what is around it: take the motors off, or at least do not let them run, and keep the vehicle away from the bench and from the computer. Turn it slowly in every direction while the calibration runs.":
      "La brújula tiene que estar lejos del metal del vehículo y de lo que lo rodea: quita los motores, o al menos no los dejes correr, y mantén el vehículo lejos de la mesa de trabajo y del ordenador. Gíralo despacio en todas las direcciones mientras la calibración corre.",
    "The electronic speed controllers have a calibration of their own: the vehicle tells them to listen, they learn the position of the throttle and the timing of the signal, and then they are ready. The width of the pulse the vehicle sends them is a parameter of the vehicle.":
      "Los controladores electrónicos de velocidad tienen una calibración propia: el vehículo les dice que escuchen, aprenden la posición del acelerador y el momento de la señal, y entonces están listos. El ancho del pulso que el vehículo les manda es un parámetro del vehículo.",
    "The compensation of the motors is a parameter of the vehicle — the compass of a vehicle with motors is never quite right without it.":
      "La compensación de los motores es un parámetro del vehículo — la brújula de un vehículo con motores nunca está del todo bien sin ella.",
    "The vehicle was told to calibrate the accelerometer.":
      "Se le dijo al vehículo que calibrara el acelerómetro.",
    "The vehicle was told to calibrate the accelerometer level.":
      "Se le dijo al vehículo que calibrara el nivel del acelerómetro.",
    "The vehicle was told to calibrate the compass.":
      "Se le dijo al vehículo que calibrara la brújula.",
    "The vehicle was told to calibrate the electronic speed controllers.":
      "Se le dijo al vehículo que calibrara los controladores electrónicos de velocidad.",

    // The sheet of the transponder.
    "The transponder": "El transpondedor",
    "The vehicle is told where it is and what to say, and it relays the message. The aircraft around it see it.":
      "Se le dice al vehículo dónde está y qué decir, y él retransmite el mensaje. Las aeronaves de alrededor lo ven.",
    "The address of the aircraft": "La dirección de la aeronave",
    "The address of the aircraft, in hexadecimal":
      "La dirección de la aeronave, en hexadecimal",
    "The name of the flight": "El nombre del vuelo",
    "The code": "El código",
    "The code of the transponder, in octal":
      "El código del transpondedor, en octal",
    Start: "Empezar",
    Stop: "Parar",
    "There is no link to a vehicle.": "No hay enlace con el vehículo.",
    "What it is doing": "Lo que está haciendo",
    "Saying where the vehicle is": "Dice dónde está el vehículo",
    "Not saying": "No dice nada",
    "How many times": "Cuántas veces",
    "The address of the aircraft is not one that can be used.":
      "La dirección de la aeronave no es una que se pueda usar.",
    "The code is not one that can be used.":
      "El código no es uno que se pueda usar.",
    "The vehicle is being told what its transponder is to say.":
      "Se está diciendo al vehículo lo que su transpondedor ha de decir.",
    "The transponder has been left alone.":
      "Se ha dejado el transpondedor en paz.",

    // The sheet of the pictures.
    "The pictures and where they were taken": "Las fotos y dónde se hicieron",
    Close: "Cerrar",
    "The place a picture says is what its own file carries. The place the log says is where the vehicle was at the instant the camera made it, once the difference between the two clocks is worked out from the pictures at both ends of the folder. A picture that is more than two seconds from any sample of the log keeps only its own place.":
      "El lugar que una foto dice es el que lleva su propio archivo. El lugar que dice el registro es donde estaba el vehículo en el instante en que la cámara la hizo, una vez trabajada la diferencia entre los dos relojes desde las fotos de los dos extremos de la carpeta. Una foto que está a más de dos segundos de cualquier muestra del registro se queda solo con su lugar.",
    "Nothing is read yet. A folder that holds no picture with a block inside it says nothing, and a picture whose instant the log does not hold has no second place.":
      "Todavía no se ha leído nada. Una carpeta que no tiene ninguna foto con un bloque dentro no dice nada, y una foto cuyo instante el registro no tiene no tiene segundo lugar.",
    "The folder of the pictures": "La carpeta de las fotos",
    "the folder the pictures are in": "la carpeta donde están las fotos",
    "Choose the folder": "Elegir la carpeta",
    "The log of the flight": "El registro del vuelo",
    "the telemetry log to match with":
      "el registro de telemetría con el que cotejar",
    "Choose the log": "Elegir el registro",
    "The two clocks, in seconds": "Los dos relojes, en segundos",
    "What the shutter took, in seconds":
      "Lo que tardó el obturador, en segundos",
    "Read the pictures": "Leer las fotos",
    "Work the clock out": "Trabajar el reloj",
    "Match them with the log": "Cotejarlas con el registro",
    "Write the copies": "Escribir las copias",
    "The file": "El archivo",
    "When the camera says it was made": "Cuándo dice la cámara que se hizo",
    "The place the file says": "El lugar que dice el archivo",
    "The place the log says": "El lugar que dice el registro",
    "Seconds apart": "Segundos de diferencia",
    "The instants of the folder cannot be lined up with the log, so nothing was worked out.":
      "Los instantes de la carpeta no se pueden alinear con el registro, así que no se trabajó nada.",
    "The clock of the camera is {seconds} seconds from the clock of the log.":
      "El reloj de la cámara está a {seconds} segundos del reloj del registro.",
    "The folder holds {count} picture.": "La carpeta contiene {count} foto.",
    "The folder holds {count} pictures.": "La carpeta contiene {count} fotos.",
    "{count} of {total} pictures were matched with a sample of the log.":
      "{count} de {total} fotos cotejadas con una muestra del registro.",
    "{written} copies were written to {folder}/geotagged.":
      "{written} copias escritas en {folder}/geotagged.",

    // The sheet of the plugins.
    "The plugins": "Los complementos",
    "A plugin is somebody else's code. It is not run because its file happens to be in the folder: the operator has to say so, and the decision is kept for the next run.":
      "Un complemento es código de otra persona. No se ejecuta porque su archivo esté en la carpeta: el operador tiene que decirlo, y la decisión se guarda para la próxima ejecución.",
    "The folder the plugins are read from":
      "La carpeta donde se leen los complementos",
    "The folder holds nothing.": "La carpeta no contiene nada.",
    "Who wrote it": "Quién lo escribió",
    "nothing said": "no dice quién",
    "What it asks for": "Lo que pide",
    nothing: "nada",
    "How often it is called": "Cada cuánto se llama",
    "Where it stands": "Dónde está",
    "What went wrong": "Qué salió mal",
    "What the plugins said": "Lo que dijeron los complementos",
    "Nothing yet.": "Todavía nada.",
    Run: "Ejecutar",
    "Leave alone": "Dejar en paz",
    "Not run": "No se ejecuta",
    Read: "Leído",
    "Being called": "Se está llamando",
    "Could not be read": "No se pudo leer",
    "{name} is being run.": "{name} se está ejecutando.",
    "{name} has been left alone.": "{name} se ha dejado en paz.",

    // The sheet of the on-screen display.
    "The on-screen display": "La pantalla escrita",
    "The vehicle draws the display over the picture of the camera. Each slot holds one thing, and the screens five and six are the ones that are configured here.":
      "El vehículo dibuja la pantalla sobre la imagen de la cámara. Cada hueco guarda una cosa, y las pantallas cinco y seis son las que se configuran aquí.",
    "Read the slots": "Leer los huecos",
    "Screen {screen}": "Pantalla {screen}",
    "What the slot {index} of the screen {screen} holds":
      "Lo que guarda el hueco {index} de la pantalla {screen}",
    "— nothing —": "— nada —",
    "The slot {index} of the screen {screen} holds {name}.":
      "El hueco {index} de la pantalla {screen} guarda {name}.",
    "The vehicle did not say what its slots hold. It may not know the questions.":
      "El vehículo no dijo qué guardan sus huecos. Puede que no conozca las preguntas.",

    // The sheet of the corrections of a base station.
    "The corrections of a base station": "Las correcciones de una base",
    "A base station sends the corrections of the satellites it sees to a caster, and the caster passes them to whoever asks. The corrections go to the receiver of the vehicle, and the position of the vehicle travels with the request.":
      "Una base manda las correcciones de los satélites que ve a una emisora, y la emisora las pasa a quien las pide. Las correcciones van al receptor del vehículo, y la posición del vehículo viaja con la petición.",
    "The address of the caster": "La dirección de la emisora",
    "The port of the vehicle": "El puerto del vehículo",
    "The port of the vehicle the corrections go to":
      "El puerto del vehículo donde llegan las correcciones",
    "Ask the caster the old way, with the headers of the first version":
      "Se pregunta a la emisora de la manera vieja, con las cabeceras de la primera versión",
    "The first version": "La primera versión",
    "Sending the corrections": "Manda las correcciones",
    "Not sending": "No manda nada",
    "How many bytes": "Cuántos bytes",
    "The corrections are being sent to the vehicle.":
      "Las correcciones se están mandando al vehículo.",
    "The corrections have stopped.": "Las correcciones han parado.",

    // The sheet of the traffic of the air.
    "The traffic of the air": "El tráfico del aire",
    "The aircraft the vehicle sent are already on the link. What the station reads from a source of its own is added to it, one each second, from the ten closest to the vehicle, so that the other stations listening here see them too.":
      "Las aeronaves que mandó el vehículo ya están en el enlace. Lo que la estación lee de una fuente suya se añade, una cada segundo, de las diez más cercanas al vehículo, para que las otras estaciones que escuchan aquí también las vean.",
    "The address of the source": "La dirección de la fuente",
    "The address of the source of the traffic":
      "La dirección de la fuente del tráfico",
    "How far round the vehicle": "Cuánto alrededor del vehículo",
    "How far round the vehicle the list is asked for, in nautical miles":
      "Cuánto alrededor del vehículo se pide la lista, en millas náuticas",
    "Sending one aircraft each second": "Manda una aeronave cada segundo",
    "How many aircraft the station reads": "Cuántas aeronaves lee la estación",
    "How many have been sent": "Cuántas se han mandado",
    "Which place of the list comes next": "Qué lugar de la lista sigue",
    "Where the vehicle is": "Dónde está el vehículo",
    "the vehicle has not said": "el vehículo no lo ha dicho",
    "The aircraft the station reads are being sent onto the link.":
      "Las aeronaves que lee la estación se están mandando al enlace.",
    "The relay has been stopped.": "El rele se ha parado.",
    "The address of the source is not one that can be used. MPCraft reads the traffic from an HTTP address; a receiver that speaks over a plain connection is not read.":
      "La dirección de la fuente no es una que se pueda usar. MPCraft lee el tráfico de una dirección HTTP; un receptor que habla por una conexión simple no se lee.",
    "The distance round the vehicle is not one that can be used.":
      "La distancia alrededor del vehículo no es una que se pueda usar.",

    // The sheet of the video streams.
    "The video streams of the vehicle": "Los canales de vídeo del vehículo",
    "The camera describes its streams with a URI, their resolution and frame rate. The stream travels outside MAVLink: use the URI with a video player.":
      "La cámara describe sus canales con la URI, su resolución y su velocidad de imagen. El canal viaja fuera de MAVLink: usa la URI con un reproductor de vídeo.",
    "Ask the camera": "Preguntar a la cámara",
    "There is no link.": "No hay enlace.",
    "The camera is being asked.": "Se está preguntando a la cámara.",
    "The camera did not describe any video streams.":
      "La cámara no describió ningún canal de vídeo.",
    "Stream {id}": "Canal {id}",
    Running: "Funcionando",
    Stopped: "Parado",
    Address: "La dirección",
    "Not provided": "No se dijo",
    Picture: "La imagen",
    "Frame rate": "Velocidad de imagen",
    "Bit rate": "Velocidad de bits",
    "Start the stream": "Empezar el canal",
    "Stop the stream": "Parar el canal",
    "The stream {name} has stopped.": "El canal {name} se ha parado.",
    "The stream {name} has started.": "El canal {name} ha empezado.",

    // The sheet of the camera and the gimbal.
    "The camera and the gimbal": "La cámara y la montura móvil",
    "What the camera says about itself is what can be asked of it. A camera that cannot record video in the mode it is in is refused before anything is sent.":
      "Lo que la cámara dice de sí es lo que se le puede pedir. Una cámara que no puede grabar vídeo en el modo en que está, se rechaza antes de mandar nada.",
    "The vehicle has not said what its camera can do.":
      "El vehículo no ha dicho qué puede hacer su cámara.",
    "Which camera it is": "Qué cámara es",
    "not said": "no lo dijo",
    "The mode it is in": "El modo en que está",
    "The size of the picture": "El tamaño de la imagen",
    "{width} by {height} pixels": "{width} por {height} píxeles",
    "What it says about its pictures": "Lo que dice de sus fotos",
    "What it says about its video": "Lo que dice de su vídeo",
    "Take one picture": "Hacer una foto",
    "The camera was asked for a picture.": "Se pidió a la cámara una foto.",
    "Seconds between pictures": "Segundos entre fotos",
    "The seconds between pictures": "Los segundos entre fotos",
    "Take them at an interval": "Hacerlas a un intervalo",
    "The camera was asked to take pictures at that interval.":
      "Se pidió a la cámara que hiciera fotos a ese intervalo.",
    "Stop the interval": "Quitar el intervalo",
    "The interval was taken off.": "El intervalo se ha quitado.",
    Record: "Grabar",
    "The camera was asked to record.": "Se pidió a la cámara que grabara.",
    "Stop recording": "Parar la grabación",
    "The recording was stopped.": "La grabación se ha parado.",
    "Zoom level": "El nivel de zoom",
    "The zoom level": "El nivel de zoom",
    "What the level means": "Qué significa el nivel",
    "The kind of zoom": "La clase de zoom",
    "Set the zoom": "Poner el zoom",
    "The zoom was set.": "El zoom se ha puesto.",
    "A place in the picture is read from `-1` to `1`: `-1` is the left or the top, `1` is the right or the bottom. Picking it from the picture itself waits on drawing the video.":
      "Un lugar en la imagen se lee de `-1` a `1`: `-1` es la izquierda o arriba, `1` es la derecha o abajo. Elegirlo de la propia imagen espera a dibujar el vídeo.",
    "Across the picture": "A lo ancho de la imagen",
    "The place across the picture": "El lugar a lo ancho de la imagen",
    "Down the picture": "Hacia abajo de la imagen",
    "The place down the picture": "El lugar hacia abajo de la imagen",
    "Follow that point": "Seguir ese punto",
    "The camera was told what to follow.": "Se le dijo a la cámara qué seguir.",
    "First corner, across": "Primer rincón, a lo ancho",
    "The first corner of the area, across the picture":
      "El primer rincón del área, a lo ancho de la imagen",
    "First corner, down": "Primer rincón, hacia abajo",
    "The first corner of the area, down the picture":
      "El primer rincón del área, hacia abajo de la imagen",
    "Second corner, across": "Segundo rincón, a lo ancho",
    "The second corner of the area, across the picture":
      "El segundo rincón del área, a lo ancho de la imagen",
    "Second corner, down": "Segundo rincón, hacia abajo",
    "The second corner of the area, down the picture":
      "El segundo rincón del área, hacia abajo de la imagen",
    "Follow that area": "Seguir esa área",
    "The camera was told what area to follow.":
      "Se le dijo a la cámara qué área seguir.",
    "The gimbal is pointed at an angle, or at a rate. An angle the device cannot reach is refused before anything is sent, and the yaw is read in the frame the device says it uses.":
      "La montura se apunta a un ángulo, o a una velocidad. Un ángulo que el aparato no puede alcanzar se rechaza antes de mandar nada, y la guiñada se lee en el marco que el aparato dice usar.",
    "Which gimbal": "Qué montura",
    "Which gimbal the command names": "Qué montura nombra la orden",
    "All gimbals": "Todas las monturas",
    "The gimbal {id}": "La montura {id}",
    "The vehicle has not said what its gimbals can do.":
      "El vehículo no ha dicho qué pueden hacer sus monturas.",
    "Where it points": "Hacia dónde apunta",
    "roll {roll}°, pitch {pitch}°, yaw {yaw}°":
      "alabeo {roll}°, inclinación {pitch}°, guiñada {yaw}°",
    "How far its axes go": "Hasta dónde llegan sus ejes",
    "pitch {pitchMin}° to {pitchMax}°, yaw {yawMin}° to {yawMax}°":
      "inclinación {pitchMin}° a {pitchMax}°, guiñada {yawMin}° a {yawMax}°",
    "What it can do": "Qué puede hacer",
    "Whether something is wrong with it": "Si algo va mal con ella",
    "It says it is failing": "Dice que está fallando",
    Nothing: "Nada",
    "Pitch, in degrees": "Inclinación, en grados",
    "The pitch angle of the gimbal, in degrees":
      "El ángulo de inclinación de la montura, en grados",
    "Yaw, in degrees": "Guiñada, en grados",
    "The yaw angle of the gimbal, in degrees":
      "El ángulo de guiñada de la montura, en grados",
    "Point it there": "Apuntarla ahí",
    "The gimbal was pointed at that angle.":
      "La montura se apuntó a ese ángulo.",
    "Hold the yaw": "Sujetar la guiñada",
    "The yaw is held against the ground.":
      "La guiñada se sujeta contra el suelo.",
    "The yaw follows the vehicle.": "La guiñada sigue al vehículo.",
    Retract: "Recogerla",
    "The gimbal was put in its safe position.":
      "La montura se puso en su posición segura.",
    Neutral: "Neutra",
    "The gimbal was put in its default position.":
      "La montura se puso en su posición de siempre.",
    "Pitch rate": "Velocidad de inclinación",
    "The rate of the pitch, in degrees a second":
      "La velocidad de la inclinación, en grados por segundo",
    "Yaw rate": "Velocidad de guiñada",
    "The rate of the yaw, in degrees a second":
      "La velocidad de la guiñada, en grados por segundo",
    "Move it at those rates": "Moverla a esas velocidades",
    "The gimbal was told to move at those rates.":
      "Se le dijo a la montura que se moviera a esas velocidades.",
    "A gimbal can also be pointed at a place on the ground, or at another vehicle. The point of interest is held until it is taken off.":
      "Una montura también se puede apuntar a un lugar del suelo, o a otro vehículo. El punto de interés se mantiene hasta que se quita.",
    Latitude: "Latitud",
    "The latitude of the point of interest": "La latitud del punto de interés",
    Longitude: "Longitud",
    "The longitude of the point of interest":
      "La longitud del punto de interés",
    "Height above the ground": "La altura sobre el suelo",
    "The height of the point of interest above the ground, in metres":
      "La altura del punto de interés sobre el suelo, en metros",
    "System id of the other vehicle": "El número del otro vehículo",
    "The system id of the vehicle to point at":
      "El número del vehículo al que apuntar",
    "Point it at that place": "Apuntarla a ese lugar",
    "The gimbal was pointed at that place.":
      "La montura se apuntó a ese lugar.",
    "Take the point off": "Quitar el punto",
    "The point of interest was taken off.":
      "El punto de interés se ha quitado.",
    "Point it at that vehicle": "Apuntarla a ese vehículo",
    "The gimbal was pointed at that vehicle.":
      "La montura se apuntó a ese vehículo.",
    // What the two lines about a device say, piece by piece.
    "a roll axis": "un eje de alabeo",
    "a pitch axis": "un eje de inclinación",
    "a yaw axis": "un eje de guiñada",
    "holds its yaw against the ground": "sujeta su guiñada contra el suelo",
    "follows the vehicle": "sigue al vehículo",
    "can be retracted": "se puede recoger",
    "can go to neutral": "puede ponerse en neutro",
    "points at a place on the ground": "apunta a un lugar del suelo",
    "nothing it has said": "nada que haya dicho",
    retracted: "recogida",
    "at neutral": "en neutro",
    "yaw held against the ground": "guiñada sujetada contra el suelo",
    "yaw read from the way the vehicle points":
      "guiñada leída del modo en que apunta el vehículo",
    "yaw read from the ground": "guiñada leída del suelo",

    // The sheet of the joystick.
    "Joystick…": "El joystick…",
    Joystick: "El joystick",
    "No joystick has been seen. Plug one in and press a button on it: the operating system only tells the application about a joystick that has been touched.":
      "No se ha visto ningún joystick. Enchúfalo y pulsa un botón de él: el sistema operativo solo avisa de un joystick que se ha tocado.",
    "The joystick": "El joystick",
    "The joystick that drives the vehicle":
      "El joystick que maneja el vehículo",
    "Channel 1 — roll": "Canal 1 — alabeo",
    "Channel 2 — pitch": "Canal 2 — inclinación",
    "Channel 3 — throttle": "Canal 3 — gas",
    "Channel 4 — yaw": "Canal 4 — guiñada",
    "The axis that drives {name}": "El eje que maneja {name}",
    "Not driven": "No se maneja",
    "Axis {number}": "Eje {number}",
    "Whether {name} is read the other way round": "Si {name} se lee al revés",
    Inverted: "Al revés",
    "Take the radio channels over": "Tomar los canales de la radio",
    "there is no link to the vehicle": "no hay enlace con el vehículo",
    "Send to the vehicle": "Mandar al vehículo",
    "Stop sending": "Dejar de mandar",
    "Let the radio channels go back to the radio":
      "Devolver los canales de la radio a la radio",
    "Release the channels": "Soltar los canales",

    // The sheet of the terrain.
    "Terrain…": "El terreno…",
    "The cells of the ground": "Las celdas del suelo",
    "Open a folder of cells…": "Abrir una carpeta de celdas…",
    "The height under the vehicle": "La altura bajo el vehículo",
    "The ground under the vehicle is at {metres} metres over the sea.":
      "El suelo bajo el vehículo está a {metres} metros sobre el mar.",
    Level: "Nivel",
    "South-west corner": "Rincón suroeste",
    Posts: "Postes",
    "No folder of cells has been opened. The heights of the service are used until one is.":
      "No se ha abierto ninguna carpeta de celdas. Se usan las alturas del servicio hasta que se abra una.",
    "A survey that follows the ground": "Un levantamiento que sigue el suelo",
    "The strips run across the line that joins the two places, one after the other, and the vehicle flies at the same height over the ground everywhere.":
      "Las franjas cruzan la línea que une los dos lugares, una tras otra, y el vehículo vuela a la misma altura sobre el suelo en todas partes.",
    "From latitude": "Latitud de inicio",
    "The latitude the survey starts at":
      "La latitud donde empieza el levantamiento",
    "From longitude": "Longitud de inicio",
    "The longitude the survey starts at":
      "La longitud donde empieza el levantamiento",
    "To latitude": "Latitud de fin",
    "The latitude the survey ends at":
      "La latitud donde termina el levantamiento",
    "To longitude": "Longitud de fin",
    "The longitude the survey ends at":
      "La longitud donde termina el levantamiento",
    "Strip m": "Franja, m",
    "How long the strips are": "Lo largas que son las franjas",
    "Apart m": "Separadas, m",
    "How far apart the strips are": "Lo separadas que están las franjas",
    "Over the ground m": "Sobre el suelo, m",
    "How high over the ground the vehicle flies":
      "La altura sobre el suelo a la que vuela el vehículo",
    "Work out the survey": "Calcular el levantamiento",
    "Add it to the plan": "Añadirlo al plan",
    "The survey was added to the plan.": "El levantamiento se añadió al plan.",
    "the vehicle has not said where it is.":
      "el vehículo no ha dicho dónde está.",
    "the two places of the survey have to be told.":
      "hay que decir los dos lugares del levantamiento.",
    // What the sheet says about how many things there are, one and more than one.
    "{count} cell was read.": "Se ha leído {count} celda.",
    "{count} cells were read.": "Se han leído {count} celdas.",
    "{count} file could not be read.": "{count} archivo no se ha podido leer.",
    "{count} files could not be read.":
      "{count} archivos no se han podido leer.",
    "The survey has {count} point.": "El levantamiento tiene {count} punto.",
    "The survey has {count} points.": "El levantamiento tiene {count} puntos.",
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
