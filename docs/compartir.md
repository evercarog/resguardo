# Compartir destinos entre tus equipos

Nota de diseño de las fases 3 y 4:

- **Fase 3** comparte un destino de la nube o un rest-server con tus otros equipos, con cifrado de extremo a extremo.
- **Fase 4** convierte un equipo Windows en el «Servidor de copias de Resguardo».

Vocabulario: ver [destinos.md](destinos.md).

## Principios

1. **Se comparte el lugar, nunca un repositorio.** Se entregan las credenciales del destino: la clave y el bucket, o el usuario del rest-server. Las contraseñas de los repositorios no salen nunca del equipo que las creó. Cada equipo crea su propio repositorio, con su propia contraseña, o usa uno existente si escribe su contraseña.
2. **La web solo transporta texto cifrado.** Las credenciales se cifran en el equipo que comparte, para la clave pública del equipo que las pide. La web nunca ve el texto en claro.
3. **Todo queda a la vista.** Cada entrega genera un aviso en los dos equipos y una entrada en la historia del destino.
4. **Se puede revocar,** y se explica qué significa: lo ya entregado solo deja de valer si se rota la clave en el proveedor.

## Fase 3: compartir un destino de la nube o un rest-server

### Para quién y cómo se ve

En la página de un destino de la nube (S3, B2, Azure, GCS) o de un rest-server, hay un interruptor: **«Compartir este destino con mis equipos»**.

- Viene desactivado. Activarlo pide abrir Resguardo como administrador y la contraseña de uno de los repositorios del destino. Eso prueba que quien lo activa tiene autoridad sobre ese lugar.
- Debajo, la lista de los equipos que ya lo recibieron, con su fecha.

En otro equipo de la misma cuenta, con la sesión de «Todos mis equipos» iniciada (aal2):

1. En **Añadir repositorio** aparece **«Usar un destino compartido»**. Lista los destinos que comparten tus otros equipos: tipo, servidor, bucket o ruta base, equipo de origen y fecha.
2. Al elegir uno, se pide. El equipo de origen lo entrega en su siguiente ciclo, normalmente en menos de 5 minutos.
3. Al llegar, aparece como destino en la barra lateral, con «Crear uno nuevo aquí». La ruta sugerida es `<bucket>/<equipo>/<nombre>`, con una contraseña generada que va al kit. También tiene «Usar uno existente», que pide la contraseña de ese repositorio.

**Aviso entre clientes.** Si el destino compartido parece de otro cliente (el nombre o el bucket no coinciden con los del equipo que lo recibe), se recomienda: «Un bucket y una clave por cliente (o un usuario de rest-server por cliente)». Una clave de bucket compartida puede, técnicamente, borrar objetos. Object Lock o el modo append-only lo mitigan.

### Ayuda: el esquema

```
Backblaze B2 · copias-ana  (una clave)      ← destino compartido
├── siigo/      (contraseña propia, solo la conoce «Siigo»)
├── altamar/    (contraseña propia, solo la conoce «Altamar»)
└── portatil/   (contraseña propia, solo la conoce «Portátil»)
```

### Criptografía

- **Claves de cada equipo.** Cada equipo genera un par X25519 al vincularse con la web o al activar la función.
  - La privada vive en los secretos del agente: DPAPI de máquina, en un archivo que solo leen SYSTEM y los administradores.
  - La pública se publica con `device_set_key`.
- **Cifrado.** Sobres sellados compatibles con libsodium (crate `crypto_box`, feature `seal`):
  - X25519 efímero + XSalsa20-Poly1305;
  - el remitente es anónimo y la integridad va autenticada.
- **Qué se cifra:**

  ```json
  { "v": 1, "share": "<id>", "kind": "s3|b2|azure|gs|rest",
    "base": "s3:https://host/bucket", "region": "…",
    "key_id": "…", "key_secret": "…",
    "rest_user": "…", "rest_password": "…",
    "issued_at": "RFC 3339", "to_device": "<id>" }
  ```

  - Solo van los campos de ese tipo.
  - `to_device` y `share` van dentro del sobre: al abrirlo se comprueba que son los esperados. Así no se puede reenviar un sobre a otro equipo ni hacerlo pasar por otro destino.

### Flujo

