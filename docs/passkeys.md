# Llaves de acceso (passkeys) para entrar en la consola

Plan 0.8.1, H3 (plan-mejoras 12c). Rama `ia/081-h`. Código: `crates/servidor/src/webauthn.rs` (comprobaciones), `crates/servidor/src/api/passkeys.rs` (rutas), `consola/src/lib/passkeys.ts` y `LlavesAcceso.svelte` (navegador). Contrato en [api-servidor.md](api-servidor.md), «Cambios».

## Qué hace

- En «Mi cuenta y servidor», cada persona da de alta una o varias **llaves de acceso** (la huella o el PIN del equipo con Windows Hello, el móvil, una llave USB FIDO2), con un nombre, y las quita cuando quiera. Dar de alta y quitar piden **la contraseña y un código TOTP**, como cambiar de aplicación de verificación.
- Al entrar, tras la contraseña, si la cuenta tiene llaves para esa dirección, la consola ofrece **«Usar mi llave de acceso»** en vez de escribir el código de la aplicación.
- El TOTP sigue siendo obligatorio y sigue valiendo, igual que los códigos de recuperación. Perder la llave no deja a nadie fuera.
- Si un propietario restablece la verificación en dos pasos de alguien (v1.27), también se quitan sus llaves.

## Decisión: segundo factor, no entrada sin contraseña

Se pidió valorar la entrada sin contraseña. **No se hace en esta versión**, por tres motivos:

1. Con una llave sola, la llave pasa a ser el único factor. Una passkey con verificación del usuario (huella, PIN) equivale a dos factores, pero no todas lo garantizan: una llave USB sin PIN solo prueba que alguien la tocó. Para exigirla habría que pedir `userVerification: "required"` y rechazar las que no la hacen, lo que deja fuera llaves que la gente ya tiene.
2. La consola local entra muchas veces por una IP o con un certificado propio; allí no hay llaves de acceso (ver abajo). Una entrada sin contraseña obligaría a mantener dos caminos de entrada distintos según la dirección.
3. Es más sencillo de auditar: la contraseña sigue siendo el primer paso, con sus límites de siempre, y la llave solo sustituye al código TOTP, con el mismo contador de intentos. No hay un camino de entrada nuevo que proteger.

Se puede revisar más adelante (con `userVerification: "required"` y credenciales detectables) si se pide.

## Cómo funciona

**Alta** (`POST /api/cuenta/passkeys/opciones` y `POST /api/cuenta/passkeys`):

1. La consola manda la contraseña y un código TOTP. El servidor los comprueba (`reautenticar`, con su límite de intentos) y da las opciones de `navigator.credentials.create`: un **reto** de 32 bytes al azar, de **un solo uso** y que **caduca a los 5 minutos**; el `rp.id` (el nombre de la consola, de la cabecera `Host`, sin el puerto); el usuario (el id de la cuenta, su correo y su nombre); los algoritmos que se aceptan (ES256, EdDSA y RS256); las llaves que ya tiene, para no repetirlas; `attestation: "none"`.
2. El navegador crea la llave (la persona pone la huella, el PIN o toca la llave) y la consola manda la respuesta con un nombre.
3. El servidor gasta el reto (vale una vez, se acierte o no) y comprueba: `type` es `webauthn.create`; el reto es el suyo; el `origin` es exactamente `https://<Host>`; no es `crossOrigin`; el `rpIdHash` es el SHA-256 del `rp.id`; la **presencia del usuario** (UP); la clave pública es COSE válida y de un algoritmo admitido; el id que manda la consola es el de la credencial. Guarda el id, la clave pública, el algoritmo, el contador, el `rp.id` y las fechas. Como mucho 10 por cuenta. Queda en la auditoría del servidor (`alta_passkey`) con el **nombre** de la llave, nunca su id ni su clave.

**Entrada** (`POST /api/sesion/passkey/opciones` y `POST /api/sesion/passkey`):

1. Tras la contraseña (sesión a medias), la consola pide las opciones de `navigator.credentials.get`: otro reto de un solo uso que **caduca a los 2 minutos**, atado a esa sesión a medias, y las llaves de la cuenta **de esa dirección** (las de otra dirección no se ofrecen).
2. El servidor gasta el reto y comprueba: `type` es `webauthn.get`; el reto; el `origin`; no `crossOrigin`; el `rpIdHash` del `rp.id` **con el que se dio de alta esa llave** y que es el de esta dirección; la presencia del usuario; la **firma** sobre `authenticatorData ‖ SHA-256(clientDataJSON)` con la clave guardada; y el **contador de firmas**: si el autenticador lo lleva (no es 0), tiene que subir; si no sube, puede ser una llave copiada y se rechaza (y se anota `passkey_contador` en la auditoría).
3. Si todo cuadra, la sesión a medias se cierra y se abre una completa con una ficha nueva, como con el TOTP. Queda en la auditoría (`entrar_passkey`, con el nombre de la llave).

