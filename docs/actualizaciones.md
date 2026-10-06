# Actualización automática de los agentes

Diseño acordado (plan de mejoras, tarea 11). Parte de lo ya decidido en [plataforma.md](plataforma.md) §6 y [plataforma-web.md](plataforma-web.md) §10: manifiestos firmados con **minisign** (Ed25519) con la llave **fuera de línea**, SHA-256 de cada archivo, **anillos** y **vuelta atrás automática** si la versión nueva no está sana en 10 minutos. Lo que cambia respecto a aquel diseño:

- los equipos buscan la versión nueva primero en **sus consolas** (cada Resguardo Server puede servirla de espejo) y, si se permite, en las publicaciones de GitHub;
- dos anillos por equipo, **«prueba»** y **«general»**, y una política por cliente (automática, «solo cuando apruebe» o en pausa);
- la consola **no firma nunca**: solo guarda y sirve lo que ya viene firmado, y lo comprueba antes de aceptarlo.

Comandos para publicar: [publicar.md](publicar.md).

---

## 1. Piezas

| Pieza | Qué hace |
|---|---|
| `packaging/llave-publicacion.pub` | Las llaves **públicas** de publicación (una o varias, para poder rotar). Va dentro del agente y del servidor al compilar. Mientras sea el marcador de posición, el agente se compila **sin** actualizaciones automáticas. |
| `scripts/firmar-publicacion.mjs` | En el equipo de quien publica: hace el manifiesto a partir de la carpeta de la versión (hashes y tamaños) y llama a `minisign -S` (que pide la contraseña él mismo). Nuestro código nunca ve la llave privada ni su contraseña. |
| `crates/protocolo/src/publicacion.rs` | Formato del manifiesto, comprobación de la firma (crate `minisign-verify`), comparación de versiones, plataformas, combinación de políticas y la decisión «¿toca actualizar?». Con vectores (`crates/protocolo/vectors/publicacion.json`). |
| `crates/agente/src/actualizacion.rs` | Busca, comprueba, descarga, decide y lanza el actualizador; informa a las consolas. |
| `resguardo-agente --actualizar-agente <plan>` | El **actualizador**: una copia del programa **anterior** (el que funciona), en la carpeta privada. Instala, espera la salud y, si no llega, vuelve atrás. |
| `crates/servidor/src/publicaciones.rs` | El espejo del servidor: acepta una publicación solo si la firma es buena, la guarda y la sirve a sus equipos. Política por cliente y anillo por equipo. |
| Consola | Por cliente, «Versiones»: versión disponible, política, «Actualizar ahora», pausa y la tabla de equipos (versión, anillo, estado). En cada equipo, su versión, anillo y estado. En «Servidor», la versión que sirve y cómo poner otra. |

---

## 2. El manifiesto

Un JSON por versión, `manifiesto-agente.json`, firmado con minisign (`manifiesto-agente.json.minisig`, firma «prehash» de minisign ≥ 0.10). Ejemplo:

```json
{
  "formato": 1,
  "producto": "resguardo-agente",
  "version": "0.7.25",
  "fecha": "2026-10-20T10:00:00Z",
  "canal": "estable",
  "minimo_desde": "0.7.0",
  "notas": "Arreglos de las copias externas.",
  "revocadas": [],
  "archivos": [
    { "plataforma": "windows-x86_64", "tipo": "instalador-nsis", "nombre": "Resguardo-Agente_0.7.25_x64-setup.exe", "sha256": "…64 hex…", "tamano": 23456789,
      "url": "https://github.com/evercarog/resguardo/releases/download/v0.7.25/Resguardo-Agente_0.7.25_x64-setup.exe" },
    { "plataforma": "linux-x86_64", "tipo": "tar.gz", "nombre": "resguardo-agente-x86_64-linux-musl.tar.gz", "sha256": "…", "tamano": 34567890, "url": "…" },
    { "plataforma": "linux-aarch64", "tipo": "tar.gz", "nombre": "resguardo-agente-aarch64-linux-musl.tar.gz", "sha256": "…", "tamano": 12345678, "url": "…" }
  ]
}
```

Reglas (las comprueban el agente **y** el servidor; lo que no cumple se rechaza entero):

