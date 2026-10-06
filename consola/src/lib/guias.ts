// Centro de ayuda: guías cortas de lo que se puede hacer con Resguardo Server
// y «¿Qué hago si…?». Mismo tono que la app: de tú, tranquilo y diciendo qué
// hacer. Los comandos del agente se ejecutan en el propio equipo, en una
// consola abierta como administrador (o con sudo en Linux).

export interface Guia {
  id: string;
  titulo: string;
  /** Una frase: para qué sirve. */
  resumen: string;
  /** Párrafos. */
  texto: string[];
  /** Pasos, si los hay. */
  pasos?: string[];
  /** Un comando para copiar (en el equipo, como administrador). */
  comando?: string;
  /** Enlaces del glosario relacionados. */
  ver?: string[];
}

export interface Pregunta {
  id: string;
  si: string;
  respuesta: string[];
  comando?: string;
}

export const GUIAS: Guia[] = [
  {
    id: "guia-llaves",
    titulo: "Tu sesión, la clave de administración y la contraseña del repositorio",
    resumen: "Tres llaves distintas, y cada una protege una cosa.",
    texto: [
      "Tu sesión (correo, contraseña y el segundo paso) te deja entrar en la consola: ver el estado y pedir lo que no pone nada en riesgo, como «Copiar ahora». Quien la robe no puede cambiar qué se copia ni ver archivos.",
      "La clave de administración es del cliente, no de una persona. Autoriza lo que cambia la estructura: qué se copia, dónde y cuándo, dar de alta o de baja equipos, pausar o mover el cliente a otro servidor. Se escribe en este navegador y viaja sellada solo para el equipo: el servidor nunca la ve.",
      "La contraseña de cada repositorio cifra sus copias. Hace falta para ver archivos, restaurar, descargar o borrar versiones. Está en el kit de recuperación de ese repositorio. También viaja sellada solo para el equipo.",
    ],
    ver: ["clave-admin", "contrasena-repo", "totp"],
  },
  {
    id: "guia-guarda-copias",
    titulo: "«Este equipo guarda copias»",
    resumen: "Un equipo de la oficina recibe las copias de los demás, sin que nadie pueda borrarlas desde el equipo que copia.",
    texto: [
      "Se instala en ese equipo un servidor de copias de solo añadir (rest-server) con su propio certificado y su regla del cortafuegos. Cada equipo que copia en él tiene su usuario y no puede borrar lo ya copiado, ni siquiera un ransomware que se cuele en él.",
      "Es rápido para restaurar y no hace falta tocar el router. Lo recomendado es tener además una copia fuera de la oficina: el espejo en otro disco o en la nube, o una copia externa.",
    ],
    pasos: [
      "En la ficha del equipo, «Este equipo guarda copias»: elige la carpeta (mejor un disco dedicado) y el puerto.",
      "En la ficha de cada equipo que vaya a copiar ahí, «Copiar en «…»»: se crea su repositorio y se muestra su kit.",
      "Imprime o guarda cada kit fuera del equipo.",
    ],
    ver: ["guarda-copias", "inmutable"],
  },
  {
    id: "guia-espejo",
    titulo: "Espejo del equipo que guarda copias (otro disco o la nube)",
    resumen: "Lo que guarda se copia a otros sitios, cuando tú elijas. Nunca borra allí, salvo que le pongas retención.",
    texto: [
      "Puede ir a otra carpeta (mejor en otro disco físico) o a una nube conectada en ese equipo (Dropbox, Google Drive, Backblaze B2, S3, SFTP, una carpeta de red por SMB o WebDAV). Lo que se sube son paquetes ya cifrados: la nube no puede leerlos ni hace falta ninguna contraseña de repositorio.",
      "Cada destino tiene su horario (el mismo editor que las copias) y, si quieres, también «después de cada copia nueva». Puede llevar todos los repositorios o solo algunos; si llega uno nuevo, la consola te pregunta si entra.",
      "Antes de copiar, cada archivo se comprueba con su huella: uno dañado en el almacén no se copia y se avisa. Cada día se comprueba además una parte de lo que ya está en el destino.",
      "Sin retención, el espejo nunca borra y crece sin fin. Con retención, lo que el almacén ya no tiene se borra del espejo pasados unos días; si de golpe falta mucho, no borra nada y te avisa. Conviene que al menos un destino no borre nunca.",
      "Dropbox y Google Drive no son inmutables: quien tenga la cuenta, o el permiso que se dio al equipo, puede borrar lo subido. El historial de versiones de la nube ayuda a recuperarlo durante un tiempo, pero no es lo mismo que un destino con bloqueo de objetos (Backblaze B2 u otro S3 con Object Lock), que es lo recomendable cuando se pueda.",
      "Dropbox se conecta desde esta consola, con «Conectar Dropbox» en la ficha del equipo: das permiso en Dropbox (solo a la carpeta Aplicaciones/Resguardo) y el permiso viaja cifrado solo para ese equipo, que lo guarda protegido. El servidor no lo ve.",
    ],
    pasos: [
      "Para la nube: en la ficha del equipo, «Conectar Dropbox», da permiso en Dropbox y pega aquí el código que te muestra (con la clave de administración).",
      "En su ficha, «Añadir destino del espejo»: otra carpeta (eliges la carpeta en el propio equipo) o la nube conectada, con su carpeta y, si quieres, un límite de subida.",
      "Cada destino muestra su última subida. Quitar uno espera unas horas y se puede cancelar.",
    ],
    ver: ["espejo", "nube"],
  },
  {
    id: "guia-restaurar-espejo",
    titulo: "Si se pierde el equipo que guarda copias: restaurar desde el espejo",
    resumen: "Cada repositorio está entero en el espejo y se abre con la contraseña de su kit.",
    texto: [
      "El espejo copia los repositorios tal cual, cada uno en su carpeta (<destino>/<equipo>/<repositorio>), así que se abren con la misma contraseña: la del kit de recuperación de cada repositorio.",
      "Lo que se copió en el almacén después de la última copia al espejo no está en él.",
    ],
    pasos: [
      "En la ficha del equipo que guardaba copias, «Más… → Restaurar desde el espejo…».",
      "Elige el destino del espejo y el repositorio. Si es una carpeta, conecta ese disco al equipo donde vas a restaurar; si es Dropbox, Drive, SFTP, SMB o WebDAV, descarga antes esa carpeta a un disco de ese equipo.",
      "Elige el equipo donde restaurar y escribe la contraseña del kit: el equipo lo abre solo de lectura y desde ahí restauras como siempre.",
    ],
    ver: ["espejo"],
  },
  {
    id: "guia-copia-externa",
    titulo: "Copia externa de un repositorio",
    resumen: "Cada día, a la hora que elijas, el equipo copia las versiones de un repositorio a otro destino.",
    texto: [
      "La hace el equipo dueño del repositorio. Sirve para tener otra copia fuera (la nube, otro disco u otro servidor) por si el destino principal falla, se lo roban o lo cifra un ransomware.",
      "Usa la contraseña del repositorio de origen, salvo que pongas otra (si la pones, apúntala en el kit). Si el repositorio está en un equipo que guarda copias, suele ser más sencillo el espejo de ese equipo.",
    ],
    pasos: ["En la ficha del equipo, en la tarjeta del repositorio: «Más… → Copia externa…».", "Elige un destino que ya tenga el equipo o crea uno nuevo, la hora y, si quieres, una retención propia."],
    ver: ["copia-externa", "prot-externa"],
  },
  {
    id: "guia-mover",
    titulo: "Mover un cliente a otro servidor",
    resumen: "Los equipos se van solos al nuevo con sus llaves, sus copias y su configuración. La clave de administración sigue siendo la misma.",
    texto: [
      "En el servidor nuevo, alguien superusuario «recibe» el cliente con el bloque que le da el antiguo (nombre, sal y espera: nada secreto). El nuevo devuelve otro bloque con una ficha de un solo uso, su identidad y su autoridad TLS.",
      "En el antiguo, en «Servidor → Mover…», pegas ese bloque y confirmas con la clave de administración. Cada equipo se da de alta en el nuevo, comprueba su identidad, sube su configuración y solo entonces deja el antiguo. Si en 24 h no lo consigue, se queda donde estaba.",
      "El historial (actividad, informes y avisos) se lleva aparte: «Exportar el historial» lo cifra en el navegador con una llave que sale de la clave de administración, y en el nuevo «Importar el de otro servidor» lo abre con la misma clave.",
    ],
    ver: ["ficha", "paquete", "identidad"],
  },
  {
    id: "guia-respaldo",
    titulo: "Servidores de respaldo",
    resumen: "Hasta tres servidores a los que los equipos se van solos si este deja de responder unos días.",
    texto: [
      "En cada servidor de respaldo, el cliente tiene que estar ya recibido y dar una ficha larga (hasta 365 días) en «Servidor → Dar una ficha». Pega aquí su bloque en «Servidores de respaldo» y elige tras cuántos días sin respuesta se van.",
    ],
    ver: ["respaldo", "ficha"],
  },
  {
    id: "guia-restaurar-otro",
    titulo: "Restaurar en otro equipo",
    resumen: "Para recuperar los archivos de un equipo en otro: porque se perdió, o porque los necesitas allí.",
    texto: [
      "Si el equipo original sigue conectado, comparte su acceso al destino con el otro, sellado solo para él, y el otro añade el repositorio como solo lectura: puede explorarlo y restaurar, pero ninguna copia escribe en él.",
      "Si el equipo original ya no existe, escribes los datos de su kit de recuperación (destino, id del repositorio y contraseña): viajan sellados solo para el equipo que lo importa.",
    ],
    pasos: ["En «Restaurar», «Restaurar en otro equipo».", "Elige el repositorio y el equipo donde lo quieres, o los datos del kit.", "Confirma con la contraseña del repositorio y la clave de administración.", "Después, restaura en ese equipo como en cualquier otro."],
    ver: ["solo-lectura", "contrasena-repo"],
  },
  {
    id: "guia-regla-321",
    titulo: "La regla 3-2-1-1-0",
    resumen: "Cómo saber si cada copia aguanta un ransomware, un robo o un incendio, y qué le falta.",
    texto: [
      "3 copias de los datos (contando los originales), en 2 soportes distintos (otro equipo u otro disco), 1 fuera de la oficina, 1 que no se pueda borrar desde los equipos (solo añadir, bloqueo de objetos, instantáneas del anfitrión o un disco desconectado) y 0 errores al verificar y al probar la restauración.",
      "Solo cuenta lo que está al día. Es una guía: puedes guardar una copia que no la cumple, y la consola te dice qué le falta y dónde se arregla.",
      "Dónde está cada destino y si es inmutable se deduce de su tipo. Si no acierta (un servidor de fuera que en realidad está en la oficina, un almacén con instantáneas en el anfitrión), cámbialo en «Repositorios y destinos» → el destino → «Regla 3-2-1».",
    ],
    pasos: ["Mira la tira «3 · 2 · 1 · 1 · 0» en la página de cada copia.", "Pulsa lo que falta: te lleva a donde se hace (copia externa, espejo, verificación, prueba de restauración).", "Para una copia nueva, «Con la plantilla 3-2-1» en «Cambiar las copias»."],
    ver: ["regla-321", "inmutable", "instantaneas"],
  },
  {
    id: "guia-almacen-inmutable",
    titulo: "Instantáneas que el almacén no pueda borrar",
    resumen: "Para que un ransomware con control del almacén tampoco pueda borrar lo guardado.",
    texto: [
      "Lo recomendado: el almacén en un contenedor o máquina virtual Debian sobre Proxmox con ZFS, con instantáneas del anfitrión cada hora o cada día (sanoid o zfs-auto-snapshot). El almacén no las ve ni las puede borrar. El anfitrión, fuera de la red de la oficina y con otras credenciales; ZFS en espejo y un scrub periódico.",
      "Otras formas: un Linux endurecido (sin acceso remoto) o discos USB que se rotan y se guardan desconectados. En un almacén con Windows, las instantáneas de Windows las borra cualquier administrador: déjalo fuera del dominio, con su propia cuenta de administrador, sin escritorio remoto expuesto, y con una copia fuera de su alcance.",
      "El almacén no puede comprobar lo que hace su anfitrión: márcalo tú en el destino («Con instantáneas inmutables fuera de su alcance» o «Desconectado»). La regla 3-2-1 lo cuenta como inmutable, pero recuerda que es local: no protege de un incendio o un robo de la oficina.",
    ],
    ver: ["instantaneas", "regla-321"],
  },
];