1. **Compartir (equipo A, administrador y contraseña de un repositorio del destino).**
   - La app copia las credenciales del destino a los secretos del agente (`shared_places[place_id]`).
   - Llama a `place_share_set` con los metadatos. No se envía ningún secreto.
2. **Pedir (equipo B, sesión aal2).** `share_request(share_id, device_id)`. La web anota:
   - el equipo que lo pide;
   - el usuario aal2;
   - la hora;
   - `not_before = ahora + 5 min` (se envía un aviso al momento: en ese tiempo se puede cancelar con `share_cancel`);
   - `expires_at = ahora + 24 h`.
3. **Entregar (agente de A, en su ciclo).** `share_pending` devuelve las peticiones maduras (después de `not_before` y antes de que caduquen). Para cada una, el agente comprueba:
   - que el destino sigue compartido localmente;
   - que el equipo que pide no está revocado y lleva vinculado más de 10 minutos (la web devuelve `requester_linked_at`);
   - que hay clave pública.

   Si todo está bien, sella y llama a `share_deliver`. Si no, llama a `share_reject` con el motivo.
   - Anota en su historia: «Entregada la cuenta de «Backblaze · copias-ana» a «Altamar»».
   - Avisa en Windows (y en el buzón).
4. **Recibir (agente de B, en su ciclo).** `share_inbox` devuelve los sobres.
   - Abre cada uno con su privada y comprueba `to_device` y `share`.
   - Lo guarda en sus secretos (`received_places`) y confirma con `share_ack`, que borra el sobre de la web.
   - Anota y avisa: «Altamar recibió la cuenta de nube «Backblaze · copias-ana»».
5. **Usarlo (app de B como administrador).**
   - `shares_received` lista lo recibido.
   - «Crear uno nuevo aquí» o «Usar uno existente» crean un repositorio normal: las credenciales pasan al almacén del usuario, igual que las de cualquier repositorio.
6. **Dejar de compartir (A).**
   - `place_share_set(null)`: la web borra las peticiones y los sobres pendientes y rechaza las nuevas.
   - El agente borra `shared_places[place_id]`.
   - La interfaz explica: «Los equipos que ya la recibieron la conservan. Para revocarla del todo, rota la clave en la consola del proveedor y actualízala aquí.» Lleva un enlace guiado a «Rotar la clave».

### Límites

- **Peticiones (web):**
  - 1 pendiente por par destino-equipo;
  - 10 por usuario y hora;
  - caducan a las 24 h.
- **Entregas (agente):**
  - como mucho 5 por ciclo;
  - una petición rechazada no se reintenta.

### Modelo de amenazas

| Atacante | Qué puede | Mitigación |
|---|---|---|
| La web (o quien la controle) | Ver metadatos (servidor, bucket, nombres de equipos) y quién pide qué. | No ve credenciales: solo sobres sellados para la clave pública de cada equipo. |
| La web publica una clave pública falsa para un equipo | Recibir credenciales destinadas a ese equipo. | La clave pública solo la publica el propio equipo con su secreto de dispositivo. La entrega exige que el equipo lleve más de 10 min vinculado, y cada entrega avisa en los dos equipos. **Riesgo residual:** una web comprometida y activa podría sustituirla. Mejora futura: verificar la huella de la clave en la app («código de 6 cifras») la primera vez. |
| Alguien con tu cuenta (contraseña + TOTP) y un equipo vinculado | Pedir el destino a otro equipo tuyo. | Hace falta el segundo factor y un equipo vinculado más de 10 min. La entrega se retrasa 5 min, con un aviso inmediato para poder cancelarla; avisa también al entregarse en los dos equipos y queda en la historia. **Riesgo residual documentado.** |
| Un administrador del equipo que recibe | Leer las credenciales recibidas. | Es tu equipo: tiene el mismo acceso que darías al escribirlas a mano. Para limitarlo: un bucket o usuario por cliente, claves con permisos mínimos y Object Lock / append-only. |
| Un usuario sin privilegios del equipo | Nada: los secretos del agente solo los leen SYSTEM y los administradores. | — |
| Reenvío de un sobre | Hacerlo pasar por otro destino u otro equipo. | `to_device` y `share` van dentro del sobre y se comprueban al abrirlo. |

### Contrato con la web (fase 3)

**Tablas** (RLS activada; solo las RPC tocan las tablas):