- `formato` = 1; `producto` = `resguardo-agente` (una firma de otro producto, p. ej. el servidor más adelante, no vale aquí).
- `version`: `MAYOR.MENOR.PARCHE` con un sufijo opcional `-algo` (preversión, menor que la versión sin sufijo). Se comparan los números, no el texto.
- `fecha`: RFC 3339. `canal`: opcional, de momento solo `estable`.
- `minimo_desde` (opcional): un equipo con una versión anterior no se actualiza solo a esta (tiene que pasar antes por una intermedia o instalarse a mano). La consola lo dice.
- `revocadas` (opcional): identificadores de llave (16 hex, el «key id» de minisign) que ya no valen. El agente las recuerda para siempre (ver §7.3).
- `archivos`: como mucho uno por plataforma (`windows-x86_64`, `linux-x86_64`, `linux-aarch64`); `tipo` `instalador-nsis` (Windows) o `tar.gz` (Linux); `nombre` solo con letras, cifras, `.`, `_` y `-` (sin `/`, sin empezar por punto, ≤ 128); `sha256` en hex; `tamano` entre 1 byte y 512 MB; `url` opcional y **solo `https://`**.
- Manifiesto ≤ 64 KB, firma ≤ 4 KB. Campos desconocidos: se ignoran (compatibilidad hacia delante).

La firma es la de minisign: se comprueba la firma del archivo (Ed25519 sobre el BLAKE2b-512 del manifiesto) **y** la del comentario de confianza. Las firmas «heredadas» (sin prehash) no se aceptan. La llave que firmó tiene que ser una de las fijadas y no estar revocada.

---

## 3. Las llaves

- **Llave privada:** solo la tiene el responsable del proyecto, fuera de línea, con contraseña (minisign la cifra con scrypt). Copia de seguridad impresa o cifrada en otro sitio. Ningún programa de Resguardo la lee ni la guarda.
- **Llaves públicas fijadas** en `packaging/llave-publicacion.pub`, dentro del agente y del servidor al compilar (`include_str!`). Puede haber varias (la actual y la siguiente) para rotar sin cortar las actualizaciones. Las líneas que empiezan por `#` y las `untrusted comment:` se ignoran.
- **Marcador de posición:** mientras el archivo no tenga ninguna llave (lleva la marca `PENDIENTE-SIN-LLAVE`), el agente y el servidor no aceptan ninguna publicación. `npm run build:agente` (y `construir-paquetes.sh`) **se niegan a compilar** así, salvo con `RESGUARDO_SIN_ACTUALIZACIONES=1`, que compila un agente con la actualización automática **apagada** (lo dice en `resguardo-agente estado` y en la consola).
- **Llave de pruebas:** `crates/protocolo/tests/fixtures/llave-pruebas.txt` (privada y pública, marcada en grande como «SOLO PRUEBAS»). Solo la aceptan las **compilaciones de desarrollo** (`debug_assertions`) con `RESGUARDO_LLAVES_PRUEBAS=<archivo .pub>`; en una compilación de publicación esa variable no existe para el programa (como `RESGUARDO_AGENT_DIR`). Nunca se usa para publicar.

---

## 4. Dónde busca el agente

Cada **6 horas** (más un rato al azar de hasta 30 min para no llegar todos a la vez), 10 minutos después de arrancar el servicio y cuando una consola le da un toque (`{"t":"actualizacion"}` por el canal, tras «Actualizar ahora»; como mucho uno por minuto):

1. **Sus consolas**, una a una: `GET /api/agente/actualizacion` (con su credencial de equipo, por el mismo TLS fijado de siempre). Contesta la **política** de esa consola para este equipo y, si la tiene, la publicación (manifiesto y firma tal cual) y de dónde bajar los archivos (`/api/agente/actualizacion/archivos/<versión>/<nombre>`).
2. **GitHub**, si está permitido (`actualizaciones.github`, por defecto sí; se apaga con `resguardo-agente actualizaciones github no` o si no hay salida a Internet, que simplemente falla y se reintenta en 6 h): `https://github.com/evercarog/resguardo/releases/latest/download/manifiesto-agente.json` y su `.minisig`. Los archivos, de la `url` del manifiesto (firmada).

De todo lo que llega se queda con la versión **más alta con firma buena** que tenga archivo para su plataforma. Si la tienen varias fuentes, la baja de una consola antes que de GitHub (la consola está en la red local). Una consola que no conoce la ruta (404, servidor anterior) simplemente no cuenta.

