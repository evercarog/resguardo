// Novedades de cada versión, para quien usa la app (sin tecnicismos). La
// más reciente primero. Al publicar una versión, añade aquí su entrada.

export interface Release {
  version: string;
  /** AAAA-MM-DD */
  date: string;
  /** Resumen en pocas palabras. */
  title: string;
  items: string[];
}

export const CHANGELOG: Release[] = [
  {
    version: "0.6.8",
    date: "2026-10-02",
    title: "Arreglo para servidores de copias",
    items: [
      "En un equipo sin repositorios propios (por ejemplo, el que solo hace de Servidor de copias), la barra lateral no mostraba «Equipos gestionados» ni «Todos mis equipos». Ahora están siempre a mano.",
    ],
  },
  {
    version: "0.6.7",
    date: "2026-10-02",
    title: "Servidor de copias y equipos gestionados",
    items: [
      "Convierte un equipo Windows en el servidor de copias de tus otros equipos (Ajustes → Este equipo → Servidor de copias): elige la carpeta y añade un equipo por cada uno que vaya a copiar ahí.",
      "Seguro por diseño: solo se puede añadir (nadie borra copias desde los equipos), cada equipo ve solo su carpeta, todo va con TLS y el firewall solo abre ese puerto, por defecto solo para tu red local.",
      "Los equipos lo piden como un destino compartido y les llega cifrado; desde otra sede, la app explica qué puerto abrir en el router (nunca lo abre sola).",
      "Nueva sección «Equipos gestionados» (en el equipo que hace de Servidor de copias): empareja con un código los equipos que administras y decide desde aquí qué copian y cuándo.",
      "En esos equipos basta Resguardo Agente: sin ventana, un servicio y un icono discreto en la bandeja que dice a su usuario que sus archivos están protegidos y quién gestiona el equipo.",
      "Seguro por diseño: un código de comprobación al emparejar y cada orden firmada por la consola y cifrada solo para ese equipo; ni la web puede falsificarlas.",
      "«Copiar ahora», «Restaurar sus archivos…» y «Dejar de gestionar» para cada equipo.",
      "«Preparar un equipo»: guarda el instalador de Resguardo Agente en una memoria USB; en ese equipo basta abrirlo y escribir el código. Para muchos equipos, también sin ventanas.",
      "Cada equipo muestra su última versión y cuánto ocupa, y esta consola aplica su retención en el servidor cada semana (por defecto «Frecuente»).",
      "Si cambia la IP del Servidor de copias, se renueva su certificado y los equipos reciben la dirección nueva solos.",
      "«Desbloquear» cuando una copia cortada deja el repositorio bloqueado, y «Actualizar la contraseña guardada…» si la cambiaste en otro sitio.",
    ],
  },
  {
    version: "0.6.6",
    date: "2026-10-02",
    title: "Compartir un destino con tus equipos",
    items: [
      "Comparte la cuenta de la nube o el servidor de un destino con tus otros equipos sin volver a escribir las claves: se pide desde el otro equipo y llega cifrada solo para él.",
      "Cada equipo crea ahí su propio repositorio con su propia contraseña: las contraseñas de los repositorios no se comparten nunca.",
      "Tienes 5 minutos para cancelar una petición, cada entrega avisa en los dos equipos y queda en la Actividad, y puedes dejar de compartir cuando quieras.",
    ],
  },
  {
    version: "0.6.5",
    date: "2026-10-02",
    title: "Dentro de un destino",
    items: [
      "Pulsa un destino en la barra lateral para ver sus repositorios: los de este equipo, los que encuentra Resguardo (en carpetas y en buckets S3) y los que ya se usaron ahí.",
      "«Usar este» añade un repositorio que ya existe, con su contraseña. «Crear uno nuevo aquí» propone la ruta y una contraseña fuerte, y te lleva al kit de recuperación para guardarla.",
      "«Clonar…» copia todas las versiones de un repositorio a uno nuevo en otro destino, con su propia contraseña y viendo el avance.",
      "Ctrl+K encuentra también los destinos.",
    ],
  },
  {
    version: "0.6.4",
    date: "2026-10-02",
    title: "Destinos y repositorios",
    items: [
      "Destinos y repositorios: un destino es el lugar (un disco, un servidor, un bucket de la nube) y dentro puede haber varios repositorios, cada uno con su contraseña. La barra lateral los agrupa así, sin que tengas que hacer nada.",
      "Los textos de toda la app usan siempre las mismas palabras: «destino» es el lugar y «repositorio» la caja cifrada con su contraseña. Un «?» lo explica donde aparece por primera vez.",
      "Puedes renombrar un destino (por ejemplo, «Servidor de la oficina») sin tocar sus repositorios.",
    ],
  },
  {
    version: "0.6.3",
    date: "2026-10-02",
    title: "Copias más claras",
    items: [
      "Arriba de cada repositorio y de cada copia, un resumen en pocas palabras: cómo está, lo último que pasó y lo más importante que falta.",
      "Nuevo asistente «Mejorar la protección»: te lleva por lo que le falta a un repositorio, uno por uno, con el anillo actualizándose mientras lo configuras.",
      "Nueva pestaña «Historia» en cada repositorio: copias, subidas, verificaciones, pausas y, a partir de ahora, quién cambió qué en su configuración y cuándo.",
      "Un «?» junto a cada estado, cifra y comprobación explica qué significa y qué hacer (por ejemplo, por qué el repositorio ocupa menos que lo protegido). Todo está también en la ayuda, en «Qué significa cada cosa».",
      "Ctrl+K encuentra todo: además de repositorios y copias, acciones (copiar ahora, pausar, restaurar, el kit, verificar…), ajustes por su nombre o con otras palabras y la ayuda («qué es la retención»).",
      "Nueva campana de avisos: las copias que fallan, las subidas frenadas, lo que sigue pendiente (un repositorio atrasado, el kit) y un botón para ir a arreglarlo, con leído y sin leer.",
      "Los editores son más sencillos: lo que casi nadie cambia (etiquetas, umbrales, límites, certificados…) está plegado en «Opciones avanzadas» con los valores recomendados, y Resguardo recuerda cuáles dejas abiertas.",
      "Cada repositorio enseña el recorrido de tus copias: tus archivos → el repositorio → la copia externa, con el estado y la hora de cada paso. Pulsa uno para ir a su sección.",
      "«Ver versiones en Resguardo» muestra una fila por cada vez que cambió el archivo; despliégala para ver todas las fechas con ese contenido y abrir o restaurar cualquiera.",
      "Si el archivo no ha cambiado, te lo dice tal cual, y puedes buscar también en las versiones más antiguas del repositorio (de antes de cambiar la copia o de otras copias).",
      "Como administrador, las copias a mano también copian los archivos abiertos (como Outlook), igual que las automáticas.",
      "Los errores más comunes (disco lleno, repositorio ocupado, disco desconectado, servidor que no responde) dicen qué hacer, y una copia con archivos sin leer explica por qué y cómo arreglarlo.",
      "Nueva copia: al elegir la carpeta se propone su nombre. Si cierras el editor con cambios sin guardar, te pregunta antes de descartarlos.",
    ],
  },
  {
    version: "0.6.2",
    date: "2026-10-02",
    title: "Todos los ajustes en un sitio",
    items: [
      "Ajustes es ahora una página con secciones: General, Este equipo, Bandeja y avisos, Explorador de archivos y Seguridad, con un índice a la izquierda (también con Ctrl+,).",
      "Las copias a distancia, Resguardo Web, el modo discreto y el registro del agente están en «Este equipo»; el kit de recuperación y la sesión de «Todos mis equipos», en «Seguridad».",
      "Cada ajuste indica si pide abrir Resguardo como administrador, la contraseña de un destino o Windows Hello.",
    ],
  },
  {
    version: "0.6.1",
    date: "2026-10-02",
    title: "Todos tus equipos desde aquí",
    items: [
      "Nueva sección «Todos mis equipos»: entra con tu cuenta de Resguardo Web y ve el estado de tus otros equipos (el servidor, otros portátiles…).",
      "Desde ahí puedes pedirles «Copiar ahora» de una copia y ver cómo va: pedida, en marcha, hecha.",
      "Cada equipo decide si acepta copias a distancia (Estado → Resguardo Web). A distancia solo se piden copias: nada se puede borrar, restaurar ni cambiar.",
      "Resguardo vive ahora en la bandeja: un escudo verde, ámbar o rojo te dice de un vistazo cómo van tus copias, y desde su menú puedes copiar ahora o pausar una hora.",
      "Avisos de Windows si falla una copia, si se frena una subida a la nube por un cambio inusual, cuando termina esa subida o si falta el kit de recuperación. Se ajustan en Ajustes → Bandeja y avisos.",
      "Resguardo se inicia con Windows en la bandeja (puedes desactivarlo), y abrirlo otra vez trae al frente la ventana que ya había.",
      "Modo discreto: mientras se trabaja (por defecto, de lunes a sábado de 7 a 19), las copias automáticas, subidas y verificaciones van con prioridad baja y, si quieres, con la subida limitada. Actívalo en Ajustes.",
      "Nuevo «Ver versiones en Resguardo» al hacer clic derecho en un archivo o carpeta del Explorador (actívalo en Ajustes → Explorador de archivos): ves sus versiones y restauras la que quieras junto al original, con otro nombre y sin reemplazar nada.",
    ],
  },
  {
    version: "0.6.0",
    date: "2026-10-01",
    title: "Nuevo diseño",
    items: [
      "Resguardo estrena diseño: más limpio, con más aire, tipografía Inter y modo claro y oscuro igual de cuidados (sigue el de Windows; puedes cambiarlo en Apariencia).",
      "El inicio empieza con un resumen: «Todo protegido» o lo que necesita tu atención, ordenado por importancia y con un botón para resolver cada cosa.",
      "Los asistentes de nueva copia y de restauración muestran sus pasos con claridad, y todos los diálogos tienen el mismo aspecto.",
      "Si empiezas desde cero, una bienvenida te guía en 3 pasos: añadir un destino, crear tu primera copia y programarla.",
      "La retención, el kit de recuperación y el bloqueo de objetos que guardas se reflejan ahora en la web y en la salud de la protección.",
    ],
  },
  {
    version: "0.5.20",
    date: "2026-10-01",
    title: "Qué archivos fallaron",
    items: [
      "Si una copia automática termina con archivos que no se pudieron leer, abre Resguardo como administrador y pulsa «Ver qué archivos» para ver cuáles y por qué.",
      "El registro del agente, abierto como administrador, puede mostrar el detalle con las rutas completas.",
    ],
  },
  {
    version: "0.5.19",
    date: "2026-10-01",
    title: "Más seguridad",
    items: [
      "Revisión de seguridad completa del agente: se cierran vías por las que otro usuario del equipo podría haber engañado al agente, y el certificado propio de un servidor se guarda en la carpeta protegida.",
      "Los registros del agente y lo que se envía a la web ya no incluyen rutas ni nombres de archivos.",
      "Al desinstalar puedes elegir borrar también los datos del agente (por defecto se conservan).",
      "Confirmar que guardaste el kit de recuperación pide la contraseña del destino.",
      "La app comprueba que el restic incluido es exactamente el oficial.",
    ],
  },
  {
    version: "0.5.18",
    date: "2026-10-01",
    title: "Buscar un archivo y bloquear la app",
    items: [
      "Nuevo «Buscar un archivo»: escribe un nombre (o parte) y verás en qué versiones está, cuándo apareció por primera y por última vez y si sigue en la más reciente. Desde ahí lo abres o lo restauras.",
      "También con Ctrl+K: «Buscar archivo en…» el destino que elijas.",
      "Opción para pedir Windows Hello (rostro, huella o PIN) al abrir Resguardo (en Ajustes), y para volver a pedirla tras un rato sin usar la app. Útil en equipos compartidos.",
      "Esta ventana de «Novedades» aparece tras cada actualización; puedes volver a verla desde Ajustes.",
    ],
  },
  {
    version: "0.5.17",
    date: "2026-09-30",
    title: "Más clara y más cómoda",
    items: [
      "La vista de cada destino sigue un orden lógico: salud, copias automáticas, copias, mantenimiento y versiones.",
      "«Copias automáticas» y «Mantenimiento» se pueden plegar, y la app recuerda cómo las dejaste y qué pestaña viste la última vez.",
      "Ctrl+K para saltar a cualquier destino o copia escribiendo su nombre.",
      "Al pasar el ratón por «hace 5 minutos» ves la fecha y hora exactas; los botones desactivados explican qué falta.",
    ],
  },
  {
    version: "0.5.16",
    date: "2026-09-30",
    title: "Kit de recuperación y salud de la protección",
    items: [
      "Kit de recuperación: una hoja para imprimir con lo necesario para recuperar tus copias si pierdes este equipo.",
      "Prueba de restauración: el agente restaura de vez en cuando unos archivos al azar y comprueba que salen bien.",
      "Salud de la protección: cada destino muestra qué está bien y qué falta (copias automáticas, verificación, copia externa, retención…) con un botón para arreglarlo.",
    ],
  },
  {
    version: "0.5.15",
    date: "2026-09-30",
    title: "La copia en la nube, bajo control",
    items: [
      "Puedes verificar la copia que subes a la nube desde el destino de origen.",
      "Un destino que solo recibe la copia externa de otro lo explica y te lleva a donde se gestiona.",
    ],
  },
  {
    version: "0.5.14",
    date: "2026-09-30",
    title: "Verificación rotativa",
    items: [
      "Nueva verificación rotativa: cada vez se leen datos distintos y, tras unas cuantas, se han comprobado todos, sin hacer una verificación larga de golpe.",
      "Puedes elegir en cuántas partes se reparte y ver cuándo se leyó todo por última vez.",
    ],
  },
  {
    version: "0.5.13",
    date: "2026-09-30",
    title: "Corrección",
    items: ["Si la cuenta de la nube llega a su límite diario, el aviso dice eso en lugar de «repositorio ocupado»."],
  },
  {
    version: "0.5.12",
    date: "2026-09-30",
    title: "Progreso de la subida y la verificación",
    items: [
      "Mientras se sube la copia a la nube o se verifica un destino, ves el progreso y cuánto falta.",
    ],
  },
  {
    version: "0.5.11",
    date: "2026-09-30",
    title: "Subida tras cada copia y freno ante cambios raros",
    items: [
      "La copia externa puede subirse justo después de cada copia con cambios.",
      "Si una copia cambia muchísimo más de lo normal (por ejemplo, por un virus que cifra archivos), la subida a la nube se frena hasta que confirmes que es normal.",
      "Desde el aviso puedes ver exactamente qué cambió.",
    ],
  },
  {
    version: "0.5.10",
    date: "2026-09-30",
    title: "Corrección",
    items: ["La vista previa de la retención ya no falla cuando no hay nada que borrar."],
  },
  {
    version: "0.5.9",
    date: "2026-09-30",
    title: "Retención más flexible",
    items: [
      "Retención por plazos: por ejemplo, una versión por hora durante dos días, una por día durante un mes y una por mes durante un año.",
      "Puedes agrupar, filtrar por etiquetas y conservar siempre las versiones con ciertas etiquetas.",
      "La copia externa puede tener su propia retención en el destino.",
    ],
  },
  {
    version: "0.5.8",
    date: "2026-09-30",
    title: "Solo guardar si hay cambios",
    items: [
      "Nueva opción por copia: si nada cambió desde la última vez, no se guarda una versión nueva (la copia sigue contando como hecha).",
    ],
  },
  {
    version: "0.5.7",
    date: "2026-09-30",
    title: "Pausar las copias automáticas",
    items: [
      "Puedes pausar las copias automáticas de un destino un tiempo (por ejemplo, durante un mantenimiento) sin perder su configuración. Se reanudan solas.",
      "Un fallo sigue a la vista aunque el destino esté en pausa.",
    ],
  },
  {
    version: "0.5.6",
    date: "2026-09-29",
    title: "Lo que más ocupa",
    items: [
      "«Lo que más ocupa»: las carpetas y archivos más grandes de una versión.",
      "Desde ahí puedes excluir algo de las próximas copias en un par de clics.",
      "Corrección: la ruta del explorador de versiones ya se muestra en orden.",
    ],
  },
  {
    version: "0.5.5",
    date: "2026-09-29",
    title: "Retención en la copia externa",
    items: ["Si un destino solo recibe la copia externa de otro, puedes aplicar allí la retención sin tocar nada más."],
  },
  {
    version: "0.5.4",
    date: "2026-09-29",
    title: "Correcciones",
    items: [
      "La actividad ya no repite días ni cierra «Mostrar más» al refrescar.",
      "El explorador distingue una carpeta vacía de una que no existía en esa versión.",
      "Exportar la actividad a CSV es más seguro.",
    ],
  },
  {
    version: "0.5.3",
    date: "2026-09-29",
    title: "Repaso de la interfaz",
    items: [
      "Primer uso guiado: destino, copia y horario seguidos, paso a paso.",
      "La ventana funciona bien también en tamaños pequeños.",
      "Mejor uso con teclado y lector de pantalla.",
    ],
  },
  {
    version: "0.5.2",
    date: "2026-09-29",
    title: "Historial de actividad",
    items: [
      "Nueva sección «Actividad» con el historial de copias, verificaciones y subidas.",
      "Si el agente no puede ver una carpeta (unidad de red, disco desconectado), te lo explica.",
    ],
  },
  {
    version: "0.5.1",
    date: "2026-09-29",
    title: "Restaurar paso a paso y ayuda",
    items: [
      "Asistente para restaurar archivos en cuatro pasos: versión, qué, dónde y listo.",
      "Centro de ayuda (F1) con explicaciones de cada parte de la app.",
    ],
  },
  {
    version: "0.5.0",
    date: "2026-09-29",
    title: "Copias y destinos",
    items: [
      "La app se organiza en Copias (qué guardas y cuándo) y Destinos (dónde se guarda).",
      "Varias copias por destino, cada una con sus carpetas, exclusiones y horario.",
      "Destinos en la nube (Backblaze, S3…) y copia externa hacia otro destino.",
      "Si una copia programada falla, se reintenta sola.",
    ],
  },
];

/** Compara versiones «1.2.3» (negativo si a < b). */
export function compareVersions(a: string, b: string): number {
  const pa = a.split(/[.-]/).map((x) => parseInt(x, 10) || 0);
  const pb = b.split(/[.-]/).map((x) => parseInt(x, 10) || 0);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const d = (pa[i] ?? 0) - (pb[i] ?? 0);
    if (d) return d;
  }
  return 0;
}

/** Entradas posteriores a `from` y hasta `to` (incluida), de la más reciente a la más antigua. */
export const releasesBetween = (from: string, to: string) =>
  CHANGELOG.filter((r) => compareVersions(r.version, from) > 0 && compareVersions(r.version, to) <= 0);
