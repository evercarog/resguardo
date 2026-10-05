# Consola de Resguardo Server

La SPA que Resguardo Server lleva dentro (`--consola DIR`, y más adelante con `rust-embed`): SvelteKit con `adapter-static`, Svelte 5 y TypeScript. Usa el sistema de diseño de la app de escritorio ([`ui/`](../ui), [docs/diseno.md](../docs/diseno.md)) y habla con la API de [docs/api-servidor.md](../docs/api-servidor.md).

## Uso

```sh
cd consola
npm ci
npm run dev:mock        # con el servidor simulado (no hace falta Rust): http://127.0.0.1:5180
npx vite preview --mode mock  # la consola compilada (con su service worker, PWA) contra el simulador
npm run dev             # contra un servidor real: RESGUARDO_SERVIDOR=https://127.0.0.1:8443 (por defecto)
npm run check           # svelte-check
npm run build           # build/ (lo que se le pasa al servidor con --consola)
npm run test:vectores   # criptografía contra crates/protocolo/vectors y libsodium
```

**Simulador** (`src/mock`): un Resguardo Server en memoria dentro de Vite, con cookies `HttpOnly`, la cabecera CSRF, roles, órdenes, avisos, auditoría encadenada con la misma huella que el servidor, sesiones con espera larga y relé. Su «agente» abre los sobres de verdad con su X25519, comprueba la prueba de administración contra el verificador y las contraseñas, firma los resultados y contesta cifrado. Entrar: `ana@ejemplo.com` / `resguardo` y cualquier código de 6 cifras; la clave de administración de prueba y la contraseña de los repositorios están en `src/mock/estado.ts` (`DEMO`). `POST /api/__mock/reiniciar?vacio=1` deja el servidor sin cuentas (primer arranque, código `MOCK-1234`).

**Contra el servidor real** (probado con `main` en 9344dc2):

```sh
resguardo-server --datos ./datos --escuchar 127.0.0.1:8470 --sin-tls --consola consola/build
npx tsx scripts/agente-de-prueba.ts --servidor http://127.0.0.1:8470 --codigo ABCD-EFGH-JK
```

`scripts/agente-de-prueba.ts` es un agente mínimo (sondeo, sin restic) para probar emparejamiento, alta, órdenes firmadas, configuración cifrada y sesiones sin el agente real. Probado así de punta a punta: primer arranque, TOTP, cliente, emparejamiento con SAS v2, alta, crear repositorio, editor de copias (descifrando la configuración que sube el agente), elegir carpetas en vivo, orden con espera y la cadena de auditoría comprobada también en el navegador.

**Escenario de extremo a extremo** (`scripts/e2e`, `npm run e2e`): los programas de verdad juntos, como procesos normales con carpetas temporales y puertos libres en 127.0.0.1 (nunca toca los servicios instalados): `resguardo-server` y dos `resguardo-agente` de desarrollo (en modo de pruebas, `RESGUARDO_AGENT_DIR`), con restic y rest-server. «La consola» es el script, con la criptografía y la lógica de `src/lib`: primer arranque con TOTP, emparejar con SAS v3, «Este equipo guarda copias», «Copiar en», una copia con horario y exclusiones, «Copiar ahora» con el progreso en vivo, lo que enseña la consola (versiones, tamaño, próxima, cifras de la copia, protección), restaurar «junto al original», retención en el almacén con plazos y versiones de hora falsa, la consola en vivo (el canal `…/vivo` ve empezar una copia programada y las cifras nuevas llegan en segundos al terminar, sin preguntar; desde otra web no se abre), correo de aviso y «Volvió a funcionar» (un SMTP de mentira), verificación automática, copia de la consola restaurada en otra carpeta (los equipos vuelven solos) volver a vincular con otro servidor (sube el historial) y, con un tercer servidor, varias consolas a la vez (conectar con un código de conexión, órdenes de las dos, quitar cualquiera). Unos 10 minutos (la copia programada espera a que pasen 5 desde la anterior).

```sh
cargo build -p resguardo-servidor -p resguardo-agente --bins
npm run e2e
```

