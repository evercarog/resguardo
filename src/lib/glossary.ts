// «¿Qué significa?»: explicaciones cortas de cada estado, cifra y
// comprobación, con qué hacer. Es la única fuente de estos textos: la usan
// los «?» de la interfaz (InfoTip) y la sección «Qué significa cada cosa» del
// centro de ayuda, que se genera a partir de aquí.

export interface GlossaryEntry {
  title: string;
  /** Qué es, en una o dos frases. */
  text: string;
  /** Qué hacer (si hay algo que hacer). */
  todo?: string;
  /** Apartado de la ayuda con más detalle. */
  topic?: string;
  /** Grupo en la ayuda. */
  group: "conceptos" | "cifras" | "copias" | "repositorios" | "proteccion";
}

export const GLOSSARY_GROUPS: Record<GlossaryEntry["group"], string> = {
  conceptos: "Conceptos",
  cifras: "Cifras",
  copias: "Estado de una copia",
  repositorios: "Estado de un repositorio",
  proteccion: "Salud de la protección",
};

export const GLOSSARY: Record<string, GlossaryEntry> = {
  // ---------- Conceptos ----------
  destino: {
    group: "conceptos",
    title: "Destino",
    text: "El lugar donde se guardan las copias: un disco o una carpeta de la red, un servidor o un bucket de la nube con su clave. Dentro puede tener varios repositorios.",
    topic: "destinos-repositorios",
  },
  repositorio: {
    group: "conceptos",
    title: "Repositorio",
    text: "Una caja cifrada dentro de un destino, con su propia contraseña. Ahí se guardan las versiones de una o varias copias; sin su contraseña nadie puede abrirla, ni el dueño del disco o de la nube.",
    todo: "Guarda su contraseña en el kit de recuperación.",
    topic: "destinos-repositorios",
  },
  "repo-encontrado": {
    group: "conceptos",
    title: "Encontrado",
    text: "Resguardo lo ha visto al mirar dentro del destino (una carpeta o un bucket), pero este equipo todavía no lo usa. Puede ser de otro equipo o uno antiguo.",
    todo: "«Usar este» lo añade con su contraseña. Para copiar, mejor crea uno nuevo; úsalo para restaurar o verificar desde aquí.",
    topic: "destino-dentro",
  },
  "repo-recordado": {
    group: "conceptos",
    title: "Recordado",
    text: "Este equipo lo usó en este destino. Algunos destinos, como un servidor REST o SFTP, no dejan ver qué repositorios hay, así que Resguardo recuerda los que conoce.",
    todo: "Si sigue existiendo, «Usar este» lo vuelve a añadir con su contraseña.",
    topic: "destino-dentro",
  },

  // ---------- Cifras ----------
  "ultima-copia": {
    group: "cifras",
    title: "Última copia",
    text: "Cuándo terminó la copia más reciente, a mano o automática. Una copia que no encontró cambios también cuenta: estaba al día.",
    todo: "Si es más antigua de lo esperado, mira la pestaña «Historia» del repositorio o el registro del agente.",
  },
  "ultima-revision": {
    group: "cifras",
    title: "Última revisión",
    text: "La copia se hizo a su hora, pero no había nada nuevo, así que no hizo falta guardar otra versión («Solo guardar si hay cambios»).",
    topic: "copias-sin-cambios",
  },
  proxima: {
    group: "cifras",
    title: "Próxima",
    text: "Cuándo toca la siguiente copia automática según su horario. «A mano»: no tiene horario. «Sin programar»: tiene horario, pero el agente aún no la hace.",
    todo: "Para que se haga sola, edita la copia, activa «Hacerla sola, con horario» y prográmala.",
    topic: "copias-horarios",
  },
  versiones: {
    group: "cifras",
    title: "Versiones",
    text: "Cada copia que encuentra cambios guarda una versión: una foto completa de tus carpetas en ese momento, que puedes explorar y restaurar.",
    todo: "La retención decide cuántas versiones antiguas se conservan.",
    topic: "mant-retencion",
  },
  "tamano-protegido": {
    group: "cifras",
    title: "Tamaño protegido",
    text: "Lo que ocupan tus archivos en la última versión, tal como están en tu equipo. No es lo que ocupa el repositorio.",
  },
  "espacio-disco": {
    group: "cifras",
    title: "Espacio en disco",
    text: "Lo que ocupa de verdad el repositorio con todas sus versiones. Suele ser mucho menos que su suma: restic guarda una sola vez cada trozo de archivo que se repite (deduplicación) y lo comprime. Así, 8,8 GB protegidos pueden ocupar 3,1 GB.",
    todo: "Para liberar espacio, define una retención: las versiones antiguas se borran y su espacio se recupera.",
    topic: "ocupa-espacio",
  },
  "total-protegido": {
    group: "cifras",
    title: "Protegidos",
    text: "La suma del tamaño protegido de todos tus repositorios (lo que ocupan tus archivos en su última versión). En disco ocupa menos gracias a la deduplicación y la compresión.",
  },

  // ---------- Estado de una copia ----------
  "copia-ok": { group: "copias", title: "Al día", text: "La última copia terminó bien y no se ha saltado ninguna de su horario." },
  "copia-running": { group: "copias", title: "Copiando", text: "Se está haciendo ahora mismo, a mano o por el agente. Puedes seguir usando el equipo." },
  "copia-warning": {
    group: "copias",
    title: "Última copia con avisos",
    text: "Se guardó todo menos algunos archivos que no se pudieron leer, normalmente porque estaban abiertos o sin permiso.",
    todo: "Abre la copia y mira qué archivos fueron: cierra el programa que los usa o abre Resguardo como administrador.",
  },
  "copia-error": {
    group: "copias",
    title: "Falló la última copia",
    text: "La última copia no se pudo hacer, así que no se guardó ninguna versión.",
    todo: "El mensaje de la copia dice por qué (disco desconectado, servidor que no responde…). Arréglalo y pulsa «Copiar ahora».",
    topic: "restaurar-problemas",
  },
  "copia-late": {
    group: "copias",
    title: "Con retraso",
    text: "Pasó la hora y media siguiente a una copia prevista y no se ha hecho. Suele pasar si el equipo estuvo apagado o el repositorio desconectado.",
    todo: "Pulsa «Copiar ahora» o mira el registro del agente.",
  },
  "copia-paused": { group: "copias", title: "En pausa", text: "Las copias automáticas de su repositorio están en pausa a propósito.", todo: "Se reanudan solas a la hora indicada, o pulsa «Reanudar».", topic: "auto-pausar" },
  "copia-never": { group: "copias", title: "Sin copias todavía", text: "Esta copia aún no se ha hecho nunca.", todo: "Pulsa «Copiar ahora» para guardar la primera versión." },
  "copia-loading": { group: "copias", title: "Comprobando…", text: "Resguardo está leyendo las versiones del repositorio para saber cómo va." },

  // ---------- Estado de un destino ----------
  "destino-ok": { group: "repositorios", title: "Al día", text: "Sus copias llegan al ritmo esperado." },
  "destino-late": {
    group: "repositorios",
    title: "Con retraso",
    text: "La última versión es algo más antigua de lo habitual para este repositorio.",
    todo: "Revisa las copias que se guardan aquí.",
  },
  "destino-overdue": {
    group: "repositorios",
    title: "Atrasado",
    text: "Lleva más del doble de lo habitual sin versiones nuevas.",
    todo: "Abre el repositorio y mira la «Historia»: puede que el disco esté desconectado o que una copia falle.",
  },
  "destino-empty": { group: "repositorios", title: "Sin versiones", text: "Aún no se ha guardado ninguna versión aquí.", todo: "Crea una copia que se guarde aquí y pulsa «Copiar ahora»." },
  "destino-error": {
    group: "repositorios",
    title: "Sin conexión",
    text: "Resguardo no pudo leer el repositorio la última vez que lo intentó.",
    todo: "Conecta el disco o comprueba que el servidor esté encendido y accesible.",
  },
  "destino-paused": { group: "repositorios", title: "En pausa", text: "Sus copias automáticas están en pausa a propósito: no se avisa de retrasos.", topic: "auto-pausar" },
  "destino-loading": { group: "repositorios", title: "Comprobando…", text: "Resguardo está leyendo sus versiones." },

  // ---------- Salud de la protección ----------
  "prot-copias": {
    group: "proteccion",
    title: "Copias automáticas",
    text: "Que las copias se hagan solas a su hora, aunque nadie se acuerde y la app esté cerrada.",
    todo: "Pon horario a sus copias y prográmalas como administrador.",
    topic: "auto-agente",
  },
  "prot-borrado": {
    group: "proteccion",
    title: "Protegida contra borrado",
    text: "Que nadie, ni un ransomware con acceso a este equipo, pueda borrar las versiones: un servidor REST de solo añadir o un bucket con bloqueo de objetos. Un disco normal se puede borrar.",
    todo: "Usa un servidor de solo añadir o una copia externa a un bucket con bloqueo de objetos.",
    topic: "mant-proteger",
  },
  "prot-externa": {
    group: "proteccion",
    title: "Copia externa",
    text: "Otra copia fuera de este sitio (la nube u otro disco), por si hay un robo, un incendio o un ransomware.",
    todo: "Configúrala en «Mantenimiento» del repositorio.",
    topic: "mant-copia-externa",
  },
  "prot-verificacion": {
    group: "proteccion",
    title: "Verificación",
    text: "Comprueba con regularidad que lo guardado no se ha dañado en el disco o en el servidor.",
    todo: "Prográmala en «Mantenimiento»: la rotativa lee todos los datos poco a poco.",
    topic: "mant-verificacion",
  },
  "prot-restauracion": {
    group: "proteccion",
    title: "Prueba de restauración",
    text: "Restaura unos archivos al azar y comprueba que salen enteros: así sabes que las copias se pueden recuperar, no solo que existen.",
    todo: "Prográmala en «Mantenimiento».",
    topic: "mant-prueba-restauracion",
  },
  "prot-kit": {
    group: "proteccion",
    title: "Kit de recuperación",
    text: "Una hoja con lo necesario para abrir tus copias desde otro equipo si pierdes este (la ubicación y la contraseña).",
    todo: "Prepáralo, imprímelo o guárdalo fuera del equipo y confírmalo.",
    topic: "kit-recuperacion",
  },
  "prot-retencion": {
    group: "proteccion",
    title: "Retención",
    text: "Cuántas versiones antiguas se conservan. Sin ella, el repositorio crece sin fin.",
    todo: "Elige un ajuste rápido en la pestaña «Retención».",
    topic: "retencion-plazos",
  },
};

/** El HTML de un apartado de la ayuda a partir de una entrada. */
export function glossaryHtml(e: GlossaryEntry) {
  const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;");
  return `<p>${esc(e.text)}</p>${e.todo ? `<p><strong>Qué hacer:</strong> ${esc(e.todo)}</p>` : ""}${
    e.topic ? `<p><a href="#" data-topic="${e.topic}">Más detalles</a></p>` : ""
  }`;
}
