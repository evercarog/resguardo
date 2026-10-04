# Resguardo Agente: equipos gestionados

Nota de diseño de la fase 5. Los PC de los empleados de una oficina (por ejemplo, Ferretería Altamar) copian en el «Servidor de copias de Resguardo» (fase 4). En esos PC no hay app completa: todo se administra desde el Resguardo del servidor (la **consola**) y también desde la web o «Todos mis equipos».

## Piezas

| Pieza | Qué es |
|---|---|
| **Consola** | La app completa en el servidor. Nueva sección **Equipos gestionados**. |
| **Agente gestionado** | Instalador pequeño, «Resguardo Agente», con un servicio de Windows (LocalSystem) y, opcional, un icono en la bandeja. |
| **Web** | Hace de relevo de mensajes cifrados y firmados, y muestra el estado. Nunca puede dar órdenes por su cuenta. |

## ¿Mismo código o binario aparte?

**Recomendación: el mismo código (la biblioteca `resguardo_lib`) con un binario aparte, `resguardo-agente.exe`.**

- El agente gestionado reutiliza sin cambios lo más delicado y ya probado: restic, planes, copias, reintentos, VSS, historial e informe a la web.
- El binario aparte no enlaza la interfaz web (WebView2, Svelte) y se instala con su propio instalador pequeño. Si cada pieza tuviera su propio código, el corazón de las copias se duplicaría y sus errores también.
- Un *feature flag* sobre el mismo ejecutable obligaría a instalar WebView2 en cada PC y a llevar la app entera. Por eso se descarta.

`resguardo-agente.exe` tiene cuatro modos:

