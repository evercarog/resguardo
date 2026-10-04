# API de Resguardo Server (v1)

Contrato entre Resguardo Server, la **consola** (navegador) y los **agentes**. Lo implementa `crates/servidor`. Diseño general: [plataforma.md](plataforma.md).

Estado: **v1 en construcción (fase F1)**. Los cambios incompatibles se anotarán al final, en «Cambios».

---

## 0. Convenciones

- **Base:** `https://<servidor>:<puerto>`. Todo es JSON (UTF-8), salvo los trozos del relé (binario).
- **Errores:** código HTTP adecuado y cuerpo `{ "error": "<código>", "mensaje": "<texto para el usuario, en español>" }`.

  | Código | HTTP | Cuándo |
  |---|---|---|
  | `sin_sesion` | 401 | No hay sesión |
  | `necesita_totp` | 401 | Falta el segundo paso |
  | `prohibido` | 403 | El rol no lo permite |
  | `no_existe` | 404 | No existe |
  | `conflicto` | 409 | Por ejemplo, un `seq` ya usado |
  | `datos` | 422 | Datos no válidos |
  | `demasiados_intentos` | 429 | Límite de intentos |
  | `interno` | 500 | Error del servidor |

- **Fechas:** RFC 3339 con zona (`2026-10-02T10:00:00-05:00`).
- **Ids:** cadenas opacas (UUID v4).
- **Binario en JSON:** base64 estándar con relleno (`b64`).
- **Sesión de la consola:**
  - cookie `resguardo_sesion_<6 caracteres de la identidad del servidor>` (`HttpOnly`, `Secure`, `SameSite=Strict`, `Path=/`): dos servidores en el mismo nombre de equipo no se pisan la sesión. El navegador la envía solo; la consola no la lee;
  - **todas las peticiones que cambian algo** (POST, PUT, PATCH, DELETE) desde el navegador deben llevar la cabecera `X-Resguardo: 1` (defensa CSRF; el servidor comprueba además `Origin`).
- **Agentes:** cabecera `Authorization: Equipo <equipo_id>:<secreto_b64>`. El servidor solo guarda `SHA-256(secreto)`.
- **Clientes:** casi todo cuelga de `/api/clientes/{cliente}`. El servidor comprueba en cada petición que la cuenta pertenece al cliente (y su rol).

### Roles

| Rol | Puede |
|---|---|
| `propietario` | Todo, incluidos los usuarios del cliente y sus ajustes |
| `administrador` | Todo menos usuarios y ajustes del cliente |
| `tecnico` | Leer (también la auditoría, sin exportarla), órdenes inofensivas y protegidas, **salvo** `baja_equipo`, `desvincular`, `cambiar_servidor`, `cambiar_clave_admin`, `servidores_respaldo`, `anadir_consola` y `quitar_consola` |
| `lectura` | Solo leer (salvo la auditoría) |

---

## 1. Criptografía que comparten la consola y los agentes

Implementada en `crates/protocolo`, con vectores en `crates/protocolo/vectors/`. La consola debe reproducirla exactamente (y pasar los vectores).

| Concepto | Definición |
|---|---|
| Sobre sellado | `crypto_box_seal` de libsodium (X25519 + XSalsa20-Poly1305) para la clave pública X25519 del equipo. En JSON, base64. |
| Clave de administración | Se normaliza a **Unicode NFC** antes de Argon2id (`clave.normalize("NFC")` en la consola, `derivaciones::normalizar_clave` en Rust). Vector `derivaciones.nfc` |
| `prueba_e` (prueba de administración para el equipo `e`) | `Argon2id(NFC(clave_admin)_utf8, sal_e, m=65536 KiB, t=3, p=1, salida 32 bytes)` |
| `verificador_e` | `SHA-256(prueba_e)`: lo guarda solo el equipo |
| `K_cfg` | `HKDF-SHA256(ikm = Argon2id(clave_admin_utf8, sal_cliente, mismos parámetros), salt = "", info = "resguardo-kcfg-v1", 32 bytes)` |
| `K_exp` | Igual, con `info = "resguardo-kexp-v1"` |
| Etiqueta del equipo | `HMAC-SHA256(K_cfg, "resguardo-etiqueta-v1|" + equipo_id + "|" + box_pub + "|" + sign_pub)`, en base64 |
| SAS v2 | `SHA-256("resguardo-sas-v2|" + identidad_servidor + "|" + box_pub + "|" + sign_pub)`; los 4 primeros bytes big-endian mod 1 000 000, como `"NNN NNN"`. Solo para agentes anteriores a 0.7.10 |
| SAS v3 (v1.26) | Igual, con `SHA-256("resguardo-sas-v3|" + identidad_servidor + "|" + box_pub + "|" + sign_pub + "|" + huella_ca)`. `huella_ca`: la huella SHA-256 del certificado de la autoridad TLS, solo sus cifras hexadecimales en mayúsculas (sin `:`); vacía si no hay. El equipo pone la de la autoridad que **fijó** al vincular; la consola, `huella_ca` de `GET /api/servidor`. Alguien en medio que haga fijar su autoridad da otro número aunque reenvíe intactas la identidad y las llaves. Vector `derivaciones.sas_v3` |
| Prueba de identidad del servidor | Ed25519 del servidor sobre `"resguardo-servidor-v1|" + reto + "|" + equipo_id` (`reto`: 32 bytes aleatorios del agente, en base64) |
| `prueba_codigo` (solo en `alta`) | `HMAC-SHA256(clave = código de emparejamiento normalizado (letras y cifras en mayúsculas), "resguardo-alta-v1|" + equipo_id + "|" + verificador_b64)`, en base64. El agente solo acepta el primer `alta` con ella: el servidor no conoce el código (solo su hash) |
| Configuración cifrada | base64 de `nonce(24) ‖ XChaCha20-Poly1305(K_cfg, nonce, json, aad = "resguardo-config-v1|" + equipo_id + "|" + seq)`. Atada al equipo y a su número: el servidor no puede mostrar la de otro equipo ni una antigua. No lleva contraseñas ni claves (se quedan en el equipo) |
| Plantilla de copia (v1.20) | `K_pla = HKDF-SHA256(ikm = K_cfg, salt = "", info = "resguardo-kplantilla-v1")`; base64 de `nonce(24) ‖ XChaCha20-Poly1305(K_pla, nonce, json, aad = "resguardo-plantilla-v1|" + cliente_id + "|" + plantilla_id)`. El JSON: `{ v: 1, id, nombre, copia: { carpetas, exclusiones, horario, solo_si_cambios, gancho }, retencion, creada }`. Vector `plantilla` en `v1.json`. El servidor no puede leerla, cambiarla de id ni pasarla a otro cliente |
| Claves de una sesión | `HKDF-SHA256(ikm = clave_sesion, salt = utf8(sesion_id), info = "resguardo-sesion-v1|consola")` (lo que cifra la consola) y `…|equipo` (lo que cifra el equipo). `clave_sesion`: 32 bytes aleatorios de la consola, dentro del sobre de la orden que abre la sesión |
| Mensaje de sesión | base64 de `nonce(24) ‖ XChaCha20-Poly1305(k_lado, nonce, json, aad = "resguardo-sesion-v1|" + sesion_id)` |
| Clave de respaldo de la consola (v1.23) | `secreto = HKDF-SHA256(ikm = Argon2id(NFC(clave), sal (16 bytes), mismos parámetros), salt = "", info = "resguardo-respaldo-consola-v1")`; `publica = X25519(secreto)` (la de `crypto_box`). La consola la calcula en el navegador y solo manda `publica` y `sal`. Vector `respaldo_consola` en `v1.json`. Formato del archivo: §2, «Copia de la consola» |
| Trozo del relé | `nonce(24) ‖ XChaCha20-Poly1305(clave_relevo, nonce, datos, aad = relevo_id + "|" + n + "|" + (último ? "1" : "0"))`, binario; 4 MiB sin cifrar (el último, menos). No se pueden reordenar, repetir ni recortar sin que se note |

Vectores de todo lo simétrico: `simetrico` en `v1.json` (nonces fijos), comprobados en Rust y en Node (`verificar.mjs`, con HChaCha20 a mano).

### Órdenes (sobre v2)

Lo que la consola sella para el equipo (JSON, antes de sellar):

```json
{
  "v": 2,
  "cliente": "…", "equipo": "…",
  "seq": 18, "nonce": "<16 bytes b64>",
  "emitida": "…", "caduca": "…", "not_before": "…" | null,
  "tipo": "copiar_ahora",
  "cuerpo": { … },
  "autorizacion": {
    "prueba_admin": "<prueba_e b64>" | null,
    "clave_repo": { "repo": "<id>", "contrasena": "…" } | null,
    "prueba_codigo": "<solo en alta>" | null
  },
  "responder_a": "<X25519 pública efímera del navegador, b64>" | null
}
```

El servidor no ve nada de esto. Solo recibe el sobre y unos **metadatos en claro**: `tipo`, `seq`, `not_before` y `caduca`. El agente comprueba que coinciden con los de dentro.

**Tipos y nivel** (`crates/protocolo/src/ordenes.rs`):

| Nivel | Tipos |
|---|---|
| Inofensiva (sesión) | `copiar_ahora`, `verificar_ahora`, `probar_restauracion`, `subir_ahora`, `desbloquear`, `reanudar`, `actualizar_agente`, `abrir_sesion` (para el progreso) |
| Contraseña del repositorio | `explorar` (abre sesión), `restaurar`, `descargar` (relé), `cambiar_retencion`*, `aplicar_retencion`*, `quitar_repositorio`* (+ clave), `dejar_de_copiar`*, `cambiar_copia_externa`, `rotar_contrasena_repo`, `compartir_acceso` (+ clave), `clave_almacen` (+ clave, v1.22) |
| Clave de administración | `alta` (verificador, `K_cfg`, espera mínima), `config` (configuración declarativa), `elegir_carpetas` (abre sesión), `pausar`*, `baja_equipo`*, `desvincular` (`*` si es «dejar de copiar»), `cambiar_servidor`, `cambiar_espera`, `cambiar_clave_admin`, `guarda_copias`* (si se desactiva), `retencion_almacen`* (salvo `quitar`, v1.22), `aplicar_retencion_almacen`* (v1.22), `crear_repositorio`, `adoptar_repositorio`, `copiar_historial`, `conectar_nube`, `quitar_nube` (`*` si el espejo usa esa nube), `anadir_consola`, `quitar_consola` (v1.35) |

`*` = destructiva: el servidor exige `not_before ≥ ahora + espera mínima` y el agente lo vuelve a comprobar.

**Destructivas según el cuerpo.** El servidor no ve el cuerpo, así que en estas se fía del `not_before` que declara la consola; el agente, que sí lo ve, exige la espera igualmente (y rechaza la orden si no la cumple):
- `desvincular` con `modo: "dejar_de_copiar"`;
- `guarda_copias` con `activo: false`, con `quitar`, quitando el `espejo` o alguno de sus destinos (el agente compara con los destinos que tiene);
- `cambiar_copia_externa` con `hora: null`;
- `quitar_nube` de una nube que usa el espejo;
- `cambiar_espera` a menos horas de las que tiene el equipo;
- `restaurar` con `destino: "original"` y `reemplazar: true`;
- `retencion_almacen` salvo con `quitar: true` (v1.22).

La consola debe poner el `not_before` en estas (si no, el agente la rechaza y se pierde el `seq`).

### Modelo de amenazas: claves de los equipos

- La etiqueta (`HMAC(K_cfg, …)`) solo se puede comprobar con la clave de administración. Una orden de nivel **repositorio** no la lleva, y un servidor manipulado podría cambiar la `box_pub` de un equipo para quedarse con la contraseña del repositorio.
- Por eso la consola **fija las claves de cada equipo (TOFU)**: la primera vez que comprueba la etiqueta con la clave de administración, guarda `box_pub` y `sign_pub` en IndexedDB (por navegador, cliente y equipo).
- Sin claves fijadas para ese equipo, una orden con contraseña de repositorio pide **también** la clave de administración la primera vez (para comprobar la etiqueta y fijarlas).
- Si el servidor da otras claves para un equipo ya fijado: **parada** y aviso; no se sella nada. Cambiar de claves exige volver a emparejar.
- El servidor y el agente no añaden nada que debilite esto: el servidor no puede cambiar `box_pub`/`sign_pub` de un equipo confirmado (solo se fijan al unirse y al confirmar), y el agente nunca acepta claves nuevas por una orden.

---

## 2. Servidor y cuentas

### `GET /api/servidor`

Sin sesión.

```json
{
  "version": "0.1.0",
  "nombre": "Resguardo Server",
  "identidad": "<Ed25519 pública b64>",
  "huella_ca": "AB:CD:…",
  "inicializado": true,
  "dropbox_app_key": "beobf3c13cvlrup",
  "url_agentes": "https://agentes.consola.ejemplo.com",
  "publico": true
}
```

`url_agentes` (v1.34): la dirección que la consola da a los agentes y a las otras consolas cuando no es la suya; `null` si es la misma. En una consola en internet (`--dominio`) es `https://agentes.<dominio>`: ahí el servidor sirve el certificado de su autoridad propia (la que fijan al vincularse), mientras que el dominio lleva el certificado público de Let's Encrypt (ver [consola-en-linea.md](consola-en-linea.md)). `publico` (v1.34): consola en internet (`--dominio` o `--publico`); cambia las cuotas predeterminadas (§3).

`dropbox_app_key` (v1.11): la app key **pública** de la app «Resguardo» de Dropbox (`beobf3c13cvlrup`; OAuth con PKCE, sin app secret). Se puede cambiar al compilar con `RESGUARDO_DROPBOX_APP_KEY`; vacía, no sale y la consola dice que falta configurarla. La consola la usa en «Conectar Dropbox» y la manda en `conectar_nube`.

La cabecera CSP de la consola admite, de fuera, solo `connect-src https://api.dropboxapi.com` (el navegador cambia ahí el código de Dropbox por el token).

### `GET /api/servidor/ca`

Sin sesión. La autoridad TLS propia del servidor, en PEM. El agente la descarga al vincularse (sin comprobar el certificado esa única vez) y la fija para siempre; la autenticidad la garantiza el SAS v2, que incluye la identidad Ed25519 del servidor.

Con un certificado público (v1.34, `--dominio`) sigue siendo la autoridad **propia**, no la de Let's Encrypt: el servidor la sirve en `url_agentes` (`agentes.<dominio>`, por SNI) y por IP, y el SAS v3 la sigue cubriendo. Así una renovación (cada ~60 días) o un cambio de raíz de Let's Encrypt no cambia lo que el agente fijó. Un agente que se vinculara contra el dominio público fijaría una autoridad que no es la del certificado que recibe y no podría conectar: la consola le da siempre `url_agentes`.

### Copia de la consola (v1.23)

Solo el **propietario del servidor** (`superusuario`), con la sesión completa. Lo que hace que la consola se pueda restaurar **con la misma identidad** (los equipos la reconocen y vuelven solos): las bases de datos, `identidad.key`, la autoridad TLS (`tls/`), los paquetes de cliente guardados y este ajuste.

| Método y ruta | Pide | Responde |
|---|---|---|
| `GET /api/servidor/respaldo` | — | `RespaldoConsola` |
| `PUT /api/servidor/respaldo` | `{ activo?, publica?, sal?, conservar? (1–60, 7), hora? ("HH:MM", "03:30") }`; `publica` (X25519, b64) y `sal` (16 bytes, b64) van juntas | `RespaldoConsola`. Auditado en el servidor (`copia_consola`, sin la clave) |
| `POST /api/servidor/respaldo/ahora` | — | `RespaldoConsola` con la copia hecha, o 422 con el motivo (sin clave, sin espacio…). Como mucho 12 por hora |

`RespaldoConsola`: `{ activo, clave_puesta: <RFC 3339> | null, sal, conservar, hora, carpeta, ultima: { cuando, ok, mensaje, archivo, bytes, motivo: "diaria" | "manual" | "actualizacion" } | null, proxima, copias: [{ archivo, bytes }], identidad }`. La clave pública no se devuelve.

- **La clave de respaldo nunca llega al servidor**: la consola deriva `publica` (§1) y solo manda esa y su sal. Cada copia lleva una clave de archivo al azar sellada para `publica`; el servidor puede hacer copias, pero no abrirlas.
- **Cuándo:** cada día a `hora` (una al día; si falla, otra vez pasada una hora), al arrancar una versión nueva **antes de abrir la base de datos** y con «Hacer ahora». Las bases de datos se copian con `VACUUM INTO` en una conexión aparte: el servidor sigue funcionando mientras.
- **Dónde:** `<datos>/respaldos/consola-AAAAMMDD-HHMMSS-mmm.resguardo-consola`, las `conservar` más recientes. **No hay ruta para descargarlas**: para sacarlas de la máquina, el agente de esa misma máquina las copia como una copia más (carpeta `<datos>/respaldos`), o un almacén las recibe como cualquier otra copia. Así se reutiliza todo lo que ya hay (restic, retención, copia externa, espejo) y quien entre en la consola con la cuenta del propietario no puede llevárselas.
- **Formato** (`crates/protocolo/src/respaldo_consola.rs`):

  ```text
  "RESGUARDO-CONSOLA-2\n" ‖ cabecera (JSON en una línea) ‖ "\n" ‖ trozo₀ ‖ … ‖ trozoₙ ‖ 0xFFFFFFFF ‖ firma(64)
  cabecera = { v: 2, sal, publica, clave: crypto_box_seal(publica, K), creado, identidad, version }
  trozoₙ   = u32 big-endian (longitud) ‖ nonce(24) ‖ XChaCha20-Poly1305(K, nonce, datosₙ,
             aad = "resguardo-consola-v2|" + hex(SHA-256(cabecera)) + "|" + n + "|" + (último ? "1" : "0"))
  firma    = Ed25519(identidad del servidor, "resguardo-consola-v2-firma|" ‖ SHA-256(todo lo anterior a 0xFFFFFFFF))
  datos    = por cada archivo: u16 BE (largo de la ruta) ‖ ruta ‖ u64 BE (largo) ‖ bytes; al final un u16 a 0
  ```

  Desde 0.7.11 la copia va **firmada con la identidad del servidor** (la que fijan los agentes): la pública de la clave de respaldo está en claro en la cabecera y en el servidor, así que sin firma cualquiera que la tenga podría fabricar otra copia que también se abre con la clave. Las anteriores (`RESGUARDO-CONSOLA-1`, `v: 1`, aad con `resguardo-consola-v1|`, sin marca ni firma) se siguen restaurando, con un aviso.

  Rutas posibles: `control.db`, `clientes/<id>.db`, `identidad.key`, `tls/<archivo>`, `paquetes/<archivo>`, `respaldo-consola.json`.
- **Restaurar:** `resguardo-server restaurar-respaldo <archivo> [--datos DIR] [--reemplazar] [--confiar-en <huella>]` (Windows y Linux), con el servicio parado. Primero, sin la clave, comprueba la estructura y la firma y enseña la huella de la identidad que hizo la copia (los 8 primeros bytes, `AB:CD:…`, como en el kit): tiene que ser la de `--confiar-en` (la del kit, cuya línea ya la trae), la del servidor que ya hay en esa carpeta, o confirmarla en la terminal; si no, no sigue (sin terminal ni `--confiar-en`, tampoco). Después pide la clave (o `RESGUARDO_CLAVE_RESPALDO`), lo descifra todo aparte (con una clave equivocada o un archivo dañado no toca nada), comprueba que la identidad descifrada es la de la cabecera (antes de tocar la carpeta de datos), aparta lo que hubiera en `antes-de-restaurar-<fecha>` (solo con `--reemplazar`) y deja lo restaurado en su sitio; si algo falla a medias, lo quita y devuelve lo de antes. En Linux, lo restaurado (y solo eso, sin seguir enlaces) pasa a ser del dueño de la carpeta de datos. Las instantáneas en claro de las bases de datos se hacen en `<datos>/.respaldo-instantanea-*` (fuera de `respaldos/`, que el agente copia tal cual) y se borran al terminar. `hacer-respaldo` hace una al momento desde la línea de órdenes.

### `POST /api/inicio`

Primer arranque, solo una vez. El servidor muestra el `codigo_arranque` en su registro al arrancar sin cuentas.

- Pide: `{ "codigo_arranque": "…", "correo": "…", "nombre": "…", "contrasena": "…" }`
- Responde: `{ "totp": { "secreto": "<base32>", "uri": "otpauth://totp/Resguardo:<correo>?secret=…&issuer=Resguardo" } }`

Abre una sesión **pendiente de TOTP**. La cuenta queda como propietaria del servidor (`superusuario: true`).