Comprobaciones antes de usar nada (en este orden): tamaño de la firma y del manifiesto → firma minisign con una llave fijada no revocada → JSON y reglas de §2 → `producto` → `version` **mayor** que la instalada (nunca se baja de versión; la única excepción es la vuelta atrás automática de §6, que no descarga nada) → archivo de su plataforma → `minimo_desde`.

---

## 5. Cuándo se instala: anillos y políticas

### 5.1 Lo que decide cada consola

- **Anillo de cada equipo:** `prueba` o `general` (por defecto `general`). Se cambia en la consola, en el equipo o en «Versiones».
- **Política del cliente:**
  - `auto` (por defecto): los equipos en `prueba` se actualizan en cuanto la ven (dentro de la ventana); los de `general`, **N días** después (por defecto 2; de 0 a 30).
  - `manual` («Solo cuando apruebe»): nadie se actualiza hasta pulsar «Actualizar ahora» (para el cliente entero o un equipo).
  - `pausada`: nadie se actualiza, ni siquiera con «Actualizar ahora» desde otra consola. Para cuando algo va mal.
- **Ventana de mantenimiento** (opcional): `desde`–`hasta` en la hora del equipo (p. ej. 22:00–06:00, que cruza la medianoche). Sin ventana, a cualquier hora en que el equipo no esté haciendo nada.
- **Retenidas:** la consola añade sola las versiones que **fallaron** (vuelta atrás) en algún equipo del cliente: los demás no se actualizan solos a esa versión. «Actualizar ahora» la libera.

«Actualizar ahora» guarda la aprobación de **esa** versión (la disponible) y da un toque a los equipos conectados. Es inofensiva (como dice plataforma-web.md §2.2): solo puede llevar a una versión **firmada** y más nueva. Pide el papel de administrador del cliente y queda en la auditoría.

### 5.2 Varias consolas a la vez

Un equipo puede estar en varias consolas (consolas-multiples.md). Se combina lo más prudente:

- modo: `pausada` si alguna lo pausa; si no, `manual` si alguna lo pide; si no, `auto`;
- anillo: `general` si alguna lo tiene en general;
- días: el mayor; retenidas: todas;
- ventanas: el equipo tiene que estar dentro de **todas** (si no coinciden nunca, no se actualiza solo y lo dice: `ventanas_sin_coincidir`);
- aprobación: vale la de cualquier consola, salvo que alguna lo tenga en pausa.

Sin ninguna consola que diga nada (modo local, o consolas anteriores a esta función): `auto`, `general`, 2 días, sin ventana. Así un equipo suelto también recibe los arreglos de seguridad.

### 5.3 La decisión (`publicacion::decidir`, con vectores)

En este orden:

1. Sin versión más nueva → **al día**.
2. Sin archivo para su plataforma → `sin_paquete`.
3. Versión instalada menor que `minimo_desde` → `necesita_intermedia`.
4. Modo `pausada` → **pausada**.
5. Aprobada esa versión → se instala ya (sin mirar anillo, días, ventana ni retenidas), salvo que haya algo en marcha (punto 10).
6. Modo `manual` → `espera_aprobacion`.
7. Retenida o ya falló en este equipo → `retenida`.
8. Anillo `general` y no han pasado los días desde que está disponible → `espera_anillo` (con la fecha).
9. Fuera de la ventana → `espera_ventana` (con la hora en que abre).
10. Algo en marcha (copia, verificación, prueba de restauración, copia externa, espejo, restauración, retención, traer historial…) → `en_marcha`; se vuelve a mirar en la siguiente vuelta (5 min).
11. Un **almacén** («Este equipo guarda copias» activo) sin ventana → `almacen_sin_ventana`: actualizarlo corta un momento el servidor de copias de los demás equipos (en Windows el instalador lo para). Necesita una ventana o «Actualizar ahora».
12. Si no → **instalar**.

«Disponible desde» = la más tardía entre la `fecha` del manifiesto y la primera vez que **este equipo** la vio (se guarda): una versión firmada hace un mes y subida hoy no salta los días del anillo.

---

## 6. Instalar, comprobar la salud y volver atrás

### 6.1 Antes