restic y rest-server: en Windows, los de `src-tauri/binaries`; en Linux, `RESGUARDO_E2E_BINARIOS=<carpeta de scripts/fetch-binarios-linux.sh>`. Sin ellos se salta. `RESGUARDO_E2E_CONSERVAR=1` deja la carpeta con los registros (`registros/`, `agente-*/agent.log`, los correos recibidos). Los programas arrancan con `RESGUARDO_PRUEBA_SIN_ESPERA=1` (solo en compilaciones de desarrollo): las órdenes que reducen la protección no esperan horas.

## Seguridad

- **Secretos.** La clave de administración y las contraseñas de los repositorios solo existen en el campo donde se escriben y en variables locales mientras se usan; los bytes derivados (prueba, `K_cfg`, claves de sesión y del relé) se borran al terminar. Nunca se envían en claro ni se guardan (ni en `localStorage`). `localStorage` solo guarda el tema, el acento y el último cliente abierto.
- **Etiqueta.** Antes de sellar algo con la clave de administración se comprueba la etiqueta HMAC del equipo (que sus llaves son las confirmadas al emparejar); si no cuadra, no se envía nada.
- **Llaves fijadas (TOFU, `src/lib/fijadas.ts`).** Tras comprobar la etiqueta, las claves públicas del equipo se guardan en IndexedDB (por cliente y equipo; solo claves públicas). Una orden con la contraseña del repositorio a un equipo sin llaves fijadas en este navegador pide también la clave de administración la primera vez. Si las llaves que da el servidor cambian, la consola para, muestra una alerta de seguridad y no envía nada; en la ficha del equipo se pueden volver a comprobar con la clave de administración.
- **NFC.** La clave de administración se normaliza a Unicode NFC antes de Argon2id (los dos lados).
- **SAS.** La consola calcula el SAS v2 con la identidad del servidor y lo compara con el que da el servidor; si no coinciden, no deja seguir.
- **Respuestas.** La firma Ed25519 de cada resultado se comprueba con la `sign_pub` del equipo y se muestra («Firmada por el equipo»).
- **Auditoría.** «Verificar la cadena» pregunta al servidor y, además, recalcula en el navegador las huellas de lo cargado (`SHA-256(prev|n|creado|actor|accion|objetivo|datos)`, como `hash_entrada` del servidor).
- **CSP.** El servidor envía `script-src 'self' 'wasm-unsafe-eval'` (sin hashes). SvelteKit genera un `<script>` de arranque en línea, así que `scripts/arranque-externo.mjs` lo saca a `_app/arranque-<hash>.js` tras el build (con `kit.paths.relative = false`). El `<meta>` CSP repite lo mismo para el modo de desarrollo; `frame-ancestors 'none'` solo puede ir por cabecera (ya la envía el servidor).
- **`'wasm-unsafe-eval'`** hace falta para Argon2id con hash-wasm (WASM, la implementación de referencia en C: varias veces más rápido que JavaScript, importante en móviles). Si una CSP más estricta lo impide, el worker usa `@noble/hashes` (JavaScript puro, ~1 s en un PC) y da lo mismo.

## Criptografía (`src/lib/cripto`)

| Pieza | Implementación | Por qué |
|---|---|---|
| Sobre sellado (`crypto_box_seal`) | `@noble/curves` (X25519), `@noble/ciphers` (HSalsa20, XSalsa20-Poly1305), `@noble/hashes` (BLAKE2b) | JavaScript puro y auditado, sin WASM ni dependencias nativas, igual en todos los navegadores. Se comprueba contra libsodium en las dos direcciones, byte a byte, y abre el sobre que genera Rust y los vectores «simetrico». |
| Argon2id (64 MiB, t=3, p=1) | hash-wasm en un worker, respaldo `@noble/hashes` | Velocidad sin congelar la página; las dos implementaciones se comparan en las pruebas. |
| HKDF, HMAC, SHA-256, Ed25519 | `@noble/*` | Las derivaciones de `crates/protocolo/src/derivaciones.rs`. |
| Sesiones, relé y configuración | XChaCha20-Poly1305 (`@noble/ciphers`) | Los formatos del contrato (abajo), con sus vectores. |

`npm run test:vectores` pasa los 62 casos: los vectores de `crates/protocolo/vectors/v1.json` (código, SAS v1, X25519, Ed25519, sobre firmado, el sobre sellado de Rust y las **derivaciones**: prueba, verificador, Argon2id del cliente, `K_cfg`, `K_exp`, etiqueta, SAS v2, identidad del servidor y texto del resultado), los vectores «simetrico» (configuración, sesión y relé), «paquete» y «derivaciones.nfc», libsodium, la segunda implementación de Argon2id y las idas y vueltas.