### `POST /api/sesion`

- Pide: `{ "correo", "contrasena" }`
- Responde:
  - `{ "necesita": "totp" }`, o
  - `{ "necesita": "alta_totp", "totp": { secreto, uri } }` (la cuenta aún no tiene TOTP), o
  - v1.27: `{ "necesita": "restablecimiento", "restablecida": { cuando, por }, "caducado": bool }`: un propietario le restableció la verificación en dos pasos (§3). La contraseña sola no basta para dar de alta otro autenticador: hace falta el código que le dio, con `POST /api/sesion/restablecimiento`. `caducado: true` si pasaron sus 24 h (hay que pedir otro).
- Deja la cookie de sesión **pendiente** (aal1).

### `POST /api/sesion/totp`

- Pide: `{ "codigo": "123456" }` o `{ "recuperacion": "xxxx-xxxx" }`
- Responde:
  - `{ "cuenta": { … } }` y la sesión pasa a **completa** (aal2);
  - la primera vez, además `"codigos_recuperacion": ["…" ×10]` (se muestran una sola vez).

### `POST /api/sesion/restablecimiento` (v1.27)

- Pide: `{ "codigo": "XXXX-XXXX-XXXX-XXXX" }` (el que dio el propietario; da igual mayúsculas, espacios y guiones), con la sesión pendiente de `POST /api/sesion`.
- Responde `{ "necesita": "alta_totp", "totp": { secreto, uri }, "restablecida": { cuando, por } }`. Después, `POST /api/sesion/totp` con un código del autenticador nuevo, **desde esta misma sesión** (otra sesión a medias recibe 403 `restablecimiento`), lo activa, da códigos de recuperación nuevos y repite `restablecida` (la consola enseña «Tu verificación en dos pasos se restableció el … por …»). Ahí el código se gasta. Si no se termina, el código sigue valiendo hasta que caduque.
- Errores: 401 `codigo` (no es ese), 410 `caducado` (pasaron 24 h), 422 (no hay restablecimiento pendiente). Mismo límite de intentos que el TOTP.

### `DELETE /api/sesion`

Cierra la sesión.

### `GET /api/cuenta`

```json
{ "id", "correo", "nombre", "superusuario": false, "clientes": [{ "id", "nombre", "rol" }] }
```

### `PUT /api/cuenta/contrasena`

`{ "actual", "nueva" }`. Cierra las demás sesiones.

### `PATCH /api/cuenta`

`{ "nombre" }` (1–80 caracteres) → la cuenta.

### `POST /api/cuenta/totp` y `POST /api/cuenta/totp/confirmar`

Cambiar el autenticador (otro móvil):
1. `POST /api/cuenta/totp` con `{ "contrasena", "codigo" }` (un código del autenticador **actual**) → `{ "totp": { secreto, uri }, "caduca" }`. El actual sigue valiendo.
2. `POST /api/cuenta/totp/confirmar` con `{ "codigo" }` (del **nuevo**, antes de 15 min) → `{ "cuenta" }`. Desde ahí solo vale el nuevo y se cierran las demás sesiones.

### `POST /api/cuenta/recuperacion`

`{ "contrasena", "codigo" }` (código TOTP actual) → `{ "codigos_recuperacion": ["…" ×10] }`. Los anteriores dejan de valer.

**Límites:** 10 intentos de contraseña o TOTP por cuenta y por IP cada 15 min; después, 429.

---

## 3. Clientes y usuarios

| Método y ruta | Rol | Pide | Responde |
|---|---|---|---|
| `GET /api/clientes` | cualquiera | — | `[{ id, nombre, rol, equipos, avisos, marca }]` (`marca`, v1.32: ver abajo) |
| `POST /api/clientes` | superusuario | `{ nombre, espera_min_horas }` | `{ id, nombre, sal_cliente }` (sal aleatoria, pública) |
| `GET /api/clientes/{c}` | miembro | — | `{ id, nombre, sal_cliente, espera_min_horas, rol, marca }` |
| `PATCH /api/clientes/{c}` | propietario | `{ nombre? }` | el cliente |
| `GET /api/clientes/{c}/miembros` | propietario | — | `[{ cuenta, correo, nombre, rol }]` |
| `PUT /api/clientes/{c}/miembros/{cuenta}` | propietario | `{ rol }` | — |
| `DELETE /api/clientes/{c}/miembros/{cuenta}` | propietario | — | — |
| `POST /api/clientes/{c}/miembros/{cuenta}/restablecer-totp` (v1.27) | propietario (ver abajo) | `{ codigo }` (un código TOTP **de quien lo hace**, recién sacado) | `{ codigo: "XXXX-XXXX-XXXX-XXXX", caduca }` (24 h, un solo uso) |
| `POST /api/clientes/{c}/invitaciones` | propietario | `{ rol }` | `{ enlace: "/invitacion#<token>", caduca }` (7 días, un solo uso) |
| `POST /api/invitaciones/aceptar` | sin sesión o con ella | `{ token, correo?, nombre?, contrasena? }` | Como `/api/sesion`, si crea la cuenta |
| `GET /api/clientes/{c}/marca` (v1.32) | miembro | — | `{ acento, logo, actualizada, por }` |
| `PUT /api/clientes/{c}/marca` (v1.32) | administrador | `{ acento, logo?, quitar_logo? }` | `{ acento, logo, actualizada, por }` |
| `GET /api/clientes/{c}/marca/logo` (v1.32) | miembro | — | El logo: `image/png`, `Cache-Control: private, max-age=86400` (404 si no hay) |

**Marca del cliente** (v1.32): un logo y un acento que la consola enseña en el selector de clientes, la cabecera del cliente y la portada de los informes.
- `acento`: uno de `teal`, `blue`, `indigo`, `violet`, `rose`, `amber` o `graphite` (los de la consola, con su contraste ya comprobado en claro y en oscuro; docs/diseno.md §2), o `null` (sin acento propio). Otro valor: 422. Cada `PUT` lo sustituye.
- `logo` (al pedir): un **PNG** en base64, de hasta 200 KB y 2048 px de lado; sin el campo se queda el que había y con `quitar_logo: true` se quita. El servidor comprueba firma, cabecera `IHDR` y final `IEND`; otra cosa (un SVG, un JPG, un PNG cortado): 422. La consola convierte antes en el navegador cualquier SVG, JPG o WebP a PNG (así ningún SVG con scripts, manejadores o referencias externas llega al servidor ni a otros navegadores).
- `logo` (al responder): la URL del PNG con su versión (`/api/clientes/{c}/marca/logo?v=<16 hex del SHA-256>`), o `null`. Se sirve desde el propio servidor (cabe en `img-src 'self'`) con `nosniff`.
- Lo cambian propietarios y administradores; queda en la auditoría del cliente (`cambiar_marca`, `datos`: `{ acento, logo: "puesto" | "quitado" | "igual" }`). Se guarda en los valores del servidor (`marca:<cliente>`), sin cambios en el esquema. No viaja en el paquete de exportación.

**Aislamiento entre clientes** (v1.34): todo lo que cuelga de `/api/clientes/{c}` pasa antes por una guardia que exige una sesión completa de alguien que es **miembro** de ese cliente, antes de leer el cuerpo o llegar a la ruta; si no lo es, 404 `no_existe` (como si no existiera). Vale también para el propietario del servidor: si no es miembro, no entra. Cada ruta comprueba además el rol. Cada cliente tiene su propia base de datos (`clientes/<id>.db`). La prueba `crates/servidor/tests/aislamiento.rs` recorre la tabla de rutas entera.

**Clientes del servidor** (v1.34), solo el propietario del servidor (`superusuario`; si no, 403). Solo cifras: nada de dentro de los clientes (ni nombres de equipos, ni correos, ni avisos, ni órdenes).

| Método y ruta | Pide | Responde |
|---|---|---|
| `GET /api/servidor/clientes` | — | `{ publico, mes: "2026-10", predeterminadas, de_fabrica, clientes: [{ id, nombre, creado, personas, propietarios, soy_miembro, equipos, equipos_confirmados, equipos_conectados, historial, ordenes_30d, ultima_actividad, bytes, relevo_mes_bytes, cuotas, efectivas }] }` |
| `POST /api/servidor/clientes` | `{ nombre, espera_min_horas?, cuotas? }` | `{ id, nombre, invitacion: { enlace, caduca } }`: un cliente **para otra persona**, sin que quien lo crea sea miembro, con una invitación de **propietario** (7 días, un solo uso) |
| `POST /api/servidor/clientes/{c}/invitacion` | — | `{ invitacion }`; solo si el cliente no tiene ningún propietario (409 si lo tiene: entonces invita él) |
| `PUT /api/servidor/clientes/{c}/cuotas` | `Cuotas` | `{ cuotas, efectivas }` |
| `PUT /api/servidor/cuotas` | `Cuotas` | `{ predeterminadas }` |

`Cuotas` = `{ equipos, historial, relevo_mb_mes, ordenes_min }`; en cada campo, `null` = la de arriba (la predeterminada del servidor y, si no, la de fábrica), `0` = sin límite, un número = el límite (422 si es negativo o pasa del máximo). De fábrica: en una consola en internet, 50 equipos, 20 000 entradas de historial por equipo, 20 480 MB del relé al mes y 60 órdenes por minuto; en la red local, sin límite de equipos ni de relé, 20 000 entradas y 300 órdenes por minuto. Lo que no cabe da 403 `cuota` (equipos al abrir un emparejamiento, preparar un instalador, «Vincular este servidor», `POST /api/agente/unirse` y `/api/agente/recibir`; el relé al pedir una descarga y en cada trozo) o 429 `cuota` (órdenes por minuto), con el motivo en `mensaje`. El historial no da error: se guardan las más recientes hasta la cuota. Los cambios de cuotas quedan en la auditoría del servidor y en la del cliente.

**Restablecer la verificación en dos pasos de otro** (v1.27): para quien perdió el móvil y sus códigos de recuperación. Lo hace el **propietario del cliente** con miembros de ese cliente que no son propietarios (de ningún cliente) y que solo están en clientes de los que él es propietario; el **propietario del servidor** (`superusuario`, miembro del cliente), con cualquiera de ese cliente. Nadie con su propia cuenta (422) ni con la del propietario del servidor (403). Primero se comprueba el permiso (403 con el motivo; 404 si no es miembro) y después el código TOTP de quien lo hace (401 `codigo`; un código gastado no vale otra vez). Al otro se le quitan el autenticador, los códigos de recuperación, un cambio de autenticador a medias y **todas las sesiones**; la próxima vez que entre, `POST /api/sesion` contesta `necesita: "restablecimiento"` y hace falta el código. Queda en la auditoría del cliente y en la del servidor (`restablecer_totp`, objetivo: la cuenta; `datos`: correo y nombre). El servidor solo guarda el hash del código.

La espera mínima (`espera_min_horas`, 1–168) **la guarda cada equipo**. Cambiarla es la orden `cambiar_espera`, con la clave de administración; **nunca** se cambia desde la consola directamente.
- El equipo confirma la nueva en el `detalle` firmado de su resultado: `{"espera_min_horas": h}`.
- Con ese resultado verificado, el servidor guarda la espera **de ese equipo** (`Equipo.espera_min_horas`) y la usa para sus órdenes; si todos los equipos gestionados coinciden, también la del cliente (la que se usa para los equipos nuevos).
- Si el equipo aún no confirmó ninguna, vale la del cliente.

---

## 4. Equipos y emparejamiento

### Consola

| Método y ruta | Rol | Responde / hace |
|---|---|---|
| `GET /api/clientes/{c}/equipos` | miembro | `[Equipo]` |
| `GET /api/clientes/{c}/equipos/{e}` | miembro | `Equipo` y su último informe |
| `PATCH /api/clientes/{c}/equipos/{e}` | administrador | `{ nombre }` (solo el nombre visible) |
| `PUT /api/clientes/{c}/equipos/{e}/etiquetas` (v1.18) | técnico | `{ etiquetas: ["Contabilidad", …] }` → `Equipo`. Etiquetas libres para agrupar y filtrar (hasta 10, de 1 a 32 caracteres, sin comas; sin repetir sin distinguir mayúsculas). En claro: son metadatos como el nombre. Auditado (`etiquetas_equipo`) |
| `POST /api/clientes/{c}/equipos/{e}/atencion` | miembro | Pide al agente que consulte cada 2 s durante 10 min (si no está por WebSocket) |
| `POST /api/clientes/{c}/emparejamientos` | administrador | `{ id, codigo: "ABCD-EFGH-JK", caduca }` (15 min) |
| `GET /api/clientes/{c}/emparejamientos/{p}` | administrador | `{ estado: "abierto" \| "unido" \| "confirmado" \| "cancelado" \| "caducado", caduca, equipo?: { id, nombre, so, box_pub, sign_pub, sal_equipo }, sas?, sas_version?: 2 \| 3 }` (`sas_version`, v1.26: la que anunció el equipo al unirse; la consola calcula el SAS de esa versión por su cuenta y, con 2, pide comprobar también la huella de la autoridad TLS) |
| `POST /api/clientes/{c}/emparejamientos/{p}/confirmar` | administrador | `{ etiqueta }`: tras comparar el SAS. Después, la consola envía la orden `alta` |
| `DELETE /api/clientes/{c}/emparejamientos/{p}` | administrador | Cancela; si el equipo ya se unió, lo borra |
| `POST /api/clientes/{c}/instaladores` (v1.17) | administrador | `{ nombre, so: "windows" \| "linux", servidor: "https://…" }`. Prepara un equipo con un código de un solo uso que caduca en 24 h (20 por hora y cliente, con los de 15 min). `windows`: responde el instalador del agente con la cola de `crates/protocolo/src/instalador.rs` al final (`Content-Disposition: attachment; filename="Resguardo-Agente_<cliente>_<equipo>.exe"`, cabeceras `X-Resguardo-Emparejamiento` y `X-Resguardo-Caduca`); sin instalador en el servidor, 404 `sin_instalador` (no se gasta nada). `linux`: `{ id, nombre, so, codigo, caduca, servidor, huella_ca }` |
| `GET /api/clientes/{c}/emparejamientos` (v1.17) | administrador | Los preparados que aún sirven: `[{ id, nombre, so, estado: "abierto" \| "unido", caduca, creado, equipo }]` (sin el código) |
| `GET /api/clientes/{c}/preparados` (v1.17) | miembro | `{ esperando, unidos }` |
| `POST /api/clientes/{c}/equipo-local` (v1.19) | administrador | «Vincular este servidor»: prepara un emparejamiento de 30 min con el nombre de la máquina y deja sus datos (los de la cola del instalador, hacia `https://127.0.0.1:<puerto>`) en `vincular-local.json` de la carpeta de datos del servidor. → el preparado. 404 `sin_agente_local` si no hay Resguardo Agente en la máquina |

**Preparados (v1.17).** El equipo entra con el `nombre` de la consola (no el suyo). `GET …/emparejamientos/{p}` de un preparado lleva además `nombre`, `so` y, mientras está `abierto` o `unido`, `codigo` (la consola lo necesita para la `prueba_codigo` del `alta`); al confirmar, anular o caducar, el servidor lo borra. La cola del instalador: `"RESGUARDO-COLA-1" ‖ u32 BE n ‖ JSON (n ≤ 4096) ‖ u32 BE n ‖ "RESGUARDO-FIN-01"`, con el JSON `{ v: 1, servidor, huella_ca, cliente, nombre, codigo }` (ni un campo más). El agente ≥ 0.7.7 la lee (`resguardo-agente leer-instalador <ruta>`, `vincular --instalador <ruta>`) y se vincula comprobando que la autoridad TLS del servidor tiene esa huella y que el cliente es ese; en Linux, `vincular CÓDIGO --servidor URL --huella-ca AB:CD:…`. Riesgo residual: [plataforma.md §7.3.1](plataforma.md).

**Equipo:**

```json
{
  "id", "nombre", "so", "version_agente",
  "box_pub", "sign_pub", "sal_equipo", "etiqueta",
  "rol": "agente" | "almacenamiento",
  "modo": "gestionado" | "local" | "trasladado",
  "confirmado": true,
  "conectado": true,
  "ultimo_contacto",
  "estado_servicio": "en_marcha" | "detenido_por_admin" | null,
  "siguiente_seq": 19,
  "espera_min_horas": 24 | null,
  "etiquetas": ["Contabilidad"],
  "resumen": { … }
}
```

`conectado` significa que tiene el WebSocket abierto. `espera_min_horas`: la que confirmó el equipo (`null`: la del cliente).

**`resumen`** (lo escribe el agente, en claro: sin rutas ni secretos):

```json
{
  "copias": [{ "id", "nombre", "repo", "horario": { "dias": [1, 5], "horas": ["13:00"] }, "carpetas": 3, "activa": true,
               "ultima": { "cuando", "estado": "ok" | "aviso" | "fallo", "mensaje" } | null }],
  "repositorios": [{ "id", "nombre", "destino": "<nombre del destino>", "retencion": "7 diarias · 4 semanales · …" | null,
                     "solo_anadir": true | false | null }],
  "destinos": [{ "id", "nombre", "tipo": "local" | "rest" | "s3" | "b2" | "sftp", "donde": "<servidor o bucket>" | null,
                 "equipo_almacen": "<id del equipo que guarda copias>" | null }],
  "pausado_hasta": "<RFC 3339>" | "indefinido" | null
}
```

En un destino `local`, `donde` es `null` (sería una ruta del equipo). `destinos[].equipo_almacen` (v1.30, agente ≥ 0.7.14): el equipo que guarda copias de ese destino, si la consola lo dijo al crearlo con «Copiar en …» (`crear_repositorio.destino.equipo_almacen`); `null` en los demás y en los creados antes (la consola los reconoce entonces por el id `almacen-<8 primeros del equipo>` o por el nombre). `repositorios[].solo_anadir` (v1.14): si su rest-server es de solo añadir (la comprobación diaria del agente o la de cuando se adoptó); `null` si no se sabe o no es un rest-server. Desde v1.22 la comprobación usa la autoridad TLS propia del destino (`ca_pem`, la de un almacén), así que un repositorio en un almacén sale `true`. `repositorios[].ruta` (v1.22): la carpeta del repositorio en su servidor rest cuando no es su `id` (uno adoptado); solo un nombre.

v1.35 (varias consolas, [consolas-multiples.md](consolas-multiples.md)): el resumen lleva también `consolas: [{ id, nombre, url, identidad, sal_cliente | null, ultimo_contacto | null (redondeado a 15 min), desde | null, esta }]` (las consolas que gestionan el equipo; `esta: true` la que recibe el resumen; sin credenciales), `cambio_config: { tipo, cuando, consola: { nombre, url, identidad } } | null` (el último cambio y desde qué consola llegó: si no es esta, la consola enseña «Cambiado desde otra consola») y `admite` con `"consolas_multiples"`.

v1.36: `admite` incluye `"escritorio"` y el resumen lleva `escritorio: { ventana, avisos }` (lo que tiene el equipo, también deducido de `bandeja`) y `escritorio_cambiado_en_equipo` (RFC 3339 o `null`): la consola enseña «En el equipo» sin descifrar la configuración.

v1.28: el resumen lleva `admite: ["retencion_plazos", "verificacion_auto", "almacen_propio"]` (lo nuevo que entiende el agente: la consola no ofrece lo que no) y, por repositorio, `repositorios[].retencion_regla` (la `Retencion` tal cual, §5; `retencion` sigue siendo su texto) y `repositorios[].verificacion_auto: { cada_dias, porcentaje, proxima, todo_leido } | null` (la verificación automática, §6; `todo_leido`: cuándo terminó la última vuelta completa de la rotativa).

En un equipo que guarda copias, `guarda_copias.retenciones` (v1.22) es la retención que aplica ese almacén: `[{ usuario, repo, retencion: Retencion, texto, horario: { dias, hora }, horario_texto, verificar, clave: "ok" | "pendiente" | "sin_probar", ultima, resultado: "ok" | "fallo" | null, mensaje, versiones, proxima }]`. Nunca lleva la clave (§5, `retencion_almacen`).

Espacio del almacén (v1.31; para «¿Cuándo se llena?» de la consola): `guarda_copias.espacio: { libre, total, leido } | null`, los bytes libres y totales del volumen de su carpeta (`GetDiskFreeSpaceExW` en Windows, `statvfs` en Linux; se miden al hacer el resumen), y `guarda_copias.espejo.destinos[].espacio` igual: de una carpeta, su volumen (al hacer el resumen); de una nube, la cuenta (`rclone about`, una vez tras cada espejo a ella; `leido` es esa hora), o `null` si la nube no lo dice. Solo números: nunca rutas. Un agente anterior no los manda y la consola calcula el crecimiento sin decir cuándo se llena.