- La versión instalada y la nueva se anotan en `privado/actualizacion/plan.json` (carpeta solo para SYSTEM y Administradores en Windows, `0700` de root en Linux; se rehace con esos permisos y se niega si es un enlace).
- El archivo se descarga a `privado/actualizacion/descarga.part`, sin pasar del `tamano` del manifiesto, calculando el SHA-256 al vuelo; si coincide, se renombra. Si no, se borra y se anota.
- Si el equipo tiene consolas, solo se actualiza si ha hablado con alguna en los últimos 15 min (si no, la salud del paso siguiente no se podría comprobar y volvería atrás sin motivo).
- Se copia el programa **actual** a `privado/actualizacion/actualizador(.exe)` y se guarda una copia de lo instalado en `privado/actualizacion/anterior/` (Windows: `resguardo-agente.exe`, `restic.exe`, `rest-server.exe`, `rclone.exe`; Linux: `resguardo-agente`, `restic`, `rest-server`, `rclone`, `VERSION`).
- Se lanza el actualizador **fuera** del servicio (Windows: proceso separado, sin ventana, fuera del trabajo del servicio; Linux: `systemd-run --unit resguardo-agente-actualizacion`, para que `systemctl restart` no lo mate con el resto del grupo del servicio) y el servicio sigue normal hasta que lo paren.

### 6.2 Windows

El actualizador vuelve a comprobar el SHA-256 **con el archivo abierto sin permitir escribir** (lo mantiene abierto mientras lo ejecuta: nadie puede cambiarlo entre comprobarlo y lanzarlo, TOCTOU) y ejecuta el instalador NSIS por su ruta absoluta con `/S /ACTUALIZACION=1`, con `TEMP` y `TMP` en una subcarpeta privada (los complementos de NSIS se extraen ahí, no en `C:\Windows\Temp`).

El instalador ya sabía actualizar (`packaging/windows/agente.nsi`): deja la marca `actualizando` (el agente no lo cuenta como «detenido por un administrador»), para el servicio y el Servidor de copias, aparta los programas en uso, copia los nuevos, comprueba la huella del agente, vuelve a registrar el servicio (`--install-service`, que lo arranca) y no toca la configuración, las claves ni los vínculos (`ProgramData\ResguardoAgente`). Con `/ACTUALIZACION=1`, además:

- conserva el icono de la bandeja como estaba (si no estaba en «Ejecutar», no lo pone);
- no intenta vincular nada (ni con la cola de un instalador «listo»).

El icono de la bandeja de la sesión abierta se cierra al actualizar y vuelve al siguiente inicio de sesión (como con una actualización a mano silenciosa).

### 6.3 Linux

El actualizador comprueba el SHA-256 del `.tar.gz` (igual, abierto), lo extrae con el `tar` del sistema (por su ruta, `/usr/bin/tar` o `/bin/tar`, sin dueños ni permisos del archivo) en una carpeta privada vacía y, por cada programa que trae, lo copia junto al instalado (`/opt/resguardo-agente/.<nombre>.nuevo`, `0755` de root, `fsync`) y lo **renombra** encima (atómico: o el viejo o el nuevo, nunca a medias). Después `systemctl restart resguardo-agente`. La unidad de systemd no se cambia (si una versión necesita cambiarla, lo dirán sus notas y se instala a mano). Con el paquete `.deb` también vale: `dpkg --verify` dirá que cambiaron esos archivos hasta el siguiente `apt install` del `.deb`, que los deja como su versión.

### 6.4 La salud

La versión nueva, al arrancar, ve `plan.json` con su versión como objetivo y escribe `privado/actualizacion/salud.json` cuando:

- con consolas: abre el canal con **al menos una** y esta demuestra quién es (la firma de identidad de siempre);
- sin consolas (modo local): se lee su configuración y el `restic` que lleva responde.

El actualizador espera esa marca **10 minutos** como mucho. Si llega: anota `actualizada` y termina. Si no llega, o el instalador falló, o el servicio no arranca: **vuelta atrás**.

### 6.5 Vuelta atrás

- Windows: para el servicio, termina `resguardo-agente.exe`, devuelve los programas de `anterior/` (apartando los nuevos), pone la versión anterior en «Aplicaciones» y arranca el servicio.
- Linux: devuelve los programas de `anterior/` (copiar al lado y renombrar) y `systemctl restart`.

Lo anota en `privado/actualizacion/resultado.json` (`vuelta_atras`, versión, motivo). La versión anterior, al arrancar, lo lee: guarda la versión como **fallida en este equipo** (no lo vuelve a intentar sola), lo pone en su informe (`actualizacion.estado = "vuelta_atras"`, `version_fallida`, `motivo`) y manda el aviso **`actualizacion_fallida`** a **todas** sus consolas. Cada consola retiene esa versión para el resto del cliente (§5.1).