GUIAS.push({
  id: "guia-ganchos",
  titulo: "Antes de copiar: volcar SQL Server o vigilar las copias de una aplicación",
  resumen: "Dos pasos cerrados que el agente hace antes de cada copia.",
  texto: [
    "«Volcar bases de datos de SQL Server antes de copiar»: el agente hace una copia COPY_ONLY de cada base en una carpeta del equipo, la incluye en la copia y borra el volcado al terminar. COPY_ONLY no estorba a las copias propias de SQL Server. Si un volcado falla, la copia sale como fallida (el resto de carpetas se copia igual).",
    "«Avisar si la carpeta de copias de una aplicación lleva más de N horas sin archivos nuevos»: para programas que hacen sus propias copias (World Office, por ejemplo). Si el archivo más nuevo es más viejo que lo indicado, la copia sale con avisos.",
    "La cuenta del equipo (NT AUTHORITY\\SYSTEM) necesita el rol db_backupoperator en cada base que se vuelca (o sysadmin). Se da en SQL Server Management Studio, en Seguridad → Inicios de sesión.",
    "Hace falta el agente 0.7.2 o posterior: con uno anterior, la consola no lo ofrece.",
  ],
  pasos: ["En la ficha del equipo, «Cambiar las copias».", "En la copia, «Antes de copiar»: elige el paso y rellena sus datos.", "«Enviar al equipo» con la clave de administración."],
  ver: ["ganchos"],
});

