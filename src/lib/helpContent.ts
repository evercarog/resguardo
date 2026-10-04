// Contenido del centro de ayuda (estático, sin red). El HTML lo escribimos
// nosotros: solo <p>, <ul>, <ol>, <strong>, <em>, <code>, <kbd> y enlaces
// internos <a href="#" data-topic="id"> a otros apartados.
import { GLOSSARY, GLOSSARY_GROUPS, glossaryHtml } from "$lib/glossary";

export interface HelpItem {
  /** Único en toda la ayuda: se usa para enlazar directamente («?» junto a un campo). */
  id: string;
  title: string;
  html: string;
}

export interface HelpSection {
  id: string;
  title: string;
  /** Nombre del icono (ver ICONS en HelpCenter.svelte). */
  icon: string;
  items: HelpItem[];
}

export const HELP: HelpSection[] = [
  {
    id: "primeros-pasos",
    title: "Primeros pasos",
    icon: "rocket",
    items: [
      {
        id: "como-funciona",
        title: "Cómo se organiza Resguardo",
        html: `
          <p>Resguardo tiene dos piezas:</p>
          <ul>
            <li><strong>Destinos y repositorios</strong>: dónde se guardan las copias. El destino es el lugar (un disco externo, un servidor, la nube…) y dentro tiene repositorios: cajas cifradas, cada una con su propia contraseña (ver <a href="#" data-topic="destinos-repositorios">destinos y repositorios</a>).</li>
            <li><strong>Copias</strong>: qué carpetas se protegen y cuándo. Cada copia se guarda en un repositorio.</li>
          </ul>
          <p>Cada vez que se hace una copia se guarda una <strong>versión</strong>: una foto de tus carpetas en ese momento. Solo se sube lo que cambió, así que las versiones ocupan poco.</p>`,
      },
      {
        id: "anadir-destino",
        title: "1. Añade un repositorio",
        html: `
          <ol>
            <li>Pulsa <strong>+</strong> junto a «Destinos» en la barra lateral (o <kbd>Ctrl</kbd>+<kbd>Mayús</kbd>+<kbd>N</kbd>).</li>
            <li>Elige el tipo: carpeta o disco, servidor REST, nube o SFTP (ver <a href="#" data-topic="destinos-tipos">tipos de destino</a>).</li>
            <li>Elige si quieres <strong>crear</strong> un repositorio nuevo o <strong>conectar</strong> uno que ya existe (por ejemplo, en otro equipo).</li>
            <li>Escribe la contraseña del repositorio y <strong>guárdala en un gestor de contraseñas</strong>. Sin ella no se pueden recuperar los datos.</li>
          </ol>`,
      },
      {
        id: "crear-copia",
        title: "2. Crea una copia",
        html: `
          <ol>
            <li>Pulsa <strong>+</strong> junto a «Copias» (o <kbd>Ctrl</kbd>+<kbd>N</kbd>).</li>
            <li>Elige el repositorio, ponle un nombre y añade las carpetas que quieres proteger.</li>
            <li>Si quieres, añade <a href="#" data-topic="copias-exclusiones">exclusiones</a> para no copiar archivos que no hacen falta.</li>
            <li>Pulsa <strong>Copiar ahora</strong> para hacer la primera versión. La primera tarda más: las siguientes solo suben lo que cambió.</li>
          </ol>`,
      },
      {
        id: "programar",
        title: "3. Prográmala",
        html: `
          <p>Para que la copia se haga sola, edítala y activa <strong>Hacerla sola, con horario</strong>: elige los días y las horas (ver <a href="#" data-topic="copias-horarios">horarios</a>).</p>
          <p>Después pulsa <strong>Programar</strong>. Hace falta abrir Resguardo como administrador una vez, porque se crea una tarea del sistema que hace las copias <strong>aunque la app esté cerrada</strong> (ver <a href="#" data-topic="auto-agente">el agente</a>).</p>`,
      },
      {
        id: "opciones-avanzadas",
        title: "Opciones avanzadas",
        html: `
          <p>En los editores, lo que casi nadie necesita cambiar está plegado bajo <strong>Opciones avanzadas</strong>, con los valores recomendados ya puestos. Plegado, te dice qué hay dentro y cómo está.</p>
          <ul>
            <li>Copias: las etiquetas y «Solo guardar una versión si hay cambios».</li>
            <li>Retención: a qué versiones se aplica y cómo se agrupan.</li>
            <li>Verificación y prueba de restauración: qué datos se leen y cuántos archivos se prueban.</li>
            <li>Copia externa: la velocidad máxima de subida, cuándo se frena una subida y cómo se verifica la copia en la nube.</li>
            <li>Repositorios REST y Resguardo Web: el certificado propio y el servidor; Ajustes: el límite de subida del modo discreto; copias automáticas: «Solo vigilar».</li>
          </ul>
          <p>Si algo no está en su valor recomendado, verás «personalizadas» y se abren solas. Resguardo recuerda, para tu usuario, cuáles dejas abiertas.</p>`,
      },
    ],
  },
  {
    id: "destinos",
    title: "Destinos y repositorios",
    icon: "harddrive",
    items: [
      {
        id: "destinos-repositorios",
        title: "Destinos y repositorios",
        html: `
          <p>Resguardo usa cuatro palabras:</p>
          <ul>
            <li><strong>Destino</strong>: el lugar donde se guardan las copias. Un disco o una carpeta de la red, un servidor o un <em>bucket</em> de la nube con su clave.</li>
            <li><strong>Repositorio</strong>: una caja cifrada dentro de un destino, con su propia contraseña. Un destino puede tener varios: uno por equipo, por cliente o por tipo de datos.</li>
            <li><strong>Copia</strong>: qué carpetas se copian y cuándo. Se guarda en un repositorio.</li>
            <li><strong>Versión</strong>: cada foto que guarda una copia.</li>
          </ul>
          <pre>Backblaze B2 · copias-ana      ← destino
├── portatil/      (contraseña propia)   ← repositorio
├── disco-externo/ (contraseña propia)
└── servidor/      (contraseña propia)</pre>
          <p>En la barra lateral, cada destino se despliega con sus repositorios. Resguardo agrupa solo los que ya tenías, por bucket, servidor o carpeta, sin que tengas que hacer nada. Puedes cambiar el nombre de un destino desde la página de cualquiera de sus repositorios, con el lápiz junto a su nombre; no hace falta contraseña, porque no toca lo guardado.</p>`,
      },
      {
        id: "destino-dentro",
        title: "Dentro de un destino",
        html: `
          <p>Pulsa el nombre de un destino en la barra lateral para ver sus repositorios:</p>
          <ul>
            <li><strong>De este equipo</strong>: los que ya usas, con su estado. Desde aquí puedes <strong>Clonar…</strong> uno a otro destino: se crea un repositorio nuevo con todas sus versiones y su propia contraseña (con los mismos parámetros, así la deduplicación sigue funcionando).</li>
            <li><strong>Encontrado</strong>: está en el destino pero este equipo no lo usa. Resguardo lo encuentra en carpetas locales o de red y en buckets S3 (si la clave puede listar). <strong>Usar este</strong> pide su contraseña y lo añade. Úsalo sobre todo para restaurar o verificar desde aquí: si dos equipos copian en el mismo repositorio, quien tenga la contraseña ve las versiones de los dos.</li>
            <li><strong>Recordado</strong>: un servidor REST o SFTP no deja listar, así que Resguardo recuerda los que se han usado ahí.</li>
          </ul>
          <p><strong>Crear uno nuevo aquí</strong> propone una ruta junto a los demás y una contraseña fuerte que solo tendrá este equipo; al terminar se abre el kit de recuperación para que la guardes.</p>
          <p>Clonar entre dos nubes con claves distintas no es posible directamente (restic usa una sola clave para las dos): clona primero a un disco o a un servidor.</p>`,
      },
      {
        id: "compartir-destino",
        title: "Compartir un destino con tus equipos",
        html: `
          <p>Si varios de tus equipos guardan copias en la misma cuenta de la nube (o en el mismo servidor REST), no hace falta escribir las claves en cada uno: compártela desde el equipo que ya la tiene.</p>
          <ol>
            <li>En ese equipo, abre el destino y activa <strong>Compartir con mis equipos</strong>. Hace falta abrir Resguardo como administrador, tener el equipo vinculado con Resguardo Web y la contraseña de uno de sus repositorios.</li>
            <li>En el otro equipo, con la sesión de «Todos mis equipos» iniciada: <strong>+</strong> junto a «Destinos» → <strong>Un destino compartido por otro equipo tuyo</strong> → <strong>Pedir</strong>. Recibirás un aviso y tendrás 5 minutos para cancelarlo; después, el primer equipo lo entrega en unos minutos.</li>
            <li>Al llegar, pulsa <strong>Crear repositorio aquí</strong>: tendrá su propia contraseña, que solo conoce ese equipo.</li>
          </ol>
          <pre>Backblaze B2 · copias-ana  (una clave)
├── siigo/      (contraseña propia de «Siigo»)
├── altamar/    (contraseña propia de «Altamar»)
└── portatil/   (contraseña propia de «Portátil»)</pre>
          <ul>
            <li><strong>Lo que viaja</strong> es la cuenta (la clave del bucket o el usuario del servidor), cifrada solo para el equipo que la pide: la web no puede leerla. Las contraseñas de los repositorios no se comparten nunca.</li>
            <li>Cada entrega avisa en los dos equipos y queda en la Actividad.</li>
            <li><strong>Dejar de compartir</strong> impide nuevas entregas, pero los equipos que ya la recibieron la conservan: para revocarla del todo, crea una clave nueva en el proveedor (o cambia la contraseña del usuario del servidor) y borra la antigua.</li>
            <li>Recomendado: <strong>un bucket y una clave por cliente</strong>. Una clave compartida puede, técnicamente, borrar lo de otros; el bloqueo de objetos (Object Lock) o un servidor de solo añadir lo evitan.</li>
          </ul>`,
      },
      {
        id: "servidor-copias",
        title: "Servidor de copias de Resguardo",
        html: `
          <p>Cualquier equipo con Windows puede guardar las copias de tus otros equipos, por ejemplo el ordenador de la oficina para los portátiles. Actívalo en <strong>Ajustes → Este equipo → Servidor de copias</strong> (como administrador): elige la carpeta (mejor en un disco con espacio) y el puerto.</p>
          <ol>
            <li><strong>Añadir equipo</strong>: uno por cada equipo que vaya a copiar aquí. Cada uno tiene su usuario y una contraseña aleatoria.</li>
            <li>En ese equipo: <strong>+</strong> junto a «Destinos» → «Un destino compartido por otro equipo tuyo» → <strong>Pedir</strong>. Le llega cifrado y crea allí su repositorio, con su propia contraseña. Si el servidor no está vinculado con Resguardo Web, se muestran los datos una vez para configurarlo a mano.</li>
          </ol>
          <ul>
            <li><strong>Seguro por diseño</strong>: el servidor solo deja <em>añadir</em> (nadie puede borrar copias desde los equipos), cada equipo solo ve su carpeta, todo va cifrado con TLS y un certificado propio que los equipos comprueban, y el firewall solo abre ese puerto (por defecto, solo para tu red local).</li>
            <li>El administrador del servidor tiene los datos, pero cifrados: sin la contraseña de cada repositorio no puede abrirlos.</li>
            <li><strong>Desde otra sede</strong>: abre en el router ese puerto hacia el servidor y desactiva «solo red local». Resguardo nunca abre puertos solo. Restringe en el router quién puede llegar si puedes.</li>
            <li>Si desactivas el servidor, las copias se quedan en la carpeta.</li>
          </ul>`,
      },
      {
        id: "equipos-gestionados",
        title: "Equipos gestionados (Resguardo Agente)",
        html: `
          <p>Para los equipos que administras tú (los de un cliente, una oficina…): instalas en ellos <strong>Resguardo Agente</strong>, sin ventana, solo un servicio y un icono discreto en la bandeja, y desde la consola (este equipo, con el Servidor de copias activado) decides qué copian y cuándo.</p>
          <ol>
            <li>En <strong>Equipos gestionados → Preparar un equipo</strong>, pulsa <strong>Guardar el instalador del agente…</strong> y llévalo a ese equipo (en una memoria USB o una carpeta compartida).</li>
            <li>Pulsa <strong>Emparejar un equipo</strong>: aparece un código de un solo uso. Abre el instalador en ese equipo (pide permiso de administrador) y escribe el código.</li>
            <li>El instalador y la consola muestran un <strong>número de comprobación</strong> de 6 cifras. Si es el mismo, pulsa <strong>Coincide: confirmar</strong>; si no, <strong>No coincide</strong>. Tienes 15 minutos.</li>
            <li>La consola le crea su usuario y su repositorio en este servidor. Añádele una copia: carpetas de ese equipo, exclusiones y horario.</li>
          </ol>
          <ul>
            <li><strong>Seguro por diseño</strong>: cada orden va firmada por la consola y cifrada solo para ese equipo. El equipo solo acepta lo firmado por la consola con la que se emparejó, en orden y sin caducar: ni Resguardo Web ni nadie en medio puede falsificarla.</li>
            <li><strong>Muchos equipos</strong>: el instalador funciona sin ventanas, como administrador: <code>Resguardo-Agente-…-setup.exe /S /CODE=CÓDIGO</code> (y <code>/TRAY=0</code> para no mostrar el icono). Cada equipo necesita su propio código. Sin pantalla, comprueba en la consola que el nombre que aparece es el del equipo; el número queda en «emparejamiento.txt», en la carpeta del agente.</li>
            <li><strong>Su estado</strong>: cada equipo muestra su última versión, cuántas hay y cuánto ocupan, mirando solo su carpeta del servidor (sin abrir sus copias). «Sin copias recientes» si pasan 2 días sin versión nueva.</li>
            <li><strong>Versiones que se guardan</strong>: el equipo no puede borrar nada del servidor (solo añadir). Esta consola aplica la retención de cada equipo (por defecto «Frecuente») una vez por semana, o al pulsar «Aplicar ahora».</li>
            <li><strong>Si cambia la IP del servidor</strong>, Resguardo renueva su certificado (con la misma autoridad, así que los equipos siguen confiando en él) y manda a cada equipo su dirección nueva, firmada como siempre.</li>
            <li><strong>Restaurar</strong>: «Restaurar sus archivos…» abre sus copias aquí, como cualquier repositorio, para ver versiones y recuperar archivos.</li>
            <li><strong>Su usuario</strong> ve en la bandeja un estado tranquilo («Tus archivos están protegidos») y quién gestiona el equipo. Los avisos de cada copia vienen desactivados.</li>
            <li><strong>Dejar de gestionar</strong>: el equipo deja de copiar; lo ya copiado se queda en el servidor. Volver a emparejarlo exige un administrador en ese equipo.</li>
            <li><strong>Desinstalar</strong> el agente (Configuración de Windows → Aplicaciones, como administrador) quita el servicio; pregunta si se borran también sus datos.</li>
            <li>En los equipos gestionados no se pueden activar las copias a distancia de «Todos mis equipos»: se gestionan solo desde la consola.</li>
          </ul>`,
      },
      {
        id: "destinos-tipos",
        title: "Tipos de destino",
        html: `
          <ul>
            <li><strong>Carpeta o disco</strong>: un disco externo, una memoria USB o una carpeta compartida de la red. Lo más sencillo. Si es un disco externo, conéctalo antes de copiar o restaurar.</li>
            <li><strong>Servidor REST</strong>: un servidor <em>rest-server</em> de restic, por ejemplo en un NAS de casa. Usa <code>https://</code> siempre que puedas; si el servidor usa un certificado propio, indícalo en «Opciones avanzadas».</li>
            <li><strong>Nube (S3)</strong>: Backblaze B2, Wasabi, Cloudflare R2, Amazon S3 u otro compatible con S3. Necesitas un <em>bucket</em> y unas claves de acceso (ID y clave secreta).</li>
            <li><strong>SFTP</strong>: cualquier servidor al que entres por SSH, como <code>usuario@servidor:/ruta</code>. Hace falta entrar con clave SSH (con el agente de SSH o <code>~/.ssh/config</code>), no con contraseña.</li>
          </ul>
          <p>Lo ideal es tener al menos un repositorio fuera de casa (ver <a href="#" data-topic="regla-321">la regla 3-2-1</a>).</p>`,
      },
      {
        id: "destinos-contrasena",
        title: "La contraseña del repositorio",
        html: `
          <p>Todo lo que se guarda en un repositorio se cifra con su contraseña antes de salir de tu equipo. Ni el proveedor de la nube ni quien tenga el disco pueden ver tus archivos.</p>
          <p><strong>Si pierdes la contraseña, nadie puede recuperar los datos</strong>: ni tú, ni Resguardo, ni el proveedor. Guárdala en un gestor de contraseñas (Bitwarden, 1Password, KeePass…) o en papel en un lugar seguro.</p>
          <p>Resguardo la guarda en el almacén de credenciales de Windows para no pedírtela en cada copia, pero la pide de nuevo para las acciones delicadas (ver <a href="#" data-topic="seguridad-contrasenas">qué pide contraseña</a>).</p>
          <p><strong>Si la cambiaste en otro sitio</strong> (en otro equipo o con <code>restic key passwd</code>), Resguardo ya no puede abrir el repositorio y lo dice: pulsa <strong>Actualizar la contraseña guardada…</strong> y escribe la nueva. Se comprueba que abre el repositorio antes de guardarla; si las copias automáticas la usan, hace falta abrir Resguardo como administrador para cambiarla también para ellas.</p>`,
      },
      {
        id: "destinos-claves-nube",
        title: "Reutilizar las claves de la nube",
        html: `
          <p>Si ya tienes un repositorio en la nube, al añadir otro del mismo proveedor puedes elegir <strong>Usar las claves de…</strong> en lugar de escribirlas otra vez. Las claves se copian del almacén de credenciales; nunca se muestran.</p>
          <p>Consejo: crea en el proveedor una clave solo para Resguardo, limitada a su bucket. Si algún día la cambias, cámbiala en todos los repositorios que la usan.</p>`,
      },
    ],
  },
  {
    id: "copias",
    title: "Copias",
    icon: "foldersync",
    items: [
      {
        id: "copias-carpetas",
        title: "Carpetas",
        html: `
          <p>Añade las carpetas que quieres proteger: Documentos, Escritorio, Imágenes… Se copia todo su contenido, incluidas las subcarpetas.</p>
          <p>Puedes tener varias copias en el mismo repositorio (por ejemplo «Trabajo» cada hora y «Fotos» los domingos). Las versiones de cada copia se muestran en su historial; en el repositorio se ven todas.</p>`,
      },
      {
        id: "copias-exclusiones",
        title: "Exclusiones",
        html: `
          <p>Sirven para no copiar lo que no hace falta. Escribe un patrón por línea:</p>
          <ul>
            <li><code>*.tmp</code>: todos los archivos que acaban en .tmp.</li>
            <li><code>node_modules</code>: cualquier carpeta o archivo con ese nombre, esté donde esté.</li>
            <li><code>Caché*</code>: lo que empiece por «Caché», como las carpetas de caché de algunos programas.</li>
            <li><code>~$*</code>: los archivos temporales de Office.</li>
          </ul>
          <p><strong>No distinguen mayúsculas</strong>, igual que Windows: <code>*.TMP</code> también excluye <code>informe.tmp</code>.</p>
          <p>Lo excluido no se copia, así que tampoco se puede restaurar. Ante la duda, no lo excluyas.</p>
          <p>¿No sabes qué excluir? Mira <a href="#" data-topic="ocupa-espacio">qué ocupa espacio</a> en una versión.</p>`,
      },
      {
        id: "ocupa-espacio",
        title: "Encontrar qué ocupa espacio y excluirlo",
        html: `
          <ol>
            <li>En el historial de versiones pulsa <strong>Lo que más ocupa</strong> (la versión más reciente) o el icono de barras de una versión. También está en la cabecera al explorar una versión.</li>
            <li>Verás las <strong>carpetas</strong> y los <strong>archivos</strong> más grandes, con su tamaño y qué parte de la versión suponen. Pulsa una carpeta para abrirla.</li>
            <li>Marca lo que no necesitas copiar y pulsa <strong>Excluir de las próximas copias</strong>. Al explorar una versión también puedes marcar archivos o carpetas y usar el mismo botón.</li>
            <li>Elige en qué copias excluirlo: se añade su ruta completa (por ejemplo <code>D:\\Vídeos\\Brutos</code>) a sus exclusiones.</li>
          </ol>
          <p>El tamaño es el que tiene dentro de esa versión. Lo que ya estaba en otras versiones no vuelve a ocupar espacio en el repositorio (deduplicación), así que excluir algo que no cambia ahorra menos de lo que parece.</p>
          <p>Las <strong>próximas copias</strong> ya no lo incluirán, pero las versiones que ya existen lo siguen guardando hasta que la <a href="#" data-topic="mant-retencion">retención</a> las elimine. Para quitarlo también de ellas hay que usar <code>restic rewrite</code> y <code>restic prune</code> donde el repositorio se pueda modificar (en un servidor en modo solo añadir, en el propio servidor). Resguardo te muestra los comandos, pero es irreversible: pruébalos antes con <code>--dry-run</code>.</p>`,
      },
      {
        id: "copias-etiquetas",
        title: "Etiquetas",
        html: `
          <p>Palabras que se guardan en cada versión, como <code>diaria</code> o <code>trabajo</code>. Ayudan a distinguir versiones y a que la <a href="#" data-topic="mant-retencion">retención</a> trate cada copia por separado.</p>
          <p>Son opcionales: si no sabes qué poner, déjalas vacías.</p>`,
      },
      {
        id: "copias-horarios",
        title: "Horarios",
        html: `
          <p>Elige <strong>los días</strong> de la semana y después <strong>las horas</strong>, de dos formas:</p>
          <ul>
            <li><strong>A horas concretas</strong>: por ejemplo, a las 13:00 y a las 20:00.</li>
            <li><strong>Cada N horas entre A y B</strong>: por ejemplo, cada 2 horas desde las 08:00 hasta las 20:00 (a las 8, 10, 12… y 20).</li>
          </ul>
          <p>El editor te muestra las próximas copias para que compruebes que es lo que quieres. Después pulsa <strong>Programar</strong> (o «Aplicar cambios») para que el agente use el nuevo horario.</p>`,
      },
      {
        id: "copias-apagado",
        title: "¿Y si el equipo está apagado?",
        html: `
          <p>Si a la hora prevista el equipo estaba apagado o suspendido, la copia se hace <strong>al encenderlo</strong>. Solo una vez, aunque se hayan perdido varias horas.</p>
          <p>Si el repositorio es un disco externo que no está conectado, la copia falla y se reintenta (ver <a href="#" data-topic="copias-reintentos">reintentos</a>).</p>`,
      },
      {
        id: "copias-reintentos",
        title: "Reintentos",
        html: `
          <p>Si una copia programada falla (por ejemplo, por un corte de red), el agente la <strong>reintenta hasta 2 veces</strong>, con al menos 15 minutos entre intentos. Si sigue fallando, la copia se marca en rojo y vuelve a intentarse a la siguiente hora prevista.</p>
          <p>Si una copia termina «con avisos», se guardó todo menos algunos archivos que no se pudieron leer (normalmente, archivos abiertos o sin permiso).</p>
          <p><strong>Bloqueo antiguo</strong>: mientras copia, restic marca el repositorio como ocupado y renueva esa marca cada pocos minutos. Si una copia se corta (un apagón, un equipo apagado a mitad), la marca se queda y las siguientes fallan. Cuando lleva más de media hora sin renovarse, Resguardo lo dice y ofrece <strong>Desbloquear</strong>: quita solo las marcas antiguas, nunca la de una operación en marcha. Si hay una copia en marcha en otro equipo, espera a que termine.</p>`,
      },
      {
        id: "copias-sin-cambios",
        title: "Solo guardar si hay cambios",
        html: `
          <p>Con <strong>Solo guardar una versión si hay cambios</strong>, la copia se revisa igual a su hora, pero si ningún archivo cambió <strong>no crea una versión nueva</strong>. Así una copia cada hora no llena la lista de versiones idénticas por las noches y los fines de semana.</p>
          <ul>
            <li>Una revisión sin cambios es una copia correcta: la copia sigue <strong>al día</strong> y no se reintenta. En la Actividad aparece como «Sin cambios».</li>
            <li>La última versión sigue siendo la anterior, que contiene exactamente lo mismo.</li>
            <li>Viene activada en las copias nuevas. En las que ya tenías, actívala con <strong>Editar</strong>.</li>
          </ul>`,
      },
    ],
  },
  {
    id: "automaticas",
    title: "Copias automáticas",
    icon: "calendarclock",
    items: [
      {
        id: "auto-agente",
        title: "El agente",
        html: `
          <p>Las copias con horario las hace el <strong>agente</strong>: una tarea del sistema que funciona <strong>con la app cerrada</strong> e incluso sin haber iniciado sesión.</p>
          <ul>
            <li>Para crearlo o cambiarlo hace falta abrir Resguardo <strong>como administrador</strong>. La app te lo ofrece y Windows te lo pide; después vuelves al mismo sitio.</li>
            <li>Las contraseñas que necesita se guardan cifradas solo para el agente de este equipo.</li>
            <li>Si cambias el horario de una copia, pulsa <strong>Aplicar cambios</strong> para que el agente lo sepa.</li>
          </ul>`,
      },
      {
        id: "auto-registro",
        title: "Registro del agente",
        html: `
          <p>En la tarjeta «Copias automáticas» de un repositorio, pulsa <strong>Ver registro del agente</strong> para ver qué hizo y cuándo: copias hechas, avisos y errores. Es lo primero que conviene mirar si algo no se copió.</p>`,
      },
      {
        id: "historia-destino",
        title: "La historia de un repositorio",
        html: `
          <p>En cada repositorio, la pestaña <strong>Historia</strong> (junto a Versiones y Retención) reúne todo lo que le ha pasado, de lo más reciente a lo más antiguo:</p>
          <ul>
            <li>Copias: a mano (con el usuario de Windows que la hizo), automáticas, reintentos y pedidas a distancia.</li>
            <li>Subidas de la copia externa, verificaciones y pruebas de restauración.</li>
            <li>Pausas, reanudaciones y subidas frenadas por un cambio inusual.</li>
            <li><strong>Cambios de configuración</strong>: quién cambió qué y cuándo (copias creadas o editadas y qué parte, retención, verificación, copia externa, copias automáticas, nombre…) y los kits de recuperación guardados. Nunca se anotan contraseñas, rutas ni patrones, solo qué se cambió.</li>
          </ul>
          <p>Filtra por tipo o marca <strong>Solo problemas</strong>; «Mostrar más» carga lo anterior. Los cambios se anotan desde la versión 0.6.3. La vista <strong>Actividad</strong> muestra lo mismo para todos los repositorios juntos.</p>`,
      },
      {
        id: "auto-solo-vigilar",
        title: "«Solo vigilar»",
        html: `
          <p>Si las copias de un repositorio las hace <strong>otro programa</strong> (por ejemplo, un script de restic), elige <strong>Solo vigilar</strong>: Resguardo no copia nada, pero te avisa si pasa demasiado tiempo sin versiones nuevas.</p>`,
      },
      {
        id: "auto-pausar",
        title: "Pausar las copias automáticas",
        html: `
          <p>Si vas a hacer mantenimiento en un repositorio (por ejemplo, mover o revisar el servidor), pulsa <strong>Pausar copias automáticas</strong> en el repositorio o en cualquiera de sus copias y elige cuánto tiempo: desde 1 hora hasta 30 días, o <strong>hasta que la reanudes</strong>.</p>
          <ul>
            <li>Durante la pausa no se hacen copias programadas, reintentos, verificaciones ni copias externas de ese repositorio. «Copiar ahora» sigue funcionando.</li>
            <li>Las copias en curso terminan normalmente. No se borra nada ni cambia la programación, y la web sigue viendo el repositorio.</li>
            <li>Al terminar la pausa las copias se reanudan solas; si se saltó alguna hora, se hace <strong>una sola</strong> copia para ponerse al día. También puedes pulsar <strong>Reanudar ahora</strong>.</li>
            <li>Pausar y reanudar piden la contraseña del repositorio y abrir Resguardo como administrador, y quedan en la Actividad.</li>
          </ul>`,
      },
      {
        id: "modo-discreto",
        title: "Modo discreto: copiar sin que se note",
        html: `
          <p>En <strong>Ajustes → Este equipo</strong>, activa <strong>Mientras se trabaja, copiar con prioridad baja</strong> y elige los días y las horas (por defecto, de lunes a sábado de 07:00 a 19:00).</p>
          <ul>
            <li>En ese horario, las copias automáticas, las subidas a la nube, las verificaciones y las pruebas de restauración usan el procesador, el disco y la memoria con <strong>prioridad baja</strong>: ceden el equipo a lo que estés haciendo. Tardan algo más, pero no se notan.</li>
            <li>Opcional: <strong>limitar la subida</strong> a repositorios remotos (servidor, nube) en ese horario, para no saturar la conexión de la oficina.</li>
            <li>Fuera del horario, todo va a velocidad normal. Lo que haces a mano desde la app (<strong>Copiar ahora</strong>, restaurar) no cambia.</li>
            <li>Cuando está en marcha, el panel <strong>Estado</strong> lo indica: «Modo discreto ahora: las copias van con prioridad baja».</li>
            <li>Cambiarlo pide abrir Resguardo como administrador y la contraseña de un repositorio, como el resto de la configuración del agente.</li>
          </ul>`,
      },
      {
        id: "avisos",
        title: "Avisos (la campana)",
        html: `
          <p>La <strong>campana</strong> de abajo a la izquierda reúne lo que conviene saber aunque no estuvieras mirando; el número rojo es lo que aún no has leído.</p>
          <ul>
            <li>Lo que pasó: copias que fallaron o terminaron con avisos, copias pedidas a distancia, subidas, verificaciones o pruebas que fallaron, subidas frenadas por un cambio inusual y pausas que terminaron solas.</li>
            <li>Lo que sigue pasando: un repositorio sin conexión o atrasado, o que falta el kit de recuperación. Se quita solo cuando se arregla.</li>
          </ul>
          <p>Cada aviso tiene un botón que te lleva a arreglarlo. Puedes marcarlos como leídos o <strong>vaciar</strong> la lista. Resguardo lo recuerda para tu usuario. Son los mismos sucesos que los avisos de Windows de la <a href="#" data-topic="bandeja">bandeja</a>, y se guardan los de los últimos 30 días.</p>`,
      },
      {
        id: "bandeja",
        title: "El icono de la bandeja y los avisos",
        html: `
          <p>Resguardo tiene un icono junto al reloj de Windows: un escudo <strong>verde</strong> si todo está protegido, <strong>ámbar</strong> si algo va con retraso y <strong>rojo</strong> si algo necesita atención. Al pasar el ratón ves el resumen («Resguardo: todo protegido», «2 cosas necesitan atención») y lo que está en marcha.</p>
          <ul>
            <li>Con un clic se abre Resguardo. Con el botón derecho: <strong>Copiar ahora</strong> cualquiera de las copias de este equipo, <strong>Pausar copias automáticas 1 hora</strong> (abre la app, porque pide la contraseña del repositorio y administrador) y <strong>Salir</strong>.</li>
            <li>Al cerrar la ventana, Resguardo sigue en la bandeja. Si prefieres que se cierre del todo, desactiva <strong>Al cerrar, seguir en la bandeja</strong> en Ajustes. Si está activado el bloqueo con Windows Hello, al ocultarse se bloquea.</li>
            <li><strong>Iniciar con Windows (en la bandeja)</strong> viene activado en las instalaciones nuevas y es solo para tu usuario. Las copias automáticas no dependen de él: las hace el agente aunque Resguardo esté cerrado.</li>
            <li>Si abres Resguardo cuando ya está abierto, se trae al frente la ventana que ya había.</li>
          </ul>
          <p>Con la app en la bandeja, Windows te avisa cuando <strong>falla una copia</strong>, cuando se <strong>frena una subida a la nube</strong> por un cambio inusual, cuando esa subida <strong>termina</strong> después de reanudarla y, como mucho una vez por semana, si <strong>falta el kit de recuperación</strong>. Cada cosa se avisa una sola vez; los avisos esperan si estás en pantalla completa o en una presentación y respetan el modo «No molestar». Al pulsar uno se abre lo que corresponde. Puedes desactivarlos en <strong>Ajustes → Bandeja y avisos</strong>.</p>`,
      },
    ],
  },
  {
    id: "mantenimiento",
    title: "Mantenimiento",
    icon: "wrench",
    items: [
      {
        id: "mant-verificacion",
        title: "Verificación",
        html: `
          <p>Comprueba que el repositorio está sano y que las copias se podrán restaurar. Puedes elegir cuánto se lee cada vez: solo la estructura (rápido) o un porcentaje de los datos (más lento, pero detecta datos dañados).</p>
          <p>Una verificación semanal que lea un 5 % de los datos es un buen punto de partida: en unos meses se habrá revisado todo.</p>`,
      },
      {
        id: "mant-copia-externa",
        title: "Copia externa",
        html: `
          <p>Sube automáticamente las versiones de un repositorio a otro sitio, normalmente la nube. Así, si pierdes el disco (robo, incendio…), tienes otra copia fuera de casa (ver <a href="#" data-topic="regla-321">la regla 3-2-1</a>).</p>
          <p>Puede ir a un bucket de la nube o a otro repositorio de la app. Si aplicas la retención allí, se borran las versiones antiguas igual que en el origen; si no, se guardan todas.</p>`,
      },
      {
        id: "mant-proteger",
        title: "Proteger la copia externa contra borrados",
        html: `
          <p>Un virus o alguien con acceso a tu equipo podría intentar borrar también las copias. Para evitarlo:</p>
          <ul>
            <li>Activa el <strong>versionado</strong> del bucket (o <strong>Object Lock</strong>) en el panel del proveedor: lo que se borre o sobrescriba se puede recuperar durante un tiempo.</li>
            <li>Usa una <strong>clave sin permiso de borrado</strong> (en B2: sin <em>deleteFiles</em>; en S3: sin <em>DeleteObjectVersion</em>). Así, aunque alguien la robe, no puede eliminar lo ya subido.</li>
          </ul>
          <p>Con una clave sin borrado no se puede aplicar la retención desde este equipo: hazla desde el proveedor (reglas de ciclo de vida).</p>`,
      },
      {
        id: "mant-limite",
        title: "Límite de subida",
        html: `
          <p>En la copia externa puedes fijar una <strong>velocidad máxima de subida</strong> en Mbit/s para no saturar tu conexión mientras trabajas. Como referencia, la mitad de tu velocidad de subida es un buen punto de partida. 0 significa sin límite.</p>`,
      },
      {
        id: "mant-retencion",
        title: "Retención",
        html: `
          <p>Decide cuántas versiones se guardan: por ejemplo, las de los últimos 7 días, una por semana del último mes y una por mes del último año. Las demás se borran y el repositorio deja de crecer sin fin.</p>
          <p>Antes de aplicarla, la pestaña «Retención» del repositorio te muestra qué versiones se conservarían y cuáles se borrarían.</p>`,
      },
      {
        id: "mant-prueba-restauracion",
        title: "Prueba de restauración",
        html: `
          <p>La verificación comprueba que el repositorio está sano; la <strong>prueba de restauración</strong> demuestra que se puede <strong>recuperar</strong>. Cada vez, el agente elige una versión (la más reciente o una de los últimos 30 días) y unos cuantos archivos de distintos tamaños, los restaura con comprobación del contenido y mira que salen enteros.</p>
          <ul>
            <li>Por defecto, 20 archivos y hasta 200 MB, una vez por semana.</li>
            <li>Los archivos se restauran en una carpeta a la que solo tienen acceso el sistema y los administradores, y se borran siempre al terminar.</li>
            <li>Nunca coincide con una copia ni con otra tarea, y no se hace con las copias automáticas en pausa.</li>
            <li>En un repositorio que solo recibe la copia externa de otro, no hace falta: se prueba desde el origen.</li>
          </ul>`,
      },
      {
        id: "mant-cambio-inusual",
        title: "Cambio inusual: la subida a la nube se frena",
        html: `
          <p>Con <strong>Frenar la subida si una copia cambia mucho más de lo normal</strong>, el agente compara cada copia con las anteriores. Si una añade muchísimos más datos o archivos de lo habitual (por defecto, más de 20 veces lo normal y, como mínimo, 2 GB o 5.000 archivos), <strong>deja de subir a la nube</strong> y te avisa en el repositorio, en Estado y en la web. Así, si un virus cifró los archivos, esa versión no sustituye a las buenas en la copia externa, y la retención de allí no borra nada mientras tanto.</p>
          <p>La copia externa puede subir <strong>después de cada copia con cambios</strong> (recomendado): en cuanto hay una versión nueva, con un mínimo de minutos entre subidas. Si no hubo cambios, no sube nada.</p>
          <p><strong>Si fue algo normal</strong> (una carpeta grande nueva, una actualización del programa…), pulsa <strong>Es normal, reanudar la subida</strong>.</p>
          <p><strong>Si puede ser un ransomware</strong> (archivos que no abren, extensiones raras, notas pidiendo un rescate):</p>
          <ul>
            <li><strong>No pagues</strong> ni borres nada todavía.</li>
            <li><strong>Aísla el equipo</strong>: desconéctalo de la red (cable y wifi) y pausa también las copias automáticas.</li>
            <li>Las versiones anteriores están a salvo: en un servidor en modo solo-añadir (<code>--append-only</code>) y en la copia externa nadie pudo borrarlas.</li>
            <li>Con el equipo limpio (o reinstalado), <strong>restaura desde una versión anterior al cambio</strong>. «Ver qué cambió» te ayuda a saber desde cuándo.</li>
          </ul>`,
      },
      {
        id: "retencion-plazos",
        title: "Retención por plazos y en servidores append-only",
        html: `
          <p><strong>Por plazos</strong> (recomendado) dices cuánto tiempo guardar cada ritmo: por ejemplo, <em>una por hora durante 15 días, una por día durante 1 año y una por mes siempre</em>. Los plazos se cuentan desde la versión más reciente, así que un equipo apagado unos días no pierde versiones. <strong>Por cantidad</strong> cuenta versiones (7 diarias, 12 mensuales…).</p>
          <ul>
            <li><strong>Agrupar:</strong> restic aplica la política por separado a cada equipo y carpeta de origen. Si el repositorio recibe versiones de varios orígenes (p. ej. un script antiguo), elige «Todas juntas, un solo grupo».</li>
            <li><strong>Aplicar a:</strong> puedes limitarla a las versiones con ciertas etiquetas, de un equipo o de unas carpetas. Las demás no se tocan nunca.</li>
            <li><strong>Nunca borrar:</strong> las versiones con la etiqueta que indiques (p. ej. <code>conservar</code>) se quedan siempre.</li>
            <li><strong>Copia externa a otro repositorio:</strong> se aplica la retención de ese repositorio (edítala desde «Mantenimiento»), después de cada subida. Con <em>Object Lock</em>, lo borrado solo libera espacio cuando vence el bloqueo.</li>
          </ul>
          <p><strong>Servidores REST en modo solo-añadir</strong> (<code>--append-only</code>): Resguardo no puede borrar en ellos. La pestaña «Retención» te da los comandos para ejecutar en el propio servidor: primero la prueba con <code>--dry-run</code>, después <code>forget</code> y <code>prune</code>, y una línea de <code>cron</code> para hacerlo cada domingo.</p>`,
      },
    ],
  },
  {
    id: "restaurar",
    title: "Restaurar",
    icon: "rotateccw",
    items: [
      {
        id: "restaurar-pasos",
        title: "Cómo recuperar archivos",
        html: `
          <ol>
            <li>Abre la copia (o el repositorio) y pulsa <strong>Restaurar archivos…</strong>.</li>
            <li><strong>Elige la versión</strong>: por defecto, la más reciente. Si buscas un archivo tal como estaba otro día, usa el calendario.</li>
            <li><strong>Marca</strong> los archivos y carpetas que quieres recuperar. El filtro busca dentro de la carpeta que estás viendo.</li>
            <li><strong>Elige dónde</strong> dejarlos y pulsa <strong>Restaurar</strong>. Al terminar, <strong>Abrir carpeta</strong> te lleva a ellos.</li>
          </ol>
          <p>También puedes pulsar cualquier versión del historial para explorarla directamente.</p>`,
      },
      {
        id: "restaurar-donde",
        title: "¿Dónde restaurar?",
        html: `
          <ul>
            <li><strong>En otra carpeta</strong> (recomendado): por defecto, una carpeta nueva en el Escritorio llamada «Restaurado» con la fecha. No se toca nada de lo que ya tienes; después copias lo que necesites.</li>
            <li><strong>En su ubicación original</strong>: los archivos vuelven a la carpeta de donde se copiaron. Úsalo si se borraron o si el equipo es nuevo.</li>
          </ul>`,
      },
      {
        id: "buscar-archivo",
        title: "Buscar un archivo en las versiones",
        html: `
          <p>¿No sabes en qué versión está un archivo, o cuándo desapareció? En un repositorio pulsa <strong>Buscar un archivo</strong> (o <kbd>Ctrl</kbd>+<kbd>K</kbd> y «Buscar archivo en…») y escribe su nombre o una parte.</p>
          <ul>
            <li>No distingue mayúsculas. Sin comodines se busca como parte del nombre: <code>factura</code> encuentra <code>Factura_123.xlsx</code>. Con <code>*</code> buscas un patrón: <code>*.xlsx</code>.</li>
            <li>Para ir más rápido, busca solo en las versiones más recientes o en las de una copia.</li>
            <li>Cada resultado dice cuándo aparece por primera y por última vez, y si sigue en la versión más reciente. Las versiones en las que el archivo era igual se agrupan; si cambió, verás cada variante con su fecha y tamaño.</li>
            <li><strong>Abrir en esa versión</strong> te lleva a la carpeta del archivo en esa versión; <strong>Restaurar</strong> recupera ese archivo tal como estaba.</li>
          </ul>
          <p>La búsqueda solo lee: no cambia nada y puedes detenerla cuando quieras. En repositorios grandes o en la nube puede tardar unos minutos.</p>`,
      },
      {
        id: "ver-versiones",
        title: "Ver versiones desde el Explorador",
        html: `
          <p>Haz clic derecho en un archivo o carpeta del Explorador de Windows y elige <strong>Ver versiones en Resguardo</strong> (en Windows 11, dentro de <strong>Mostrar más opciones</strong>). Resguardo busca en qué copias está y te muestra sus versiones guardadas, la más reciente primero.</p>
          <ul>
            <li>Primero hay que activarlo en <strong>Ajustes → Explorador de archivos</strong>. Es solo para tu usuario.</li>
            <li><strong>Restaurar esta versión</strong> la guarda junto al original con otro nombre, por ejemplo «Informe (versión del 30-09).xlsx». El original no se toca y nunca se reemplaza nada: si ese nombre ya existe, se añade «(2)».</li>
            <li><strong>Abrir en esa versión</strong> abre esa versión en Resguardo, en la carpeta del archivo, para ver qué más había o restaurar otras cosas.</li>
            <li>En los archivos, cada fila es un <strong>contenido distinto</strong>: una fila por cada vez que cambió. Si el mismo contenido está en varias versiones, despliega la fila para ver todas sus fechas y abrir o restaurar cualquiera.</li>
            <li>Cada fila dice si es <strong>igual que la actual</strong> (mismo tamaño y fecha de modificación) o distinta, y cuánto ocupa de más o de menos.</li>
            <li>Se busca en las versiones de la copia elegida. Debajo verás en cuántas se ha buscado y, si el repositorio tiene más (de otras copias, o de antes de cambiar las carpetas de esta), puedes <strong>buscar también en esas</strong>.</li>
            <li>Si ninguna copia incluye ese archivo o carpeta, Resguardo te lo dice: solo se guardan las carpetas que eliges en cada copia. Si está en una copia pero no aparece, puede que sea nuevo o que esté excluido.</li>
          </ul>`,
      },
      {
        id: "restaurar-existentes",
        title: "Si un archivo ya existe",
        html: `
          <ul>
            <li><strong>Conservarlo</strong> (recomendado): solo se crean los archivos que falten; lo que ya tienes no se toca.</li>
            <li><strong>Reemplazarlo</strong>: se sobrescribe con la versión de la copia. No se puede deshacer, por eso Resguardo te pide la contraseña del repositorio.</li>
          </ul>
          <p>¿Quieres tener las dos versiones? Restaura en otra carpeta y compáralas.</p>`,
      },
      {
        id: "restaurar-problemas",
        title: "Si algo sale mal",
        html: `
          <ul>
            <li><strong>«¿Está conectado el disco?»</strong>: conecta el disco externo del repositorio y pulsa «Reintentar».</li>
            <li><strong>«Contraseña incorrecta»</strong>: la contraseña guardada ya no coincide; quita el repositorio y vuelve a conectarlo con la correcta.</li>
            <li><strong>Carpeta vacía o que no existe</strong>: esa versión no tenía la carpeta; prueba con otra versión.</li>
            <li><strong>Algunos elementos no se pudieron restaurar</strong>: suele ser un archivo abierto o sin permiso en la carpeta donde restauras. Ciérralo o restaura en otra carpeta.</li>
          </ul>`,
      },
    ],
  },
  {
    id: "regla-321",
    title: "La regla 3-2-1",
    icon: "layers",
    items: [
      {
        id: "regla-321-que-es",
        title: "Qué es y cómo cumplirla",
        html: `
          <p>Es la regla de oro de las copias de seguridad:</p>
          <ul>
            <li><strong>3 copias</strong> de tus datos: los originales y dos copias.</li>
            <li><strong>2 soportes distintos</strong>: por ejemplo, el disco del equipo y un disco externo.</li>
            <li><strong>1 copia fuera de casa</strong>: en la nube o en otro edificio, por si hay un robo, un incendio o una inundación.</li>
          </ul>
          <p>Con Resguardo: una copia a un <strong>disco externo</strong> y una <a href="#" data-topic="mant-copia-externa">copia externa</a> a la nube. Y de vez en cuando, prueba a <a href="#" data-topic="restaurar-pasos">restaurar</a> algún archivo: una copia que nunca se ha probado no es una copia segura.</p>`,
      },
    ],
  },
  {
    id: "seguridad",
    title: "Seguridad",
    icon: "shieldcheck",
    items: [
      {
        id: "salud-proteccion",
        title: "Salud de la protección",
        html: `
          <p>Arriba de cada repositorio, un anillo resume lo que protege de verdad sus copias («5 de 7») y una lista te dice qué falta y te lleva a arreglarlo:</p>
          <ul>
            <li><strong>Copias automáticas</strong> activas y sin fallos.</li>
            <li><strong>Protegida contra borrado</strong>: un servidor REST de solo añadir (<code>--append-only</code>, Resguardo lo comprueba una vez al día sin borrar nada) o un bucket con bloqueo de objetos. Un disco local no lo está: un ransomware podría borrarlo.</li>
            <li><strong>Copia externa</strong> configurada y al día.</li>
            <li><strong>Verificación</strong> programada y sin errores (también la de la copia en la nube).</li>
            <li><strong>Prueba de restauración</strong> reciente y correcta.</li>
            <li><strong>Kit de recuperación</strong> guardado.</li>
            <li><strong>Retención</strong> configurada (en un servidor de solo añadir se aplica allí).</li>
          </ul>
          <p>En Estado ves la versión corta de cada repositorio; la web muestra la misma lista.</p>
          <p>Si falta algo, pulsa <a href="#" data-topic="mejorar-proteccion">Mejorar la protección</a> y te guía paso a paso.</p>`,
      },
      {
        id: "mejorar-proteccion",
        title: "Mejorar la protección, paso a paso",
        html: `
          <p>En un repositorio al que le falta algo, pulsa <strong>Mejorar la protección</strong> (en el resumen de arriba o en la salud de la protección). Se abre un panel en la esquina que se queda a la vista mientras trabajas:</p>
          <ol>
            <li>Te propone lo que falta, de uno en uno y en orden: las copias automáticas, el kit de recuperación, la prueba de restauración, la verificación, la copia externa, la retención y la protección contra borrado.</li>
            <li>Cada paso explica para qué sirve y su botón abre el editor de siempre (el kit, la verificación…). Al guardarlo, el anillo se actualiza solo y pasa al siguiente.</li>
            <li>Puedes <strong>saltar</strong> un paso o elegir otro de la lista, y cerrar el panel con Escape cuando quieras.</li>
          </ol>
          <p>Al completar todo verás «7 de 7». Algunas cosas no dependen de Resguardo (por ejemplo, que un disco normal se pueda borrar): el panel te dice cómo resolverlas.</p>`,
      },
      {
        id: "resumen",
        title: "El resumen de arriba",
        html: `
          <p>Arriba de cada repositorio y de cada copia, una o dos frases te cuentan cómo está, por ejemplo: «Disco externo está protegido. Última copia hace 12 minutos, la copia externa va al día y se verificó el domingo. Falta: preparar el kit de recuperación.»</p>
          <ul>
            <li>Sale de los mismos datos que la <a href="#" data-topic="salud-proteccion">salud de la protección</a> y el estado de las copias: no hay nada que configurar.</li>
            <li>Solo menciona <strong>lo más importante que falta</strong> (primero lo que falla, luego lo que no está configurado). La lista completa está en la salud de la protección.</li>
            <li>En una copia, además de cómo va, te dice lo más importante que le falta a su repositorio.</li>
          </ul>`,
      },
      {
        id: "esquema-destino",
        title: "El recorrido de tus copias",
        html: `
          <p>Debajo del resumen de cada repositorio, tres pasos te enseñan por dónde pasan tus archivos:</p>
          <ol>
            <li><strong>Tus archivos</strong>: las carpetas de las copias que se guardan aquí y cómo fue la última.</li>
            <li><strong>Este repositorio</strong>: su última versión y si está protegido contra borrado (un servidor de solo añadir o un bucket con bloqueo de objetos; un disco normal se podría borrar).</li>
            <li><strong>Copia externa</strong>: a dónde se suben las versiones fuera de este equipo y cuándo fue la última subida. Si no hay ninguna, el paso sale en gris con «Configurar».</li>
          </ol>
          <p>El punto de color de cada paso es su estado: verde al día, ámbar algo mejorable, rojo algo falla. Pulsa un paso para ir a su sección.</p>`,
      },
      {
        id: "kit-recuperacion",
        title: "Kit de recuperación",
        html: `
          <p>Las contraseñas de los repositorios se guardan cifradas <strong>solo en este equipo</strong>. Si el equipo se pierde o se estropea, sin ellas nadie puede abrir las copias, tampoco Resguardo.</p>
          <p>El <strong>kit de recuperación</strong> es una hoja para imprimir (o guardar como PDF) con todo lo necesario: la ubicación de cada repositorio, el ID de la clave de la nube, el ID del repositorio y los pasos para recuperar con o sin Resguardo. No incluye las credenciales del servidor ni las claves secretas de la nube.</p>
          <ul>
            <li>Ábrelo desde Estado o desde un repositorio (<strong>Kit de recuperación</strong>).</li>
            <li>La contraseña se puede escribir a mano en la hoja o, si lo eliges, imprimirla: tendrás que escribirla tú (Resguardo nunca la muestra). Guárdalo como guardarías las llaves de la oficina.</li>
            <li>Al terminar, pulsa <strong>Ya lo guardé en un lugar seguro</strong>. Si cambias la ubicación del repositorio, Resguardo te pedirá uno nuevo.</li>
          </ul>`,
      },
      {
        id: "seguridad-bloqueo",
        title: "Bloquear Resguardo con Windows Hello",
        html: `
          <p>En un equipo que usan varias personas, cualquiera que se siente delante podría abrir Resguardo y ver o restaurar tus archivos. Para evitarlo, en <strong>Ajustes → Seguridad</strong> activa <strong>Pedir Windows Hello (rostro, huella o PIN) al abrir Resguardo</strong>.</p>
          <ul>
            <li>Al abrir la app, Windows te pide confirmar que eres tú (huella, cara, PIN o contraseña, según lo que tengas configurado). Mientras tanto no se ve nada de tus copias.</li>
            <li>Opcional: <strong>Volver a pedirla tras</strong> unos minutos sin usar la app.</li>
            <li>Activarlo, cambiarlo o quitarlo también pide confirmarlo.</li>
            <li>Necesita Windows Hello configurado (Configuración de Windows → Cuentas → Opciones de inicio de sesión). Si deja de estarlo, Resguardo se abre sin pedirlo y te avisa, para no dejarte fuera de tus copias.</li>
            <li>Las copias automáticas no se ven afectadas: siguen funcionando con la app bloqueada o cerrada.</li>
          </ul>`,
      },
      {
        id: "copias-a-distancia",
        title: "Todos mis equipos y copias a distancia",
        html: `
          <p>En <strong>Todos mis equipos</strong> ves el estado de los demás equipos de tu cuenta de Resguardo Web (el servidor, otro portátil…) y puedes pedirles <strong>Copiar ahora</strong> de una copia.</p>
          <ul>
            <li>Entra con la misma cuenta que en la web, con la verificación en dos pasos (el código del autenticador). La contraseña no se guarda; la sesión se guarda cifrada para tu usuario de Windows y puedes cerrarla cuando quieras.</li>
            <li>Para que un equipo acepte copias a distancia, actívalo en ese equipo: <strong>Ajustes → Este equipo → Copias a distancia</strong>. Hace falta abrir Resguardo como administrador y la contraseña de uno de sus repositorios. Viene desactivado.</li>
            <li>A distancia <strong>solo se pueden pedir copias</strong> de una copia que ya existe: nada se puede borrar, restaurar, pausar ni cambiar.</li>
            <li>La copia empieza en menos de 5 minutos (cuando el agente de ese equipo se despierta) y la verás pasar a «En marcha» y «Hecha». Si el repositorio está en pausa o la petición tarda más de 30 minutos en llegar, se rechaza.</li>
            <li>En la Actividad de ese equipo queda anotada como «A distancia», con el equipo desde el que se pidió.</li>
          </ul>`,
      },
      {
        id: "seguridad-contrasenas",
        title: "Qué pide contraseña y por qué",
        html: `
          <p>Las acciones que podrían hacer perder datos o cambiar qué se copia piden la contraseña del repositorio, aunque esté guardada. Así, alguien que use tu equipo un momento no puede estropear tus copias:</p>
          <ul>
            <li>Crear, editar, mover o eliminar copias.</li>
            <li>Programar copias automáticas, la verificación o la copia externa.</li>
            <li>Cambiar la retención o renombrar y quitar un repositorio.</li>
            <li>Restaurar <strong>reemplazando</strong> archivos existentes.</li>
          </ul>
          <p>Explorar versiones y restaurar sin reemplazar no la piden.</p>`,
      },
      {
        id: "seguridad-web",
        title: "Qué ve Resguardo Web",
        html: `
          <p>Si vinculas el equipo con Resguardo Web para ver el estado de tus copias desde el navegador, solo se envían <strong>metadatos</strong>: nombres de copias y repositorios, fechas, tamaños y si hubo errores.</p>
          <p>Nunca se envían contraseñas, claves, rutas ni nombres de archivos, y la web no puede leer ni restaurar tus datos.</p>
          <p>Se vincula y desvincula en <strong>Ajustes → Este equipo → Resguardo Web</strong> (pide abrir Resguardo como administrador).</p>`,
      },
    ],
  },
  {
    id: "atajos",
    title: "Atajos de teclado",
    icon: "keyboard",
    items: [
      {
        id: "buscar-todo",
        title: "Encontrarlo todo con Ctrl+K",
        html: `
          <p>Pulsa <kbd>Ctrl</kbd>+<kbd>K</kbd> y escribe lo que buscas. Los resultados salen agrupados:</p>
          <ul>
            <li><strong>Ir a</strong>, <strong>Copias</strong> y <strong>Repositorios</strong>: abre cualquiera.</li>
            <li><strong>Acciones</strong>: «copiar ahora», pausar o reanudar, restaurar, buscar un archivo, el kit, verificar o subir ahora, la historia, mejorar la protección, nueva copia, añadir repositorio…</li>
            <li><strong>Ajustes</strong>: por su nombre o con otras palabras («oscuro», «notificaciones», «huella»…).</li>
            <li><strong>Ayuda</strong>: escribe la pregunta tal cual, por ejemplo «qué es la retención».</li>
          </ul>
          <p>Muévete con <kbd>↑</kbd> <kbd>↓</kbd> y pulsa <kbd>Intro</kbd>. No distingue tildes ni mayúsculas.</p>`,
      },
      {
        id: "atajos-lista",
        title: "Atajos",
        html: `
          <ul class="keys">
            <li><span><kbd>F1</kbd></span> Abrir esta ayuda</li>
            <li><span><kbd>Ctrl</kbd>+<kbd>K</kbd></span> Buscar cualquier cosa: repositorios, copias, acciones, ajustes y la ayuda</li>
            <li><span><kbd>Ctrl</kbd>+<kbd>N</kbd></span> Nueva copia</li>
            <li><span><kbd>Ctrl</kbd>+<kbd>Mayús</kbd>+<kbd>N</kbd></span> Añadir repositorio</li>
            <li><span><kbd>Ctrl</kbd>+<kbd>1</kbd></span> Ir a Estado</li>
            <li><span><kbd>Ctrl</kbd>+<kbd>2</kbd></span> Ir a Actividad</li>
            <li><span><kbd>Ctrl</kbd>+<kbd>↑</kbd> / <kbd>↓</kbd></span> Copia o repositorio anterior / siguiente</li>
            <li><span><kbd>Retroceso</kbd></span> Subir un nivel al explorar una versión</li>
            <li><span><kbd>Esc</kbd></span> Cerrar un diálogo, quitar la selección o volver</li>
          </ul>`,
      },
    ],
  },
];

// «Qué significa cada cosa»: los mismos textos que los «?» de la interfaz (glossary.ts).
const GROUP_PREFIX = { conceptos: "", cifras: "", copias: "Copia: ", repositorios: "Repositorio: ", proteccion: "Protección: " } as const;
HELP.push({
  id: "que-significa",
  title: "Qué significa cada cosa",
  icon: "book",
  items: Object.entries(GLOSSARY).map(([key, e]) => ({
    id: `glosario-${key}`,
    title: `${GROUP_PREFIX[e.group]}${e.title}`,
    html: `<p class="faint">${GLOSSARY_GROUPS[e.group]}</p>${glossaryHtml(e)}`,
  })),
});

/** Texto plano sin acentos ni mayúsculas, para buscar. */
export const normalize = (s: string) =>
  s
    .replace(/<[^>]+>/g, " ")
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase();

/** Sección que contiene un id (de sección o de apartado). */
export function sectionOf(topic: string): HelpSection | undefined {
  return HELP.find((s) => s.id === topic || s.items.some((i) => i.id === topic));
}