Si la vuelta atrás también falla, el actualizador lo anota (`vuelta_atras_fallida`) y deja el servicio arrancado con lo que haya; el aviso lo manda quien arranque (y Windows reinicia el servicio solo, `recuperacion`).

---

## 7. Seguridad

### 7.1 Una consola comprometida

- **No puede meter código**: el agente solo instala lo firmado con una llave fijada al compilar. El servidor tampoco acepta nada sin firma buena, pero la protección de verdad es la del agente (el servidor podría estar cambiado).
- **Puede no dar actualizaciones** (o pausar, o retener): el equipo sigue mirando GitHub si está permitido, y la consola enseña la versión de cada equipo, así que se ve. Si además está en `pausada` o `manual` por esa consola, no se actualiza: es lo prudente con varias consolas (§5.2), y queda en su auditoría.
- **Puede servir una versión firmada más antigua**: el agente nunca baja de versión (§4).
- **Puede servir un archivo cambiado**: no coincide con el SHA-256 firmado y se descarta.
- **Puede dar «Actualizar ahora» a destiempo**: solo hacia una versión firmada y más nueva, y nunca con algo en marcha.

### 7.2 GitHub comprometido

Quien controle el repositorio o sus publicaciones puede cambiar archivos y manifiestos, pero **no firmar**: sin la llave privada (fuera de línea) no hay manifiesto válido. Puede quitar publicaciones (no habría actualizaciones) o volver a poner una antigua (el agente no baja).

### 7.3 La llave comprometida o perdida

- **Rotación:** el agente lleva varias llaves fijadas. Para cambiar de llave: se publica una versión firmada con la **antigua** que ya lleva la **nueva** fijada; a partir de ahí se firma con la nueva.
- **Revocación:** un manifiesto válido puede listar en `revocadas` los identificadores de llaves que ya no valen. El agente los guarda (`actualizacion.json`, nunca se quitan) y rechaza desde entonces lo firmado con ellas. Si la llave robada es la única fijada, la salida es la de arriba con una versión firmada por la buena, cuanto antes; los equipos que no se actualicen siguen expuestos (se ve en la consola).
- **Perdida** (sin robo): igual que la rotación si queda otra fijada; si no, hay que instalar a mano una versión con la llave nueva.

### 7.4 El archivo descargado (TOCTOU)

La carpeta es solo de SYSTEM y Administradores (Windows) o de root (Linux), no un enlace, y el archivo se comprueba con el SHA-256 **en el mismo manejador** con el que se ejecuta (Windows: abierto sin permitir escribir mientras corre el instalador). Quien pudiera cambiarlo ya es administrador del equipo.

### 7.5 El instalador de Windows

- Se ejecuta por su **ruta absoluta** (nunca del `PATH` ni con una línea de órdenes sin comillas) desde la carpeta privada: nadie sin ser administrador puede dejar DLL a su lado (los instaladores NSIS cargan algunas de su carpeta).
- `TEMP`/`TMP` apuntan a la carpeta privada, no a `C:\Windows\Temp`, donde cualquier usuario puede crear carpetas antes.
- El actualizador es una copia del programa que ya estaba instalado, en la misma carpeta privada; no se ejecuta nada de una carpeta de usuario.
- `taskkill /IM resguardo-agente.exe` del instalador no lo mata (se llama `actualizador.exe`).

### 7.6 Linux

Todo como root: archivos nuevos `0755` de root en `/opt/resguardo-agente` (que ya es `0755` de root), carpeta de trabajo `0700`, `tar` y `systemctl` por su ruta (nunca del `PATH`), sin seguir enlaces al escribir (se escribe en un archivo nuevo, `O_EXCL`, y se renombra).

### 7.7 Lo que no cubre

- Un administrador del equipo puede cambiar el programa: no es un atacante de este modelo (ya lo controla todo).
- Las actualizaciones no van firmadas con Authenticode (decisión de plataforma.md §6): el agente comprueba minisign; Windows no.
- Un equipo apagado o sin red no se actualiza (lo dice la consola: versión vieja).

---

## 8. El servidor como espejo