Los intentos de entrar con una llave cuentan **junto con los del TOTP** (`totp:<cuenta>`, 10 cada 15 minutos): no se suman, así que tener las dos cosas no da más intentos. Pedir opciones también tiene su límite.

**Baja** (`DELETE /api/cuenta/passkeys/{ref}`): con la contraseña y un código TOTP; en la auditoría (`baja_passkey`). La referencia es un resumen del id, no el id.

## Sin bibliotecas nuevas

`webauthn-rs` arrastra OpenSSL (y su compilación en Windows) y mucho más de lo que hace falta. Se implementó solo lo necesario con `ring` (que ya usan rustls y ACME): un lector de CBOR con longitudes definidas, límites de tamaño (64 KiB por trozo, 256 elementos) y de profundidad (8); la clave COSE; ECDSA P-256 con SHA-256, Ed25519 y RSA PKCS#1 v1.5 con SHA-256 (de 2048 a 8192 bits). En el navegador, la API nativa (`navigator.credentials`). Ninguna dependencia nueva: `cargo deny` no cambia.

Las pruebas del servidor usan un **autenticador simulado** (P-256 y Ed25519 con `ring`): alta y entrada de las dos clases, y lo que se rechaza (otro reto, otro origen, otro `rpId`, tipo cambiado, sin presencia, firma cambiada, contador que no sube, `crossOrigin`, CBOR malformado), y el camino entero por HTTP (alta con la contraseña y el código, reto de un solo uso, firma desde otra dirección, entrada, firma repetida, auditoría sin datos de la llave y baja).

## Dónde funciona

- **Consola en línea** (un nombre con certificado público): sí.
- **Consola local por su nombre** (`https://consola.oficina.lan:8443`) con el certificado de su autoridad propia **instalado como de confianza** en el equipo: sí. Los navegadores no dejan usar llaves de acceso en una página con errores de certificado.
- **Consola local por IP** (`https://192.0.2.10:8443`): **no**. Las llaves se atan a un nombre de dominio y los navegadores no las admiten con una IP. La consola lo dice en «Mi cuenta y servidor» («Entras por una IP: …») y al entrar sigue pidiendo el código.
- `http://localhost` (desarrollo): sí.
- Detrás de un proxy, el proxy tiene que pasar la cabecera `Host` original: el servidor saca de ella el `rp.id` y el origen que espera.

Una llave dada de alta por un nombre solo sirve por ese nombre: si se entra a la misma consola por otro nombre, hay que darla de alta también ahí.

## Amenazas

| Amenaza | Qué la frena |
|---|---|
| Página falsa que imita la consola y reenvía la firma (phishing) | El navegador firma el `origin` real dentro de `clientDataJSON` y la llave está atada al `rp.id`: el servidor exige su propio origen y su `rpIdHash`. Una firma hecha en otra dirección no vale. |
| Repetir una firma capturada | Reto de un solo uso, atado a la sesión a medias y que caduca a los 2 minutos; la sesión a medias se cierra al entrar. |
| Llave copiada (clonada) | El contador de firmas tiene que subir; si no, se rechaza y queda anotado. Las passkeys sincronizadas (de una cuenta en la nube) llevan el contador a 0 y no se puede saber: es el límite conocido de WebAuthn. |
| Robo de la contraseña | Sigue haciendo falta el segundo paso: el código TOTP o la llave. |
| Alguien con la sesión abierta da de alta su propia llave | Dar de alta pide la contraseña y un código TOTP; queda en la auditoría; el propietario puede restablecer la verificación en dos pasos, lo que quita todas las llaves. |
| Adivinar a base de intentos | Mismo límite que el TOTP (10 cada 15 minutos por cuenta), y la contraseña antes. |
| Servidor comprometido | Puede dar de alta una llave suya para una cuenta o aceptar cualquier firma: es lo mismo que ya podía hacer con el TOTP (guarda su secreto). Las llaves no dan acceso a las copias: restaurar o cambiar algo delicado pide la contraseña del repositorio o la clave de administración, que el servidor no tiene. |
| Filtrar datos de la llave | El servidor solo guarda la clave **pública**; la auditoría lleva el nombre de la llave, nunca su id, su clave ni la firma; la lista en la consola no lleva el id de la credencial. |
| Atestación (saber el fabricante) | No se pide (`none`): no hace falta y así no se guarda nada que identifique el aparato. Tampoco se puede limitar a fabricantes concretos (no se pidió). |

## Lo que queda sin probar

- Con un autenticador de verdad (Windows Hello, un móvil, una llave USB) en una consola real: las pruebas usan un autenticador simulado y el simulador de la consola no comprueba firmas.
- Detrás de un proxy inverso que cambie la cabecera `Host`.
