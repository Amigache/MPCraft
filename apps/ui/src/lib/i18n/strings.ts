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
    "The folder of tiles…": "La carpeta de las fichas…",
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
    "Main menu": "El menú principal",

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

    // The sheet of the formation.
    "The formation": "La formación",
    "The formation…": "La formación…",
    "One vehicle leads and the others sit in the places that were written for them. The loop runs ten times a second and tells each member where its place is.":
      "Un vehículo lidera y los otros están en los lugares que se escribieron para ellos. El ciclo va diez veces por segundo y dice a cada miembro dónde está su lugar.",
    "Who leads": "Quién lidera",
    "The vehicle the formation follows": "El vehículo que sigue la formación",
    "nobody yet": "nadie todavía",
    "Put a vehicle in the formation": "Poner un vehículo en la formación",
    "The vehicle to put in the formation":
      "El vehículo que se pone en la formación",
    "choose one": "elige uno",
    Add: "Añadir",
    Across: "Lateral",
    Ahead: "Por delante",
    Above: "Por encima",
    "Take out": "Sacar",
    "Nothing has been written yet.": "Todavía no se ha escrito nada.",
    "The numbers are metres from the leader.":
      "Los números son metros desde el líder.",
    "The places were saved.": "Los lugares se han guardado.",
    "The loop is being run.": "El ciclo se está ejecutando.",
    "The loop has been stopped.": "El ciclo se ha parado.",
    "There is no link to a vehicle. The places can be written, but nothing is sent until a board answers.":
      "No hay enlace con el vehículo. Los lugares se pueden escribir, pero no se manda nada hasta que una placa responde.",
    "Telling each member its place": "Dice a cada miembro dónde está su lugar",
    "Not running": "No se ejecuta",
    "How many were told on the last tick":
      "Cuántos se dijeron en el último ciclo",
    "The names that could not be read as vehicles":
      "Los nombres que no se pudieron leer como vehículos",
    none: "ninguno",
    "The last failure": "El último fallo",

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

    // The sheet of the serial passthrough.
    "Serial passthrough": "Puerto serie directo",
    "The bytes travel inside a MAVLink message and the vehicle hands them over to the port. The speed of the port is left alone when it is zero.":
      "Los bytes viajan dentro de un mensaje MAVLink y el vehículo los entrega al puerto. La velocidad del puerto se deja como está cuando es cero.",
    "Telemetry 1": "Telemetría 1",
    "Telemetry 2": "Telemetría 2",
    "Telemetry 3": "Telemetría 3",
    "GPS 1": "GPS 1",
    "GPS 2": "GPS 2",
    Speed: "Velocidad",
    "Timeout ms": "Espera, ms",
    "How long the vehicle waits for the answer":
      "Cuánto espera el vehículo la respuesta",
    "What is sent": "Lo que se manda",
    "The text that goes to the port": "El texto que va al puerto",
    Send: "Mandar",
    "What came back": "Lo que volvió",
    "{count} byte went to the port.": "Se mandó {count} byte al puerto.",
    "{count} bytes went to the port.": "Se mandaron {count} bytes al puerto.",
    "{count} byte came from the port {device}.":
      "Se recibió {count} byte del puerto {device}.",
    "{count} bytes came from the port {device}.":
      "Se recibieron {count} bytes del puerto {device}.",

    // The sheet of the firmware.
    "Firmware…": "El firmware…",
    "The vehicle": "El vehículo",
    Vehicle: "Vehículo",
    "The vehicle the firmware is for": "El vehículo para el que es el firmware",
    Version: "Versión",
    "The version of the firmware": "La versión del firmware",
    Board: "La placa",
    "The board the firmware is for": "La placa para la que es el firmware",
    "How big": "Lo grande que es",
    "Its hash": "Su huella",
    "Fetch the file": "Traer el archivo",
    "The file is fetched to the disk. It can then be written to a board that is in its bootloader: the board answers the questions of the bootloader instead of the MAVLink of a flight, and the file is written to it block by block.":
      "El archivo se trae al disco. Después se puede escribir en una placa que esté en su cargador de arranque: la placa responde a las preguntas del cargador en lugar del MAVLink de un vuelo, y el archivo se escribe en ella bloque a bloque.",
    "Writing it to a board": "Escribirlo en una placa",
    "Ask the vehicle to restart into its bootloader":
      "Pedir al vehículo que vuelva a empezar en su cargador de arranque",
    "Restart the vehicle into its bootloader":
      "Reiniciar el vehículo en su cargador de arranque",
    "The vehicle is asked to restart into its bootloader. It stops speaking MAVLink when it is there, and the port is then opened in the manner of the bootloader.":
      "Se pide al vehículo que vuelva a empezar en su cargador de arranque. Deja de hablar MAVLink cuando está ahí, y el puerto se abre entonces como el cargador lo entiende.",
    "The port the board is on": "El puerto donde está la placa",
    "Write it to the board": "Escribirlo en la placa",
    "Leave the board without a firmware": "Dejar la placa sin firmware",
    "Erase the board": "Borrar la placa",
    "The board has to be in its bootloader for this to work: the link has to be closed, the board has to be restarted into the bootloader, and the file is written over the port. The board is asked what it is before anything is erased, and the file is not written if it does not fit in the flash.":
      "La placa tiene que estar en su cargador de arranque para que esto funcione: el enlace tiene que estar cerrado, la placa tiene que haber vuelto al cargador, y el archivo se escribe por el puerto. Se pregunta a la placa qué es antes de borrar nada, y el archivo no se escribe si no cabe en la memoria.",
    "The board is the {board} (revision {revision}), with {kilobytes} kB of flash, speaking the protocol {protocol}.":
      "La placa es la {board} (revisión {revision}), con {kilobytes} kB de memoria, que habla el protocolo {protocol}.",
    "there is no file chosen.": "no hay archivo elegido.",
    "The file of the firmware": "El archivo del firmware",
    "The file was written to {path} ({size}).":
      "El archivo se escribió en {path} ({size}).",
    "The flash of the board was erased.":
      "La memoria de la placa se ha borrado.",
    "there is no file to write.": "no hay archivo que escribir.",
    "The board was written and told to start again.":
      "La placa se escribió y se le dijo que volviera a empezar.",
    "The vehicle was told to restart into its bootloader. The link is closed when the board stops answering.":
      "Se pidió al vehículo que volviera a empezar en su cargador de arranque. El enlace se cierra cuando la placa deja de responder.",

    // The sheet of the files of the vehicle.
    "The files of the vehicle": "Los archivos del vehículo",
    "The vehicle holds a filesystem of its own. The folders are listed the way the vehicle says them, and a file is brought to the disk of the computer.":
      "El vehículo tiene un sistema de archivos propio. Las carpetas se listan como el vehículo las dice, y un archivo se trae al disco del ordenador.",
    Up: "Subir",
    "The folder that is listed": "La carpeta que se lista",
    Refresh: "Actualizar",
    "The vehicle is being asked.": "Se está preguntando al vehículo.",
    "The folder is empty.": "La carpeta está vacía.",
    Open: "Abrirla",
    Download: "Traerlo",
    Everything: "Todo",
    "{count} byte of {name} was written to {path}.":
      "Se escribió {count} byte de {name} en {path}.",
    "{count} bytes of {name} were written to {path}.":
      "Se escribieron {count} bytes de {name} en {path}.",

    // The console of the messages the vehicle sends.
    Clear: "Vaciar",
    "The vehicle has not said anything yet.":
      "El vehículo todavía no ha dicho nada.",

    // The quick view: the tiles, one value at a glance.
    "{count} tile": "{count} ficha",
    "{count} tiles": "{count} fichas",
    Done: "Terminado",
    Configure: "Elegir las fichas",
    Altitude: "Altitud",
    "Ground speed": "Velocidad sobre el suelo",
    Heading: "Rumbo",
    Battery: "Batería",
    Charge: "Carga",
    Current: "Corriente",
    Satellites: "Satélites",
    Throttle: "Gas",
    "Air speed": "Velocidad del aire",
    Climb: "Ascenso",
    Signal: "Señal",

    // The status tab: what the vehicle is and what the link is doing.
    "Global positioning": "El posicionamiento",
    System: "El sistema",
    Component: "El componente",
    Family: "La familia",
    Fix: "El arreglo",
    "Second receiver": "El segundo receptor",
    "Second satellites": "Satélites del segundo",
    "Home latitude": "Latitud de casa",
    "Home longitude": "Longitud de casa",
    "Home altitude": "Altitud de casa",
    Voltage: "Voltaje",
    Remaining: "Lo que queda",
    "Bank {number}": "Banco {number}",
    Wind: "El viento",
    From: "De donde viene",
    Identification: "La identificación",
    Kind: "La clase",
    Id: "El número",
    Height: "La altura",
    Says: "Lo que dice",
    Engine: "El motor",
    Revolutions: "Revoluciones",
    Load: "La carga",
    "Fuel flow": "El consumo de combustible",
    "Fuel burnt": "Combustible quemado",
    "Fuel pressure": "Presión del combustible",
    Intake: "La entrada",
    Head: "La culata",
    Exhaust: "El escape",
    Estimator: "El estimador",
    Velocity: "La velocidad",
    Horizontal: "Lo horizontal",
    Vertical: "Lo vertical",
    Vibration: "La vibración",
    Route: "La ruta",
    Link: "El enlace",
    State: "El estado",
    Item: "El elemento",
    Items: "Los elementos",
    "{number} of {count}": "{number} de {count}",
    // What the state of the satellites is called, one by one.
    "no GPS": "sin GPS",
    "no fix": "sin arreglo",
    "RTK float": "RTK flotante",
    static: "estático",
    "fix {number}": "arreglo {number}",

    // The bar of the route.
    "The route": "La ruta",
    "{distance} flown": "{distance} volado",
    "{distance} to go": "{distance} por recorrer",
    "{distance} in all": "{distance} en total",

    // The outputs of the vehicle and what they are for.
    Outputs: "Las salidas",
    "Output {number}": "Salida {number}",
    "Radio channels": "Los canales de la radio",
    "Channel {number}": "Canal {number}",
    Strength: "La fuerza",
    "Auxiliary functions": "Las funciones auxiliares",
    "The vehicle has not said what its outputs are for.":
      "El vehículo no ha dicho para qué son sus salidas.",
    "It tells it with the parameters": "Lo dice con los parámetros",
    "and its brothers.": "y sus hermanos.",
    "Function {number}": "Función {number}",
    "Motor {number}": "Motor {number}",

    // The check list before the flight.
    "The checklist is empty.": "La lista de comprobación está vacía.",

    // The dials.
    "V-speed": "Velocidad vertical",

    // The graphs.
    "The graph": "El gráfico",
    "Hide the graphs and give the map the whole area":
      "Quitar los gráficos y dar al mapa todo el espacio",
    "Hide graphs": "Quitar los gráficos",
    "The catalogue of graphs could not be read.":
      "No se pudo leer el catálogo de gráficos.",
    "Waiting for the vehicle to report.":
      "Se espera a que el vehículo informe.",

    // The grid of the items of a plan.
    Command: "La orden",
    Frame: "El marco",
    Alt: "Altitud",
    Auto: "Seguir",
    "The item the vehicle is executing":
      "El elemento que el vehículo está ejecutando",
    "The frame of item {number}": "El marco del elemento {number}",
    "Whether the vehicle goes on by itself after item {number}":
      "Si el vehículo sigue solo después del elemento {number}",
    "Move the item up": "Subir el elemento",
    "Move item {number} up": "Subir el elemento {number}",
    "Move the item down": "Bajar el elemento",
    "Move item {number} down": "Bajar el elemento {number}",
    "Delete the item": "Borrar el elemento",
    "Delete item {number}": "Borrar el elemento {number}",
    "The items of the plan": "Los elementos del plan",
    "The command of item {number}": "La orden del elemento {number}",
    "Parameter {parameter} of item {number}":
      "El parámetro {parameter} del elemento {number}",
    "The altitude of item {number}": "La altitud del elemento {number}",
    "The latitude of item {number}": "La latitud del elemento {number}",
    "The longitude of item {number}": "La longitud del elemento {number}",
    // The frames a plan is expressed in.
    "Above the sea": "Sobre el mar",
    "Above the home": "Sobre casa",
    "Above the ground": "Sobre el suelo",
    "Frame {number}": "El marco {number}",
    // The screen of the parameters.
    "Parameter file": "Archivo de parámetros",
    "All files": "Todos los archivos",
    "The parameters were written to {path}.":
      "Los parámetros se escribieron en {path}.",
    "{code} — the file was not written.": "{code} — el archivo no se escribió.",
    "{code} — the file was not read.": "{code} — el archivo no se leyó.",
    "{count} parameter was read from {path}.":
      "Se leyó {count} parámetro de {path}.",
    "{count} parameters were read from {path}.":
      "Se leyeron {count} parámetros de {path}.",
    "The vehicle and the file agree.":
      "El vehículo y el archivo están de acuerdo.",
    "{name}: the vehicle has {vehicleValue}, the file has {fileValue}":
      "{name}: el vehículo tiene {vehicleValue}, el archivo tiene {fileValue}",
    "{count} parameter differs. {lines}":
      "{count} parámetro se diferencia. {lines}",
    "{count} parameters differ. {lines}":
      "{count} parámetros se diferencian. {lines}",
    "{count} parameter was written to the vehicle.":
      "Se escribió {count} parámetro en el vehículo.",
    "{count} parameters were written to the vehicle.":
      "Se escribieron {count} parámetros en el vehículo.",
    "There is nothing to put back: no parameter that was changed has a default in the documentation.":
      "No hay nada que devolver: ningún parámetro cambiado tiene un valor de siempre en la documentación.",
    "{count} parameter was put back as it was.":
      "Se devolvió {count} parámetro como era antes.",
    "{count} parameters were put back as they were.":
      "Se devolvieron {count} parámetros como eran antes.",
    "{code} — the parameters were not put back.":
      "{code} — los parámetros no se devolvieron.",
    "The vehicle has written its parameters to its own memory.":
      "El vehículo ha escrito sus parámetros en su propia memoria.",
    "{code} — the vehicle did not write them.":
      "{code} — el vehículo no los escribió.",
    "{count} parameter reported": "{count} parámetro reportado",
    "{count} parameters reported": "{count} parámetros reportados",
    "Reading…": "Se están leyendo…",
    "Read all": "Leerlos todos",
    "Read the parameters of a file and write them to the vehicle":
      "Leer los parámetros de un archivo y escribirlos en el vehículo",
    "Load…": "Cargar…",
    "Write the parameters of the vehicle to a file":
      "Escribir los parámetros del vehículo en un archivo",
    "Save…": "Guardar…",
    "Say how the parameters of the vehicle differ from the ones of a file":
      "Decir en qué se diferencian los parámetros del vehículo de los de un archivo",
    "Compare…": "Comparar…",
    "Put back the parameters that were changed, as they were before":
      "Devolver los parámetros que se cambiaron como eran antes",
    "Restore the defaults": "Devolver los valores de siempre",
    "Ask the vehicle to write its parameters to its own memory":
      "Pedir al vehículo que escriba sus parámetros en su propia memoria",
    "Write to flash": "Escribir en la memoria",
    "the descriptions of the parameters are not shown.":
      "las descripciones de los parámetros no se muestran.",
    "Search the parameters by name": "Buscar los parámetros por su nombre",
    "Show every parameter again": "Volver a mostrar todos los parámetros",
    All: "Todos",
    "The width of the tree of the parameters":
      "El ancho del árbol de los parámetros",
    "The parameters could not be read. The link may be down.":
      "No se pudieron leer los parámetros. Puede que el enlace esté caído.",
    "Reading the parameters…": "Se están leyendo los parámetros…",
    "No parameter matches “{query}”.":
      "Ningún parámetro coincide con «{query}».",
    "Nothing under {label} matches the search.":
      "Bajo {label} nada coincide con la búsqueda.",
    "The vehicle has not reported a parameter yet.":
      "El vehículo todavía no ha dicho ningún parámetro.",
    Press: "Pulsa",
    "to ask it for the whole list.": "para pedirle la lista entera.",
    "No parameter has been written yet.":
      "Todavía no se ha escrito ningún parámetro.",
    "{name} written — the vehicle stored {value}":
      "{name} se escribió — el vehículo guardó {value}",
    Yes: "Sí",
    No: "No",
    "{count} parameter is waiting to be written to the vehicle.":
      "{count} parámetro espera que se escriba en el vehículo.",
    "{count} parameters are waiting to be written to the vehicle.":
      "{count} parámetros esperan que se escriban en el vehículo.",
    "the favourites have not been changed.":
      "los favoritos no se han cambiado.",
    "The parameters by prefix": "Los parámetros por prefijo",
    "The parameters the vehicle has reported":
      "Los parámetros que el vehículo ha dicho",
    "Default: {value}": "Valor de siempre: {value}",
    "Range: {minimum} to {maximum}": "Rango: {minimum} a {maximum}",
    "Group: {group}": "Grupo: {group}",
    "A change to it needs the vehicle to be restarted":
      "Cambiarlo obliga a reiniciar el vehículo",
    "Take {name} out of the favourites": "Quitar {name} de los favoritos",
    "Make {name} a favourite": "Hacer {name} favorito",
    "Not a favourite any more": "Ya no es favorito",
    "A favourite — kept between two runs":
      "Un favorito — se guarda entre dos ejecuciones",
    "{flag} of {name}": "{flag} de {name}",
    "The value of {name}": "El valor de {name}",
    "Writing…": "Se escribe…",
    Write: "Escribir",
    Cancel: "Cancelar",
    "The value is outside the range the documentation gives":
      "El valor está fuera del rango que da la documentación",
    "Choose one of the values it can be given":
      "Elegir uno de los valores que puede tener",
    "Click to change the value": "Pulsar para cambiar el valor",
    "{name}, {value}. Change the value": "{name}, {value}. Cambiar el valor",
    "A change to this parameter needs the vehicle to be restarted to take effect":
      "Cambiar este parámetro obliga a reiniciar el vehículo para que haga efecto",
    Reboot: "Reinicio",
    // The actions tab, its tests, and the attitude indicator.
    "Attitude indicator": "El indicador de actitud",
    "Attitude: roll {roll} degrees, pitch {pitch} degrees, heading {heading} degrees":
      "Actitud: alabeo {roll} grados, inclinación {pitch} grados, rumbo {heading} grados",
    ARMED: "ARMADO",
    DISARMED: "DESARMADO",
    "The height of the attitude indicator": "El alto del indicador de actitud",
    "Flight mode": "El modo de vuelo",
    "no modes": "sin modos",
    "Set Mode": "Cambiar el modo",
    "Confirm arm": "Confirmar el armado",
    "Confirm disarm": "Confirmar el desarmado",
    "Confirm reboot": "Confirmar el reinicio",
    Mission: "La misión",
    Fence: "El cercado",
    Rally: "Los puntos de regreso",
    Waypoint: "El punto del plan",
    "no plan": "sin plan",
    current: "el actual",
    "Set Current": "Poner el actual",
    Jump: "Saltar",
    "Fly to": "Volar hasta",
    "Tell the vehicle to fly to this waypoint":
      "Decir al vehículo que vuele hasta este punto",
    "Confirm clear": "Confirmar el quite",
    "Confirm upload": "Confirmar el envío",
    "Close editor": "Cerrar el editor",
    "Edit plan": "Editar el plan",
    "Edit the plan in a grid above the map":
      "Editar el plan en una rejilla sobre el mapa",
    "The plan above the map is being edited. Uploading sends the edited plan; setting the current item and jumping act on the plan the vehicle holds.":
      "El plan sobre el mapa se está editando. Subirlo envía el plan editado; poner el elemento actual y saltar actúan sobre el plan que el vehículo tiene.",
    Home: "La casa",
    "Set the home here": "Poner la casa aquí",
    "Confirm the home": "Confirmar la casa",
    "Ask the vehicle where its home is":
      "Preguntar al vehículo dónde está su casa",
    "Ask the vehicle": "Preguntar al vehículo",
    "The home is the place the vehicle measures the distance from. Moving it is what an operator does when the vehicle was armed in the wrong place.":
      "La casa es el lugar desde el que el vehículo mide la distancia. Moverla es lo que hace el operador cuando el vehículo se armó en el sitio equivocado.",
    "Motor test": "La prueba de motor",
    "One motor at a time, and only for a moment. The props are taken off, so the vehicle must not be flying.":
      "Un motor cada vez, y solo un momento. Las hélices están quitadas, así que el vehículo no tiene que estar volando.",
    "The motor to test": "El motor que se prueba",
    "Throttle %": "Gas %",
    "How hard the motor is driven": "Lo fuerte que se lleva el motor",
    Seconds: "Segundos",
    "For how long the motor is driven": "Cuánto tiempo se lleva el motor",
    "Test motor": "Probar el motor",
    "motor {motor} was driven at {throttle}% for {seconds} s":
      "el motor {motor} se llevó al {throttle}% durante {seconds} s",
    "Radio override": "Poner un canal a mano",
    "One channel at a time. The others are left to the radio, and the vehicle goes back to the radio when the channel is let go.":
      "Un canal cada vez. Los demás se dejan a la radio, y el vehículo vuelve a la radio cuando el canal se suelta.",
    Channel: "El canal",
    "The channel to take over": "El canal que se toma",
    "Value µs": "El valor, µs",
    "The value of the channel": "El valor del canal",
    Override: "Tomarlo",
    Release: "Soltarlo",
    "channel {channel} was taken over at {value} µs":
      "el canal {channel} se tomó a {value} µs",
    "the radio has the channels again": "la radio vuelve a tener los canales",
    "Manual control": "El mando manual",
    "The vehicle is flown with the sticks of this window. The vehicle has to be in the manual mode, and the sticks are sent again and again while they are held.":
      "El vehículo se vuela con los mandos de esta ventana. El vehículo tiene que estar en el modo manual, y los mandos se envían una y otra vez mientras se mantienen.",
    Roll: "Alabeo",
    Pitch: "Inclinación",
    Thrust: "Gas",
    Yaw: "Guiñada",
    "Hold to keep the sticks being sent":
      "Mantener pulsado para que los mandos sigan enviándose",
    "Sending…": "Se envían…",
    "Hold to send": "Mantener para enviar",
    "The width of the flight data column":
      "El ancho de la columna de los datos del vuelo",
    // The map: the tiles, the traffic, and going to a place.
    "The aircraft the vehicle has heard":
      "Las aeronaves que el vehículo ha oído",
    "{count} in the air": "{count} en el aire",
    "The provider of the tiles": "El proveedor de las fichas",
    Follow: "Seguir",
    Following: "Siguiendo",
    "Click the map to tell the vehicle to go there":
      "Pulsa el mapa para decir al vehículo que vaya ahí",
    "Click the map": "Pulsa el mapa",
    "Go here": "Ir aquí",
    "Fetch the tiles of this area and keep them, for a flight without a signal":
      "Pedir las fichas de esta zona y guardarlas, para un vuelo sin señal",
    "Save the area": "Guardar la zona",
    "Hide controls": "Quitar los controles",
    "Show controls": "Mostrar los controles",
    "The tiles of the area are fetched and kept, so that the map is drawn with them when there is no signal.":
      "Las fichas de la zona se piden y se guardan, para que el mapa se dibuje con ellas cuando no hay señal.",
    "Levels below": "Niveles debajo",
    "How many levels of zoom below the present one are fetched":
      "Cuántos niveles de zoom debajo del presente se piden",
    "Fetching…": "Se piden…",
    "Fetch the tiles": "Pedir las fichas",
    "Click the map to add a waypoint":
      "Pulsa el mapa para añadir un punto del plan",
    "The map could not be drawn: {failure}":
      "El mapa no se pudo dibujar: {failure}",
    "The sources of tiles could not be read.":
      "Las fuentes de fichas no se pudieron leer.",
    "{count} tile was fetched and kept.": "{count} ficha se pidió y se guardó.",
    "{count} tiles were fetched and kept.":
      "{count} fichas se pidieron y se guardaron.",
    "{code} — the tiles could not be fetched.":
      "{code} — las fichas no se pudieron pedir.",
    "The tiles of the area were already on the disk.":
      "Las fichas de la zona ya estaban en el disco.",
    "{count} vehicle on the link": "{count} vehículo en el enlace",
    "{count} vehicles on the link": "{count} vehículos en el enlace",
    "No vehicle on the link": "Ningún vehículo en el enlace",
    // The editor of the plan, and the profile of the ground.
    "QGC waypoints": "El archivo de QGC",
    "ESRI shapefile": "El archivo de forma de ESRI",
    "DXF drawing": "El dibujo DXF",
    "ESRI shapefile (the places of the drawing)":
      "El archivo de forma de ESRI (los lugares del dibujo)",
    "DXF drawing (the places of the drawing)":
      "El dibujo DXF (los lugares del dibujo)",
    "the altitude is zero — the vehicle would fly down to the ground.":
      "la altitud es cero — el vehículo bajaría hasta el suelo.",
    "the fence has fewer than three vertices — a polygon needs three.":
      "el cercado tiene menos de tres vértices — un polígono necesita tres.",
    "Mission plan": "El plan de la misión",
    "Rally points": "Los puntos de regreso",
    waypoint: "punto del plan",
    vertex: "vértice",
    "excluded vertex": "vértice excluido",
    "rally point": "punto de regreso",
    "Add a {name} on the map": "Añadir {name} en el mapa",
    "Click the map to add a {name}": "Pulsa el mapa para añadir {name}",
    "Adding a {name}": "Se añade {name}",
    "Add a {name}": "Añadir {name}",
    "Add an excluded vertex on the map":
      "Añadir un vértice excluido en el mapa",
    "Click the map to add a point that is excluded from the polygon":
      "Pulsa el mapa para añadir un punto que está fuera del polígono",
    "Add exclusion": "Añadir la exclusión",
    "Take the last change back": "Devolver el último cambio",
    "Take the last change back (Ctrl+Z)": "Devolver el último cambio (Ctrl+Z)",
    "Do again the change that was taken back":
      "Repetir el cambio que se devolvió",
    "Do again the change that was taken back (Ctrl+Shift+Z)":
      "Repetir el cambio que se devolvió (Ctrl+Shift+Z)",
    Undo: "Devolver",
    Redo: "Repetir",
    "Draw the path of the plan as a smooth curve":
      "Dibujar el camino del plan como una curva suave",
    Spline: "Curva suave",
    "Carry the plan in the file the vehicle keeps it in":
      "Llevar el plan en el archivo que el vehículo guarda",
    "Put the graphs back and close the editor":
      "Volver a poner los gráficos y cerrar el editor",
    "Write the plan to": "Escribir el plan en",
    "Read the plan from": "Leer el plan de",
    "The name of the file": "El nombre del archivo",
    "The name must end in .waypoints, .kml, .shp or .dxf":
      "El nombre tiene que terminar en .waypoints, .kml, .shp o .dxf",
    "Browse…": "Buscar…",
    "Write the plan": "Escribir el plan",
    "Read the plan": "Leer el plan",
    Confirm: "La confirmación",
    "The plan has {count} warning.": "El plan tiene {count} aviso.",
    "The plan has {count} warnings.": "El plan tiene {count} avisos.",
    "Item {number}: {warning}": "El elemento {number}: {warning}",
    "Write {count} item to the vehicle?":
      "¿Escribir {count} elemento en el vehículo?",
    "Write {count} items to the vehicle?":
      "¿Escribir {count} elementos en el vehículo?",
    "Read the plan from the vehicle again?":
      "¿Volver a leer el plan del vehículo?",
    "Read the plan from the vehicle again? The edits that have not been sent will be lost.":
      "¿Volver a leer el plan del vehículo? Los cambios que no se han enviado se perderán.",
    "Read the plan from {path}?": "¿Leer el plan de {path}?",
    "Read the plan from {path}? The edits that have not been sent will be lost.":
      "¿Leer el plan de {path}? Los cambios que no se han enviado se perderán.",
    Upload: "Escribir",
    "Go on": "Seguir",
    Strips: "Las franjas",
    Angle: "El ángulo",
    "How many strips the survey has": "Cuántas franjas tiene el levantamiento",
    "The angle of the survey": "El ángulo del levantamiento",
    "Add the reticle": "Añadir la retícula",
    "Radius m": "Radio, m",
    Points: "Los puntos",
    Direction: "El sentido",
    Anticlockwise: "Antihorario",
    Clockwise: "Horario",
    "The radius of the circle": "El radio del círculo",
    "How many points the circle has": "Cuántos puntos tiene el círculo",
    "The direction of the circle": "El sentido del círculo",
    "Add the circle": "Añadir el círculo",
    "The circle is centred on the waypoint that is selected, and it is added to the plan without throwing anything away.":
      "El círculo se centra en el punto del plan que está elegido, y se añade al plan sin tirar nada.",
    "Put the failure away": "Quitar el fallo",
    "{count} item": "{count} elemento",
    "{count} items": "{count} elementos",
    "The edits are not on the vehicle yet":
      "Los cambios todavía no están en el vehículo",
    "unsent changes": "los cambios sin enviar",
    "the same as the vehicle": "igual que en el vehículo",
    "Click the map to add a waypoint — Escape stops it.":
      "Pulsa el mapa para añadir un punto del plan — Escape lo para.",
    "The map marks {marked} of {places} items — zoom in for the rest.":
      "El mapa marca {marked} de {places} elementos — acerca el zoom para ver el resto.",
    "No file was chosen.": "No se eligió ningún archivo.",
    "This build has no file dialog — type the name of the file.":
      "Esta versión no tiene diálogo de archivos — escribe el nombre del archivo.",
    "A drawing can be read, not written — name a .waypoints or .kml file.":
      "Un dibujo se lee, no se escribe — nombra un archivo .waypoints o .kml.",
    "Add a survey": "Añadir un levantamiento",
    "Add the reticle of a survey to the plan":
      "Añadir la retícula de un levantamiento al plan",
    "Add a circle": "Añadir un círculo",
    "Add a circle of points to the plan": "Añadir un círculo de puntos al plan",
    "Profile of the ground": "El perfil del suelo",
    "Show the shape of the ground the plan flies over":
      "Mostrar la forma del suelo sobre el que el plan vuela",
    "Write the plan from the item that is selected":
      "Escribir el plan desde el elemento que está elegido",
    "The items before it are left on the vehicle as they are":
      "Los elementos antes de él se dejan en el vehículo como están",
    "Save to a file": "Guardar en un archivo",
    "Load from a file": "Leer de un archivo",
    "The profile of the terrain": "El perfil del terreno",
    "Profile of the terrain": "El perfil del terreno",
    "Looking for the ground…": "Se busca el suelo…",
    "Put the profile away": "Quitar el perfil",
    "the ground could not be looked up.": "el suelo no se pudo consultar.",
    "The plan has no places to look at, or the ground under them is not known.":
      "El plan no tiene lugares que mirar, o el suelo bajo ellos no se conoce.",
    "The elevation of the terrain along the plan":
      "La altura del terreno a lo largo del plan",
    More: "Más",
    "The plan editor": "El editor del plan",
    KML: "El archivo KML",
    "KML (Google Earth)": "El archivo KML (Google Earth)",
    "The kind of plan": "El tipo de plan",
    // The screen of the logs.
    "Telemetry log": "El registro de telemetría",
    "Log of PX4": "El registro de PX4",
    "Log of the vehicle": "El registro del vehículo",
    "KML document": "El documento KML",
    "CSV document": "El documento CSV",
    "Open a log…": "Abrir un registro…",
    "Read the log": "Leer el registro",
    "the name of the log": "el nombre del registro",
    "The name of the log": "El nombre del registro",
    "No log is open. A log is what a flight is remembered by: it is the stream of the frames that crossed the link, each of them with the instant it was seen.":
      "Ningún registro está abierto. Un registro es lo que guarda la memoria de un vuelo: es la corriente de los cuadros que cruzaron el enlace, cada uno con el instante en que se vio.",
    "Pause the flight": "Parar el vuelo",
    "Play the flight": "Reproducir el vuelo",
    Pause: "Parar",
    Play: "Reproducir",
    "Where the flight is": "Dónde va el vuelo",
    "{elapsed} s of {total} s": "{elapsed} s de {total} s",
    "{records} record": "{records} registro",
    "{records} records": "{records} registros",
    "{seconds} second": "{seconds} segundo",
    "{seconds} seconds": "{seconds} segundos",
    "{kinds} kind of message": "{kinds} tipo de mensaje",
    "{kinds} kinds of message": "{kinds} tipos de mensaje",
    "The logs of the vehicle": "Los registros del vehículo",
    "Ask the vehicle for the logs it holds in its own memory":
      "Pedir al vehículo los registros que tiene en su propia memoria",
    "Log {number} · {size} kB": "Registro {number} · {size} kB",
    "The samples of": "Las muestras de",
    "The message the samples are of": "El mensaje del que son las muestras",
    "Choose a message": "Elegir un mensaje",
    Graph: "El gráfico",
    "The graph drawn over the log": "El gráfico dibujado sobre el registro",
    "{shown} of {total} sample. Click a row and the flight is moved to that moment.":
      "{shown} de {total} muestra. Al pulsar una fila, el vuelo se mueve a ese momento.",
    "{shown} of {total} samples. Click a row and the flight is moved to that moment.":
      "{shown} de {total} muestras. Al pulsar una fila, el vuelo se mueve a ese momento.",
    "Write the track of the log as a KML document":
      "Escribir el camino del registro como documento KML",
    "Write the track (KML)": "Escribir el camino (KML)",
    "Write the samples of the graph as a CSV document":
      "Escribir las muestras del gráfico como documento CSV",
    "Write the samples (CSV)": "Escribir las muestras (CSV)",
    "The log was written to {path}.": "El registro se ha escrito en {path}.",
    "The track was written to {path}.": "El camino se ha escrito en {path}.",
    "The samples were written to {path}.":
      "Las muestras se han escrito en {path}.",
    "MPCraft {version} — a ground station for ArduPilot and PX4, written in Rust and React.":
      "MPCraft {version} — una estación de tierra para ArduPilot y PX4, escrita en Rust y React.",
    "The map uses the demo tiles of MapLibre and needs no account.":
      "El mapa usa las fichas de demostración de MapLibre y no necesita cuenta.",
    "Parameters…": "Los parámetros…",
    Motor: "Motor",
    "Open this link when the application starts":
      "Abrir este enlace cuando la aplicación arranca",
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