- `resguardo-server poner-publicacion <carpeta>` (en la máquina del servidor) o «Servidor → Actualizaciones de los agentes → Subir…» en la consola (propietario del servidor): el manifiesto, su firma y los archivos. El servidor comprueba la firma y cada SHA-256 **antes** de aceptarlos, los guarda en `publicaciones/agente/<versión>/` (solo para el usuario del servicio) y, cuando están todos, la sirve. Guarda como mucho las 3 últimas.
- Nunca firma ni cambia nada: sirve el manifiesto y la firma **tal cual** (byte a byte).
- Rutas en `docs/api-servidor.md` §14.
- **Más adelante:** que el servidor la baje solo de GitHub (con la misma comprobación) y que se actualice él mismo con el mismo mecanismo (otro `producto` en el manifiesto). Ver el plan, tarea 11.

---

## 9. En la consola

- **Cada equipo:** versión, anillo (con el selector) y estado: «Al día», «Pendiente» (con el motivo: espera su anillo hasta tal día, fuera de la ventana, esperando aprobación, retenida, algo en marcha), «Actualizando», «Falló» / «Volvió a la anterior» (con el motivo), «En pausa», «Sin actualización automática» (agente compilado sin llave) o nada (agente anterior a esta función).
- **«Versiones» del cliente:** la versión disponible (y en qué consola), la política (automática con los días del anillo general, solo cuando apruebe, en pausa), la ventana, «Actualizar ahora» (todos o un equipo) y la tabla de equipos.
- **Servidor:** la versión que sirve el servidor, «Subir una publicación…» y los pasos a mano para el propio servidor (que aún no se actualiza solo).

El estado lo dice el **agente** en su informe (`actualizacion`); la consola no lo calcula, solo lo enseña (`lib/actualizaciones.ts`, con vectores).

---

## 10. Pruebas

- Firma (protocolo): buena, mala, llave equivocada, manifiesto cambiado, firma heredada, llave revocada, marcador de posición; con la llave de pruebas (`tests/fixtures`).
- Versiones, combinación de políticas y decisión: vectores compartidos (`vectors/publicacion.json`).
- Agente: el actualizador con una plataforma falsa (instala bien / falla el instalador / no llega la salud → vuelta atrás; vuelta atrás que falla), la descarga con tamaño y hash, y el informe.
- Servidor: solo acepta publicaciones firmadas (sin firma, firma mala, archivo cambiado, versión con archivos que faltan), sirve byte a byte, permisos de las rutas, política y anillo.
- Consola: vectores de los estados.
- e2e: una publicación de prueba firmada con la llave de pruebas, subida a la consola; el equipo B la ve, la comprueba, la baja y «se actualiza» con el instalador **simulado** (solo en compilaciones de desarrollo: `RESGUARDO_ACTUALIZACION_SIMULADA`); la consola ve su estado. La sustitución real del programa y el servicio se prueba en máquinas virtuales (§11).

## 11. Lista para probar en máquinas virtuales

Con una llave de pruebas propia (nunca la de publicar) y un agente compilado con ella:

1. **Windows 10/11 y Server 2019/2022**, agente instalado con el instalador normal, vinculado a una consola: subir una versión firmada más nueva a la consola, anillo `prueba` → se actualiza en ≤ 6 h o al momento con «Actualizar ahora»; la bandeja vuelve al iniciar sesión; la configuración, las claves y los vínculos siguen; el Servidor de copias vuelve a arrancar.
2. Igual, con una versión que no arranca (p. ej. una compilación que sale al instante) → a los 10 min vuelve a la anterior, la consola recibe «actualizacion_fallida» y los demás equipos del cliente no la instalan.
3. Con una copia en marcha → no se actualiza hasta que termina.
4. Con ventana 22:00–06:00 → solo en ese tramo.
5. Archivo cambiado en la consola (otro SHA-256) → se descarta y lo dice.
6. Consola apagada y GitHub accesible → la baja de GitHub; sin Internet → sigue igual y lo reintenta.
7. **Debian 12 y Ubuntu 24.04** (y un CT de Proxmox), con `instalar-agente.sh` y con el `.deb`: lo mismo que 1–3; `systemctl status` muestra la unidad `resguardo-agente-actualizacion` terminada; `ls -l /opt/resguardo-agente` con `root:root 0755`.
8. Linux sin `systemd-run` (raro): no se actualiza solo y lo dice.
9. Apagar el equipo en mitad de la actualización → al arrancar, o la versión vieja o la nueva (nunca a medias); si quedó la nueva sin salud, el actualizador ya no está: el servicio nuevo arranca normal y lo anota (la vuelta atrás solo la hace el actualizador).