- `--service`: el servicio de Windows. Recibe la configuración firmada, hace las copias e informa.
- `--tray`: el icono de la bandeja, nativo, con `tray-icon` y sin WebView2 (`bandeja.rs`). Corre como el usuario y solo lee `gestionado-bandeja.json`, que el servicio escribe cada 30 s (y en cada vuelta) sin rutas ni secretos. El icono es el escudo de Resguardo pintado según el estado: verde azulado al día, con un anillo de progreso mientras copia, gris esperando (sin vincular, sin copias asignadas o antes de la primera), ámbar si hay que mirar algo (avisos, la última copia buena tiene 7 días o más, o el servicio lleva 15 min sin escribir) y rojo si una copia falló. El menú: una línea de estado, quién gestiona el equipo (en líneas cortas), «Copias» (cada copia con su última vez y la próxima), «Copiar ahora» (las copias que no están en pausa: una, o un submenú con «Todas» y cada una), «Abrir la consola» (solo https://), «Ver el registro» y «Ocultar el icono hasta el próximo inicio de sesión». «Copiar ahora» no da autoridad nueva: deja una solicitud en `solicitudes\` con el mecanismo de siempre (`agent::request_backup`) y el agente solo la atiende para copias que tiene programadas. Si la consola lo permite, avisa cuando una copia falla y cuando vuelve a funcionar. Abierta por el instalador (como administrador) no ofrece «Ver el registro» y abre la consola a través del Explorador, con los permisos del usuario.
- `--pair <código>`: emparejar (como administrador local).
- `--unpair`: olvidar la consola (como administrador local).

## Seguridad (lo crítico)

### Claves

- **Consola:** un par **Ed25519** para firmar. La privada vive en los secretos del agente del servidor (DPAPI de máquina, carpeta privada).
- **Agente gestionado:** un par **X25519** para recibir cifrado (el de la fase 3) y otro **Ed25519** para firmar sus informes (opcional en esta fase).

### Emparejamiento

1. En la consola: **Equipos gestionados → Añadir equipo**. Muestra un código corto de 10 caracteres (unos 50 bits) que caduca en 15 minutos. La web guarda solo un hash del código y la clave pública Ed25519 de la consola.
2. En el PC, como administrador local: `resguardo-agente.exe --pair ABCD-EFGH-JK`, o el instalador, que lo pide.
   - El agente envía a la web el hash del código y su clave pública X25519.
   - Recibe la clave pública Ed25519 de la consola y la **fija** en su configuración protegida.
3. **Comprobación visual.** Los dos muestran el mismo **código de 6 cifras**, derivado del hash de las dos claves públicas (SAS). La consola pide confirmar que coincide antes de mandar nada. Así, una web comprometida no puede colar su propia clave.

> Con Resguardo Server (agente v2, [api-servidor.md](api-servidor.md) §1) el SAS liga la identidad del servidor y las dos llaves del equipo (v2) y, desde v1.26 (agente ≥ 0.7.10), también la huella SHA-256 de la autoridad TLS (v3): el equipo pone la de la autoridad que fijó al vincularse y la consola la que da el servidor. Si alguien en medio hizo que el equipo fijase otra autoridad, los números no coinciden. Con un agente anterior (v2), la consola avisa de que hay que comprobar también la huella del certificado.

### Órdenes y configuración

Todo lo que la consola manda va **firmado con Ed25519** y **cifrado para el agente** (sobre sellado X25519, como en la fase 3):

```json
{ "v": 1, "endpoint": "<id>", "seq": 42, "issued_at": "…", "kind": "config|backup_now|restore|unpair",
  "body": { … } }
```

- `seq` es monotónico: el agente rechaza cualquier `seq <= último aceptado`, lo que protege de repeticiones.
- `endpoint` tiene que ser el suyo.
- Las órdenes caducan: `issued_at` no puede tener más de 24 h.
- La firma cubre el JSON exacto (los bytes), no una versión reconstruida.
- `config` lleva:
  - las copias (carpetas, exclusiones, horario);
  - el repositorio (`rest:https://servidor:puerto/<usuario>/<repo>/`, usuario y contraseña del rest-server, contraseña del repositorio, certificado fijado);
  - las opciones de la bandeja.
- **Ni una web comprometida ni una cuenta robada pueden redirigir las copias a otro sitio:** sin la clave privada de la consola no hay firma válida.

### Aislamiento

- Cada agente tiene **su propio usuario del rest-server y su propio repositorio**, en modo append-only (fase 4). Un agente no puede leer ni borrar lo de otro, ni siquiera lo suyo antiguo.
- **Volver a emparejar o cambiar de consola** exige un administrador local en el PC (`--unpair` y `--pair`). Una orden `unpair` firmada solo hace que el agente deje de copiar; nunca acepta otra consola sin intervención local.

### Nubes del espejo (Dropbox desde la consola)

- «Conectar Dropbox» en la consola manda `conectar_nube` con la **clave de administración**, sellada solo para el equipo que guarda copias: lleva el refresh token de Dropbox (OAuth 2 con PKCE, sin app secret) y la app key pública. El servidor no lo ve.
- El agente lo comprueba renovándolo una vez y lo guarda **protegido con DPAPI** en su carpeta privada (`nubes.bin`), como los demás secretos. El resultado firmado dice «conectada» o por qué no, **sin repetir el token**; el resumen solo lleva `guarda_copias.nubes: [{ nombre, tipo }]`.
- La app de Dropbox tiene permiso «App folder»: aunque el token se filtrara, solo alcanza `Aplicaciones/Resguardo`. Lo subido son paquetes de restic ya cifrados.
- `quitar_nube` olvida el permiso. Si el espejo usa esa nube, reduce la protección: espera mínima (destructiva) y sale también del espejo.
- Detalle del contrato: [api-servidor.md](api-servidor.md) §12; cómo se renueva: [destinos.md](destinos.md).

### Lo que ve la consola (privacidad)

Por diseño, la consola **puede ver los nombres y el contenido de los archivos de las carpetas que copia**: tiene la contraseña del repositorio para poder restaurar. La bandeja del PC lo dice en una línea: «Este equipo lo gestiona «Servidor Altamar»: sus copias se guardan allí y su administrador puede restaurarlas».

### Modelo de amenazas

| Atacante | Qué puede | Mitigación |
|---|---|---|
| La web o una cuenta robada | Ver el estado, retrasar o borrar mensajes. | Todo va firmado por la consola y cifrado para cada agente: no puede inventar órdenes ni configuraciones. |
| La web sustituye claves al emparejar | Hacerse pasar por la consola. | Código SAS de 6 cifras comprobado a ojo en los dos lados. |
| Una consola comprometida | Mandar configuraciones y restaurar archivos de los agentes. | Es el administrador por diseño. Se limita su daño: no puede borrar copias (append-only en el servidor) y no puede mover un agente a otra consola sin un administrador local. Queda registrado en el historial de cada agente. |
| Un administrador local malicioso en el PC | Parar el servicio, desemparejar o leer la configuración (con la contraseña de su repositorio). | Es dueño de la máquina. Si para o desinstala el servicio, el último informe a la web lo dice («detenido por un administrador») y la consola avisa si deja de informar. Lo ya copiado sigue a salvo (append-only). |
| Un empleado sin privilegios | Ver el estado en la bandeja. | No puede ver la configuración, las contraseñas ni las versiones, ni parar el servicio. |
| Repetir una orden antigua | — | `seq` monotónico y caducidad de 24 h. |

## Consola: «Equipos gestionados»

Nueva sección de la barra lateral, debajo de «Todos mis equipos»:

- **Añadir equipo:** el código de emparejamiento y luego la comprobación del SAS.
- **Por equipo:**
  - estado y protección;
  - historia;
  - **Copiar ahora**;
  - **Copias** (las asignadas, con el editor de copias de siempre);
  - **Restaurar** sus archivos, en el propio PC o en la consola. En la consola es directo: tiene el repositorio. En el PC se hace con una orden firmada.
- Al asignar la primera copia, la consola crea en el Servidor de copias el usuario de ese equipo y un repositorio con una contraseña generada, que guarda en el kit.

También aparece en «Todos mis equipos» y en la web, con el estado. Las órdenes siempre las firma la consola.

## Contrato con la web (fase 5)

**Tablas:**

```sql
create table managed_consoles (
  device_id uuid primary key references devices(id) on delete cascade,  -- el servidor
  sign_public_key text not null                                       -- Ed25519, base64
);
create table managed_pairings (
  id uuid primary key default gen_random_uuid(),
  console_device uuid not null references devices(id) on delete cascade,
  code_hash text not null,               -- sha256(código), nunca el código
  expires_at timestamptz not null,       -- 15 min
  endpoint_device uuid references devices(id),
  endpoint_box_key text,                 -- X25519 del agente, base64
  status text not null default 'open' check (status in ('open','joined','confirmed','expired'))
);
create table managed_messages (
  id uuid primary key default gen_random_uuid(),
  console_device uuid not null references devices(id) on delete cascade,
  endpoint_device uuid not null references devices(id) on delete cascade,
  seq bigint not null,
  ciphertext text not null check (length(ciphertext) <= 65536),  -- sobre sellado de {payload, firma}
  created_at timestamptz not null default now(),
  taken_at timestamptz,
  unique (endpoint_device, seq)
);
```

**RPC de equipo** (`p_device`, `p_secret`):

| RPC | Para qué |
|---|---|
| `managed_console_set_key(p_public_key)` | La consola publica su clave Ed25519. La web lo rechaza si quien llama es un equipo gestionado. |
| `managed_pairing_open(p_code_hash)` | `{ id, expires_at }`. Exige la clave publicada antes. Límites: 5 abiertos a la vez, 20 por hora. |
| `managed_pairing_status(p_pairing)` | Para la consola: `{ status, endpoint_device, endpoint_name, endpoint_box_key, expires_at }`. Tras unirse el equipo, la consola tiene 15 minutos para confirmar. |
| `managed_pairing_confirm(p_pairing)` | Para la consola, tras comprobar el SAS. |
| `managed_pairing_cancel(p_pairing)` | Para la consola, si el SAS no coincide: la web borra el equipo sin confirmar. |
| `managed_send(p_endpoint, p_seq, p_ciphertext)` | Para la consola. Solo a sus agentes confirmados. Devuelve el id (uuid). Como mucho 100 sin tomar por equipo y 65 536 caracteres por mensaje. |
| `managed_take()` | Para el agente: `[{ id, seq, console_device, ciphertext, created_at }]`, sus mensajes no tomados, por `seq`. Los marca como tomados. `[]` hasta que la consola confirma. El agente descarta lo que no venga de su `console_device` (y, en todo caso, lo que no firme su consola). |

**RPC anónima** (solo con la clave pública de la web):

| RPC | Para qué |
|---|---|
| `managed_pairing_join(p_code_hash, p_box_key, p_name, p_os = null, p_app_version = null)` | Para el agente. Devuelve `{ pairing_id, console_device, console_name, console_sign_key, device_id, secret }` y crea su equipo en la cuenta del dueño de la consola. Con un código mal escrito o caducado devuelve 200 `{ "error": "codigo", "message": "Código no válido o caducado" }`. Con límites por IP. |

**Copias remotas:** la web las fuerza a desactivadas en los equipos gestionados (no hay `remote_*` que valga para ellos).

**Informe del agente:** el `device_report` de siempre, con `"managed": { "console_device": "…", "seq": 42, "service": "running|stopped_by_admin" }`.

## Plan de implementación

1. `managed.rs`: firma y verificación Ed25519, sobres firmados y cifrados, `seq` monotónico, SAS, código de emparejamiento. Tests de todo.
2. Consola: la sección «Equipos gestionados» (emparejar, lista, detalle) contra el mock.
3. `resguardo-agente.exe`: `--pair`, `--unpair`, `--service` (servicio de Windows con `windows-service`, que en cada vuelta aplica lo recibido y ejecuta el ciclo del agente de siempre) y `--tray` (`tray-icon`, nativo).
4. Instalador del agente (hecho, ver «Despliegue»). Falta firmarlo (Authenticode) en la canalización de publicación.

## Venir de la app de escritorio

Si un equipo ya hacía copias con la app de escritorio (0.6.x) o con restic a mano, sus repositorios **no se pierden ni hay que empezar de cero**: el agente gestionado puede seguir copiando en ellos con todo su historial. Detalle técnico en [api-servidor.md](api-servidor.md), §5 (`adoptar_repositorio`, `copiar_historial`, v1.14).

1. **Instala el agente y vincula el equipo** como cualquier otro (página «Emparejar»). La app de escritorio y el agente gestionado no comparten configuración: deja de programar copias en la app de escritorio para ese repositorio (o desinstálala) cuando el agente ya copie, para no copiar dos veces.
2. **Usa el repositorio que ya existe.** En la consola, «Repositorios» → «Nuevo repositorio» → **«Usar uno que ya existe»**:
   - **Dónde está:** el tipo (servidor de copias, disco o carpeta, Backblaze B2, S3, SFTP) y la **dirección completa del repositorio**, la misma que tenía la app de escritorio, con su carpeta al final. Por ejemplo `http://192.168.1.30:8001/Siigo`, `D:\Copias\Siigo` o `mi-bucket:siigo`. Si el servidor pide usuario y contraseña, escríbelos; si usa HTTPS con un certificado propio, pega su autoridad (PEM).
   - **Contraseña del repositorio:** la del kit de recuperación o la que guardaba la app de escritorio.
   - **«Probar»:** el equipo abre el repositorio con esos datos y dice cuántas versiones tiene, de cuándo es la última, qué equipos copiaron en él y si el servidor es de solo añadir. No cambia nada.
   - **«Usar este repositorio»** (con la clave de administración): queda como un repositorio más del equipo, **de lectura y escritura**. Se conserva todo: las versiones de siempre siguen ahí y las copias nuevas se añaden a ellas. Después, elige en «Copias» qué carpetas van a él y cuándo.
3. **Servidores de solo añadir** (un rest-server con `--append-only`, como el de «Este equipo guarda copias»): desde el equipo no se puede borrar nada, que es justo lo que lo protege. Por eso la consola muestra la retención como **«en el servidor»** y «Aplicar la retención» no se ejecuta desde el equipo:
   - **En un almacén de Resguardo** («Este equipo guarda copias»), la aplica el propio almacén: en la página del repositorio, **«Retención en el almacén»** (la regla, el horario —p. ej. los domingos a las 03:00— y si comprobar después). Se confirma una vez con la contraseña del repositorio y la clave de administración (con la espera de lo destructivo) y después se aplica sola; el panel dice «La aplica el almacén ALMACEN-01 los domingos a las 03:00», el último resultado y **«Aplicar ahora»** (también espera). El almacén usa una clave de restic propia (que el equipo añade al repositorio), así que puede leer ese repositorio: la contrapartida está en [compartir.md](compartir.md), «Retención en el almacén».
   - **En otro rest-server** (uno propio, sin Resguardo), hay que hacerla en él (`restic forget --prune` con acceso directo a su carpeta), como siempre se hizo con la app de escritorio.
4. **¿Prefieres un repositorio nuevo y traer el historial?** (p. ej. para pasar a un destino distinto, o juntar varios repositorios en uno). En la página del repositorio, **«Traer historial»**: el equipo copia (`restic copy`) las versiones de otro repositorio (o de uno suyo, como uno importado) en segundo plano; puedes cerrar la ventana y el resultado sale en sus órdenes. Solo añade: no borra nada en ninguno de los dos, sirve con destinos de solo añadir y, si se repite, no copia dos veces lo mismo. Si el origen tiene versiones de varios equipos, puedes elegir de cuáles.
   - Para que lo traído no ocupe el doble, crea el repositorio nuevo con la opción avanzada **«Para traer el historial de otro repositorio»** de «Nuevo repositorio»: nace con los mismos parámetros de troceado que el origen (`restic init --copy-chunker-params`). Si no, cada uno trocea los archivos a su manera y la deduplicación entre lo traído y lo nuevo es parcial.
   - También en un **almacén** («Este equipo guarda copias»): en «Nuevo repositorio», elige «Almacén …» como destino, abre «Para traer el historial de otro repositorio» y escribe la dirección y la contraseña del antiguo (p. ej. `http://192.168.1.30:8001/Siigo`). «Copiar en …» crea el repositorio en el almacén con el troceado del antiguo y, al terminar, abre «Traer historial» con ese origen ya puesto (solo falta su contraseña; la dirección se recuerda en ese navegador, nunca la contraseña). Resultado: el repositorio nuevo tiene todas las versiones, ocupa lo mismo que el original y las copias nuevas aprovechan todo lo anterior. **El original no se toca**: cuando ya no lo necesites, puedes apagar el rest-server antiguo y borrarlo a mano.
   - El usuario y la contraseña del almacén van solo al almacén: al leer del origen, restic no los manda al servidor antiguo.
   - Trae el historial **desde el equipo que tiene acceso a los dos**; origen y destino en dos cuentas distintas del mismo tipo de nube no se pueden copiar a la vez (restic usa una sola clave): pásalo primero a un disco o a un servidor.
5. **Mover un repositorio existente al almacén (sin ocupar más).** Si el repositorio antiguo está **en el mismo disco** que el almacén («Este equipo guarda copias»), se puede mover su carpeta dentro de la del almacén y adoptarlo: no se copia nada, conserva todo su historial y el equipo sigue copiando en él. A diferencia del punto 4, **el original deja de estar donde estaba**.
   - **Antes:** el equipo cliente ya tiene que copiar en ese almacén («Copiar en …»), para tener su usuario. El almacén guarda cada repositorio en `<carpeta del almacén>/<usuario>/<repositorio>`: la consola lo dice en «Usar uno que ya existe» → «Dónde está: Almacén … (ya configurado)», p. ej. «En ALMACEN-01, la carpeta debe estar en `/mnt/restic/resguardo/servidor-01/siigo`».
   - **Nunca dos rest-servers sirviendo la misma carpeta**, y **cualquier otro proceso que use la ruta antigua** (un script que la sube a Backblaze, una tarea de `restic forget`, la app de escritorio) hay que cambiarlo a la ruta nueva o apagarlo. Ojo si la carpeta del almacén está dentro de la que servía el rest-server antiguo (p. ej. `/mnt/restic/resguardo` dentro de `/mnt/restic`): ese servidor antiguo daría acceso a todo el almacén. Apágalo.
   - **Linux** (como root; el rest-server del almacén corre como root, así que basta con que todo sea de root y solo para root):

     ```text
     systemctl list-units --all | grep -i rest        # el rest-server antiguo (no resguardo-guarda-copias)
     systemctl disable --now <unidad-del-antiguo>.service
     mkdir -p /mnt/restic/resguardo/<usuario>
     mv /mnt/restic/<repositorio> /mnt/restic/resguardo/<usuario>/<nombre>
     chown -R root:root /mnt/restic/resguardo/<usuario>/<nombre>
     chmod -R go-rwx /mnt/restic/resguardo/<usuario>/<nombre>
     ```

     Si el rest-server antiguo no era un servicio, búscalo con `ps aux | grep rest-server` y páralo. `mv` dentro del mismo sistema de archivos es instantáneo; si `df` dice que son discos distintos, copiaría los datos (entonces mejor el punto 4).
   - **Windows** (PowerShell como administrador; la carpeta del almacén es solo para SYSTEM y Administradores): para el rest-server antiguo (su servicio o tarea programada) y mueve la carpeta dentro de la del almacén, en el **mismo disco**; después, que herede los permisos del almacén:

     ```text
     Move-Item "D:\Copias\Siigo" "D:\Resguardo\Copias\<usuario>\<nombre>"
     icacls "D:\Resguardo\Copias\<usuario>\<nombre>" /reset /T /C /Q
     ```

   - **Después**, en la consola: «Nuevo repositorio» → «Usar uno que ya existe» → equipo → «Dónde está: Almacén … (ya configurado)». El almacén muestra los repositorios que encuentra en la carpeta de ese equipo («Repositorios encontrados en tu carpeta de …», unos minutos después de moverlo): elige el suyo (o escribe `<nombre>`), escribe su contraseña, «Probar» y «Usar este repositorio». Por dentro es `adoptar_repositorio` con `destino: { id }` (el del almacén) y `ruta: "<nombre>"`.

Seguridad: las dos órdenes piden la clave de administración, como crear un repositorio. La dirección, las credenciales y las contraseñas viajan selladas solo para el equipo (el servidor no las ve ni las guarda) y los resultados no llevan rutas ni secretos. La respuesta de «Probar» (con los nombres de los equipos) llega sellada solo para el navegador que la pidió.

## Si pierdes la consola

**Lo mejor es no depender de una sola consola**: con «Conectar también a otra consola…» (cliente → Servidor) los equipos se gestionan a la vez desde dos (por ejemplo, la del cliente y una en línea), y si una se pierde la otra sigue sin hacer nada. Ver [consolas-multiples.md](consolas-multiples.md). Una consola que ya no existe se quita desde la otra o, en el equipo, con `resguardo-agente consolas quitar <n.º | dirección>`.

Si el Resguardo Server se pierde (la máquina muere, se borra el disco…), **los equipos siguen copiando**: tienen su configuración, y el almacenamiento y la nube no dependen del servidor. Solo dejas de poder gestionarlos. Lo único imprescindible es la **clave de administración** del cliente (la que se eligió con el primer equipo). Hay tres caminos ([plataforma.md](plataforma.md), §3.5; detalle técnico en [api-servidor.md](api-servidor.md), §11):

1. **Servidores de respaldo (lo mejor: prepáralo antes).** En otro servidor (un segundo Resguardo Server o tu servicio en línea), «Recibir un cliente» con el bloque de «Mover este cliente a otro servidor» y «Dar una ficha». En el servidor de siempre, en el cliente, añade ese servidor como respaldo (orden `servidores_respaldo`, hasta 3, con la clave de administración). Si el principal no responde en N días (3 por defecto, de 1 a 30), cada equipo se pasa solo al primer respaldo que le acepte, con su misma configuración. No hay que hacer nada en los equipos.
2. **Restaurar el servidor con su «Copia de la consola» (lo mejor si el servidor es tuyo: prepáralo antes).** En la consola, como propietario del servidor, **Servidor → Copia de la consola**: elige la **clave de respaldo de la consola** e imprime su kit. Desde entonces, cada noche (y al actualizar) el servidor guarda una copia cifrada de sí mismo (cuentas, clientes, equipos, historial, `identidad.key` y su autoridad TLS) en `respaldos/` dentro de su carpeta de datos (`C:\ProgramData\Resguardo Server\respaldos` o `/var/lib/resguardo-server/respaldos`). El servidor no conoce la clave: puede hacer las copias, pero no abrirlas.
   - **Sácalas de la máquina:** añade esa carpeta a una copia del agente de esa misma máquina (en su equipo, «Cambiar las copias» → «Añadir una copia», por ejemplo «Consola de Resguardo») o guárdalas en un almacén; si esa máquina es también el almacén, en un repositorio de su propio almacén («Copiar en este mismo almacén»), que así entra en su espejo y en la nube. Es una copia más: restic, retención, copia externa y espejo, como siempre.
   - **Si la máquina se pierde:** instala Resguardo Server en otra con la **misma dirección** (nombre o IP), recupera el archivo `consola-….resguardo-consola` más reciente (restaurándolo de esa copia, en cualquier equipo, con su kit de recuperación) y, con el servicio parado y como administrador:

     ```text
     resguardo-server restaurar-respaldo consola-AAAAMMDD-HHMMSS-mmm.resguardo-consola
     ```

     Antes de pedir nada enseña la identidad del servidor que hizo la copia (y comprueba su firma): confirma que es la del kit (o añade `--confiar-en <huella del kit>`, como trae la línea del kit). Escribe la clave de respaldo del kit. Si en esa carpeta ya hay un servidor (recién instalado), añade `--reemplazar`: lo que había se aparta en `antes-de-restaurar-…`, no se borra. Arranca el servicio: los equipos reconocen su identidad y vuelven solos, sin vincularlos otra vez.
   - Sin esa copia, también vale una copia de la carpeta de datos entera hecha con el servicio parado (la base SQLite, `identidad.key` y la carpeta `tls`).
3. **Volver a vincular uno a uno.** En un servidor nuevo, crea el cliente (o recíbelo, si guardaste su bloque) y añade cada equipo como uno nuevo: genera el código y, en el equipo, como administrador:

   ```text
   resguardo-agente vincular <código> --servidor https://servidor-nuevo:8443
   ```

   Comprueba el número de comprobación y escribe **la misma clave de administración** de siempre. El equipo conserva todo (repositorios, copias, contraseñas, Servidor de copias, espejo y nubes) y la consola nueva lo ve en cuanto llega el alta. Hasta entonces el equipo sigue como estaba y el servidor nuevo no recibe nada: **un código solo no basta** para quedarse con un equipo ya configurado. `resguardo-agente estado` muestra «Pendiente del alta en …»; si no llega en 7 días, se olvida.

   - Si la consola dice «Este equipo ya tiene otra clave de administración», la clave no es la de ese equipo. Repite con la buena.
   - Para dárselo a **otro cliente** (otra clave), en el equipo: `resguardo-agente vincular <código> --servidor <url> --empezar-de-cero`. Olvida el servidor, la clave y las copias que este gestionaba (lo ya copiado se queda en sus destinos; el Servidor de copias, el espejo y las nubes no se tocan) y se configura de nuevo desde la consola.

**El historial no se pierde.** Cada equipo guarda para siempre lo que cuenta a la consola (cada vuelta de sus copias con sus cifras y sus ganchos, verificaciones, pruebas de restauración, copias externas, el espejo y sus avisos; los últimos 12 meses con detalle y lo anterior resumido por día, sin rutas). Cuando llega a una consola nueva por cualquiera de los tres caminos, se lo da entero: la Historia de cada repositorio y los avisos (ya vistos) aparecen con lo de antes, no solo con lo que pase desde ese día.

Mientras no hay servidor, en cada equipo: `resguardo-agente estado`, `copias`, `copiar-ahora`, `versiones`, `restaurar` y `kit` siguen funcionando en local.

## Despliegue

- `npm run build:agente` compila `resguardo-agente.exe` y genera `Resguardo-Agente_<versión>_x64-setup.exe` en `src-tauri/target/release/bundle/nsis` (junto al de la app), con `packaging/windows/agente.nsi`. Deja una copia en `src-tauri/resources/agente/`, que la app incluye.
- `npm run build:todo` hace lo anterior y después `tauri build` con `RESGUARDO_AGENT_INSTALLER_SHA256`: la consola comprueba esa huella antes de «Guardar el instalador del agente…».
- El instalador fija la huella de `resguardo-agente.exe` (`AGENT_SHA256`) y la comprueba con `certutil` tras copiarlo.
- Instala para todo el equipo (administrador): el programa y `restic.exe` en `Program Files\Resguardo Agente`, el servicio `ResguardoAgente` (arrancado) y, si se elige, la bandeja (`HKLM\…\Run`, una por sesión).
- Página «Emparejar»: el código (solo letras, cifras y guiones) se pasa a `--pair <código> --result <archivo>`; el número de comprobación se muestra en grande. Al actualizar, si ya está emparejado, no se pide.
- Con Resguardo Server, la página pide también su dirección y se vincula con `vincular <código> --servidor <url> --result <archivo>`; si falla, muestra el motivo y qué hacer (código caducado, sin respuesta, certificado distinto…).
- Sin ventanas: `/S /CODE=… [/SERVIDOR=https://…] [/TRAY=0]`. Código de salida 2 si el emparejamiento falla, 5 si Windows es anterior a Windows 10 / Server 2016; el número queda en `emparejamiento.txt` (carpeta del programa, solo administradores).
- Al actualizar, si el equipo guarda copias de otros, el instalador para la tarea del Servidor de copias y su rest-server (por su PID), reemplaza `rest-server.exe` y vuelve a arrancar la tarea.
- Desinstalar: quita el servicio, la bandeja, el Servidor de copias (rest-server, su tarea y su regla del cortafuegos; las copias guardadas se quedan) y pregunta si se borran los datos (`--agent-purge`); si también está la app completa, los datos se conservan siempre.
- `resguardo-agente --help` muestra las órdenes en la terminal; `resguardo-agente estado`, un resumen del equipo.
- En un equipo gestionado no se crea la tarea programada del agente: su ciclo lo lanza el servicio cada 5 minutos.

## Servidor: certificado y retención

- El Servidor de copias tiene una **autoridad propia** (lo que fijan los equipos) y un certificado del servidor firmado por ella con sus IP. Si cambia la IP, `--server-run` vuelve a emitir el del servidor (misma autoridad) y reinicia rest-server; la consola manda a cada equipo su ubicación nueva, firmada.
- Retención por equipo (`frecuente` por defecto, `equilibrada`, `ligera` o `todo`): la consola ejecuta `forget --prune` directamente sobre la carpeta del equipo en el servidor (el rest-server es de solo añadir). El agente de la consola lanza `resguardo.exe --managed-maintenance` una vez por semana por equipo.
- Estado de cada equipo en la consola: última versión, número de versiones y tamaño, mirando solo los archivos de su carpeta en el servidor (sin abrir el repositorio).