```sql
-- Clave pública X25519 de cada equipo (32 bytes en base64).
create table device_keys (
  device_id uuid primary key references devices(id) on delete cascade,
  public_key text not null,          -- base64, 44 caracteres
  updated_at timestamptz not null default now()
);

-- Destinos compartidos (solo metadatos).
create table place_shares (
  id uuid primary key default gen_random_uuid(),
  owner uuid not null references auth.users(id),
  device_id uuid not null references devices(id) on delete cascade,
  place_id text not null,            -- id local del destino en ese equipo
  kind text not null check (kind in ('s3','b2','azure','gs','rest')),
  host text,                         -- servidor (sin usuario ni contraseña)
  base text not null,                -- bucket o ruta base, para mostrar
  name text not null,                -- nombre del destino
  created_at timestamptz not null default now(),
  revoked_at timestamptz,
  unique (device_id, place_id)
);

-- Peticiones y entregas.
create table share_requests (
  id uuid primary key default gen_random_uuid(),
  share_id uuid not null references place_shares(id) on delete cascade,
  requester_device uuid not null references devices(id) on delete cascade,
  requested_by uuid not null references auth.users(id),
  created_at timestamptz not null default now(),
  not_before timestamptz not null default now() + interval '5 minutes',
  expires_at timestamptz not null default now() + interval '24 hours',
  status text not null default 'pending' check (status in ('pending','delivered','received','rejected','expired','cancelled')),
  ciphertext text check (length(ciphertext) <= 8192), -- base64 del sobre sellado; se borra al confirmar
  reason text,
  delivered_at timestamptz,
  received_at timestamptz
);
create unique index on share_requests(share_id, requester_device) where status = 'pending';
```

**RPC del equipo.** Se autentican como `device_report`, con `p_device` y `p_secret`, y comprueban que el equipo no esté revocado.

| RPC | Parámetros | Devuelve |
|---|---|---|
| `device_set_key` | `p_public_key text` | `void` |
| `place_share_set` | `p_place_id text`, `p_meta jsonb \| null` (`{kind, host, base, name}`) | `{ id }`, o `null` si se deja de compartir. Al dejarlo: `revoked_at = now()`, borra los `ciphertext` y pasa a `rejected` las pendientes. |
| `share_pending` | — | `[{ request_id, share_id, place_id, requester_device, requester_name, requester_public_key, requester_linked_at, created_at }]`: solo las de mis `place_shares` no revocados, con `status = 'pending'`, `now()` entre `not_before` y `expires_at`, y el equipo que pide no revocado. |
| `share_deliver` | `p_request uuid`, `p_ciphertext text` | `void`. Pasa a `delivered` y crea un aviso para el usuario. |
| `share_reject` | `p_request uuid`, `p_reason text` | `void` |
| `share_inbox` | — | `[{ request_id, share_id, ciphertext, from_device_name, meta: {kind, host, base, name} }]` con `requester_device = yo` y `status = 'delivered'`. |
| `share_ack` | `p_request uuid` | `void`. Pasa a `received`, borra el `ciphertext` y pone `received_at`. |

**RPC del usuario.** Usan la sesión de «Todos mis equipos» y exigen `aal2`.

| RPC | Parámetros | Devuelve |
|---|---|---|
| `shares_list` | — | `[{ id, device_id, device_name, kind, host, base, name, created_at }]`: los `place_shares` no revocados de los equipos del usuario. |
| `share_request` | `p_share uuid`, `p_device uuid` | `{ id, not_before }`. Comprueba que los dos equipos son del usuario y que ninguno está revocado, y aplica los límites: 1 pendiente por par y 10 por hora. |
| `share_requests_mine` | — | El estado de mis peticiones: `pending`, `delivered`, `received`, `rejected` (con su `reason`), `expired` o `cancelled`. |
| `share_cancel` | `p_request uuid` | `void`. Solo mis peticiones aún `pending`: pasan a `cancelled`. |

Un trabajo periódico pasa a `expired` las pendientes caducadas y borra su `ciphertext`.

**Informe del equipo** (`device_report`): cada `repos[].place` puede llevar `"shared": true` cuando su destino está compartido. Es solo informativo.

## Fase 4: el Servidor de copias de Resguardo