export const PREGUNTAS: Pregunta[] = [
  {
    id: "si-copia-falla",
    si: "Una copia falla o sale con avisos",
    respuesta: [
      "Abre la copia (Equipos → el equipo → la copia): arriba dice qué pasó en palabras sencillas y, más abajo, el historial de cada copia. Lo ya copiado sigue a salvo y la próxima copia lo vuelve a intentar sola.",
      "«No se pudo llegar al destino»: comprueba que el equipo, el NAS o el que guarda copias está encendido y con red. «Algunos archivos no se pudieron leer»: estaban abiertos por otro programa; entran en la próxima copia. «Repositorio bloqueado»: en la ficha del equipo, «Detalles → Quitar bloqueos antiguos». «Sin sitio»: libera espacio en el destino o guarda menos versiones.",
      "Cuando lo hayas arreglado, «Copiar ahora» en la propia copia confirma que vuelve a ir bien. Si sigue fallando, el registro del equipo tiene el detalle.",
    ],
    comando: "resguardo-agente estado",
  },
  {
    id: "si-volcado",
    si: "El volcado de SQL Server falla",
    respuesta: [
      "Si dice que no hay permiso o que falló el inicio de sesión: la cuenta del equipo (NT AUTHORITY\\SYSTEM) necesita el rol db_backupoperator en esa base (o sysadmin).",
      "Si dice «error 5» o «acceso denegado» al escribir: el servicio de SQL Server tiene que poder escribir en la carpeta de los volcados. Si la crea el agente, ya le da permiso; si la creaste tú, dáselo a «NT SERVICE\\MSSQLSERVER».",
      "Comprueba también el nombre de la base y de la instancia (en el equipo, «.» es la predeterminada).",
    ],
  },
  {
    id: "si-clave-admin",
    si: "Perdí la clave de administración",
    respuesta: [
      "Búscala donde la guardaste al emparejar el primer equipo del cliente (en papel o en el gestor de contraseñas). No está en los kits de recuperación: esos son de cada repositorio y llevan su contraseña, no la clave de administración. Las copias siguen haciéndose: sin la clave solo no se pueden cambiar.",
      "Si no aparece: en cada equipo, como administrador, desvincúlalo del servidor (sigue copiando en local) y vuelve a emparejarlo desde «Añadir equipo» con una clave nueva. Lo ya copiado no se pierde.",
    ],
    comando: "resguardo-agente desvincular",
  },
  {
    id: "si-contrasena-repo",
    si: "Perdí la contraseña de un repositorio",
    respuesta: ["Está en su kit de recuperación. Si no lo tienes, el propio equipo la muestra en local, como administrador (cambia «documentos» por el id del repositorio, que ves en «Repositorios y destinos»)."],
    comando: "resguardo-agente kit documentos",
  },
  {
    id: "si-llaves",
    si: "Sale «Las llaves de un equipo han cambiado»",
    respuesta: [
      "Este navegador recuerda las llaves de cada equipo desde que las comprobó con la clave de administración, y el servidor ahora da otras. No escribas contraseñas para ese equipo todavía.",
      "Si sabes que el equipo se reinstaló o se volvió a emparejar, es normal: en su ficha, «Detalles → Volver a comprobar», con la clave de administración. Si no lo sabes, avisa a quien administra el servidor y revisa la Actividad: podría ser un servidor manipulado.",
    ],
  },
  {
    id: "si-espejo",
    si: "El espejo falló",
    respuesta: [
      "Cada destino del espejo muestra su último resultado. Si es una carpeta: comprueba que el disco está conectado y tiene sitio. Si es la nube: que el equipo tiene internet y que la cuenta tiene espacio.",
      "Un fallo no borra nada: la próxima noche vuelve a intentarlo y sube lo que faltaba.",
    ],
  },
  {
    id: "si-token",
    si: "Caducó o se revocó el permiso de Dropbox o Google Drive",
    respuesta: [
      "El espejo a esa nube fallará con un error de permiso. Vuelve a conectarla con el mismo nombre: en la ficha del equipo, «Conectar Dropbox». El espejo la sigue usando sin cambiar nada más.",
      "Sin consola (o para Google Drive), en ese equipo como administrador también vale el comando de abajo.",
    ],
    comando: 'resguardo-agente nube conectar dropbox --nombre "Dropbox Oficina"',
  },
  {
    id: "si-equipo-perdido",
    si: "Se perdió o robaron el equipo que guarda copias",
    respuesta: [
      "Si tenía una nube conectada para el espejo, revoca su permiso en la web de la nube (en Dropbox, «Aplicaciones conectadas»): ese permiso podría borrar lo subido.",
      "Las copias que tenía siguen en su espejo. Para recuperarlas en otro equipo, «Restaurar → Restaurar en otro equipo → El equipo original ya no existe», con los kits.",
    ],
  },
  {
    id: "si-sin-contacto",
    si: "Un equipo lleva días sin contacto",
    respuesta: ["Comprueba que está encendido y con red, y que el servicio «Resguardo Agente» está en marcha. En el equipo, el estado y el registro dicen qué pasa."],
    comando: "resguardo-agente estado",
  },
  {
    id: "si-orden",
    si: "Hay una orden destructiva que no esperaba",
    respuesta: ["Cancélala en «Órdenes» antes de que se aplique: para eso esperan. Después cambia la clave de administración y revisa quién la pidió en la Actividad."],
  },
  {
    id: "si-movil",
    si: "Perdí el móvil del segundo paso",
    respuesta: ["Entra con uno de tus códigos de recuperación y, en «Mi cuenta», vuelve a configurar el segundo paso y genera códigos nuevos. Si tampoco los tienes, pide al propietario que la restablezca (Personas → menú de la persona → «Restablecer verificación en dos pasos»): te dará un código de un solo uso, válido 24 horas, para vincular el móvil nuevo al entrar. La de un propietario solo la puede restablecer el propietario del servidor."],
  },
];