## Formatos (contrato v1.5)

Los de [docs/api-servidor.md](../docs/api-servidor.md), comprobados con los vectores de `crates/protocolo/vectors/v1.json`:

- **Simétrico** (`src/lib/cripto/simetrico.ts`): configuración con XChaCha20-Poly1305, `K_cfg` y `aad = "resguardo-config-v1|" + equipo + "|" + seq`; claves de sesión `HKDF(clave_sesion, salt = sesion_id, info = "resguardo-sesion-v1|consola" | "…|equipo")` y mensajes `nonce ‖ XChaCha20-Poly1305(…, aad = "resguardo-sesion-v1|" + id)`; trozos del relé con `aad = relevo_id|n|1-o-0` y 4 MiB sin cifrar.
- **`alta`**: `{ verificador, k_cfg, espera_min_horas }` con `prueba_admin` y `prueba_codigo` (HMAC del código de emparejamiento).
- **`config`**: `{ config: Configuracion v1 }` sin secretos (copias con `dias` 1–7, `gancho: null`; verificación y bandeja). Repositorios y destinos los escribe el equipo.
- **`crear_repositorio`**: `{ id, nombre, contrasena, destino: { id } | { id, nombre, tipo, donde, usuario?, secreto?, ca_pem? } }`: la única entrada de secretos. La contraseña la genera la consola y se imprime en el kit.
- **Resto de cuerpos**: `copiar_ahora { copia }`, `pausar { horas, repo? }`, `reanudar { repo? }`, `cambiar_retencion` (solo guarda) y `aplicar_retencion`, `dejar_de_copiar`/`quitar_repositorio { repo }`, `cambiar_espera { horas }`, `desvincular { modo }`, `explorar { repo, sesion, clave_sesion }`, `elegir_carpetas { sesion, clave_sesion }`, `restaurar { repo, version, rutas, destino, reemplazar }`, `descargar { repo, version, rutas, formato, relevo }` (varias rutas, solo en zip), `cambiar_destino`, `guarda_copias` (activar, desactivar, `anadir` con `responder_a`, `quitar`) y `desbloquear { repo? }`.
- **Copia externa y espejo (v1.5)**:
  - `cambiar_copia_externa { repo, destino: { id } | { id, nombre, tipo, donde, usuario?, secreto? }, hora | null, retencion?, contrasena_destino?, existente?, ruta?, bloqueo_dias?, solo_probar? }` va con la contraseña del repositorio. Solo se ofrece en repositorios con copias activas y hacia otro destino. Con un agente que lo admite (`admite: "externa_existente"`): «Usar uno que ya existe» (dirección completa, credenciales y su contraseña), «Probar» (`solo_probar`, la misma orden sin guardar nada) y «El destino tiene bloqueo de objetos» (`bloqueo_dias`).
  - `guarda_copias { espejo: { carpeta, hora } | null }` va con la clave de administración.
  - Las carpetas se eligen en el propio equipo, en una sesión `elegir_carpetas` de una sola carpeta.
- **Cambiar de servidor (F6, §11)**:
  - «Recibir un cliente» (superusuario) crea el cliente con la misma sal y da un bloque `{ url, identidad, ca_pem, ficha }`.
  - En el antiguo, `cambiar_servidor` sellada lleva ese bloque a cada equipo, y la página «Servidor» sigue cada traslado.
  - `servidores_respaldo { servidores (≤ 3), dias }`.
  - Las fichas nunca se guardan en el navegador.
- **Paquete `.resguardo-cliente`** (`src/lib/cripto/paquete.ts`, `src/lib/exportar.ts`):
  - JSON con cliente, equipos, configuraciones cifradas, informes, avisos y auditoría, cifrado con `K_exp` en trozos de 4 MiB.
  - Se descarga y, si se quiere, se guarda cifrado en el servidor (`PUT …/paquete`).
  - En el nuevo se abre aquí y se manda `POST …/importar`. Su actividad sale aparte en «Actividad → Servidor anterior».