Se activa en **Ajustes → Este equipo → «Servidor de copias»**. Convierte un equipo Windows en un servidor de copias para una carpeta o un disco elegidos. Los demás equipos lo ven como un destino (un rest-server) con sus repositorios.

### El servicio: binario oficial frente a uno propio

| | (a) rest-server oficial, incluido con hash fijo | (b) protocolo REST mínimo en Rust |
|---|---|---|
| Madurez | Años en producción, el mismo equipo que restic, `--append-only` y `--private-repos` probados. | Código nuevo nuestro: cada error es un riesgo para las copias de otros. |
| Superficie | Un binario Go más (unos 10 MB). Hay que seguir sus versiones de seguridad. | Menos piezas, pero un servidor HTTP con TLS y autenticación escrito desde cero. |
| Funciones | htpasswd (bcrypt), TLS, métricas, límites de tamaño. | Habría que escribirlas y probarlas. |
| Bloqueo por intentos fallidos | No lo trae. | Fácil de añadir. |

**Recomendación: (a), el rest-server oficial.**

- Se incluye en el instalador con su SHA-256 fijado y se comprueba antes de cada arranque.
- Lo lanza un servicio de Windows propio, como LocalSystem, gestionado por el agente.
- Falta el bloqueo por intentos: las credenciales son aleatorias de 128 bits por equipo, así que la fuerza bruta no es viable. Además, el firewall limita quién llega y se puede restringir a la subred.

Lo más delicado de un servidor de copias es que nadie pueda borrar ni leer lo de otro. Eso lo garantiza `--append-only --private-repos` en un código ya auditado por el uso.

### Configuración

- **Siempre** `--append-only --private-repos`: cada equipo solo ve su carpeta `/<usuario>/` y nunca puede borrar.
- **Un usuario por equipo cliente.** Contraseña aleatoria (128 bits), guardada como bcrypt en `.htpasswd`. La recibe el cliente por la entrega cifrada de la fase 3.
- **TLS con un certificado autofirmado** que genera el servidor.
  - Su huella SHA-256 se publica en la web.
  - Los clientes guardan el certificado (no solo la huella) y lo usan con `--cacert`.
  - Si cambia, el cliente avisa y no se conecta hasta que se acepte el nuevo desde la app.
- **Puerto elegido** (8000 por defecto). Una regla del Firewall de Windows solo para ese puerto, con la opción «solo mi red local» (`RemoteAddress=LocalSubnet`).
  - **Por qué 8000.** Es el puerto habitual de rest-server, y el mismo en la app de escritorio, el agente (`guardar-copias activar`), la consola y esta documentación. No se cambia: el puerto de cada equipo se guarda en su configuración, así que cambiar el predeterminado no arreglaría nada en los equipos ya activados y haría que la documentación y las guías dejaran de coincidir.
  - **Si el puerto está ocupado** (otro rest-server, otro programa): el equipo no lo activa y responde «El puerto N ya está en uso en este equipo: elige otro». No para ni toca al programa que lo usa. El diálogo de la consola lo avisa debajo del campo y propone otro (p. ej. 8002).
- **La retención y el `prune` los hace el propio servidor,** que sí puede borrar (los clientes no). Se ejecuta con la política de cada repositorio que acuerde su dueño: ver «Retención en el almacén», abajo.

### Repositorios que publica

El servidor conoce las carpetas de sus repositorios (`/<usuario>/<repo>/`). En su informe a la web publica **solo** los nombres o rutas de los repositorios por usuario, nunca su contenido ni contraseñas. Los equipos de la cuenta lo ven como destino con su lista de repositorios:

- **«Usar»** pide la contraseña de ese repositorio;
- **«Crear uno nuevo»** crea uno en la carpeta de ese equipo.

### Alcance

- **Red local:** funciona directamente (`rest:https://<ip-local>:<puerto>/<usuario>/`).
- **Entre sedes:**
  - La interfaz explica que hay que abrir **un puerto** en el router y muestra la dirección pública que se usaría.
  - Explica también la seguridad: TLS, credenciales por equipo, append-only y firewall.
  - Resguardo **nunca abre puertos solo** (sin UPnP).

### Implementado y lo que falta para publicarlo

