// Explicaciones cortas para los «?» (InfoTip) y la página de ayuda. Mismo tono
// que la app: de tú, tranquilo y diciendo qué hacer.
import type { EntradaGlosario } from "$ui/componentes/InfoTip.svelte";

export const GLOSARIO: Record<string, EntradaGlosario> = {
  "clave-admin": {
    title: "Clave de administración",
    text: "La clave del cliente que autoriza los cambios de estructura: qué se copia, dónde, cuándo, dar de alta equipos o pausar. El servidor nunca la ve: la escribes aquí y viaja cifrada solo para el equipo, que la comprueba.",
    todo: "Guárdala en un gestor de contraseñas o en el kit impreso. Sin ella, los equipos siguen copiando, pero no se pueden cambiar.",
  },
  "contrasena-repo": {
    title: "Contraseña del repositorio",
    text: "La que cifra las copias de un repositorio. Hace falta para ver archivos, restaurar, descargar o borrar versiones. Va sellada solo para el equipo que la comprueba; el servidor nunca la recibe.",
    todo: "Está en el kit de recuperación de ese repositorio.",
  },
  sas: {
    title: "Número de comprobación",
    text: "Seis cifras que calculan por separado esta consola y el equipo. Si coinciden, el equipo que se unió es el tuyo y nadie se ha colado en medio.",
    todo: "Míralo en la pantalla del equipo (o en el instalador) y compáralo cifra a cifra antes de confirmar.",
  },
  espera: {
    title: "Espera antes de borrar",
    text: "Las órdenes que pueden borrar o dejar sin copias (acortar la retención, quitar un repositorio, pausar…) esperan antes de aplicarse. Mientras tanto, cualquiera con acceso puede cancelarlas. La comprueba también el propio equipo.",
    todo: "Si ves una orden pendiente que no esperabas, cancélala y cambia la clave de administración.",
  },
  "retencion-almacen": {
    title: "Retención en el almacén",
    text: "Un almacén («Este equipo guarda copias») es de solo añadir: ningún equipo puede borrar allí, ni siquiera lo suyo antiguo. Por eso la retención la aplica el propio almacén, en su disco y a la hora que elijas, con una clave propia que el equipo añade al repositorio. Con esa clave el almacén también puede leer ese repositorio.",
    todo: "Se autoriza una vez (con la espera de lo que borra) y después se aplica sola. «Dejar de aplicarla» borra la clave del almacén del repositorio.",
  },
  "seq": {
    title: "Número de orden",
    text: "Cada orden a un equipo lleva un número que solo puede crecer. El equipo rechaza cualquier número repetido o antiguo: así nadie puede reenviar una orden vieja.",
  },
  "firma-equipo": {
    title: "Firmado por el equipo",
    text: "El equipo firma cada resultado con su llave. Si la firma es válida, el resultado lo escribió el equipo y el servidor no lo ha cambiado.",
  },
  etiqueta: {
    title: "Etiqueta del equipo",
    text: "Una huella de las llaves del equipo hecha con tu clave de administración. Antes de enviar una contraseña, la consola comprueba que las llaves que da el servidor son las que confirmaste al emparejar.",
    todo: "Si la comprobación falla, no envíes nada y revisa el servidor: alguien pudo cambiar las llaves.",
  },
  conectado: {
    title: "Conectado",
    text: "El equipo tiene abierto el canal con el servidor: las órdenes le llegan al momento. Si no, las recoge en su próxima consulta (cada minuto).",
  },
  "guarda-copias": {
    title: "Guarda copias",
    text: "Este equipo guarda las copias de los demás en su disco, con un servidor de solo añadir: cada equipo tiene su usuario y ninguno puede borrar lo ya copiado. No es la consola: Resguardo Server solo coordina.",
  },
  "almacen-propio": {
    title: "Copiar en este mismo almacén",
    text: "Un repositorio en el propio almacén del equipo, por localhost y con su propio usuario de solo añadir, como cualquier otro equipo: ese camino tampoco puede borrar lo ya copiado. Sirve para lo que vive en esa máquina, como las copias de la consola (C:\\ProgramData\\Resguardo Server\\respaldos): así entra en el espejo y en la nube. Por sí solo sigue en el mismo disco, y quien controle esa máquina controla también el almacén.",
    todo: "Ponle un espejo (otro disco o Dropbox) para que salga del equipo.",
  },
  "etiquetas-equipo": {
    title: "Etiquetas de equipos",
    text: "Texto libre para agrupar equipos («Contabilidad», «Servidores», «Sede norte») y filtrar Equipos, Avisos y Estado. Cada etiqueta tiene siempre el mismo color. Se guardan en el servidor sin cifrar, como el nombre del equipo, y no cambian nada en el equipo.",
    todo: "Las ponen técnicos, administradores y propietarios desde la ficha del equipo. No pongas datos privados en ellas.",
  },
  plantillas: {
    title: "Plantillas de copia",
    text: "Una copia guardada para reutilizarla en otros equipos del cliente: carpetas, exclusiones, horario, «solo si hay cambios» y «Antes de copiar». Se cifran con la clave de administración: el servidor no ve ni su nombre ni sus carpetas. Usar una solo rellena el editor; se revisa y se envía como siempre.",
    todo: "En el editor de copias, el menú de cada copia: «Guardar como plantilla…» o «Rellenar con…»; abajo, «Desde una plantilla».",
  },
  "consola-y-almacen": {
    title: "Consola y almacén: ¿qué diferencia hay?",
    text: "La consola (Resguardo Server) solo coordina: manda órdenes, recibe informes y avisa; no guarda copias. El almacén es un equipo con el agente que «guarda copias» de los demás en su disco. Pueden estar en la misma máquina (lo habitual en una oficina pequeña) o en máquinas distintas. Si la consola se apaga, los equipos siguen copiando a su hora, también en el almacén.",
    todo: "Para tener un almacén, abre el equipo que tiene el disco y pulsa «Este equipo guarda copias».",
  },
  "regla-321": {
    title: "Regla 3-2-1-1-0",
    text: "3 copias de los datos (con los originales), en 2 soportes distintos, 1 fuera de la oficina, 1 que no se pueda borrar desde los equipos y 0 errores al verificar y probar la restauración. Solo cuenta lo que está al día.",
    todo: "Es una guía, no una obligación: la tira de cada copia dice qué falta y dónde se arregla.",
  },
  instantaneas: {
    title: "Instantáneas fuera de su alcance",
    text: "Instantáneas que hace el anfitrión del almacén (por ejemplo, ZFS en Proxmox) y que el almacén no ve ni puede borrar. El almacén no puede comprobarlas: lo marcas tú en el destino.",
    todo: "Son locales: no protegen de un incendio o un robo de la oficina. Mantén también una copia fuera.",
  },
  inmutable: {
    title: "Inmutable",
    text: "Destino en la nube con bloqueo de objetos (Object Lock): durante el plazo fijado, nadie puede borrar ni cambiar las copias, ni siquiera con la clave de la nube.",
  },
  retencion: {
    title: "Retención",
    text: "Cuántas versiones se guardan: por ejemplo, las de los últimos 7 días, 4 semanas y 12 meses. Acortarla borra versiones antiguas, por eso espera y pide la contraseña del repositorio.",
  },
  relevo: {
    title: "Descarga por el servidor",
    text: "El equipo prepara los archivos, los cifra para este navegador y los sube por trozos al servidor, que no puede leerlos. Al terminar se borran del servidor.",
    todo: "Para más de 500 MB, mejor restaurar en el propio equipo.",
  },
  auditoria: {
    title: "Registro de actividad",
    text: "Todo lo que se hace en el cliente, en orden y encadenado: cada entrada lleva la huella de la anterior. Si alguien borra o cambia una, la cadena se rompe y «Verificar la cadena» lo detecta.",
  },
  ancla: {
    title: "Ancla de la actividad",
    text: "La huella de la última entrada en un momento dado. El servidor la manda en el resumen por correo y a los equipos del cliente, que avisan si una consola la rehace. Si alguien rehiciera toda la cadena, «Verificar la cadena» no lo vería, pero un ancla de antes ya no cuadraría.",
    todo: "Guarda los correos del resumen y, de vez en cuando, usa «Comprobar con un ancla».",
  },
  rol: {
    title: "Papeles",
    text: "Propietario: todo, también las personas y los ajustes. Administrador: todo menos personas y ajustes. Técnico: ver y mandar órdenes, salvo dar de baja, desvincular, cambiar de servidor o la clave. Lectura: solo ver.",
  },
  "huella-ca": {
    title: "Huella del certificado",
    text: "El servidor usa su propio certificado. Compara esta huella con la que muestra el servidor al instalarlo para saber que hablas con el tuyo.",
  },
  identidad: {
    title: "Identidad del servidor",
    text: "Una llave propia del servidor, distinta del certificado. Los equipos la fijan al emparejarse y comprueban en cada conexión que hablan con este servidor.",
  },
  totp: {
    title: "Verificación en dos pasos",
    text: "Además de la contraseña, un código de 6 cifras de una aplicación (Google Authenticator, Microsoft Authenticator, 1Password, Aegis…). Es obligatoria.",
    todo: "Guarda los códigos de recuperación: sirven si pierdes el móvil.",
  },
  ficha: {
    title: "Ficha de un servidor",
    text: "Un permiso de un solo uso (o de unos pocos) para que los equipos de un cliente se den de alta en ese servidor. Va dentro de la orden sellada: este servidor no la ve.",
    todo: "Pídela en el otro servidor con «Recibir un cliente» o «Dar una ficha». Solo se muestra una vez.",
  },
  respaldo: {
    title: "Servidores de respaldo",
    text: "Hasta tres servidores a los que los equipos se van solos si este deja de responder unos días. Cada uno necesita el cliente ya recibido allí y una ficha suya de larga duración.",
  },
  "copia-consola": {
    title: "Copia de la consola",
    text: "Cada noche, este servidor guarda una copia cifrada de sí mismo (cuentas, clientes, equipos, historial, su identidad y su certificado) con la clave de respaldo de la consola. Con ella se restaura en otra máquina tal cual, y los equipos vuelven solos sin vincularlos otra vez.",
    todo: "Pon la clave de respaldo, imprime su kit y añade la carpeta de las copias a una copia del agente de esta máquina, para que también salgan de aquí.",
  },
  notificaciones: {
    title: "Notificaciones",
    text: "El servidor avisa por correo, webhook, ntfy o Telegram de lo que falla (copias, verificaciones, espejo, equipos que no conectan, intentos con la clave…) y de cuando vuelve a funcionar, y manda un resumen diario o semanal. Lo repetido se agrupa y hay un tope por hora. Solo cuenta el estado: nunca contraseñas, claves ni nombres de archivos.",
    todo: "El propietario del servidor pone el correo (Servidor → Notificaciones); cada persona elige qué le llega (Personas, o Ajustes → Mis notificaciones).",
  },
  paquete: {
    title: "Paquete de exportación",
    text: "El historial del cliente (equipos, configuraciones cifradas, informes, avisos y la actividad con su cadena de huellas) cifrado en este navegador con una llave que sale de la clave de administración. Ningún servidor puede leerlo.",
    todo: "Impórtalo en el servidor nuevo, con la misma clave de administración.",
  },
  "solo-lectura": {
    title: "Repositorio de solo lectura",
    text: "Un repositorio de otro equipo que se añadió aquí para explorarlo y restaurar. Ninguna copia de este equipo escribe en él.",
  },
  "copia-externa": {
    title: "Copia externa",
    text: "Cada día, a la hora fijada, el equipo copia las versiones del repositorio a otro destino (por ejemplo, un segundo disco o la nube). Si el destino principal falla, queda esta.",
    todo: "Usa la misma contraseña que el repositorio salvo que pongas otra; en ese caso, apúntala en el kit. También puede ir a un repositorio que ya existe (p. ej. el de la nube de la app de escritorio): solo sube lo que le falte. Si el destino tiene bloqueo de objetos, márcalo: allí no se libera espacio.",
  },
  "copia-derivada": {
    title: "Copia derivada",
    text: "Otra copia de un repositorio en otro destino, además de la copia externa: el equipo crea allí un repositorio con el mismo troceado (deduplica con el original) y le sube las versiones, todas o las que pasen un filtro (etiquetas, equipo, carpetas, fechas). Puede ir después de cada copia o a una hora, con su propia contraseña, su retención y su verificación.",
    todo: "Si le pones otra contraseña, guárdala en el kit. Para Dropbox o Google Drive, conecta antes la nube en este mismo equipo.",
  },
  cadena: {
    title: "Copias en cadena",
    text: "Una copia puede empezar «después de la anterior»: cuando aquella termina bien, empieza esta. Si la anterior falla, la cadena se para y avisa («Cadena parada»); al arreglarse, sigue sola. Los espejos y las copias derivadas de cada repositorio se ven en la misma línea.",
    todo: "Recomendado: al menos un destino fuera del alcance de la retención del original (un espejo sin retención, una copia derivada con la suya o una nube con bloqueo de objetos).",
  },
  espejo: {
    title: "Espejo en otro disco",
    text: "El equipo que guarda copias copia lo que recibe a otros destinos: otra carpeta (mejor en otro disco) o una nube conectada en ese equipo (Dropbox, Google Drive, B2, S3, SFTP, un NAS o WebDAV), con el horario de cada destino. Comprueba cada archivo antes de copiarlo y, si no le pones retención, nunca borra allí. Si falla el disco principal, las copias siguen en el espejo y se abren con la contraseña del kit.",
  },
  repositorio: {
    title: "Repositorio",
    text: "El lugar cifrado donde se guardan las versiones de unas copias, en un destino: un disco, un almacén o la nube. Se abre con su contraseña, que está en su kit de recuperación.",
  },
  destino: {
    title: "Destino",
    text: "Dónde se guardan los repositorios: un almacén (un equipo de la oficina que guarda las copias de los demás), un disco o carpeta del propio equipo, o la nube (Backblaze B2, S3, SFTP).",
    todo: "Lo ideal: una copia cerca, en un almacén, para restaurar rápido, y otra fuera, en la nube e inmutable.",
  },
  zona: {
    title: "Zona del almacén",
    text: "Otra carpeta (normalmente otro disco) que sirve el mismo almacén, con su propio puerto: «Almacén · Disco D», «Almacén · Disco E». Cada zona es un destino: los equipos que copian en ella tienen allí su propio usuario y tampoco pueden borrar lo ya copiado. Dos discos del mismo almacén protegen de que falle uno, pero no de un robo o un incendio del equipo.",
    todo: "Añádela en la ficha del almacén («Añadir una zona…») con la clave de administración. Abre su puerto en el cortafuegos del router solo si hace falta copiar desde otra sede.",
  },
  "catalogo-destinos": {
    title: "Destinos sin repositorio",
    text: "Un destino se puede crear antes que sus repositorios y ponerle el nombre que quieras. El servidor solo guarda el nombre, el tipo y el servidor o bucket: las credenciales van selladas para cada equipo cuando crea un repositorio allí.",
  },
  "dias-actividad": {
    title: "Cuadros de actividad",
    text: "Un cuadro por día, el más reciente a la derecha. Verde: se guardó una versión; verde claro: se hizo la copia sin cambios; ámbar: con avisos; rojo: falló; gris: no hubo copia.",
  },
  "salud-proteccion": {
    title: "Salud de la protección",
    text: "Siete comprobaciones, las mismas que la app de escritorio: copias automáticas, protección contra borrado, copia externa, verificación, prueba de restauración, kit de recuperación y retención.",
  },
  "prot-copias": {
    title: "Copias automáticas",
    text: "Que las copias se hagan solas a su hora, aunque nadie se acuerde.",
    todo: "Revisa el horario en «Cambiar las copias» del equipo.",
  },
  "prot-borrado": {
    title: "Protegida contra borrado",
    text: "Que nadie, ni un ransomware con acceso al equipo, pueda borrar las versiones: un servidor de solo añadir (como «Este equipo guarda copias») o un bucket con bloqueo de objetos. Un disco normal se puede borrar.",
  },
  "prot-externa": {
    title: "Copia externa",
    text: "Otra copia fuera de este sitio (la nube u otro disco), por si hay un robo, un incendio o un ransomware.",
    todo: "Configúrala en «Más… → Copia externa» del repositorio, o con el espejo del equipo que guarda copias.",
  },
  "prot-verificacion": {
    title: "Verificación",
    text: "Comprueba con regularidad que lo guardado no se ha dañado en el disco o en el servidor.",
  },
  "prot-restauracion": {
    title: "Prueba de restauración",
    text: "Restaura unos archivos al azar y comprueba que salen enteros: así sabes que las copias se pueden recuperar, no solo que existen.",
  },
  "prot-kit": {
    title: "Kit de recuperación",
    text: "Una hoja con lo necesario para abrir las copias desde otro equipo si se pierde este: la ubicación y la contraseña.",
  },
  "prot-retencion": {
    title: "Retención",
    text: "Cuántas versiones antiguas se conservan. Sin ella, el repositorio crece sin fin.",
  },
  copia: {
    title: "Copia",
    text: "Lo que guarda un equipo (qué carpetas, en qué repositorio y con qué horario) y también cada vez que lo hace: «la última copia», «copias correctas», «copias fallidas». Una copia sin cambios no guarda versión nueva. Las verificaciones del repositorio se cuentan como comprobaciones y lo que manda el espejo, como subidas.",
  },
  "en-vivo": {
    title: "Al día, sin recargar",
    text: "La consola se entera al momento de lo que pasa en los equipos (una copia que empieza o termina, un informe, un aviso, una orden) y lo enseña sin recargar la página. Si esa conexión no pasa (un proxy, la red), pregunta cada pocos segundos.",
    todo: "Si una cifra no cambia, mira si hay red con el servidor: la consola vuelve a conectar sola.",
  },
  kit: {
    title: "Kit de recuperación",
    text: "Una hoja por repositorio con su destino, su id y su contraseña. Con ella se pueden abrir las copias en cualquier equipo, incluso sin Resguardo: el formato es abierto.",
    todo: "Imprímelo o guárdalo en PDF fuera del equipo, en un sitio seguro.",
  },
  nube: {
    title: "Nube conectada",
    text: "Una cuenta de Dropbox o Google Drive conectada en el equipo que guarda copias, para el espejo. El permiso viaja cifrado solo para ese equipo y se guarda protegido allí; el servidor no lo ve.",
    todo: "Dropbox se conecta desde esta consola: en la ficha del equipo, «Conectar Dropbox». (Sin consola, o Google Drive: en el equipo, como administrador, resguardo-agente nube conectar.)",
  },
  ganchos: {
    title: "Antes de copiar",
    text: "Dos pasos cerrados que el agente puede hacer antes de cada copia (nunca órdenes libres): volcar bases de datos de SQL Server con COPY_ONLY (el volcado entra en la copia y luego se borra), y avisar si la carpeta de copias propias de una aplicación lleva demasiadas horas sin archivos nuevos. Piden el agente 0.7.2 o posterior.",
    todo: "Para el volcado, la cuenta del equipo (NT AUTHORITY\\SYSTEM) necesita el rol db_backupoperator en cada base, o sysadmin. En SQL Server Management Studio: Seguridad → Inicios de sesión → NT AUTHORITY\\SYSTEM → Asignación de usuarios → marca cada base y el rol db_backupoperator.",
  },
};