- **Restaurar en otro equipo (§10)**:
  - Primero se comprueban las llaves de B con su etiqueta.
  - A recibe `compartir_acceso { repo, para: { equipo, box_pub }, incluir_contrasena: false }`.
  - B recibe `importar_repositorio { id, nombre, acceso_sellado, sign_pub_origen, contrasena }`, con la contraseña sellada solo para B.
  - Sin A, se usan los datos del kit (`acceso`). Los kits imprimen ahora el id del repositorio.
  - Los importados salen como «Solo lectura» y no se ofrecen para copias. Los equipos `trasladado` salen en gris y sin órdenes.
- **Informe detallado (v1.7, `informe.datos.repos[]`)**:
  - Se lee en `src/lib/repo.ts`, junto con el resumen; sin informe, todo sigue con el resumen.
  - Alimenta la página de cada repositorio (`/c/{c}/equipos/{e}/repositorios/{r}`), las tarjetas del equipo y las de Estado.
  - Componentes en `src/lib/componentes/repo/`: cuadros de 60/14 días, anillo y salud de la protección, flujo, gráficas, versiones e Historia.
  - Restaurar acepta `?version=` (y `&todo=1`).
- **Destructivas según el cuerpo**: la consola pone el `not_before` en `desvincular` «dejar de copiar», `guarda_copias` al desactivar, quitar o quitar el espejo, `cambiar_copia_externa` con `hora: null`, `cambiar_espera` a menos horas y `restaurar` en su sitio reemplazando. La espera es la del equipo (`espera_min_horas`) o la del cliente.
- **Rutas de una versión**: las de restic (`/C/Users/…`).
- **Detalle**: JSON en texto (espera, trozos o `{ sellado }` para `responder_a`).
- **«Próximamente»**: ganchos, actualizar el agente y rotar la contraseña del repositorio.

## Nubes (Dropbox) desde la consola

- **App «Resguardo» de Dropbox** con permiso «App folder» (`files.content.read`/`write`): solo ve `Aplicaciones/Resguardo`. Sin app secret.
- **App key** (pública): `beobf3c13cvlrup`, la de la app «Resguardo» registrada, en `DROPBOX_APP_KEY_POR_DEFECTO` (`src/lib/nubes.ts`).
  - Si el servidor da `dropbox_app_key` en `GET /api/servidor`, manda la suya; vacía significa «sin configurar» y la consola lo dice.
  - En el simulador el token es de prueba.
- **Carpetas en la nube.** Son relativas a la carpeta de la app (`Aplicaciones/Resguardo` ya es la raíz): `Sur`, no `Resguardo/Sur`.
- **Flujo (OAuth 2 con PKCE, sin redirect_uri):**
  - se abre `https://www.dropbox.com/oauth2/authorize?…&code_challenge_method=S256&token_access_type=offline`;
  - la persona pega el código;
  - el navegador lo cambia en `https://api.dropboxapi.com/oauth2/token` (admite CORS: `Access-Control-Allow-Origin: *`);
  - el token va sellado al equipo en `conectar_nube { tipo: "dropbox", nombre, refresh_token, access_token?, expira?, app_key }` (clave de administración).
- **Desconectar.** `quitar_nube { nombre }` es destructiva si el espejo la usa.
- **CSP.** `connect-src` incluye `https://api.dropboxapi.com`, en `svelte.config.js` y también en la cabecera que pone el servidor.

## Decisiones

TOFU de las llaves por equipo; NFC en los dos lados; la espera la confirma el equipo en su resultado firmado; restaurar en otro equipo con A vivo o con el kit; la contraseña del repositorio va aparte para B (`incluir_contrasena: false`); los técnicos leen la actividad pero no la exportan; la consola declara el `not_before` de las destructivas según el cuerpo.

## Integrar en el servidor

- `npm run build` deja la consola en **`consola/build/`** (ruta fija): `index.html` como página de reserva de la SPA, `_app/` con los recursos y `favicon.svg`. Es lo que se pasa con `--consola consola/build` o lo que debe meter `rust-embed`.
- La versión del build es la del `package.json` (o `RESGUARDO_VERSION`) y el resultado es el mismo en cada compilación.
- Sin peticiones externas: la fuente (Inter), los iconos y el WASM van dentro; `scripts/arranque-externo.mjs` falla si el HTML carga algo de fuera. La consola solo habla con su propio origen (`connect-src 'self'`).
- Probado así contra el servidor real con su CSP por cabecera: arranque, primer uso, emparejamiento con alta, editor de copias con la configuración cifrada del agente y el worker de Argon2id (da la prueba de los vectores de Rust).