- `server.rs`: configuración en `servidor.json`; certificado propio con `rcgen` (la clave, en la carpeta privada); `.htpasswd` con bcrypt; regla del firewall con `netsh`; tarea de SYSTEM «Resguardo Servidor de copias» al arrancar (`resguardo.exe --server-run`), que comprueba el hash, relanza el rest-server si se cae y lo para por su PID. El instalador lo para y quita su tarea al desinstalar o actualizar (la versión nueva la vuelve a crear).
- **Para publicarlo hace falta:**
  1. Añadir el rest-server oficial (la versión elegida, para `x86_64-pc-windows-msvc`) en `externalBin` de `tauri.conf.json` (`binaries/rest-server`, junto a restic).
  2. Compilar con `RESGUARDO_REST_SERVER_SHA256=<sha256 de ese rest-server.exe>`. Sin esa variable, la app dice «Esta versión de Resguardo no incluye el servidor de copias» y no arranca nada.
- Pendiente: reutilizar el puerto o el certificado si cambian las IP de la red local (hoy se regenera al volver a activar). El `prune` en el servidor ya está: «Retención en el almacén».

### Retención en el almacén (v1.22)

**El problema.** Los equipos copian en el almacén con `--append-only`: un equipo comprometido no puede borrar nada, ni lo suyo antiguo. Pero por eso mismo «Aplicar la retención» desde el equipo falla (403) y el repositorio crece sin límite. Alguien tiene que podar, y solo puede hacerlo quien tiene acceso directo a la carpeta: el almacén. El almacén, en cambio, **no tiene la contraseña** de ningún repositorio (no ve los datos), y `restic forget --prune` la necesita: los índices y las versiones van cifrados.

**Opciones que se miraron:**

| | Cómo | Lo que se gana | Lo que se pierde |
|---|---|---|---|
| (a) Contraseña para el almacén | La consola manda al almacén, sellada y con la clave de administración, una clave de restic **propia del almacén** (no la del equipo), que el equipo dueño añade al repositorio. El almacén poda en local, a su hora. | La protección de solo añadir sigue entera: el equipo nunca puede borrar. Funciona con el equipo apagado. Rápido (disco local). | El almacén puede **leer** esos repositorios. |
| (b) Ventana de mantenimiento | Con una orden firmada, el almacén abre N minutos un acceso sin solo añadir (un segundo rest-server o un usuario con permiso de borrar) y el equipo poda a distancia. | El almacén sigue sin ver nada. | Durante cada ventana, un equipo comprometido puede borrar **todo** su historial; con retención programada (cada semana) la ventana se abre sola y un ransomware solo tiene que esperarla. Otro puerto o servicio que abrir (router, túnel, cortafuegos), y el `prune` por la red es lento. |
| (c) Podar sin contraseña | El equipo decide qué sobra y el almacén borra archivos por nombre. | — | El almacén no puede comprobar qué borra (va cifrado): un equipo comprometido le haría borrarlo todo. Además depende de las tripas de restic. |

**Se eligió (a).** Lo que protege el solo añadir es el caso más probable (un equipo con ransomware o un administrador malicioso en un equipo), y (a) no le da ninguna vía para borrar; (b) se la daría justo de forma programada. La autoridad para lo destructivo sigue siendo la clave de administración, con su espera.

**Cómo funciona:**

1. En la consola, en la página del repositorio, **«Retención en el almacén»**: la regla (cuántas diarias, semanales, mensuales y anuales), el horario (p. ej. los domingos a las 03:00) y si comprobar después (`restic check`). Se confirma con la contraseña del repositorio y la clave de administración.
2. La consola genera al azar una clave de restic para el almacén (32 bytes) y manda tres órdenes, todas selladas:
   - al equipo dueño, `cambiar_retencion` (la regla queda guardada en el repositorio, como siempre; espera) y `clave_almacen` (`restic key add` con esa clave: es una escritura, así que vale con solo añadir);
   - al almacén, `retencion_almacen` con la regla, el horario y la clave (clave de administración; **espera** como todo lo destructivo, y todos los usuarios del cliente lo ven y pueden cancelarla).