### Agente (sin sesión)

#### `POST /api/agente/unirse`

Anónima, con límite por IP.

- Pide:

  ```json
  { "codigo_hash", "nombre", "so", "version", "box_pub", "sign_pub", "sal_equipo", "sas_version": 3 }
  ```

  `codigo_hash` es `SHA-256` en hex del código normalizado (solo letras y cifras, en mayúsculas), como en `protocolo::mensajes::code_hash`. `sas_version` (v1.26, agentes ≥ 0.7.10): el equipo calcula el SAS v3, con la huella de la autoridad TLS que acaba de fijar; sin él, v2.

- Responde:

  ```json
  { "equipo_id", "secreto", "cliente_id", "servidor": { "identidad", "nombre" }, "sas", "sas_version" }
  ```

  `sas`: el del servidor, con su propia huella (`huella_ca`) en v3. El agente enseña **el suyo** (siempre v3 desde 0.7.10), no este.

- Si el código no vale: 404 `codigo`.

---

## 5. Órdenes

### `POST /api/clientes/{c}/equipos/{e}/ordenes`

- Pide:

  ```json
  { "tipo", "seq", "sellado", "caduca", "not_before": null, "sesion": null, "relevo": null }
  ```

  - `sesion`: si el tipo abre una sesión, el id que la consola eligió (UUID).
  - `relevo`: si es `descargar`, `{ "id": "<uuid>", "max_bytes": … }`.

- Comprobaciones del servidor:
  - el rol permite el tipo;
  - `seq` es exactamente `siguiente_seq`; si no, 409 `conflicto` con el `siguiente_seq` actual;
  - `caduca` está como mucho a 7 días;
  - en las destructivas, `not_before ≥ ahora + espera mínima`;
  - el sobre ocupa como mucho 64 KiB.

- Responde: `Orden`.

**Orden:**

```json
{
  "id", "tipo", "seq", "emitida", "emitida_por": { "id", "nombre" },
  "not_before", "caduca",
  "estado": "pendiente" | "entregada" | "en_marcha" | "hecha" | "fallida" | "rechazada" | "cancelada" | "caducada",
  "mensaje": "texto corto sin rutas" | null,
  "detalle": "<JSON en texto>" | null,
  "firma_agente": "<b64>" | null,
  "actualizada"
}
```

### Otras rutas

| Método y ruta | Qué hace |
|---|---|
| `GET /api/clientes/{c}/equipos/{e}/ordenes?limite=50` | Las últimas órdenes del equipo |
| `GET /api/clientes/{c}/ordenes?pendientes=1` | Órdenes con espera de todo el cliente (tarjetas «Pendiente: … · Cancelar»): `[Orden]` |
| `GET /api/clientes/{c}/ordenes?limite=50&antes=<cursor>&equipo=<id>&estado=<estado>` | Las últimas de todo el cliente, de la más reciente a la más antigua (`limite` hasta 200; filtros opcionales): `{ ordenes: [Orden], siguiente: "<cursor>" \| null }`. Para la página siguiente, `antes=<siguiente>` |
| `POST /api/clientes/{c}/ordenes/{o}/cancelar` | Cualquier miembro salvo `lectura`. Solo si aún no se entregó, o si es destructiva y no ha llegado su `not_before`; el servidor la marca y el agente también la recibe como cancelada |

**`firma_agente`:** Ed25519 del equipo sobre `"resguardo-resultado-v1|" + orden_id + "|" + seq + "|" + estado + "|" + (mensaje ?? "") + "|" + (detalle ?? "")`.

**`detalle`** (firmado con el resultado): un JSON en texto.
- En claro, lo que el servidor puede leer: `cambiar_espera` → `{"espera_min_horas": h}`; `descargar` → `{"trozos": n}`.
- Si la orden trae `responder_a` (X25519 efímera de la consola), el detalle privado va sellado para ella: `{"sellado": "<crypto_box_seal b64>"}`. Solo lo abre la consola. (Hoy ninguna orden lo usa; lo usará «restaurar en otro equipo», §10.)
- Las órdenes largas (`restaurar`, `descargar`, `aplicar_retencion`) contestan primero `en_marcha` y luego el resultado final, firmado igual.

### Cuerpos de las órdenes

Formatos finales (v1.2). Los ids de repositorio, destino y copia: letras, cifras, `-` y `_`, hasta 64.