3. Desde ahí el almacén la aplica **solo**, en cada hueco del horario, en local sobre `<carpeta>/<usuario>/<repo>`: decide qué versiones sobran con las reglas de `restic forget --keep-*` (por equipo y carpetas), las quita por id (`restic forget <ids>` y `restic prune`) y, si se pidió, `restic check`. **Sin fiarse de la hora de cada versión**: la escribe el equipo, y uno comprometido podría añadir versiones falsas con fecha futura o con la última hora de cada día, semana, mes y año pasados para que la regla borrase las de verdad (lo único que el solo añadir no frena por sí solo). Por eso solo cuentan las versiones cuya hora cuadra con su subida al almacén (la fecha del archivo `snapshots/<id>`, que escribe el rest-server del almacén): entre 48 h antes y 24 h después. Las demás no se quitan ni desplazan a ninguna (el resultado dice cuántas hay): las traídas de otro repositorio con «Traer el historial» o un repositorio antiguo copiado al almacén sin conservar las fechas de sus archivos se quedan enteras. Lo que aún puede hacer un equipo comprometido es lo de siempre: copiar basura durante más tiempo que la regla (o dentro de ese margen de 48 h), y así desplazar las versiones buenas de esos días; las semanales, mensuales y anuales anteriores siguen. Si estaba apagado a esa hora, al encenderse (una vez). La consola muestra «La aplica el almacén ALMACEN-01 los domingos a las 03:00», el último resultado y la próxima vez. **«Aplicar ahora»** (`aplicar_retencion_almacen`) también espera.
4. Cambiar la regla o el horario es otra `retencion_almacen` (espera); con otra clave, el almacén borra la anterior del repositorio en cuanto la nueva abre. **«Dejar de aplicarla»** (`quitar`) no espera: el almacén olvida la clave y **borra su archivo del repositorio** (`keys/<id>`), con lo que deja de poder abrirlo.

**La contrapartida, dicha claro.** Con la retención en el almacén activada para un repositorio, quien controle el almacén (su administrador, o un atacante con SYSTEM/root en él) puede **leer** ese repositorio con la clave del almacén. Antes solo podía borrarlo (es su disco). Se limita así:

- es opcional y por repositorio: sin activarla, el almacén sigue sin poder leer nada (y la retención hay que hacerla a mano, como antes);
- la clave es **propia del almacén** y distinta de la del repositorio: no sirve para autorizar órdenes en el equipo ni está en el kit; se revoca quitando la regla (o, a mano, borrando su `keys/<id>`), sin cambiar la contraseña del repositorio;
- se guarda protegida como los demás secretos del agente (DPAPI de equipo en Windows, solo root en Linux) y nunca sale del almacén: el resumen solo lleva usuario, repositorio, regla, horario y resultados;
- la autorizan la clave de administración y la contraseña del repositorio, con la espera de lo destructivo y el aviso a todos los usuarios;
- si el almacén no es de confianza para leer, no se activa: la copia externa cifrada o un almacén propio de cada cliente siguen siendo la alternativa.

**Repositorios grandes.** `forget --prune` toma el bloqueo exclusivo (`--retry-lock 30m`): si a esa hora un equipo está copiando, espera a que termine; si una copia empieza mientras se poda, espera ella. Conviene poner el horario fuera de las horas de copia.

**Plazos (v1.28).** La regla también puede ser por plazos, como `restic forget --keep-within-hourly 15d --keep-within-daily 1y --keep-monthly unlimited` («Programas contables: horarias 15 días, diarias 1 año, mensuales siempre»). El almacén los aplica con las mismas reglas que restic (comprobado contra `restic forget --dry-run` con reglas al azar) y con la misma desconfianza: el plazo cuenta hacia atrás desde la versión más reciente **de confianza** (una falsa con fecha futura es sospechosa: ni cuenta ni mueve el plazo, así que no puede «envejecer» las buenas para que caigan fuera). Lo que puede hacer un equipo comprometido sigue siendo lo de antes: dentro del margen de 48 h, subir versiones con horas de esos días, que desplazan a las buenas de esas mismas horas o días; lo anterior al plazo no se toca por ello.

**Verificación.** Con «Comprobar el repositorio después», el almacén hace `restic check` (la estructura, con su clave) tras cada poda. Es distinta de la **verificación automática** del equipo dueño (v1.28: cada N días, un porcentaje de los datos que va rotando, con la contraseña del repositorio por el rest-server): la del equipo lee los datos poco a poco; la del almacén comprueba que la poda dejó el repositorio sano. La consola enseña las dos en la página del repositorio.

### Un repositorio en su propio almacén (v1.28)

**Para qué.** Lo que vive en la máquina del almacén (por ejemplo, la carpeta `C:\ProgramData\Resguardo Server\respaldos` con las copias de la consola, si Resguardo Server está en el mismo equipo) no podía ir al almacén: el agente no se ofrecía a sí mismo como destino. Ahora, en la ficha del almacén, **«Copiar en este mismo almacén»** (o «Su propio almacén» en «Nuevo repositorio») crea un repositorio en su propio almacén. Así entra en el **espejo** (otro disco, Dropbox) como lo de los demás equipos, y su retención se aplica igual («Retención en el almacén»).

**Cómo.** Lo mismo que «Copiar en …» para otro equipo: `guarda_copias { anadir, local: true }` al almacén crea un usuario más del rest-server (con su contraseña, sellada para la consola) y da la dirección `https://localhost:<puerto>/<usuario>/` (el certificado del almacén ya cubre `localhost`; no depende de la IP ni del cortafuegos); después, `crear_repositorio` en el mismo equipo con ese destino.

**Seguridad.**

- El repositorio va por el rest-server como cualquier otro: **solo añadir** y `--private-repos` también para el usuario del propio almacén. Por ese camino, ni el agente del almacén ni un ransomware que use su usuario pueden borrar lo ya copiado (comprobado en la prueba real con rest-server: `forget` falla).
- Pero el almacén **es** esa máquina: quien tenga SYSTEM/root en ella puede borrar la carpeta del almacén entera, este repositorio incluido, igual que los de los demás equipos. El solo añadir protege de un equipo comprometido, no de un almacén comprometido. Por eso esta copia solo saca los datos de la máquina si hay **espejo** (que solo añade en su destino) o copia externa; sin espejo, sigue en el mismo disco. La consola lo dice al ofrecerla.
- No da al almacén nada que no tuviera: los datos ya eran suyos (están en su disco), la contraseña del repositorio la tiene el agente de esa misma máquina (como la de cualquier repositorio suyo) y el kit se imprime como siempre.
- Con «Retención en el almacén», el almacén poda su propio repositorio con su clave propia, con la misma espera y la misma desconfianza hacia las horas de las versiones.

### Servidores rest-server de terceros

No se pueden listar. Resguardo recuerda las rutas conocidas (`Place.known`, fase 2) y deja añadir una ruta existente o crear una nueva.

### Contrato con la web (fase 4)

Nuevo campo opcional `server` en `device_report`, solo si el servidor está activo:

```json
"server": {
  "port": 8000,
  "lan_addresses": ["192.168.1.20"],
  "public_hint": "203.0.113.7",
  "tls_sha256": "AB:CD:…",
  "local_subnet_only": true,
  "users": [
    { "user": "altamar", "device_id": "uuid|null", "repos": ["portatil", "documentos"] }
  ]
}
```

- `public_hint` es opcional: solo se envía si el usuario lo consulta.
- Las credenciales de cada usuario se entregan con `share_deliver`. El sobre lleva `kind: "rest"`, `base: "rest:https://ip:puerto/usuario/"`, `rest_user`, `rest_password` y `tls_cert_pem`.

### Modelo de amenazas (fase 4)

| Atacante | Mitigación |
|---|---|
| Alguien en Internet con el puerto abierto | TLS, contraseñas aleatorias de 128 bits, append-only y `--private-repos`. Puerto abierto solo si el usuario lo hace. Firewall limitado a la subred por defecto. |
| Un equipo cliente comprometido | Solo puede añadir a su propia carpeta: no lee ni borra la de otros, ni las suyas antiguas. |
| Suplantación del servidor | Certificado fijado en cada cliente (`--cacert`). Si cambia, se avisa. |
| Un administrador del servidor | Tiene los datos cifrados, pero no las contraseñas de los repositorios: no puede leerlos. Sí podría borrarlos (es su disco). Para eso está la copia externa. **Excepción (v1.22):** los repositorios con «Retención en el almacén» puede leerlos con la clave propia del almacén (ver arriba). |
| Un equipo cliente comprometido, con la retención en el almacén | Nada nuevo: sigue sin poder borrar. Ve la clave del almacén cuando la añade, pero una clave de restic solo sirve para descifrar (lo que ya puede con la suya); borrar lo impide el rest-server en solo añadir, que no cambia. No puede pedir al almacén que pode: eso exige la clave de administración. |