| Tipo | `cuerpo` |
|---|---|
| `alta` | `{ verificador, k_cfg, espera_min_horas }` |
| `crear_repositorio` | `{ id, nombre, contrasena, destino: { id } \| { id, nombre, tipo, donde, usuario?, secreto?, ca_pem?, equipo_almacen? }, retencion?, parametros_de? }` (`equipo_almacen`, v1.30: el id del almacén en «Copiar en …»; solo en un destino `rest`, el equipo lo guarda y lo devuelve en el resumen). Crea el repositorio (`restic init`) o, si ya existe, comprueba que la contraseña lo abre. El destino y sus credenciales se quedan en el equipo. v1.14: `parametros_de` (un origen como el de `copiar_historial`) lo crea con los parámetros de troceado de ese repositorio (`init --copy-chunker-params`): lo que se traiga después de él se deduplica del todo |
| `adoptar_repositorio` | v1.14. `{ id, nombre, contrasena, ruta, destino: { id } \| { id, nombre?, tipo, donde, usuario?, secreto?, ca_pem? }, solo_probar? }` (clave de administración). Un repositorio de restic **que ya existe** (p. ej. de la app de escritorio) pasa a ser uno gestionado más, **de lectura y escritura**, con todo su historial: copias, retención, verificación y copia externa como cualquier otro. `ruta`: su carpeta dentro del destino (`"Siigo"`, `"ana/portatil"`; `""` si el repositorio es todo el destino; sin `..`, `\`, `:`, `@`). El agente comprueba con `restic cat config` que la contraseña lo abre (si no, `fallida` sin repetirla), lo rechaza si ese equipo ya usa esa ubicación y nunca hace `init`. Detalle: `{ versiones, ultima, solo_anadir, en_uso, equipos?, etiquetas? }` (`equipos`/`etiquetas`, los nombres de los equipos y etiquetas de sus versiones, solo si la orden trae `responder_a`; entonces va sellado). `solo_probar: true` solo comprueba (no guarda nada; `id` y `nombre` sobran): es el «Probar» de la consola, también para el origen de `copiar_historial`. En un rest-server de **solo añadir** (`solo_anadir: true`, comprobado sin borrar nada) la retención no se aplica desde el equipo: `aplicar_retencion` contesta `fallida` con «se aplica en el propio servidor» |
| `copiar_historial` | v1.14. `{ repo, origen: { repo: "<id de este equipo>" } \| { destino: { tipo, donde, usuario?, secreto?, ca_pem? }, ruta, contrasena }, filtro?: { equipos?: [<host>], etiquetas?: [<tag>] } }` (clave de administración). `restic copy` al repositorio gestionado `repo` (no uno de solo lectura) de las versiones del origen que falten, en segundo plano: contesta `en_marcha` y manda `en_marcha` con el progreso («Trayendo el historial: 12 de 140 versiones…») como mucho cada minuto, y al final `hecha` («Historial traído: N versiones nuevas. M ya estaban.») o `fallida`. Solo añade (vale con destinos de solo añadir) y no repite lo ya traído; una por repositorio a la vez. El origen va al proceso en variables (`RESTIC_FROM_REPOSITORY`, `RESTIC_FROM_PASSWORD`), nunca en los argumentos. Origen y destino en dos cuentas distintas del mismo tipo de nube → `fallida`. Si el agente se reinicia a medias, la orden se queda `en_marcha`: se vuelve a pedir y sigue donde se quedó |
| `config` | `{ config: Configuracion }` (§6) |
| `copiar_ahora` | `{ copia }` (el repositorio sale de la configuración; `{ repo, copia }` también vale) |
| `verificar_ahora`, `probar_restauracion`, `subir_ahora` | `{ repo }` |
| `pausar` | `{ horas, repo? }` (`0` = hasta reanudar; sin `repo`, todos) |
| `reanudar` | `{ repo? }` |
| `cambiar_retencion` | `{ repo, diarias, semanales, mensuales, anuales, horarias?, plazos? }` (la guarda). v1.28: `Retencion` (abajo) |
| `aplicar_retencion` | `{ repo }` (`restic forget --prune` con la guardada). En un repositorio de solo añadir contesta `fallida` (el servidor no deja borrar): en un almacén la aplica él (`retencion_almacen`) |
| `clave_almacen` | v1.22. `{ repo, clave }` (contraseña del repositorio + clave de administración), al **equipo dueño**. Añade al repositorio una clave de restic con la contraseña `clave` (`restic key add --new-password-file … --user resguardo-almacen`: es una escritura, así que vale en un servidor de solo añadir). Es la que usará el almacén para aplicar la retención en local; la genera la consola (32 bytes al azar) y la manda también, sellada, en `retencion_almacen`. `clave`: de 20 a 200 caracteres |
| `retencion_almacen` | v1.22. Al **almacén** (clave de administración). `{ usuario, repo, clave?, retencion: Retencion, horario: { dias: [1–7], hora: "HH:MM" }, verificar? }` pone o cambia la regla del repositorio `<carpeta del almacén>/<usuario>/<repo>` (`usuario`: uno de sus equipos; `repo`: su carpeta, letras, cifras, `.`, `-` y `_`, hasta 4 niveles con `/`, sin enlaces). Es **destructiva según el cuerpo** (espera). Sin `clave` se conserva la que tiene; con otra, la anterior se borra del repositorio en cuanto la nueva abre. Comprueba la clave (`restic cat config`): si aún no abre (el equipo dueño no ha añadido la suya), la guarda igual y lo dice; se vuelve a probar en cada vuelta. En cada hueco del horario (si el equipo estaba apagado a esa hora, al encenderse; una vez por hueco, y los anteriores a la orden no cuentan) el almacén ejecuta en local `restic forget --prune` con la regla y, con `verificar` (por defecto `true`), `restic check`. `{ usuario, repo, quitar: true }` la quita (no espera) y borra del repositorio el archivo de la clave del almacén |
| `aplicar_retencion_almacen` | v1.22. `{ usuario, repo }` (clave de administración, destructiva): la regla guardada, ahora. Contesta `en_marcha` y después `hecha` («Retención aplicada: 3 versiones quitadas, quedan 40. Comprobado sin errores.») o `fallida`. Una a la vez por repositorio |
| `dejar_de_copiar`, `quitar_repositorio` | `{ repo }` (el repo, el de `clave_repo`). `quitar_repositorio` además lo olvida en el equipo; lo guardado sigue en su destino |
| `cambiar_espera` | `{ horas }` (1–168) |
| `cambiar_clave_admin` | `{ verificador, k_cfg, k_cfg_consolas? }`. v1.35: `k_cfg_consolas: { "<identidad de otra consola>": "<su K_cfg nueva>" }` (la consola la calcula con la sal de cada una, `resumen.consolas[].sal_cliente`). Sin ella, las consolas con la misma `K_cfg` que la que manda la orden toman la nueva; las demás se quedan como estaban |
| `desvincular` | `{ modo: "seguir_local" \| "dejar_de_copiar" }`. v1.35: si el equipo tiene otras consolas, solo se va **esta** (las demás siguen; con `dejar_de_copiar`, el equipo deja de copiar igual) |
| `baja_equipo` | `{}` (v1.35: con otras consolas, deja de copiar y se va de esta; las demás siguen vinculadas) |
| `anadir_consola` | v1.35. `{ url, identidad, huella_ca, ficha, sal_cliente, k_cfg, nombre?, sal_origen? }` (solo administradores). El equipo se conecta **también** a esa consola sin dejar esta: descarga su autoridad TLS y exige un solo certificado con `huella_ca`, se da de alta con `POST /api/agente/recibir` (`motivo: "anadir_consola"`, la etiqueta con `k_cfg`, la de esa consola), exige la `identidad` y que la demuestre, sube su configuración, informe e historial y avisa a todas sus consolas (`cambio_inusual`). Contesta `en_marcha` y luego `hecha` o `fallida`. `sal_origen`: la sal del cliente en la consola que la manda (el equipo la guarda para `resumen.consolas`). Como mucho 5 consolas; una que ya tiene → `fallida`. Caduca a los 7 días como mucho. [consolas-multiples.md](consolas-multiples.md) |
| `quitar_consola` | v1.35. `{ identidad }` (o `{ id }`, el de `resumen.consolas`; solo administradores). Deja de conectarse a esa consola (también a la propia, si quedan otras); la que se va recibe un último aviso y las demás otro. La única → `fallida` (eso es `desvincular`). La propia («Dejar esta consola», el último paso de «Mover a otra consola»): `detalle` en claro `{"deja_esta_consola": true, "siguen": ["<consola>", …]}`, firmado, y el servidor pone el equipo en `modo: "local"` (como con `desvincular`) |
| `elegir_carpetas`, `abrir_sesion` | `{ sesion, clave_sesion }` |
| `explorar` | `{ repo, sesion, clave_sesion }` |
| `restaurar` | `{ repo, version, rutas: [ruta de versión], destino: "junto" \| "original", reemplazar }`. «junto»: en una carpeta nueva `Restaurado AAAA-MM-DD HHMM` al lado de cada ruta; «original»: en su sitio, sin sobrescribir salvo `reemplazar` |
| `descargar` | `{ repo, version, rutas, formato: "archivo" \| "zip", relevo: { id, clave } }`. Una ruta: el archivo tal cual o la carpeta en zip (`restic dump`). Varias (hasta 100): solo `zip`; el agente las restaura en una carpeta temporal suya, las comprime y la borra |
| `cambiar_destino` | `{ destino, donde?, usuario?, secreto?, ca_pem? }` (clave de administración). Comprueba que cada repositorio del destino se abre con los datos nuevos antes de guardarlos; si no, no cambia nada. `null` o `""` quita usuario o secreto |
| `desbloquear` | `{ repo? }` (inofensiva): `restic unlock` (solo bloqueos antiguos) |
| `guarda_copias` | `{ activo: true, carpeta, puerto?, solo_red_local? }` · `{ activo: false }`* · `{ anadir: "<equipo>" }` · `{ quitar: "<usuario>" }`*. `anadir` exige `responder_a`: el detalle sellado lleva `{ usuario, contrasena, destino: { tipo: "rest", donde, usuario, secreto, ca_pem }, huella_tls }`, listo para `crear_repositorio` en el equipo cliente. v1.28: `{ anadir, local: true }` es para **el propio almacén** (un repositorio suyo en su almacén): `donde` es `https://localhost:<puerto>/<usuario>/` (el certificado ya cubre `localhost`); un agente anterior ignora `local` y da su IP, que también vale. Espejo nocturno: `{ espejo: { destinos: [{ tipo: "carpeta", carpeta } \| { tipo: "nube", nube, carpeta }], hora?, limite_kib? } }` (también la forma antigua `{ espejo: { carpeta, hora } }`) · `{ espejo: null }`*; quitar un destino que ya está es destructiva* |
| `actualizar_agente` | `rechazada` («próximamente») hasta que exista la llave de publicación |
| `cambiar_servidor` | `{ url, identidad, ca_pem, ficha }` (solo administradores). Contesta `en_marcha` y, cuando el equipo ya está en el nuevo, `hecha` (o `fallida` a las 24 h). §11 |
| `servidores_respaldo` | `{ servidores: [{ url, identidad, ca_pem, ficha }] (hasta 3), dias (1–30, 3 por defecto) }` (solo administradores). §11 |
| `compartir_acceso` | `{ repo, para: { equipo, box_pub }, incluir_contrasena }` (contraseña del repo + clave de administración). Detalle firmado: `{ "acceso_sellado": "<sobre para box_pub>" }`. §10 |
| `importar_repositorio` | `{ id, nombre, acceso_sellado, sign_pub_origen, contrasena? }` o, con el kit, `{ id, nombre, acceso: { destino: { tipo, donde, usuario?, secreto?, ca_pem? }, repo, contrasena } }`. §10 |
| `cambiar_copia_externa` | `{ repo, destino: { id } \| { id, nombre, tipo, donde, usuario?, secreto? }, hora: "HH:MM" \| null, retencion?, contrasena_destino? }` (contraseña del repo). Cada día a `hora`, `restic copy` a otro destino (lo crea la primera vez; la contraseña, la del origen salvo `contrasena_destino`). `hora: null` la quita (destructiva por el cuerpo). Sale en el resumen: `repositorios[].externa: { destino, hora }` |
| `rotar_contrasena_repo` | Aún no: `rechazada` («aún no admite») |
| `conectar_nube` | `{ tipo: "dropbox", nombre, refresh_token, access_token?, expira?, app_key }` (clave de administración). Solo en un equipo que guarda copias. `nombre`: letras, cifras, espacios, `_`, `.` y `-`, hasta 40. `app_key`: la pública de la app «Resguardo» (minúsculas y cifras); vacía o el marcador de la consola (`PENDIENTE_APP_KEY_DROPBOX`) → `fallida` con «aún no está configurada». El agente renueva una vez con Dropbox para comprobar el permiso (`invalid_grant`/`invalid_client` → `fallida`); si Dropbox no responde, la guarda con el `access_token` que trajo y lo dice. Se guarda protegida en el equipo; con el mismo nombre, la sustituye. El resultado **nunca** lleva el token. §12 |
| `quitar_nube` | `{ nombre }` (clave de administración). Olvida el permiso (lo subido se queda). Si el espejo la usa es destructiva* y además sale del espejo (sin más destinos, el espejo se quita). Una que no hay → `fallida` |

`sesion` (UUID) y `clave_sesion` / `relevo.clave` (32 bytes, base64) van **dentro del sobre**: el agente usa ese id, no el que el servidor diga. Las rutas de una versión son las de restic (`/C/Users/Ana/…` en Windows).

**`Retencion`** (en `cambiar_retencion`, `retencion_almacen`, `cambiar_copia_externa.retencion` y `crear_repositorio.retencion`):

```json
{ "horarias": 0, "diarias": 7, "semanales": 4, "mensuales": 12, "anuales": 2,
  "plazos": { "horarias": "15d", "diarias": "1y", "semanales": "6m", "mensuales": "2y", "anuales": "10y" } }
```

- Cantidades (`--keep-hourly/daily/weekly/monthly/yearly`): de 0 a 1000, o **`-1` = siempre** (`unlimited`: una por mes para siempre). `horarias` (v1.28) puede faltar (0).
- `plazos` (v1.28, `--keep-within-hourly/daily/weekly/monthly/yearly`): duraciones de restic (uno o más pares número+`y`/`m`/`d`/`h`: `15d`, `6m`, `1y`, `1y6m`, `48h`; ni cero ni más de 200 años). Guardan la última versión de cada hora, día… **dentro** de ese tiempo, contado hacia atrás desde la versión más reciente (no desde ahora: un equipo que deja de copiar no pierde versiones por esperar). Cada campo puede faltar.
- Al menos una regla (cantidad o plazo). «Programas contables» (horarias de los últimos 15 días, una diaria del último año y después una mensual): `{ "diarias": 0, "semanales": 0, "mensuales": -1, "anuales": 0, "plazos": { "horarias": "15d", "diarias": "1y" } }`.
- Se combinan como en restic: una versión se queda si la guarda cualquier regla. En el almacén (`retencion_almacen`) el plazo cuenta desde la versión más reciente **de confianza** (las de hora sospechosa no cuentan ni mueven el plazo: compartir.md, «Retención en el almacén»).
- Sin `horarias`, `plazos` ni `-1`, el agente la guarda, la escribe y la resume (`repositorios[].retencion`, «7 diarias · 4 semanales · 12 mensuales · 2 anuales») como antes de v1.28; con algo de eso, el texto es solo lo que guarda («horarias 15 días · diarias 1 año · mensuales siempre»).

---

## 6. Configuración, informes, avisos y auditoría

| Método y ruta | Responde |
|---|---|
| `GET /api/clientes/{c}/equipos/{e}/config` | `{ seq, cifrado (con K_cfg, b64, §1), resumen }` |
| `GET /api/clientes/{c}/plantillas` (v1.20, administrador) | `[{ id, cifrado, actualizada, por }]`: las plantillas de copia del cliente, cifradas por la consola (§1) |
| `PUT /api/clientes/{c}/plantillas/{p}` (v1.20, administrador) | `{ cifrado }` (b64, de 40 B a 64 KiB; hasta 100 por cliente) → 204. Crea o sustituye. Auditado sin contenido |
| `DELETE /api/clientes/{c}/plantillas/{p}` (v1.20, administrador) | 204, o 404. No toca ningún equipo |
| `GET /api/clientes/{c}/resumen` | `{ equipos: [Equipo], avisos_abiertos, pendientes }` (pantalla «Estado») |
| `GET /api/clientes/{c}/equipos/{e}/informes?limite=20` | `[{ recibido, datos }]` (`datos`: el informe del agente, sin rutas) |
| `GET /api/clientes/{c}/informes` | El último informe de cada equipo: `[{ equipo, recibido, datos }]` (para «Estado», sin pedir equipo a equipo) |
| `GET /api/clientes/{c}/equipos/{e}/historial?limite=500&antes=&desde=&hasta=&tipo=` (v1.23; por páginas desde v1.26) | `[EntradaHistorial]`, de la más reciente a la más antigua (por `hora` y, a igual hora, por `id`). `limite`: 500 si no se dice, como mucho 2000. `antes`: el `id` de la última entrada de la página anterior (cursor; si ya no existe, `[]`). `desde`/`hasta` (RFC 3339): `desde < hora ≤ hasta`. `tipo`: uno o varios separados por comas (422 si alguno no existe). Una página con menos de `limite` entradas es la última. Ver «Historial del equipo» |
| `GET /api/clientes/{c}/progreso` (v1.25) | Lo que está en marcha ahora en sus equipos: `[{ equipo, recibido, tareas: [Tarea] }]` (cualquier papel). Solo en memoria; un equipo sin noticias en 90 s no sale |
| `GET /api/clientes/{c}/avisos?abiertos=1` | `[{ id, equipo, tipo, mensaje, creado, visto_por }]` |
| `POST /api/clientes/{c}/avisos/{a}/visto` | — |
| `GET /api/clientes/{c}/auditoria?desde=n&limite=100` | Técnico o más. De la más antigua a la más reciente, `n > desde`: `[{ n, creado, actor, accion, objetivo, datos, hash, prev_hash }]` |
| `GET /api/clientes/{c}/auditoria?orden=desc&antes=n&limite=100` | Técnico o más. De la más reciente hacia atrás, `n < antes` (sin `antes`: desde la última). Para la página siguiente, `antes` = el `n` más bajo recibido |
| `GET /api/clientes/{c}/auditoria/verificar` | Técnico o más. `{ ok: true, entradas }`, o `{ ok: false, rota_en: n }` |

Exportar la auditoría (cuando exista) será solo para administradores y propietarios.

**Configuración del equipo** (`Configuracion` v1, lo que va cifrado; sin secretos):

```json
{
  "v": 1,
  "copias": [{ "id", "nombre", "repo", "carpetas": ["C:\\Users\\Ana\\Documents"], "exclusiones": [],
               "horario": { "dias": [1, 2, 3, 4, 5], "horas": ["13:00"] }, "activa": true, "gancho": null, "solo_si_cambios": true }],
  "repositorios": [{ "id", "nombre", "destino": "<id de destino>", "retencion": Retencion | null }],
  "destinos": [{ "id", "nombre", "tipo", "donde" }],
  "verificacion": null, "bandeja": null,
  "verificaciones": { "<id del repositorio>": { "cada_dias": 7, "porcentaje": 10 } },
  "escritorio": { "ventana": "off" | "siempre_disponible" | "al_trabajar", "avisos": "off" | "errores" | "todo" },
  "cambiado_en_equipo": "<RFC 3339>"
}
```

- La consola manda en `config` solo lo que decide: `copias` (y `verificaciones`, `bandeja`). `repositorios` y `destinos` los escribe el equipo a partir de `crear_repositorio`; los que mande la consola se ignoran. `verificacion` (global) no lo usa nadie: el agente lo guarda tal cual.
- **Verificación automática** (v1.28, `verificaciones`): por repositorio, cada `cada_dias` días (1–31) `restic check` leyendo un `porcentaje` de los datos (0–100) **rotativo**: el agente lo parte en `round(100/porcentaje)` partes (de 2 a 52) y cada vez lee la siguiente (`--read-data-subset n/t`), así en esas vueltas lee todo; 0 %: solo la estructura; 100 %: todo cada vez. La primera, a las 03:00 siguientes; después, `cada_dias` días desde el comienzo de la anterior (también si fue un «Verificar ahora»). Va con las copias del agente: un repositorio sin copias activas no se verifica solo (se pone cuando la tenga). Sin el campo (consola anterior) no se toca la que haya; con él, la de un repositorio que no está se quita, y la que no cambia no vuelve a empezar. Un repositorio que no es del equipo o es importado, o `cada_dias` fuera de 1–31 → la configuración entera `fallida`. Agentes anteriores ignoran el campo: la consola solo lo manda si el resumen trae `admite: ["verificacion_auto", …]`.
- **Ventana y avisos en el equipo** (v1.36, `escritorio`; docs/agente-ventana.md): `ventana` (`off`: sin ventana; `siempre_disponible`: «Abrir Resguardo» en el icono; `al_trabajar`: además se abre sola al empezar una copia, restauración, verificación o subida) y `avisos` (`off`; `errores`: al fallar y al recuperarse; `todo`: también al empezar y al terminar). Sin el campo (consola anterior) se queda el que haya; sin ninguno, se deduce de `bandeja` (`avisos` → `errores` u `off`; `visible` → `siempre_disponible` u `off`). La consola manda también `bandeja.avisos = avisos != "off"` para un agente anterior, y solo manda `escritorio` si el resumen trae `admite: ["escritorio", …]`. Un valor desconocido o un campo de más → la configuración entera `fallida`. **Desde el equipo**: con la clave de administración (la ventana, por un canal local autenticado), el equipo cambia `escritorio`, pone `cambiado_en_equipo` y sube la configuración como siempre; una `config` de la consola lo quita. En **modo local** (sin consola) el equipo se configura entero así; al vincularlo con la misma clave, la consola lo adopta con todo (§3.5, camino 3).
- Cada copia tiene que usar un repositorio que el equipo ya tenga. `dias`: 1 = lunes … 7 = domingo.
- **Horario con reglas** (v1.24, agente ≥ 0.7.9): `horario.reglas` (opcional, hasta 20) se suman; toca cuando toca cualquiera:
  - `{ "tipo": "horas", "dias": [1, 2, 3, 4, 5], "horas": ["08:00", "13:00"] }`: a estas horas (hasta 48), esos días.
  - `{ "tipo": "intervalo", "dias": [1, 2, 3, 4, 5], "cada_min": 10, "desde": "08:00", "hasta": "18:00" }`: cada `cada_min` minutos de `desde` a `hasta` (las dos incluidas, el mismo día). `cada_min`: 5, 10, 15, 20, 30, o horas enteras (60, 120 … 1440).
  - `{ "tipo": "cada_dias", "cada": 3, "inicio": "2026-10-05", "hora": "23:00" }`: cada `cada` días (1 a 365) contando desde `inicio` (la primera vez; días de calendario).
  - `{ "tipo": "mensual", "dia": 1, "hora": "23:00" }`: el día `dia` de cada mes (1 a 28; `-1` = el último día del mes).

  Con alguna regla, el agente usa las reglas e ignora `dias`/`horas` (pueden ir vacíos; la consola los rellena con la lista desplegada cuando se puede, para consolas anteriores). Una regla que no vale (o de un tipo desconocido) hace que se rechace la configuración entera. Sin `reglas`, todo como antes (`dias` × `horas`). Las horas son las del equipo: una que no existe al adelantar el reloj se hace en el primer minuto que existe; la que se repite al atrasarlo, una vez (los intervalos de minutos, en las dos). Si el equipo estuvo apagado o dormido, se hace **una** copia al volver, aunque se hayan perdido varias; entre el comienzo de dos copias de la misma copia pasan al menos 5 minutos y nunca se empieza una mientras otra está en marcha. `proximas` y `resumen.copias[].horario` llevan las reglas tal cual.
- **Ganchos** (`gancho`): `null`, un gancho o una lista de hasta 4. Solo **plantillas cerradas**; cualquier otro tipo o campo hace que el agente rechace la configuración entera (nunca hay órdenes libres):
  - `{ "tipo": "sqlserver", "instancia": "." | "EQUIPO\\INSTANCIA" | "EQUIPO,puerto", "bases": ["…"], "carpeta": "C:\\ResguardoVolcados" }`. Antes de la copia, `BACKUP DATABASE [base] TO DISK = N'<carpeta>\resguardo-<base>.bak' WITH COPY_ONLY, INIT, FORMAT, CHECKSUM` de cada base, con el `sqlcmd` de la instalación de SQL Server (en Archivos de programa, nunca del PATH) y autenticación de Windows (la cuenta del equipo: `NT AUTHORITY\SYSTEM` necesita el rol `db_backupoperator` en cada base, o `sysadmin`). La carpeta entra en la copia y los volcados se borran al terminar (solo esos archivos). La carpeta es **solo para los volcados**: en cada copia el agente la deja solo para SYSTEM, Administradores y el servicio de SQL Server (`NT SERVICE\MSSQLSERVER` o `NT SERVICE\MSSQL$<INSTANCIA>`; si SQL Server arranca con otra cuenta, el volcado falla). Tiene que ser de un disco del equipo, no su raíz ni de red, y sin enlaces en el camino. Nombres de base: letras, números, espacio, `_ - . $` (hasta 128; hasta 20 bases). `COPY_ONLY` no rompe la cadena de copias propia de SQL Server. Un volcado que falla deja la copia en `fallo` (el resto de carpetas se copia igual).
  - `{ "tipo": "carpeta_reciente", "carpeta": "D:\\WO\\Copias", "horas": 26, "extension": ".bak" | null }`. Comprueba que el archivo más nuevo de la carpeta (hasta 3 niveles) es de las últimas `horas` (1 a 720); si no, la copia termina en `aviso`. No cambia lo que se copia (la carpeta se marca aparte en `carpetas` si se quiere copiar).
  - Un gancho «parar y arrancar un servicio» no se ofrece: con VSS (instantánea) los archivos abiertos ya se copian, y parar una aplicación en horario de trabajo es más arriesgado que útil. Si alguna vez hace falta, irá con una lista de servicios permitidos fijada en el equipo, no en la consola.
- La retención no se cambia con `config` (reduce la protección): con `cambiar_retencion`.
- **Secretos:** `config` nunca lleva contraseñas ni credenciales. Un repositorio nuevo entra con `crear_repositorio` (que ejecuta `restic init`, o se conecta al que ya exista si la contraseña lo abre), y desde ahí la consola solo pide su contraseña para las órdenes de nivel repositorio. Cambiar la ubicación de un repositorio o las credenciales de un destino exige volver a dar los secretos: hoy, con un repositorio nuevo por `crear_repositorio` (un destino nuevo con otro id); una orden `cambiar_destino` queda pendiente.
- El formato de F2 (`config { repos: [{ …, contrasena, … }] }`) ya no se acepta: el agente responde «Configuración no válida».

**Informe del agente** (`datos`):

```json
{ "version", "servicio": "en_marcha", "so", "copias": [{ "id", "repo", "nombre", "estado": "ok" | "aviso" | "fallo", "cuando", "mensaje",
              "ganchos": [{ "tipo": "sqlserver" | "carpeta_reciente", "estado": "ok" | "aviso" | "fallo", "mensaje" }] }],
  "proximas": { "<id de la copia>": "<RFC 3339>" | null },
  "repos": [RepoInforme] }
```

**`RepoInforme`** (solo metadatos: sin rutas ni nombres de archivos; los mensajes van sin rutas):

```json
{
  "id", "nombre", "solo_lectura": false,
  "versiones": [{ "id": "<8 hex>", "hora", "copia": "<id de la copia>" | null, "total_bytes", "anadido", "anadido_empaquetado",
                  "archivos_nuevos", "archivos_cambiados", "archivos_sin_cambios", "duracion_s", "etiquetas": [] }],
  "versiones_leidas": "<RFC 3339>" | null,
  "ejecuciones": [{ "hora", "copia", "resultado": "ok" | "aviso" | "fallo" | "sin_cambios", "mensaje_corto",
                    "duracion_s", "anadido", "archivos_nuevos", "archivos_cambiados", "reintento": false }],
  "espacio": { "en_disco_bytes", "sin_comprimir", "ratio": 2.4 | null, "leido" } | null,
  "verificacion": { "ultima", "resultado": "ok" | "aviso" | "fallo", "mensaje_corto" } | null,
  "prueba_restauracion": { … igual } | null,
  "externa": { … igual } | null,
  "proteccion": { "puntuacion": 5, "total": 7, "items": [{ "id": "copias" | "borrado" | "externa" | "verificacion" | "restauracion" | "kit" | "retencion",
                  "estado": "ok" | "aviso" | "fallo" | "desconocido", "etiqueta", "detalle" }] },
  "recortado": true
}
```

- `versiones` y `ejecuciones`: los últimos 60 días, la más reciente primero, como mucho 500 y 400. Los campos numéricos de una versión pueden ser `null` (versiones de restic < 0.17 sin resumen).
- `versiones` y `espacio` salen de restic y el agente los guarda en caché: las versiones se releen cuando termina una copia (o cada 6 h), el espacio una vez al día. `versiones_leidas` y `espacio.leido` dicen de cuándo son. Desde v1.30 (agente ≥ 0.7.14), al terminar una copia el agente relee sus versiones **antes** de mandar el informe inmediato (como mucho 20 s), así que ese informe y el resumen ya las cuentan; y con la pista `refrescar` (§8) relee las versiones y el espacio del repositorio en el que un almacén acaba de aplicar la retención.
- `proteccion`: las mismas reglas que la app de escritorio (`protection.rs`). En un equipo gestionado el kit cuenta como guardado (la consola lo muestra al crear el repositorio y pide confirmarlo) y la retención es la de `cambiar_retencion`.
- `proximas` (v1.12): la próxima vez que toca cada copia de la configuración; `null` si está desactivada o su repositorio está en pausa sin fecha (con fecha, la primera hora después de la pausa). Los campos numéricos de `ejecuciones` (v1.12) pueden ser `null` (vueltas que fallaron antes de empezar o de un agente anterior).
- El informe entero ocupa como mucho ~200 KiB: si no cabe, se recortan a la mitad las ejecuciones y las versiones más antiguas (y `recortado: true`).
- `ultimo_seq` (agente ≥ 0.7.13): el último número de orden que aceptó el equipo. Si el servidor recuerda uno anterior (p. ej. tras restaurar la copia de la consola), sube su `siguiente_seq` hasta el siguiente: si no, el equipo rechazaría las órdenes nuevas por «repetidas». Nunca lo baja. Un agente anterior no lo manda.
- `progreso` (v1.25): `[Tarea]`, lo que estaba en marcha al hacerlo (no sale si nada). Es el camino de reserva del progreso en vivo (§8); el servidor lo toma para `GET …/progreso` y un informe sin él lo vacía.

**`Tarea`** (v1.25, progreso en vivo; sin rutas ni nombres de archivos):

```json
{ "tipo": "copia" | "verificar" | "verificar_externa" | "copia_externa" | "prueba_restauracion",
  "repo": "<id>", "copia": "<id de la copia>" | null, "nombre": "<nombre de la copia>" | null,
  "fase": "antes_de_copiar" | "preparando" | "escaneando" | "subiendo" | "terminando" | "en_marcha",
  "etapa": "<qué hace, en palabras>" | null, "porcentaje": 0.42 | null,
  "archivos", "archivos_total", "bytes", "bytes_total", "velocidad": "<bytes/s>",
  "lectura": "<bytes/s>", "subida": "<bytes/s>", "archivos_s", "quedan_s",
  "versiones", "versiones_total", "empezo": "<RFC 3339>", "actualizado": "<RFC 3339>" }
```

- v1.36 (copias): `lectura` (lo que restic lee del disco) y `subida` (lo que escribe o sube al destino), en bytes/s medidos en el proceso de restic (E/S del proceso; en Windows la red cuenta como «otra» E/S), y `archivos_s` (archivos por segundo), suavizados. Para las gráficas en vivo de la consola (`GraficaOndas`); un agente anterior no los manda y un servidor anterior los quita (la consola enseña la barra de siempre).
- v1.37: `lectura` y `subida` también en `verificar`, `verificar_externa`, `prueba_restauracion` y `copia_externa`, de los contadores de E/S del restic que corre (como en las copias). Sin ellos (agente anterior, o un sistema que no los da), la consola los deduce de `bytes`, como hasta ahora.
- Copias: `antes_de_copiar` mientras corren los ganchos, `preparando` sin cifras de restic todavía, `escaneando` mientras restic aún cuenta (copia a la vez, sin `quedan_s`), `subiendo` con total y `quedan_s`, `terminando` al guardar la versión. Las demás tareas: `preparando` o `en_marcha`, con `etapa`. `versiones*` solo en `copia_externa`.
- Todos los campos salvo `tipo`, `repo` y `fase` pueden faltar o ser `null`. El servidor deja solo estos campos (textos cortos, números enteros no negativos, `porcentaje` entre 0 y 1), como mucho 8 tareas y 16 KiB por mensaje.

**Historial del equipo** (v1.23, `EntradaHistorial`). Cada equipo guarda **para siempre** lo que cuenta a la consola (`crates/agente/src/bitacora.rs`) y se lo da a cada consola nueva (otro servidor, uno restaurado o tras volver a vincular): así ve la Historia, los avisos y lo de antes de llegar. Sin rutas ni secretos (los mensajes, como `public_message`).

```json
{ "id", "hora", "tipo": "copia" | "resumen_dia" | "verificacion" | "prueba_restauracion" | "externa" | "espejo" | "aviso",
  "repo"?, "copia"?, "resultado"?: "ok" | "aviso" | "fallo" | "sin_cambios", "mensaje"?,
  "duracion_s"?, "anadido"?, "archivos_nuevos"?, "archivos_cambiados"?, "reintento"?, "ganchos"?: [{ tipo, estado, mensaje }],
  "aviso"?: "<tipo de aviso>",
  "dia"?, "ok"?, "fallidas"?, "sin_cambios"?, "ultimo_error"? }
```

- En el equipo: un archivo por mes (`privado/bitacora/AAAA-MM.jsonl`), en el que solo se añade; los últimos 12 meses con todo el detalle; lo anterior, una entrada `resumen_dia` por copia y día (id fijo `dia-<repo>-<copia>-<AAAA-MM-DD>`); avisos, verificaciones, pruebas, copia externa y espejo, siempre. Se compacta una vez al día (cada archivo en uno temporal que se cambia de nombre). Tope de 50 MB: primero se resumen meses más recientes y, si aun así no cabe, se quitan los más antiguos (y se anota en el registro).
- El servidor guarda cada entrada una vez por `id` (subirla otra vez no la repite), hasta 20 000 por equipo (las más antiguas se van) y sin borrarlas por antigüedad. Las de tipo `aviso` entran también en la lista de avisos, **ya vistas** (`visto_por: "la consola anterior"`), salvo que ya hubiera uno igual del equipo con menos de 10 min de diferencia (el que llegó en su momento).
- Cómo llega: §8, «Historial del equipo».

**Tipos de aviso:**
- `intentos_fallidos`;
- `bloqueo`;
- `orden_destructiva` (pendiente);
- `equipo_sin_contacto` (más de 24 h);
- `copia_fallida`;
- `copia_atrasada`;
- `servicio_detenido`;
- `cambio_inusual`;
- (v1.29; los crea el servidor, ver §13) `verificacion_fallida`, `externa_fallida`, `prueba_fallida`, `espejo_fallido` y `cambio_clave`. `copia_fallida` también la crea el servidor a partir de los informes.

---

## 7. Sesiones interactivas (elegir carpetas, explorar, progreso)

La consola elige un `sesion` (UUID) y lo manda con la orden que la abre (`abrir_sesion`, `explorar` o `elegir_carpetas`), en claro (para que el servidor cree la sesión) y **dentro del sobre** con `clave_sesion`. Los mensajes van **cifrados de extremo a extremo** (§1); el servidor solo los reenvía.

**Mensajes** (JSON, antes de cifrar):
- de la consola: `{ "i": 1, "op": "…", … }`;
- del equipo: `{ "i": 1, "re": <i de la petición>, "op": "…", … }` o `{ …, "error": "…" }`.
- `i` empieza en 1 y crece en cada lado; se descarta lo que no crezca. El equipo empieza con `{ "op": "lista", "ops": [ … ] }`: desde v1.15, `ops` dice qué operaciones admite esa sesión (un agente anterior no lo manda: entonces solo valen las de v1.2 y la consola no ofrece lo nuevo).

| `op` | En | Pide | Responde |
|---|---|---|---|
| `carpetas` | `elegir_carpetas` | `{ ruta }` (vacía: las unidades, o `/`) | `{ entradas: [{ nombre, tipo: "dir" \| "archivo", bytes?, modificado?, sistema }] }` |
| `sugerencias` | `elegir_carpetas` | — | `{ sugerencias: [{ id, nombre, detalle?, rutas }] }` |
| `crear_carpeta` (v1.15) | `elegir_carpetas` | `{ ruta, nombre }` | `{ ruta, ya_existia }`. Crea `nombre` dentro de `ruta` (una carpeta de un disco del equipo, o la raíz de una unidad). Nombre de 1 a 100 caracteres, sin `\ / : * ? " < >` ni barra vertical, sin punto ni espacio al final ni nombres reservados de Windows; nada de red, enlaces en el camino ni carpetas del sistema, de los programas o de Resguardo. Si ya existe como carpeta, solo se devuelve (`ya_existia: true`) |
| `versiones` | `explorar` | — | `{ versiones: [{ id, cuando, archivos?, bytes?, etiquetas }] }` (la más reciente primero) |
| `listar` | `explorar` | `{ version, ruta }` | `{ entradas: [{ nombre, tipo, bytes?, modificado? }] }` |
| `buscar` | `explorar` | `{ version, texto }` | `{ resultados: [{ ruta, nombre, tipo, bytes?, modificado? }] }` (hasta 500) |
| `diferencias` (v1.33) | `explorar` | `{ hasta, desde?, tamanos?: true, indice?: 0 }` | `{ desde, hasta, desde_cuando, resumen: { nuevos, cambiados, borrados, metadatos, otros, carpetas_nuevas, carpetas_borradas, bytes_anadidos, bytes_quitados }, total, recortado, con_tamanos, indice, siguiente, cambios: [{ ruta, tipo: "nuevo" \| "cambiado" \| "borrado" \| "metadatos" \| "otro", bytes?, bytes_antes? }] }`. Qué archivos cambiaron de `desde` a `hasta` (`restic diff --json`; con `tamanos`, el tamaño antes y después con `restic ls --json` de las dos). Sin `desde`, la versión anterior de la misma copia (mismo equipo y mismas carpetas); si es la primera, `{ hasta, desde: null, primera: true }`. Se calcula una vez por sesión y se da por páginas de ≤ 150 KiB y ≤ 1000 cambios: `indice` es el primero de la página y `siguiente` el de la siguiente (`null`: no hay más). Como mucho 20 000 cambios con su ruta (`recortado`); `resumen` cuenta todos. Las carpetas solo se cuentan. Hasta 10 min por orden de restic |
| `ocupa` (v1.33) | `explorar` | `{ version }` | `{ version, total_bytes, total_archivos, carpetas: [{ ruta, bytes, archivos }], archivos: [{ ruta, bytes, archivos }] }`: las 50 carpetas y los 50 archivos más grandes de la versión (`restic ls --json`; sin las carpetas que solo envuelven a otra) |
| `historial_archivo` (v1.33) | `explorar` | `{ ruta }` (de versión, absoluta, sin `.`, `..` ni control) | `{ ruta, versiones: [{ version, cuando, bytes, modificado }] }`: las versiones en las que está ese archivo, la más reciente primero, hasta 500 (`restic find`) |
| `cerrar` | todas | — | (cierra, sin respuesta) |

En `explorar` solo se ve el repositorio autorizado; las demás operaciones responden `error`. El equipo cierra la sesión tras 10 min sin mensajes de la consola.

Mientras una operación tarda (v1.33), el equipo manda cada 20 s `{ "i", "op": "trabajando", "sobre": <i de la petición> }`, **sin** `re` (una consola anterior lo descarta): la consola vuelve a contar su plazo de espera y la sesión no caduca. Las rutas y nombres de archivo de `diferencias`, `ocupa` e `historial_archivo` son datos del cliente: solo viajan dentro de la sesión cifrada, y la consola no los pone en la URL.

| Método y ruta | Quién | Hace |
|---|---|---|
| `POST /api/clientes/{c}/sesiones/{s}/mensajes` | consola | `{ cifrado }` → `{ n }` |
| `GET /api/clientes/{c}/sesiones/{s}/mensajes?desde=n` | consola | **Espera larga** (hasta 25 s): `[{ n, de: "equipo" \| "consola", cifrado }]` |
| `DELETE /api/clientes/{c}/sesiones/{s}` | consola | Cierra |

Por el lado del equipo, lo mismo llega por el WebSocket (§8) o con `GET/POST /api/agente/sesiones/{s}/mensajes` (sondeo). Las sesiones caducan a los 10 min sin mensajes. Cada mensaje tiene como mucho 256 KiB.

---

## 8. Canal del agente

### WebSocket: `GET /api/agente/canal?reto=<b64>[&ultimo_seq=<n>]`

Con la cabecera `Authorization: Equipo …`. Mensajes JSON de texto.

`ultimo_seq` (v1.35, agente > 0.7.15): el último número de orden que el equipo aceptó **de este servidor** (con varias consolas, cada una el suyo). Como el del informe, sube `siguiente_seq` y nunca lo baja, pero **antes** de abrir el canal: el equipo no cuenta como conectado hasta que el número está al día, y la primera orden que la consola firma al verlo conectado tras restaurar su copia no sale «repetida» (el informe llega un momento después). Un agente anterior no lo manda.

**Del servidor al agente:**

| Mensaje | Cuándo |
|---|---|
| `{ "t": "hola", "firma": "<prueba de identidad>", "atencion": false, "historial": { "ultima": "<RFC 3339>" \| null } }` | Primero. El agente comprueba la firma con la identidad fijada; si falla, cierra. `historial` (v1.23): hasta dónde tiene el historial de ese equipo (`null`: nada) |
| `{ "t": "orden", "orden": { id, tipo, seq, sellado, not_before, caduca } }` | Orden nueva |
| `{ "t": "cancelada", "orden": "<id>" }` | Se canceló una orden |
| `{ "t": "sesion", "sesion": "<id>", "n", "cifrado" }` | Mensaje de la consola |
| `{ "t": "sesion_cerrada", "sesion": "<id>" }` | La consola cerró una sesión |
| `{ "t": "error", "error", "mensaje" }` | Un mensaje del agente no se pudo procesar |
| `{ "t": "ping" }` | Cada 30 s |
| `{ "t": "refrescar", "repo": "<id>" }` (v1.30) | Al equipo **dueño** de un repositorio en un almacén, cuando el resumen del almacén trae un resultado nuevo de su retención (`guarda_copias.retenciones[]`: otra `ultima` u otras `versiones`). El servidor busca el dueño entre los equipos del cliente por su destino (`equipo_almacen`, o el id/nombre del almacén), el usuario de `donde` y la carpeta (`ruta` o el id). Es solo una pista: el agente relee las versiones y el espacio de ese repositorio (si es suyo, como mucho una vez por minuto) y su informe sale con lo nuevo; no hace nada más. Solo por el WebSocket; un agente anterior la ignora |

**Del agente al servidor:**

| Mensaje | Qué lleva |
|---|---|
| `{ "t": "resultado", "orden", "estado", "mensaje", "detalle", "firma" }` | El resultado de una orden |
| `{ "t": "informe", "datos": { … } }` | Su informe |
| `{ "t": "config", "seq", "cifrado", "resumen", "etiqueta"?, "espera_min_horas"? }` | Su configuración. v1.35: `etiqueta` (la calcula el equipo con la `K_cfg` de este servidor) y `espera_min_horas` (la que aplica): si el equipo está confirmado y cambiaron (p. ej. la clave o la espera se cambiaron desde otra consola), el servidor las guarda y lo audita (`etiqueta_equipo`, `espera_confirmada`). Un servidor anterior las ignora |
| `{ "t": "sesion", "sesion", "cifrado" }` | Mensaje para la consola |
| `{ "t": "aviso", "tipo", "mensaje" }` | Un aviso |
| `{ "t": "progreso", "tareas": [Tarea] }` (v1.25) | Lo que está en marcha (§6, `Tarea`): cada 5 s mientras dura y una vez vacío al terminar. Solo en memoria del servidor, para las consolas; 60 por minuto como mucho |
| `{ "t": "pong" }` | Respuesta al ping |

### Sondeo (si el WebSocket no pasa)

| Método y ruta | Hace |
|---|---|
| `POST /api/agente/tomar` | `{ reto, ultimo_seq? }` (`ultimo_seq`, v1.35: como en el canal, §8) → `{ firma, ordenes: [Orden en bruto], canceladas: [ids], atencion: bool, sesiones: [ids de sesiones abiertas], historial: { ultima } }` (`historial`, v1.23). Marca las órdenes como entregadas. Los mensajes de cada sesión se leen con `GET /api/agente/sesiones/{s}/mensajes` |
| `POST /api/agente/resultado` | Igual que el mensaje `resultado` |
| `POST /api/agente/informe` | Igual que `informe` |
| `POST /api/agente/config` | Igual que `config` |
| `POST /api/agente/aviso` | Igual que `aviso` |
| `POST /api/agente/historial` (v1.23) | `{ entradas: [EntradaHistorial] }` (hasta 500, 4 KiB cada una, desde el año 2000) → `{ nuevas, ultima }`. Idempotente. Los avisos que trae pasan a la lista de avisos ya vistos, como mucho 1000 por equipo y día (el resto queda solo en el historial) y nunca con fecha futura. También con el WebSocket abierto (por HTTP) |
| `POST /api/agente/progreso` (v1.25) | `{ tareas }`, igual que `progreso`: cada 10 s mientras algo está en marcha → 204 |
| `POST /api/agente/sesiones/{s}/mensajes` | `{ cifrado }` |
| `GET /api/agente/sesiones/{s}/mensajes?desde=n` | Espera larga |

**Intervalos:** cada 60 s, o cada 2 s si `atencion` es verdadero o hay una sesión abierta.

**Historial del equipo (v1.23).** Con `historial.ultima` de `hola` o de `tomar`, el agente sube lo que falte con `POST /api/agente/historial`: si el servidor no tiene nada (`null`), **todo**, de lo más reciente a lo más antiguo y en tandas de 500, y lo repite (sin duplicar) hasta terminar una subida entera a ese servidor; después, solo lo posterior a `ultima` (con 10 min de margen), al mandar cada informe. Un servidor anterior no manda `historial` y el agente no sube nada; un agente anterior no sube nada y el servidor no lo echa en falta.

---

## 9. Relé de descargas (restaurar al navegador)

La consola envía la orden `descargar` con `relevo: { id, max_bytes }`.

**El agente:**
- **sube** cada trozo cifrado con `PUT /api/agente/relevos/{r}/trozos/{n}` (cuerpo binario, como mucho 4 MiB + 64 bytes; `n` desde 0);
- **cierra** con `POST /api/agente/relevos/{r}/fin`, con `{ trozos, bytes }`.

**La consola:**
- **consulta** con `GET /api/clientes/{c}/relevos/{r}`, que responde `{ estado: "subiendo" | "listo" | "caducado", trozos, bytes }`;
- **descarga** con `GET /api/clientes/{c}/relevos/{r}/trozos/{n}` (binario);
- **borra** con `DELETE /api/clientes/{c}/relevos/{r}` al terminar.

Límites:
- Configurable: 500 MB por relé por defecto, 2 GB en total en el servidor.
- Caduca 1 h después de terminar la subida.
- Cifrado de cada trozo como en §1, con `relevo.clave` (32 bytes de la consola, dentro del sobre de la orden). El servidor solo ve trozos opacos.
- El agente lo genera con `restic dump` (un archivo tal cual, o una carpeta con `--archive zip`).

---

## 10. Restaurar en otro equipo

Caso: el equipo A se perdió o hay que recuperar sus copias en B. La consola no tiene las credenciales del destino de A (solo las conoce A).

1. **Con A vivo.** La consola manda a **A** `compartir_acceso { repo, para: { equipo: B, box_pub_B }, incluir_contrasena }` (contraseña del repositorio + clave de administración). Antes comprueba las claves de B con su etiqueta (o las que tiene fijadas), como en todo lo que se sella.
   - A arma `datos = { repo: { id, nombre, ubicacion? }, destino: { id, nombre, tipo, donde, usuario, secreto, ca_pem }, contrasena? }` (la contraseña, solo con `incluir_contrasena`; `ubicacion`, v1.14: su carpeta en el destino si no es su `id`, en los adoptados e importados; B la usa si viene).
   - Lo firma: Ed25519 de A sobre `"resguardo-acceso-v1|" + equipo_B + "|" + datos`.
   - Lo sella para B: `crypto_box_seal(box_pub_B, { "datos": "<json>", "firma": "<b64>", "de": equipo_A })`.
   - Responde `detalle: {"acceso_sellado": "<b64>"}` (firmado con el resultado). El servidor solo ve el sobre.
2. La consola manda a **B** `importar_repositorio { id, nombre, acceso_sellado, sign_pub_origen: sign_pub_A, contrasena? }` (clave de administración; `sign_pub_A` comprobada o fijada en la consola). B abre el sobre, comprueba la firma de A, comprueba que el repositorio se abre y lo añade **solo de lectura**: se puede explorar, restaurar y descargar, pero ninguna copia puede escribir en él.
3. **Sin A** (perdido): la consola pide los datos del kit de recuperación y manda `importar_repositorio { id, nombre, acceso: { destino, repo: "<id del repositorio en el destino>", contrasena } }`, todo dentro del sobre.

---

## 11. Cambiar de servidor, respaldo y exportar

### Recibir un cliente (servidor nuevo)

| Método y ruta | Rol | Pide | Responde |
|---|---|---|---|
| `POST /api/clientes/recibir` | superusuario | `{ nombre, sal_cliente, espera_min_horas?, usos? (1–1000, 100), dias? (1–365, 7) }` | `{ cliente: { id, nombre, sal_cliente, espera_min_horas }, ficha, caduca, usos, servidor: { identidad, ca_pem } }` |
| `POST /api/clientes/{c}/fichas` | propietario | `{ usos?, dias? }` | `{ ficha, caduca, usos, servidor }` (más equipos, o una ficha larga para usar este servidor como respaldo) |

- `sal_cliente` es **la del servidor anterior**: `K_cfg` y `K_exp` dependen de ella, así que la misma clave de administración sigue valiendo.
- La ficha se muestra una vez (el servidor guarda su hash). Va dentro de la orden sellada, nunca en claro por el servidor antiguo.

### Agente: `POST /api/agente/recibir` (anónima, con límite por IP)

- Pide: `{ ficha, equipo_id, nombre, so, version, box_pub, sign_pub, sal_equipo, etiqueta, espera_min_horas?, motivo? }`. v1.35: `motivo: "anadir_consola"` si el equipo se conecta también aquí sin dejar su consola (solo cambia el texto del aviso: «se conectó también a este servidor»; la auditoría lo anota).
- El equipo calcula su **etiqueta** con su `K_cfg` (el mismo HMAC de §1) y conserva su **id**: la consola la comprueba como siempre y sus claves fijadas siguen valiendo.
- El equipo entra **ya confirmado**. Si ese id ya existe en el mismo cliente con las mismas claves (vuelve a un servidor donde estuvo), se reactiva; con otras claves u otro cliente: 409.
- Responde: `{ equipo_id, secreto, cliente_id, servidor: { identidad, nombre } }`. Se crea un aviso `cambio_inusual` («llegó a este servidor»).

### `cambiar_servidor` (orden, en dos fases)

1. En el nuevo, «Recibir un cliente» da la ficha, la identidad y la autoridad TLS. La consola manda `cambiar_servidor { url, identidad, ca_pem, ficha }` a cada equipo desde el antiguo.
2. El equipo contesta `en_marcha` y, en cada vuelta:
   - se da de alta en el nuevo (`/api/agente/recibir`, con la autoridad TLS fijada desde la orden);
   - comprueba que el nuevo tiene la identidad de la orden y la demuestra con un reto;
   - sube su configuración cifrada y su informe;
   - **entonces** confirma `hecha` al antiguo (que lo marca `modo: "trasladado"`) y pasa al nuevo, con `seq` desde 1.
3. Si en 24 h no lo consigue, contesta `fallida` al antiguo, crea un aviso y **sigue en el antiguo**.

No es destructiva: no espera, pero solo la mandan administradores y propietarios.

### Varias consolas a la vez (v1.35)

Un equipo puede estar vinculado a la vez a varios servidores (hasta 5), cada uno por su lado: la consola del cliente y una en línea, por ejemplo. Diseño, seguridad y límites: [consolas-multiples.md](consolas-multiples.md).

- **Código de conexión** (lo arma la consola, no hay ruta nueva): `"RGC1." + base64url(JSON)` con `{ v: 1, u: url_agentes de esa consola (v1.34; si no la da, su dirección), i: identidad, h: huella_ca (64 cifras hex), f: ficha, s: sal_cliente, n: nombre de la consola, k: nombre del cliente, c: caduca }`. La consola que lo da usa `POST /api/clientes/recibir` (cliente nuevo, con una sal nueva) o `POST /api/clientes/{c}/fichas` (cliente que ya existe) y `GET /api/servidor` (identidad y huella). La que lo recibe calcula la `K_cfg` de la otra (misma clave, su sal) y manda `anadir_consola` a cada equipo.
- **Servidor**: nada supone ser la única consola. Cada uno guarda solo lo suyo y numera sus órdenes desde 1; el equipo lleva un `seq` por consola, un `nonce` común (un sobre de una consola no vale en otra) y bloqueos por intentos por consola.
- **Configuración**: el equipo la aplica en el orden en que llegan las órdenes y la sube a todas (cada una con su `K_cfg`); `resumen.cambio_config` dice de cuál vino.
- **Servidores de respaldo y `cambiar_servidor`**: de cada consola. No se puede cambiar a un servidor que ya es otra de sus consolas.
- **CLI del agente**: `resguardo-agente consolas` (la lista) y `consolas quitar <n.º | dirección | huella>` (para una consola que ya no existe).

### Servidores de respaldo

`servidores_respaldo { servidores: [{ url, identidad, ca_pem, ficha }], dias? }` (hasta 3; `dias` de 1 a 30, 3 si falta). Cada respaldo lleva una ficha de ese servidor (`POST /api/clientes/{c}/fichas`, hasta 365 días). Si el principal no responde en `dias` días, el equipo se da de alta en el primer respaldo que le acepte (como en `cambiar_servidor`) y lo quita de la lista.

### Volver a vincular con un código (sin el servidor anterior)

Para cuando el servidor anterior ya no está y no había respaldo. En el nuevo se crea el cliente (o se recibe con su sal, si se guardó el bloque) y se añade cada equipo como siempre: código, `resguardo-agente vincular <código> --servidor <url>`, número de comprobación, **la misma clave de administración**, etiqueta y `alta`.

- Si el equipo **ya tiene clave de administración** (verificador), `vincular` no cambia de dueño: guarda el vínculo nuevo como **pendiente** y sigue con su configuración y con el servidor de antes. Mientras, el nuevo no recibe ni la configuración ni el resumen (solo lo de `unirse`), y cualquier orden que no sea `alta` se contesta `rechazada`.
- El `alta` se acepta solo si su verificador es **el que ya tiene** el equipo (y trae la prueba del código). Entonces el equipo pasa al nuevo (`seq` desde el del alta), toma la `K_cfg` del alta (si el cliente se creó otra vez, la sal es otra; la clave se acaba de demostrar, como en `cambiar_clave_admin`) y sube su configuración cifrada y su informe. Nada se reconfigura: repositorios, copias, contraseñas, Servidor de copias, espejo y nubes siguen igual.
- Con **otra** clave, el alta se rechaza («ya tiene otra clave de administración») y el equipo no cambia. Para darlo a otro cliente: `vincular <código> --servidor <url> --empezar-de-cero`, que olvida el servidor, la clave y lo que este gestionaba (repositorios, destinos y copias programadas; lo copiado se queda en sus destinos; el Servidor de copias, el espejo y las nubes no se tocan) y lo vincula como un equipo nuevo.
- Si en 7 días no llega el alta, el vínculo pendiente se olvida. Un `vincular` nuevo lo sustituye.
- Un equipo sin clave de administración (nunca dado de alta) se vincula como siempre.

### Paquete de exportación (`.resguardo-cliente`)

Formato (`crates/protocolo/src/paquete.rs`, vector `paquete`):

```text
"RESGUARDO-CLIENTE-1\n" ‖ sal_cliente_b64 ‖ "\n" ‖ por cada trozo de 4 MiB:
    u32 big-endian (longitud) ‖ nonce(24) ‖ XChaCha20-Poly1305(K_exp, nonce, datos, aad = "resguardo-cliente-v1|" + n + "|" + (último ? "1" : "0"))
```

- **Se cifra y se descifra en el navegador** con `K_exp`. Contenido (JSON, lo decide la consola): el cliente, sus equipos (públicos y etiquetas), sus configuraciones cifradas, informes, avisos y la auditoría completa.
- `PUT /api/clientes/{c}/paquete` (administrador; cuerpo binario, hasta 64 MB): el servidor solo comprueba la cabecera (que la sal es la de ese cliente) y lo guarda. `GET` (administrador) lo devuelve tal cual; `DELETE` (administrador) lo borra.
- `POST /api/clientes/{c}/importar` (propietario), en el servidor nuevo, con lo que el navegador sacó del paquete: `{ origen, auditoria: [{ n, creado (Unix o RFC 3339), actor, accion, objetivo, datos, hash, prev_hash }], informes: [{ equipo, recibido, datos }], avisos: [{ equipo, tipo, mensaje, creado }] }`.
  - La auditoría se acepta solo si su cadena está entera desde el génesis. Se guarda aparte (de solo añadir) y la propia la enlaza con una entrada `importar_cliente` que lleva su último hash.
  - Una sola vez por cliente (después, 409).
  - `GET /api/clientes/{c}/auditoria/importada?desde=&limite=` (técnico o más) la muestra.

### CLI del agente (modo local o para moverse)

`servidores`, `consolas [quitar <n.º | dirección | huella>]` (v1.35), `config exportar <archivo>` (sin contraseñas), `config importar <archivo>` (solo en modo local), `repositorio crear <id> --nombre … --tipo … --donde … [--usuario …]` (solo en modo local; contraseña y secreto por la entrada estándar) y `vincular <código> --servidor <url> [--empezar-de-cero]`.

---

## 12. Nubes del espejo desde la consola (Dropbox)

- **App «Resguardo» de Dropbox**, permiso «App folder»: solo ve `Aplicaciones/Resguardo`. Sin app secret (OAuth 2 con PKCE); la app key es pública (`GET /api/servidor` → `dropbox_app_key`).
- **Conectar.** La consola abre `https://www.dropbox.com/oauth2/authorize?client_id=…&response_type=code&code_challenge=…&code_challenge_method=S256&token_access_type=offline` (sin `redirect_uri`), la persona pega el código, el navegador lo cambia en `https://api.dropboxapi.com/oauth2/token` y manda `conectar_nube` **sellada** para el equipo. El servidor nunca ve el token.
- **Renovar.** El agente pide un access token nuevo antes de cada vuelta del espejo (`grant_type=refresh_token`, `refresh_token`, `client_id`; sin secreto) y se lo da a rclone con el refresh token y el `client_id`, por si la vuelta dura más de 4 h. Ver [destinos.md](destinos.md).
- **Resumen.** Igual que antes: `guarda_copias.nubes: [{ nombre, tipo }]`, sin tokens.

---

## 13. Notificaciones (v1.29)

Que los problemas lleguen a quien no abre la consola: por **correo** (SMTP), **webhook** (POST JSON, firmado), **ntfy** y **Telegram**. Lo implementa `crates/servidor/src/notificaciones`.

**Qué se avisa.** Los avisos de §6 (los que ya creaba el servidor o manda el agente) y, además, lo que el servidor deduce de lo que cuenta cada equipo:

| Tipo | De dónde | Gravedad |
|---|---|---|
| `copia_fallida` | informe: `copias[].estado = "fallo"` (por copia) | crítico |
| `verificacion_fallida` | informe: `repos[].verificacion.resultado = "fallo"` | crítico |
| `externa_fallida` | informe: `repos[].externa.resultado = "fallo"` | importante |
| `prueba_fallida` | informe: `repos[].prueba_restauracion.resultado = "fallo"` | importante |
| `espejo_fallido` | resumen: `guarda_copias.espejo.resultado` empieza por `ERROR` | importante |
| `cambio_clave` | resultado firmado `hecha` de `cambiar_clave_admin` | crítico |
| `intentos_fallidos`, `bloqueo`, `cambio_inusual`, `servicio_detenido`, `orden_destructiva` | §6 | crítico |
| `equipo_sin_contacto`, `copia_atrasada` | §6 | importante |
| la vuelta a la normalidad («Volvió a funcionar») | el siguiente informe que dice `ok` (o el equipo vuelve a conectar) | informativo |

Los deducidos entran también en la lista de avisos (una vez por problema, no en cada informe). `orden_destructiva` (v1.30) se cierra **sin** «Volvió a funcionar» en cuanto el equipo no tiene ninguna orden destructiva por aplicar (se aplicó, falló, se rechazó, se canceló o caducó); lo que aún no había salido se descarta, y una orden destructiva posterior vuelve a avisar. Un fallo de hace más de 3 días (p. ej. al actualizar el servidor) se anota sin avisar.

**Agrupar, no inundar.** Cada problema es un *incidente* (equipo + tipo + copia o repositorio): lo repetido mientras sigue abierto se cuenta («Falló 3 veces desde…») y se recuerda como mucho una vez al día («Sigue pasando: …»). Cuando se arregla, «Volvió a funcionar» **solo a quien recibió el aviso** (y aún puede recibirlo: sigue en el cliente y el canal sigue ahí); lo que aún no había salido (horas de silencio, tope) se descarta. Cada canal y destinatario recibe como mucho `max_por_hora` mensajes por hora (10 si no se dice); lo que no cabe espera y sale **agrupado** en un solo mensaje («N avisos de Resguardo»). Los resúmenes y las pruebas no cuentan para el tope.

**Entrega.** Cola persistente en `control.db` (`notif_envios`): sobrevive a un reinicio y nunca la espera una petición (la del agente o la consola solo apunta el evento). Una tarea de fondo entrega cada 10 s lo que toca, con 20 s como mucho por envío; si falla, reintenta a 1, 2, 4, 8, 16, 32 min y luego cada hora, hasta 8 intentos; un error que no se arregla solo (credenciales, 4xx) no se reintenta. Puede llegar dos veces si el servidor se para justo al enviar.

**Contenido.** Solo metadatos: nombres de cliente, equipo, copia o repositorio, estado, horas y el mensaje del equipo ya **sin rutas** (`[ruta]`), sin direcciones (`[dirección]`: pueden llevar usuario y contraseña) y sin fichas largas (`[…]`), aunque el agente ya los manda limpios (`public_message`). Nunca contraseñas, claves ni secretos de los canales. Si el servidor tiene `url_consola`, cada mensaje lleva el enlace a su página (`/c/<cliente>/equipos/<equipo>` o `/c/<cliente>/avisos`). El correo va en HTML (adaptable al móvil, claro u oscuro, colores de la consola) con su versión en texto (`multipart/alternative`), `Auto-Submitted: auto-generated`. Si todo lo que lleva es de **un solo cliente** y ese cliente tiene marca (§3), la cabecera lleva su logo y su nombre, y su acento (claro y oscuro) en la cabecera y en una línea sobre la tarjeta; el logo va **dentro** del correo (`multipart/related`, `Content-ID: <logo-cliente@resguardo>`, `src="cid:…"`): nunca una imagen de fuera. Lo que junta varios clientes (un resumen de todos, un grupo de avisos de varios) sale con la cabecera de siempre. La prueba de un canal de un cliente sale con su marca. Los demás canales no cambian.

**Secretos de los canales** (contraseña SMTP, dirección del webhook y su secreto de firma, dirección y token de ntfy, token de Telegram): cifrados en la base de datos (XChaCha20-Poly1305, datos asociados `resguardo-notif-v1|<ámbito>|<canal>|<campo>`) con una clave derivada de `identidad.key` (HMAC-SHA256 con la etiqueta `resguardo-notificaciones-secretos-v1`): una copia de `control.db` sola no los revela, y la copia de la consola los restaura enteros. **La API nunca los devuelve**: solo `"configurado"`. De la dirección de un webhook o de ntfy solo se enseña el servidor (`config.servidor`).

### Canales

`Canal`:

```json
{ "id", "tipo": "correo" | "webhook" | "ntfy" | "telegram", "nombre", "activo": true,
  "config": { "host", "puerto", "seguridad": "starttls" | "tls" | "ninguna", "usuario", "remitente", "chat_id", "servidor" },
  "secretos": { "<campo>": "configurado" },
  "reglas": { "severidades": ["critico", "importante"], "clientes": ["<id>"] | null, "silencio": Silencio | null,
              "resumen_diario": false, "resumen_semanal": false },
  "completo": true, "actualizado", "por" }
```

- `correo`: `host`, `puerto` (587 con STARTTLS, 465 con TLS), `seguridad` (sin cifrar, solo con un servidor en el propio equipo: `localhost`, `127.x`, `::1`), `usuario` y `remitente` («Resguardo <copias@empresa.com>»); secreto `contrasena`. Rustls con las raíces de Mozilla; sin OpenSSL. Va a las **personas** (sus preferencias, abajo). Como mucho uno por ámbito.
- `webhook`: secretos `url` (https; http solo al propio equipo) y `secreto` (opcional, 16 caracteres o más) para firmar.
- `ntfy`: secretos `url` (la del tema: `https://ntfy.sh/<tema>`) y `token` (opcional, `Authorization: Bearer`). Se publica en JSON (`topic`, `title`, `message`, `priority` 5/4/3, `tags`, `click`).
- `telegram`: `config.chat_id` (número o `@canal`) y secreto `token` del bot. `sendMessage` con `parse_mode: "HTML"`.
- `reglas` (canales que no son de correo): qué gravedades, de qué clientes (solo en los del servidor; `null`: todos), horas de silencio y si mandan los resúmenes (en texto; el webhook, en JSON).

`Silencio`: `{ "desde": "22:00", "hasta": "07:00", "salvo_criticos": true }` (hora del servidor; cruza la medianoche si `desde > hasta`). Lo que llega dentro sale a las `hasta` (agrupado); los críticos pasan si `salvo_criticos`.

**Ámbitos.** Los del **servidor** los pone su propietario (`superusuario`). Cada **cliente** puede tener los suyos (sus propietarios): un correo propio sustituye al del servidor para las personas de ese cliente; los demás se suman a los del servidor que incluyen al cliente. Los de un cliente no pueden mandar a este equipo ni a la red local (IP privadas, de enlace local o de CGNAT, `localhost`, nombres sin dominio o de dominios locales como `.local`): 422. Así su propietario no puede usar el servidor para llegar a lo que hay en su red; el del servidor sí puede (p. ej. un webhook interno).

**Paso de más.** Crear un canal, o cambiar su `config` o algún secreto (lo que decide **a dónde** va y con qué credenciales: otro servidor SMTP se llevaría la contraseña guardada), pide `codigo`: un código TOTP de quien lo hace, recién sacado (401 `codigo` si falta o no vale; un código gastado no vale otra vez). Cambiar el nombre, encenderlo o apagarlo, las reglas o borrarlo no lo pide. Todo queda en la auditoría (del servidor o del cliente: `notificaciones_canal`, con la acción, el tipo, el nombre y **qué** secretos cambiaron, nunca su valor).

| Método y ruta | Quién | Pide | Responde |
|---|---|---|---|
| `GET /api/servidor/notificaciones` | propietario del servidor | — | `{ url_consola, max_por_hora, hora_resumen, dia_semanal, canales: [Canal] }` |
| `PUT /api/servidor/notificaciones` | propietario del servidor | `{ url_consola?, max_por_hora? (1–120), hora_resumen? ("HH:MM"), dia_semanal? (1 = lunes … 7) }` (`url_consola: ""` la quita; https o http, sin usuario ni `?`) | lo mismo. Auditado (`notificaciones_ajustes`) |
| `POST /api/servidor/notificaciones/canales` | propietario del servidor | `{ tipo, nombre?, activo?, config?, secretos?: { campo: valor }, reglas?, codigo }` | `Canal` |
| `PATCH /api/servidor/notificaciones/canales/{k}` | propietario del servidor | lo que cambia (`secretos`: los que vienen se cambian, `""` quita uno, los demás se quedan); `codigo` si cambia `config` o `secretos` | `Canal` |
| `DELETE /api/servidor/notificaciones/canales/{k}` | propietario del servidor | — | 204 |
| `POST /api/servidor/notificaciones/canales/{k}/prueba` | propietario del servidor | — | `{ ok, mensaje }`. «Enviar prueba» al momento (el correo, a quien la pide); 20 por hora |
| `GET /api/servidor/notificaciones/registro` | propietario del servidor | — | `[Envio]`: los últimos 100 |
| `GET /api/clientes/{c}/notificaciones` | propietario | — | `{ canales: [Canal], correo: { de: "servidor" | "cliente", nombre } | null, servidor: { canales: [{ nombre, tipo, severidades }], url_consola } }` |
| `POST`, `PATCH`, `DELETE …/clientes/{c}/notificaciones/canales[/{k}]`, `POST …/{k}/prueba` | propietario | como los del servidor | como los del servidor (auditados en el cliente) |
| `GET /api/clientes/{c}/notificaciones/registro` | propietario | — | `[Envio]`: los últimos 100 de este cliente y de sus canales |

`Envio` (registro): `{ id, creado, enviado | null, canal: { id, nombre, tipo } | null, ambito: "servidor" | "cliente", cliente, destino (el correo o el servidor del canal), tipo: "aviso" | "recuperacion" | "resumen" | "prueba", severidad, titulo, estado: "pendiente" | "enviado" | "fallido" | "descartado", intentos, siguiente | null, error | null, nota | null }`. `nota` cuenta lo que pasó en palabras («En horas de silencio: sale a las 07:00.», «Tope de 10 por hora: sale agrupado…», «Agrupado con 3 más.», «Reintento 3 a las 10:42.», «Se arregló antes de enviarse.»). `error` nunca lleva la dirección ni un secreto.

### Personas: qué recibe cada una

Por correo (si hay un canal de correo para su cliente). Por cliente: qué gravedades al momento y si el cliente entra en sus resúmenes; si no se dijo nada, según el papel: propietario y administrador, crítico e importante y resumen; técnico, crítico; lectura, nada. Para todos sus clientes: horas de silencio y resumen diario y semanal (semanal sí, diario no, si no se dijo).

| Método y ruta | Quién | Pide | Responde |
|---|---|---|---|
| `GET /api/clientes/{c}/notificaciones/personas` | propietario | — | `[{ cuenta, nombre, correo, rol, preferencias: { inmediatos: ["critico", …], resumen, propias }, silencio, resumen_diario, resumen_semanal }]` (`propias: false`: las de su papel) |
| `PUT /api/clientes/{c}/notificaciones/personas/{cuenta}` | propietario, o la propia persona | `{ inmediatos: [Severidad], resumen }` | `{ inmediatos, resumen, propias: true }`. Auditado en el cliente (`notificaciones_preferencias`) |
| `GET /api/cuenta/notificaciones` | cualquiera | — | `{ silencio, resumen_diario, resumen_semanal, hora_resumen, dia_semanal, clientes: [{ id, nombre, rol, correo: bool, preferencias }] }` |
| `PUT /api/cuenta/notificaciones` | cualquiera | `{ silencio?: Silencio | null, resumen_diario?, resumen_semanal? }` | lo mismo |

### Resúmenes

A la `hora_resumen` del servidor: el **diario** cada día y el **semanal** («Resumen semanal de copias») el `dia_semanal`, a quien los quiere (una vez por día o semana, también tras un reinicio). Por persona, un solo correo con todos sus clientes. Por cliente y equipo: estado (bien, revisar, con fallos, sin contacto, sin datos), última copia correcta, copias correctas y fallos del periodo, espacio que ocupan sus repositorios (`espacio.en_disco_bytes`) y lo que necesita atención (incidentes abiertos y equipos que no conectan). Sale del último informe de cada equipo.

### Webhook

`POST` a la dirección con `Content-Type: application/json`, `User-Agent: Resguardo-Server/<versión>`, `X-Resguardo-Evento: aviso | recuperacion | grupo | resumen | prueba`, `X-Resguardo-Entrega: <id>` (se repite en los reintentos: sirve para no contar dos veces lo mismo) y, con secreto, `X-Resguardo-Firma: t=<unix>,v1=<hex>` donde `<hex> = HMAC-SHA256(secreto, "<unix>.<cuerpo>")` sobre los bytes exactos del cuerpo. Quien recibe compara en tiempo constante y rechaza un `t` de hace más de 5 minutos. Vector: secreto `secreto-compartido-123`, `t = 1791100800`, cuerpo `{"a":1}` → `v1=d0524a455d5fce0165253c72ecdb2ae9f8f8e523d44886ab7a258a155dd39931`. Cuerpo (v1):

```json
{ "version": 1, "id": "<entrega>", "evento": "aviso", "creado": "<RFC 3339>", "severidad": "critico",
  "titulo": "<el asunto>", "texto": "<el mensaje en texto>", "enlace": "https://…" | null,
  "avisos": [{ "tipo", "severidad", "resuelto": false, "titulo", "texto", "cliente": { "id", "nombre" },
               "equipo": { "id", "nombre" } | null, "veces", "desde", "hora", "enlace" }],
  "resumen": { "periodo": "semanal", "desde", "hasta", "clientes": [{ "id", "nombre", "copias_ok", "fallos", "bytes", "atencion": [],
               "equipos": [{ "id", "nombre", "estado", "ultima_ok", "ultimo_contacto", "copias_ok", "fallos", "bytes" }] }] } | null }
```

Un 2xx es entregado; 408, 425, 429 y 5xx se reintentan; los demás 4xx no. No se siguen redirecciones.

---

## Cambios

- v1 (F1): versión inicial.
- v1.1 (F1, implementación):
  - `POST /api/agente/tomar` devuelve en `sesiones` solo los **ids** de las sesiones abiertas (antes: los mensajes); los mensajes se leen con su ruta.
  - Mensajes nuevos del servidor al agente por WebSocket: `sesion_cerrada` y `error`.
  - Las órdenes con `not_before` futuro **no se entregan** al agente hasta su hora (el servidor las guarda; el agente vuelve a comprobarlo).
  - El primer arranque imprime en el registro el código para `POST /api/inicio`.
  - Nuevo `GET /api/servidor/ca` (la autoridad TLS, para fijarla al vincular).
  - `alta` lleva `autorizacion.prueba_codigo`; formato de la configuración cifrada (`crypto_secretbox`).
  - Cuando el equipo confirma `desvincular` o `baja_equipo`, el servidor lo marca `modo: "local"`.
  - Cuerpos de las órdenes que ya ejecuta el agente v2 (F2):
    - `alta { verificador, k_cfg, espera_min_horas }`;
    - `config { repos: [{ id, nombre, ubicacion, contrasena, rest_usuario?, rest_contrasena?, cacert_pem?, nube?: { key_id, key_secret, region? }, planes: [Plan] }] }`;
    - `copiar_ahora { repo, copia }`, `verificar_ahora` / `subir_ahora` / `probar_restauracion { repo }`;
    - `pausar { repo, hasta? }`, `reanudar { repo }`, `dejar_de_copiar` (repo de `clave_repo`);
    - `cambiar_espera { horas }`, `cambiar_clave_admin { verificador, k_cfg }`, `desvincular {}`, `baja_equipo {}`.

    El resto se responde `rechazada` («aún no admite») hasta F4.
- v1.2 (F4, reconciliación con la consola):
  - **Formatos simétricos** (§1): los de la consola, con vectores (`simetrico` en `v1.json`): sesiones (claves por dirección con HKDF y `aad` con el id), trozos del relé (`aad` con id, número y «último») y **configuración** (XChaCha20-Poly1305 con `aad` del equipo y el número; sustituye a `crypto_secretbox`).
  - La **clave de administración se normaliza a NFC** antes de Argon2id (vector `derivaciones.nfc`).
  - **Cuerpos de las órdenes** finales (§5) y **configuración v1** sin secretos (§6). Las credenciales entran solo con `crear_repositorio` y se quedan en el equipo. La configuración de F2 (con contraseñas dentro) desaparece.
  - **Destructivas según el cuerpo** (`desvincular` «dejar de copiar», `guarda_copias` al desactivar, bajar `cambiar_espera`, `restaurar` reemplazando): el servidor se fía del `not_before`; el agente exige la espera.
  - **`cambiar_espera`**: el resultado lleva `detalle: {"espera_min_horas": h}` firmado; con él, el servidor actualiza la espera del equipo (`Equipo.espera_min_horas`) y, si todos coinciden, la del cliente.
  - **`detalle`**: JSON en texto, en claro o `{"sellado": …}` para `responder_a`. Las órdenes largas contestan `en_marcha` y después el resultado final.
  - **Sesiones**: operaciones y mensajes (§7); el id de sesión válido es el de dentro del sobre.
  - **Relé**: una ruta por `descargar` (`restic dump`, o zip de una carpeta).
  - `resumen` del equipo e informe con forma fija (§4, §6).
  - Rutas nuevas: `GET /clientes/{c}/ordenes` sin `pendientes` (todas, con cursor y filtros), `GET …/auditoria?orden=desc&antes=n`, `PATCH /api/cuenta`, `POST /api/cuenta/totp` (+ `/confirmar`), `POST /api/cuenta/recuperacion`.
  - La **auditoría** la leen también los técnicos (no los de lectura).
  - Modelo de amenazas: claves de los equipos fijadas en la consola (TOFU) para las órdenes con contraseña de repositorio (§1).
  - Diseño de «restaurar en otro equipo» (§10), para F6.
- v1.3 (agente, órdenes que faltaban):
  - Nuevo tipo `cambiar_destino` (clave de administración): nuevas credenciales o dirección de un destino, comprobadas contra cada repositorio antes de guardarlas.
  - `guarda_copias` (activar, desactivar*, añadir un equipo cliente con su acceso sellado para `responder_a`, quitar*); el `resumen` lleva `guarda_copias: { activo, puerto, solo_red_local, usuarios } | null`.
  - `desbloquear { repo? }` (`restic unlock`).
  - `actualizar_agente`: `rechazada` («próximamente») hasta la llave de publicación.
  - `descargar` admite varias rutas en un zip.
- v1.4 (F6):
  - «Recibir un cliente» (`POST /api/clientes/recibir`, fichas con `POST /api/clientes/{c}/fichas`) y `POST /api/agente/recibir`.
  - Órdenes `cambiar_servidor` (dos fases, 24 h), `servidores_respaldo`, `compartir_acceso` e `importar_repositorio` (§10, §11). Nuevo `modo: "trasladado"`.
  - Paquete `.resguardo-cliente` (formato y vector) con `PUT/GET/DELETE /api/clientes/{c}/paquete` (solo cifrado) e importación con `POST /api/clientes/{c}/importar` y `GET …/auditoria/importada`.
  - Los repositorios importados salen en la configuración con `solo_lectura: true`.
  - CLI: `servidores`, `config exportar/importar`, `repositorio crear`.
- v1.5:
  - `cambiar_copia_externa` (copia externa diaria a otro destino; el equipo dueño del repositorio la hace).
  - `guarda_copias { espejo: { carpeta, hora } | null }`: espejo nocturno de todo lo que guarda el Servidor de copias en otra carpeta (otro disco), de solo añadir (nunca borra; sin `locks` ni lo escrito en los últimos 10 min; sin contraseñas). En el resumen: `guarda_copias.espejo: { hora, ultima, resultado }`. También por la CLI: `guardar-copias espejo --carpeta … [--hora 02:00]`.
  - Resguardo Server en Windows: servicio `ResguardoServer`, datos en `C:\ProgramData\Resguardo Server`, código de primer arranque en `codigo-arranque.txt` (se borra al usarse) e instalador NSIS.
- v1.6:
  - La cookie de sesión se llama `resguardo_sesion_<id>` (6 caracteres alfanuméricos de la identidad del servidor).
  - `Equipo.rol` lo pone el servidor según el resumen del equipo: `almacenamiento` si `guarda_copias.activo`, si no `agente`. La consola debe ofrecer «Este equipo guarda copias» en cualquier equipo (no solo en los `almacenamiento`).
  - `resumen.repositorios[].externa` lleva también `destino_id`; nuevos `resumen.servidores_respaldo: [{ url, identidad_corta }]`, `resumen.respaldo_dias` y `resumen.traslado: { estado: "en_marcha", hacia, hasta } | null`.
  - `GET` y `DELETE` de `/paquete` exigen administrador, como `PUT`.
- v1.7 (informe detallado): `informe.repos[]` con versiones, ejecuciones, espacio, verificación, prueba de restauración, copia externa y salud de la protección (§6, `RepoInforme`), para las vistas de la app de escritorio en la consola.
- v1.8 (0.7.1): `GET /api/clientes/{c}/informes` (último informe de cada equipo). Al actualizar el agente, el instalador deja una marca y la parada no cuenta como «detenido por un administrador».
- v1.9 (0.7.1, espejo en la nube):
  - **Destinos del espejo.** `guarda_copias { espejo: { destinos: [{ tipo: "carpeta", carpeta } | { tipo: "nube", nube: "<nombre>", carpeta: "Resguardo/Sur" }], hora?: "02:00", limite_kib? } }` (clave de administración). La forma `{ carpeta, hora }` sigue valiendo (un destino carpeta). `limite_kib` limita la subida a la nube (KiB/s). Quitar un destino que ya está, o el espejo (`espejo: null`), es **destructiva**: la consola pone el `not_before` y el agente lo comprueba comparando con sus destinos actuales. Un destino `nube` exige una nube conectada en ese equipo con ese nombre.
  - **Resumen.** `guarda_copias.espejo: { hora, ultima, resultado, limite_kib, destinos: [{ tipo, carpeta, nube, ultima, resultado }] }` (`ultima` y `resultado` de arriba: la última vuelta entera; los resultados con error empiezan por `ERROR:`) y `guarda_copias.nubes: [{ nombre, tipo }]` (`tipo`: `dropbox` o `drive`). **Nunca** lleva tokens.
  - **Conectar una nube** solo se hace en el propio equipo, como administrador: `resguardo-agente nube conectar dropbox --nombre "Dropbox Altamar"` (abre el navegador para dar permiso), `nube lista`, `nube quitar <nombre>`. No hay orden remota para conectar: el token no pasa nunca por el servidor ni por la consola.
  - **Cómo sube.** Con el rclone oficial que acompaña al agente (1.75.1, huella SHA-256 fijada y comprobada antes de cada uso): `rclone copy <carpeta del Servidor de copias> <nube>:<carpeta> --exclude locks/** --min-age 10m --immutable`. Nunca `sync` ni borrados: lo que se borra en el origen sigue en la nube, y un archivo que ya está no se reescribe (los de restic no cambian; si uno cambiara, rclone lo marca como error y no lo toca). El token se guarda solo en la carpeta privada del agente, protegido (DPAPI a nivel de equipo en Windows), y rclone lo recibe por variables de entorno (`RCLONE_CONFIG_<REMOTO>_TYPE` / `_TOKEN`): no se escribe ningún `rclone.conf` con él (si rclone renueva el token, el nuevo se guarda protegido y el archivo temporal se borra). Lo que sube son paquetes de restic ya cifrados: ninguna contraseña de repositorio interviene.
  - **Seguridad.** Dropbox (y Google Drive) no tienen bloqueo de objetos: quien tenga el token, o la cuenta, puede borrar lo subido. Lo mitiga el historial de versiones de Dropbox (archivos borrados recuperables durante 30 días o más, según el plan), pero no es inmutable. El token es un secreto de mucho valor y solo está en el equipo que guarda copias; si ese equipo se pierde, hay que revocar el permiso en la web de Dropbox («Aplicaciones conectadas»). Recomendación: pasar a un destino con bloqueo de objetos (Backblaze B2 u otro S3 con Object Lock) cuando se pueda.
- v1.10 (ganchos de plantilla): `copias[].gancho` admite las plantillas `sqlserver` (volcado `COPY_ONLY` que entra en la copia y se borra después) y `carpeta_reciente` (aviso si la carpeta de copias propias de una aplicación no tiene nada reciente), o una lista de ellas (§6). Antes, el agente rechazaba cualquier gancho; ahora rechaza lo que no sea una de estas plantillas con sus campos. El informe lleva `copias[].ganchos: [{ tipo, estado, mensaje }]`. Un agente anterior rechaza la configuración con ganchos («aún no los admite»): la consola debe ofrecerlos solo a agentes ≥ 0.7.2 (`informe.version`).
- v1.10 (seguridad, 0.7.2): la carpeta del Servidor de copias, las del espejo y la de los volcados tienen que ser de un disco del equipo (no su raíz, ni de red, ni con enlaces en el camino) y el agente las deja solo para SYSTEM y Administradores: una orden con otra ruta se rechaza. El espejo se para con error si encuentra un enlace en el destino. Las órdenes destructivas según el cuerpo que llevan `not_before` cumplen la espera también en el servidor; el agente rechaza un `seq` más de 100 000 por encima del último; repetir `alta` ya no cambia `K_cfg` ni baja la espera.
- v1.11 (Dropbox desde la consola): órdenes `conectar_nube { tipo: "dropbox", nombre, refresh_token, access_token?, expira?, app_key }` y `quitar_nube { nombre }` (clave de administración; `quitar_nube` es destructiva según el cuerpo si el espejo usa esa nube, y entonces también la quita del espejo). `GET /api/servidor` lleva `dropbox_app_key` (la de la app «Resguardo», `beobf3c13cvlrup`, salvo `RESGUARDO_DROPBOX_APP_KEY` al compilar; vacía, no sale). La CSP de la consola admite `connect-src https://api.dropboxapi.com`. El agente renueva el access token de Dropbox antes de cada vuelta del espejo (§12). La CLI `nube conectar` sigue valiendo en el propio equipo. Un agente sin estas órdenes las rechaza como tipo desconocido.
- v1.12 (detalle de una copia): el informe lleva `proximas: { <copia>: <RFC 3339> | null }` y cada entrada de `repos[].ejecuciones` añade `duracion_s`, `anadido`, `archivos_nuevos`, `archivos_cambiados` y `reintento`. El resumen en claro trae ahora `copias[].proxima` y `copias[].ultima.bytes`, y `repositorios[].versiones`, `bytes` (lo que ocupa la última versión) y `ultima_version` (de la caché del informe); y el agente lo vuelve a subir (con `config`, `seq` + 1) cuando cambia, comprobándolo con cada informe, en vez de solo al cambiar la configuración. Todo es opcional: la consola pinta la página de una copia con lo que haya (con un agente anterior, sin «próxima» ni gráficas por vuelta, y toma versiones y tamaños del informe). El servidor no lee estos campos.
- v1.12 (seguridad): una orden `config` que deja sin ninguna copia activa (lista vacía o todas con `activa: false`) a un equipo que tenía alguna es destructiva según el cuerpo (cumple la espera; la consola pone `not_before` y el agente lo vuelve a comprobar). El `mensaje` de los resultados va sin rutas del equipo (`[ruta]`; las rutas de restaurar y del espejo, entre comillas para quitarlas enteras). Al entrar, un correo de más de 254 caracteres o una contraseña de más de 1024 se rechazan sin contar intentos por correo; los límites por IP cuentan una IPv6 por su /64; un código TOTP se gasta en la misma sentencia que lo comprueba (`totp_ultimo < paso`), así que dos peticiones a la vez con el mismo código no entran las dos.
- v1.13 (volver a vincular): `vincular` en un equipo que ya tiene clave de administración deja el servidor nuevo **pendiente del `alta`** con el mismo verificador (§11, «Volver a vincular con un código»): hasta entonces el nuevo no recibe configuración ni resumen y las demás órdenes se rechazan; con el alta, el equipo toma su `K_cfg` y sube la configuración. Antes, el código bastaba para cambiar el servidor del equipo y la `K_cfg` no cambiaba (una consola con el cliente creado otra vez no podía descifrarla). El `alta` comprueba `prueba_codigo` siempre que el equipo tenga un código pendiente. CLI: `vincular … --empezar-de-cero`. `servidores_respaldo.dias` es opcional (3).
- v1.14 (venir de la app de escritorio): órdenes `adoptar_repositorio` (un repositorio que ya existe pasa a ser gestionado, de lectura y escritura, con su historial; `solo_probar` solo lo comprueba) y `copiar_historial` (`restic copy` en segundo plano desde otro repositorio, con progreso en `en_marcha`), las dos con la clave de administración (§5). `crear_repositorio` admite `parametros_de` (`init --copy-chunker-params`). El resumen lleva `repositorios[].solo_anadir`; en un repositorio de un rest-server de solo añadir, `aplicar_retencion` contesta `fallida` («se aplica en el propio servidor»). `compartir_acceso` añade `repo.ubicacion`. Todo es opcional y compatible: un agente anterior rechaza las órdenes nuevas como tipo desconocido e ignora `parametros_de`; el servidor solo necesita conocer los dos tipos (nivel de administración).
- v1.15 (explorar carpetas al elegir destino): en las sesiones, el primer mensaje del equipo lleva `ops` (lo que admite) y `elegir_carpetas` admite `crear_carpeta { ruta, nombre }` → `{ ruta, ya_existia }` (§7). La consola ofrece «Explorar…» junto a cada carpeta de un destino (Servidor de copias, espejo, destino local, volcados de «Antes de copiar») y «Nueva carpeta» solo si `ops` incluye `crear_carpeta`; con un agente anterior se elige una carpeta que exista o se escribe a mano («Actualiza el agente para usar esto»). El servidor no cambia: solo reenvía mensajes cifrados.
- v1.16 (horario y «solo si hay cambios», agente ≥ 0.7.7): `copias[].solo_si_cambios` (por defecto `true`; sin el campo, encendido, como siempre en el modo gestionado). Con `false`, cada vuelta guarda una versión aunque nada haya cambiado. Un agente anterior ignora el campo (siempre encendido): la consola solo lo manda, y deja apagarlo, a agentes ≥ 0.7.7. El resumen lleva `copias[].solo_si_cambios` (agentes ≥ 0.7.7). El horario sigue siendo `{ dias, horas }`: «cada N horas entre las X y las Y» lo despliega la consola en la lista de horas (como mucho 48 al día) y lo reconoce al abrir; no cambia nada en el agente.
- v1.17 (equipos preparados): «Descargar instalador listo» (Windows) y la línea de Linux con `POST /api/clientes/{c}/instaladores`, la lista con `GET /api/clientes/{c}/emparejamientos` y el recuento con `GET …/preparados` (§4). Código de un solo uso de 24 h; el equipo entra con el nombre de la consola; `GET …/emparejamientos/{p}` lleva `nombre`, `so` y `codigo` mientras sirve. `GET /api/servidor` lleva `instalador_agente` (¿puede dar el instalador?). Resguardo Server para Windows trae el instalador del agente (`agente\Resguardo-Agente-setup.exe`, o `--instalador-agente`). Agente ≥ 0.7.7: `leer-instalador`, `vincular --instalador` y `vincular … --huella-ca` (no acepta otra autoridad TLS ni otro cliente). Un agente anterior ignora `--huella-ca` y el instalador sin cola funciona como siempre; la consola sin `instalador_agente` solo ofrece el código de 15 min. El riesgo de que el servidor guarde el código está en plataforma.md §7.3.1. Un equipo que ya tiene clave de administración sigue las reglas de v1.13: con el instalador listo también queda pendiente del `alta` con su misma clave.
- v1.18 (etiquetas de equipos): `Equipo.etiquetas` (lista de texto libre, en claro; no es `etiqueta`, el HMAC) y `PUT /api/clientes/{c}/equipos/{e}/etiquetas` (técnico o más, auditado). La consola las enseña en las listas y la ficha, filtra por etiqueta Equipos, Avisos y Estado, y propone las que ya existen. Un servidor anterior no manda el campo: la consola no enseña etiquetas ni el filtro.
- v1.19 (el servidor también guarda copias): los instaladores del servidor ofrecen «Este equipo también guarda copias» (Windows: casilla o `/CONAGENTE`, con el agente que traen; Linux: `instalar-servidor.sh --con-agente [paquete]`), que instala el agente **sin vincular**. `GET /api/servidor` lleva `agente_local` y la consola ofrece «Vincular este servidor» (Primeros pasos y «Añadir equipo»): `POST /api/clientes/{c}/equipo-local` deja `vincular-local.json` (solo administradores, o el usuario del servidor en Linux) y el agente ≥ 0.7.7, nunca vinculado, lo lee cada 10 s y se une solo, comprobando la huella de la autoridad TLS, el cliente y que el servidor es `127.0.0.1`, `localhost` o `::1`. El número de comprobación y el alta con la clave de administración siguen igual; después, «Este equipo guarda copias» con la carpeta elegida con «Explorar…». El resumen del equipo lleva `puerto_libre` (el primero libre de 8000, 8002, 8004, 8080, 8888 y 9000) y la consola lo propone.
- v1.20 (plantillas de copia): «Guardar como plantilla» y «Rellenar con…» / «Desde una plantilla» en el editor de copias. Cifradas en el navegador con `K_pla` (derivada de `K_cfg`, §1), atadas al cliente y al id; el servidor guarda bytes (`GET/PUT/DELETE /api/clientes/{c}/plantillas`, administradores) y no ve ni el nombre ni las carpetas. Usar una solo rellena el editor: la copia se revisa y se envía con la orden `config` firmada de siempre; nada se propaga a los equipos. La retención que guardan es una sugerencia: sigue siendo del repositorio (`cambiar_retencion`, con su contraseña). Un servidor anterior responde 404 y la consola no ofrece plantillas.
- v1.21 (adoptar en un almacén): el resumen de un equipo que guarda copias lleva también `guarda_copias.carpeta` (la carpeta donde guarda, como las del espejo) y `guarda_copias.repositorios: [{ usuario, repos: [nombre] }]` (los repositorios de cada equipo cliente, `<carpeta>/<usuario>/<repo>`, solo nombres, como ya enviaba el informe web). La consola, en «Usar uno que ya existe», ofrece los destinos del equipo («Almacén … (ya configurado)») y manda `adoptar_repositorio` con `destino: { id }` y `ruta: "<nombre>"`; enseña la ruta exacta donde dejar la carpeta y los repositorios encontrados. «Copiar en …» admite `parametros_de` (crear en un almacén «para traer el historial» de otro). Un agente anterior no manda los campos nuevos: la consola pide el nombre sin ruta ni lista. Al leer de un origen (`parametros_de`, `copiar_historial`), el usuario del destino rest va en su dirección y no en `RESTIC_REST_USERNAME/PASSWORD`, para que restic no lo mande al servidor del origen.
- v1.22 (retención en el almacén; diseño y contrapartida en [compartir.md](compartir.md), «Retención en el almacén»):
  - Órdenes nuevas (§5): `clave_almacen { repo, clave }` al equipo dueño (contraseña del repositorio + clave de administración: añade una clave de restic propia del almacén, que vale con solo añadir), `retencion_almacen { usuario, repo, clave?, retencion, horario, verificar? }` / `{ usuario, repo, quitar: true }` al almacén (clave de administración; destructiva según el cuerpo, salvo `quitar`) y `aplicar_retencion_almacen { usuario, repo }` (clave de administración, destructiva; `en_marcha` y después el resultado). El almacén aplica la regla en local (`forget --prune` y, si se pide, `check`) en cada hueco de su horario, sin que nadie la vuelva a autorizar.
  - Resumen: `guarda_copias.retenciones` del almacén (regla, horario, estado de la clave, último resultado y próxima vez; nunca la clave) y `repositorios[].ruta` del equipo (su carpeta en el servidor rest, si no es su id).
  - La comprobación de «solo añadir» usa la autoridad TLS propia del destino (`ca_pem`): con un almacén ya no sale `null`. Un «no se sabe» se vuelve a comprobar a la hora (antes, al día).
  - Compatibilidad: la consola ofrece «Retención en el almacén» solo si el almacén manda `guarda_copias.retenciones` (agente posterior a 0.7.8); un agente anterior rechaza las órdenes nuevas como tipo desconocido. El servidor solo necesita conocer los tres tipos (y que `retencion_almacen` es destructiva según el cuerpo).
- v1.23 (historial del equipo y copia de la consola). Todo es opcional y compatible:
  - **Historial del equipo** (§6, §8): cada agente guarda para siempre lo que cuenta (12 meses con detalle, lo anterior resumido por copia y día, tope de 50 MB) y lo sube a cada consola nueva con `POST /api/agente/historial` cuando `hola`/`tomar` traen `historial.ultima`. La consola lo lee con `GET /api/clientes/{c}/equipos/{e}/historial` (en la Historia de un repositorio: vueltas, ganchos, verificaciones, pruebas y copia externa de antes; con un servidor anterior, 404 y la consola enseña lo de siempre). Los avisos de antes entran ya vistos, sin repetir los que llegaron en su momento. Un servidor anterior no manda `historial` y el agente no sube nada.
  - **Copia de la consola** (§1, §2): `GET/PUT /api/servidor/respaldo` y `POST /api/servidor/respaldo/ahora` (propietario del servidor), la clave de respaldo de la consola (solo su pública llega al servidor; vector `respaldo_consola`), el archivo `.resguardo-consola` en `<datos>/respaldos/` (cada noche, al arrancar una versión nueva y a mano) y `resguardo-server restaurar-respaldo` / `hacer-respaldo`. La consola ofrece «Copia de la consola» en Servidor y en Primeros pasos; con un servidor anterior (404) no la enseña.
- v1.24 (horarios flexibles, agente ≥ 0.7.9): `copias[].horario.reglas` (§6): «a estas horas», «cada N minutos u horas entre…», «cada N días desde…» y «el día D de cada mes» (1–28 o el último), que se suman. Compatible hacia atrás: sin `reglas` nada cambia, y la consola solo manda `reglas` a agentes ≥ 0.7.9 cuando el horario no se puede decir como `dias` × `horas` (con uno anterior manda la lista desplegada y no ofrece minutos, «cada N días» ni «cada mes»: «Actualiza el agente para usar esto»). Un agente anterior que recibiera `reglas` las ignoraría (usa `dias`/`horas`). El servicio del agente se despierta para la próxima copia si llega antes que su vuelta de 5 minutos. El servidor no cambia (la configuración va cifrada).
- v1.25 (progreso en vivo de las copias):
  - Agente: mientras hay una copia, una verificación, una copia externa o una prueba de restauración en marcha, manda `{ "t": "progreso", "tareas": [Tarea] }` por el canal cada 5 s y una vez vacío al terminar (§6 `Tarea`, §8). Sin canal, `POST /api/agente/progreso` cada 10 s; el informe lleva también `progreso`. El canal se despierta cada 5 s (antes, solo con mensajes del servidor) y sigue dando al servidor por caído tras 45 s sin noticias.
  - Servidor: guarda el último de cada equipo **solo en memoria** (90 s; no va a la base de datos ni a los informes guardados), lo limpia (campos conocidos y acotados) y lo da a la consola con `GET /api/clientes/{c}/progreso` (cualquier papel).
  - Consola: barra de progreso en la fila de la copia y en su página, chip «Copiando… 42 %» en Equipos y Estado, y «1 copia en marcha» en la barra lateral (y en la cabecera del móvil); pregunta cada 3 s mientras algo está en marcha (12 s si no, nada con la pestaña oculta).
  - Compatibilidad: todo es nuevo y opcional. Un servidor anterior responde `progreso` con «Tipo de mensaje desconocido» (o 404 por HTTP) y el agente deja de mandarlo en esa conexión; la consola, con 404 en `GET …/progreso`, lo toma del `progreso` del último informe (si es de hace menos de 3 min). Un agente anterior no lo manda: la consola no enseña nada en marcha.
- v1.27 (restablecer la verificación en dos pasos de otro):
  - `POST /api/clientes/{c}/miembros/{cuenta}/restablecer-totp` (§3): el propietario del cliente (a miembros que no son propietarios y solo están en clientes suyos) o el del servidor (a cualquiera de ese cliente) restablece la verificación en dos pasos de alguien que perdió el móvil y los códigos de recuperación. Pide un código TOTP de quien lo hace; quita al otro el autenticador, los códigos de recuperación y las sesiones, y devuelve un código de un solo uso (`XXXX-XXXX-XXXX-XXXX`, 24 h) para que se lo pase. Auditado (`restablecer_totp`) en la cadena del cliente y en la del servidor.
  - `POST /api/sesion` puede contestar `{ necesita: "restablecimiento", restablecida: { cuando, por }, caducado }` y hay `POST /api/sesion/restablecimiento { codigo }` (§2): sin ese código, la contraseña sola no da de alta otro autenticador. `POST /api/sesion/totp` repite `restablecida` al terminar.
  - Consola: Personas → menú del miembro → «Restablecer verificación en dos pasos» (con aviso y el código de quien lo hace); al entrar, el paso del código y el aviso «Tu verificación en dos pasos se restableció el … por …»; la ayuda «¿Perdiste el teléfono?» dice que lo pida al propietario.
  - Compatibilidad: es nuevo. Una consola anterior no ofrece restablecer y, ante `necesita: "restablecimiento"`, no sabe seguir (la persona necesita la consola nueva, que viene con el servidor). Con un servidor anterior la ruta nueva da 404 y la consola lo dice (la consola va con su servidor, así que no debería pasar).
- v1.26 (seguridad: SAS con la autoridad TLS e historial por páginas):
  - **SAS v3** (§1, §4): el número de comprobación incluye la huella SHA-256 de la autoridad TLS. Antes, al vincular sin huella de antemano (ni instalador listo ni `--huella-ca`), alguien en medio podía hacer que el equipo fijase **su** autoridad: el SAS v2 solo cubre la identidad del servidor y las llaves del equipo, que puede reenviar intactas. Ahora el equipo pone la huella de la autoridad que fijó y la consola la de `GET /api/servidor` (`huella_ca`): con alguien en medio, los números no coinciden. Agente ≥ 0.7.10: manda `sas_version: 3` en `POST /api/agente/unirse` y enseña el v3 si la respuesta trae `sas_version: 3`. Sin TLS (`http://` en el mismo equipo) pide la huella a `GET /api/servidor/ca` solo para el número. El servidor guarda la versión en el emparejamiento (`emparejamientos.sas_version`) y la da en `GET …/emparejamientos/{p}` (`sas_version`) y en la respuesta de `unirse`. Compatibilidad: un agente anterior no manda `sas_version` y sigue con v2; la consola calcula v2 y avisa: «Este agente es antiguo: comprueba también la huella del certificado» (con la huella que tiene que ver). Con un servidor anterior (sin `sas_version` en la respuesta) el agente ≥ 0.7.10 enseña el v2 y dice que hay que comprobar también la huella (`vincular` la escribe; el registro del agente la guarda junto al número). Si alguien en medio quitase `sas_version` de la petición y de la respuesta, los dos lados usarían v2, pero la consola avisa de comparar la huella, que no coincidiría. Vectores `derivaciones.sas_v3` en `v1.json`.
  - **Historial por páginas** (§6): `GET …/equipos/{e}/historial` da 500 entradas si no se pide otra cosa (antes, 5000) y como mucho 2000 por petición (antes, las 20 000 del tope: unos 80 MB para cualquier miembro). Nuevos `antes` (cursor: el `id` de la última entrada recibida), `hasta` y `tipo`. La respuesta sigue siendo la lista, así que una consola anterior funciona igual, con las 500 más recientes. La consola lee lo más reciente primero y ofrece «Cargar más»; con un servidor anterior (que no entiende `antes` y repite la página) quita las repetidas y deja de ofrecerlo.
- v1.28 (retención por plazos, verificación automática y el almacén en sí mismo). Todo es opcional y compatible; el servidor no cambia (las órdenes y la configuración van selladas, el resumen se guarda tal cual):
  - **`Retencion`** (§5): `horarias`, `-1` = «siempre» en cualquier cantidad y `plazos { horarias, diarias, semanales, mensuales, anuales }` con duraciones de restic (`--keep-within-hourly 15d`…). Vale en `cambiar_retencion`, `retencion_almacen`, la copia externa y `crear_repositorio`. El almacén aplica también los plazos con su desconfianza de siempre (cuentan desde la versión más reciente de confianza; la ventana de 48 h / 24 h no cambia), comprobado contra `restic forget --dry-run`. Sin lo nuevo, se guarda y se escribe igual que antes.
  - **`config.verificaciones`** (§6): verificación automática por repositorio, cada N días, porcentaje rotativo. Sin el campo no se toca nada.
  - **`guarda_copias { anadir, local: true }`** (§5): un repositorio en el propio almacén, por `localhost` y con su usuario de solo añadir (p. ej. para las copias de la consola, que así entran en el espejo y la nube; compartir.md, «Un repositorio en su propio almacén»).
  - **Resumen** (§4): `admite`, `repositorios[].retencion_regla` y `repositorios[].verificacion_auto`.
  - Consola: editor de retención con «Programas contables: horarias 15 días, diarias 1 año, mensuales siempre», «Diarias 30 días, semanales 6 meses», 7/4/12/2 y «A medida», la regla en palabras y cuántas versiones quedan con el horario de sus copias; «Verificación automática» en «Cambiar las copias» y en la página del repositorio (con la del almacén); «Copiar en este mismo almacén».
  - Compatibilidad: con un agente sin `admite` (anterior), la consola solo ofrece diarias, semanales, mensuales y anuales, no manda `verificaciones` ni `local`, y dice «Actualiza el agente». Un agente anterior que recibiera `plazos`/`horarias` los ignoraría (serde): por eso la consola no los manda; con `-1` o sin cantidades lo rechazaría («al menos una versión»). Un agente nuevo con una consola anterior: la consola no ve `admite` y todo sigue como antes (lee `retencion` como texto; si una regla nueva no se puede leer, propone la de siempre).
- v1.29 (notificaciones, §13):
  - **Canales** de correo (SMTP con STARTTLS o TLS, rustls), webhook (POST JSON con firma `X-Resguardo-Firma: t=…,v1=HMAC-SHA256`), ntfy y Telegram, del servidor (su propietario: `GET/PUT /api/servidor/notificaciones`, `POST/PATCH/DELETE …/canales[/{k}]`, `POST …/canales/{k}/prueba`, `GET …/registro`) y de cada cliente (sus propietarios: lo mismo bajo `/api/clientes/{c}/notificaciones`). Los secretos van cifrados en la base de datos con una clave derivada de `identidad.key` y la API solo dice `"configurado"`. Crear un canal o cambiar a dónde va o sus secretos pide un código TOTP recién sacado (`codigo`; 401 `codigo`). Auditado (`notificaciones_canal`, `notificaciones_ajustes`).
  - **Personas**: `GET /api/clientes/{c}/notificaciones/personas`, `PUT …/personas/{cuenta}` (propietario o la propia persona; `notificaciones_preferencias`) y `GET/PUT /api/cuenta/notificaciones` (horas de silencio y resúmenes). Por defecto, según el papel.
  - **Avisos nuevos** que crea el servidor: `copia_fallida`, `verificacion_fallida`, `externa_fallida` y `prueba_fallida` (de los informes), `espejo_fallido` (del resumen) y `cambio_clave` (resultado de `cambiar_clave_admin`). Entran en la lista de avisos una vez por problema.
  - **Entrega**: incidentes (lo repetido se cuenta; «Volvió a funcionar» a quien recibió el aviso), tope por hora y destinatario con lo demás agrupado, horas de silencio, cola persistente en `control.db` con reintentos, y resúmenes diario y semanal («Resumen semanal de copias», correo HTML con versión en texto).
  - Consola: Servidor → «Notificaciones» (canales, «Enviar prueba», registro; los del servidor solo para su propietario), Personas y ajustes → qué recibe cada persona y canales propios del cliente, y Ajustes de la cuenta → «Mis notificaciones».
  - Compatibilidad: todo es nuevo. Los agentes no cambian (el servidor lee lo que ya mandaban). Una consola anterior no enseña nada de esto; con un servidor anterior, las rutas nuevas dan 404 y la consola no ofrece las notificaciones. Los tipos de aviso nuevos salen en la lista de avisos de una consola anterior con su tipo tal cual.
- v1.30 (lo que enseña la consola, al día). Todo es compatible hacia atrás:
  - **Versiones tras una copia** (agente ≥ 0.7.14): el agente relee las versiones del repositorio al terminar una copia, como mucho 20 s, antes del informe inmediato. Antes llegaban en el informe siguiente y la consola enseñaba «0 versiones» 15–35 s con la copia ya terminada.
  - **Pista `refrescar`** (§8): cuando el almacén aplica la retención (`retencion_almacen` en su horario o `aplicar_retencion_almacen`), el servidor manda `{ "t": "refrescar", "repo" }` al equipo dueño y este relee sus versiones y su espacio; antes, la consola seguía contando las versiones de antes hasta su próxima copia o 6 h. No da autoridad: el agente solo lee, de un repositorio suyo y como mucho una vez por minuto. Un agente anterior ignora el mensaje; uno por sondeo no lo recibe (sigue como antes).
  - **`destinos[].equipo_almacen`** (§4) en el resumen: la consola lo manda en `crear_repositorio.destino` desde «Copiar en …» y el agente lo guarda. Sin él (agente o consola anterior, destinos de antes), la consola y el servidor reconocen el almacén por el id del destino o su nombre, como hasta ahora.
  - **Notificaciones** (§13): `orden_destructiva` se cierra cuando la orden termina (o se cancela o caduca), sin «Volvió a funcionar». Antes quedaba abierta y una orden destructiva posterior, el mismo día, no volvía a avisar.
- v1.31 (espacio del almacén). Compatible hacia atrás: `guarda_copias.espacio` y `guarda_copias.espejo.destinos[].espacio` (`{ libre, total, leido }` o `null`, §4) en el resumen de un equipo que guarda copias. El servidor no los interpreta (el resumen va tal cual a la consola). Una consola anterior los ignora; con un agente anterior, la consola estima el ritmo de crecimiento con las versiones (lo añadido por día) y no dice cuándo se llena.
- v1.32 (la consola, más visual). Todo es compatible hacia atrás:
  - **Marca del cliente** (§3): `GET`/`PUT /api/clientes/{c}/marca` y `GET /api/clientes/{c}/marca/logo` (PNG), y `marca` en `GET /api/clientes` y `GET /api/clientes/{c}`. Con un servidor anterior, la consola no recibe `marca` (sin logo ni acento propio, como antes) y no ofrece cambiarla (la ruta da 404). Una consola anterior ignora el campo.
  - **Consola instalable (PWA)**: `/manifest.webmanifest`, iconos en `/iconos/`, `/service-worker.js` y `/sin-conexion.html`, servidos como el resto de la consola. El service worker solo guarda archivos de la propia consola (nunca `/api/…` ni páginas HTML). La CSP del servidor no cambia: `manifest-src` y `worker-src` caen en `default-src 'self'` / `script-src 'self'`.
- v1.33 («pulsar para ver más»). Compatible hacia atrás; el servidor no cambia (solo reenvía la sesión cifrada):
  - **Sesión `explorar`** (§7): operaciones nuevas `diferencias` (qué archivos son nuevos, cambiaron o se borraron entre dos versiones, con tamaños y por páginas), `ocupa` (lo que más ocupa de una versión) e `historial_archivo` (en qué versiones está un archivo). El agente (≥ 0.7.14) las anuncia en `lista.ops`; uno anterior no, y la consola dice «Actualiza el agente» (sigue pudiendo explorar en Restaurar).
  - **`trabajando`** (§7): latido del equipo durante una operación larga, con `sobre` y sin `re`; una consola anterior lo descarta.
  - Consola: panel de detalle (por URL: `v`, `vista`, `con`, `filtro`, `vuelta`, `dia`; nunca nombres de archivo) con el detalle de cada versión y vuelta, «Qué cambió», «Lo que más ocupa», las versiones de un archivo y «Restaurar este archivo» (las órdenes `restaurar` y `descargar` de siempre); barras de las gráficas, recuentos, días, cifras de espacio, estados y avisos se pueden pulsar.
- v1.34 (consola en internet). Todo es compatible hacia atrás:
  - **Certificado público automático** (`--dominio`, ACME con Let's Encrypt, reto HTTP-01 en el puerto 80): solo para el dominio. Los agentes siguen con la autoridad propia en `url_agentes` (§2), sin cambios en el agente ni en el SAS v3. `GET /api/servidor` da `url_agentes` y `publico`.
  - **Clientes del servidor** y **cuotas por cliente** (§3), con los errores `cuota` (403 o 429).
  - **Guardia de cliente** (§3): lo de `/api/clientes/{c}` exige ser miembro antes de leer el cuerpo. Antes, un cuerpo mal formado a un cliente ajeno daba 422 en vez de 404 (nunca datos).
  - **Límites para internet**: 1800 peticiones a la API por IP y minuto (salvo desde el propio equipo), 1200 por cuenta con sesión y 1000 conexiones abiertas por IP; 429 `demasiados_intentos`. Detrás de un proxy (`--detras-de-proxy`) la IP es la última de `X-Forwarded-For` en las conexiones desde 127.0.0.1.
  - **Registro para fail2ban**: una línea `Acceso fallido desde <IP>: <qué> (<método> <ruta>)` por cada contraseña, código (TOTP, de recuperación, de primer arranque, de restablecimiento o de un equipo), invitación, ficha o secreto de equipo que falla.
- v1.35 (varias consolas a la vez, [consolas-multiples.md](consolas-multiples.md)). Todo es compatible hacia atrás:
  - **Órdenes nuevas** `anadir_consola` y `quitar_consola` (§5; clave de administración, solo administradores, no destructivas). Un servidor anterior no las conoce y las rechaza al crearlas; un agente anterior contesta «aún no admite». La consola solo las ofrece si el resumen trae `admite: ["consolas_multiples", …]`.
  - **Resumen** (§4): `consolas` y `cambio_config`. Una consola anterior los ignora.
  - **`cambiar_clave_admin`**: `k_cfg_consolas` (opcional). **`desvincular`** y **`baja_equipo`** con otras consolas solo quitan la que los manda. **`quitar_consola`** de la propia consola: `detalle.deja_esta_consola` y el servidor pone el equipo en `local`.
  - **Simétrico**: «otra consola» es cualquier Resguardo Server que los equipos alcancen (otra oficina por VPN, otra de la misma red, una en línea). «Mover a otra consola…» = conectar también a la otra, esperar a que cada equipo informe allí y «Dejar esta consola».
  - **`POST /api/agente/recibir`**: `motivo` (opcional). **`config`** (WebSocket y `POST /api/agente/config`): `etiqueta` y `espera_min_horas` (opcionales; el servidor los guarda si el equipo está confirmado y cambiaron).
  - **Agente**: el vínculo pasa a ser una lista (`otras` en `servidor.bin`; uno anterior es una lista de uno, sin migrar nada), un canal por consola, `seq`, `K_cfg`, bloqueos y respaldo por consola, `nonce` común; CLI `consolas`.
  - **Consola**: «Conectar también a otra consola…» y «Dar un código de conexión…» (cliente → Servidor), «Gestionarlo también desde aquí» (Clientes → Recibir un cliente), «También lo gestiona … · Quitar» y «Cambiado desde otra consola» en cada equipo, y «Dejar de gestionar desde aquí» en vez de «Desvincular» cuando hay otras.
- v1.36 (la ventana del agente, docs/agente-ventana.md). Todo opcional y compatible:
  - **Configuración** (§6): `escritorio { ventana, avisos }` y `cambiado_en_equipo` (lo pone el equipo al cambiar algo con la clave en su ventana). Sin el campo, el agente conserva el que tuviera.
  - **Resumen** (§4): `admite` con `"escritorio"`, `escritorio` y `escritorio_cambiado_en_equipo`.
  - **Progreso** (§8): `lectura`, `subida` y `archivos_s` en las copias. El servidor los deja pasar (son números como los demás); uno anterior los quita.
  - El servidor no cambia en nada más. Lo nuevo en el equipo (la ventana, los avisos, el canal local con la clave y el modo sin consola) no pasa por el servidor.
- v1.37. Todo es compatible hacia atrás:
  - **Correos con la marca del cliente** (§13): un correo de un solo cliente con marca lleva su logo (PNG dentro del correo, `multipart/related` con `Content-ID`) y su acento en la cabecera; lo que junta varios clientes, neutro. Ni la API ni los demás canales (webhook, ntfy, Telegram) cambian.
  - **Ritmos reales** (§6 `Tarea`, agente): `lectura` y `subida` también en las verificaciones, la prueba de restauración y la copia externa, de los contadores de E/S del proceso de restic (como en las copias); en la ventana del equipo, también el espejo a una nube (de rclone) y los bytes copiados del espejo a una carpeta. El servidor ya los deja pasar; una consola anterior los ignora o los usa igual que en las copias.
  - **Cambiar la clave de administración** desde la consola (cliente → Personas y ajustes): `cambiar_clave_admin` (sin cambios en la orden) a cada equipo gestionado que tiene la clave actual (comprobada con su etiqueta), con `k_cfg_consolas` para las otras consolas cuya sal se sabe. La sal del cliente **no cambia** (el servidor no guarda ningún verificador del cliente): en los equipos que aún no han aplicado el cambio sigue valiendo la clave anterior hasta que se conectan (la orden caduca a los 7 días); la consola lo enseña como «Cambio a medias» con las órdenes `cambiar_clave_admin` sin terminar. El paquete de exportación guardado (si lo hay) se vuelve a cifrar con la clave nueva. Servidor: el resultado `hecha` queda además en la auditoría (`clave_admin_cambiada`), y si la etiqueta de un equipo cambia en una `config` cuyo `resumen.cambio_config` es `cambiar_clave_admin` desde **otra** consola, el servidor crea el aviso `cambio_clave` («se cambió desde otra consola»; consolas-multiples.md §4.2). Un servidor anterior no hace esto último; una consola anterior no ofrece el cambio.
