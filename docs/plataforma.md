# Resguardo: servidor propio, agentes y código abierto

Diseño de la arquitectura de Resguardo. **Sustituye a [plataforma-web.md](plataforma-web.md)**, que se conserva como antecedente: de él se reutiliza todo lo que sigue valiendo. Estado: propuesta para revisar (2026-10-02). Este documento no incluye código.

Documentos relacionados:
- [agente-gestionado.md](agente-gestionado.md): fase 5, sobres, `seq` y emparejamiento con SAS.
- [compartir.md](compartir.md): fases 3 y 4, Servidor de copias.
- [destinos.md](destinos.md): destinos y repositorios.
- [diseno.md](diseno.md): sistema de diseño.
- [consolas-multiples.md](consolas-multiples.md): un equipo gestionado desde varias consolas a la vez.
- [capacidad.md](capacidad.md): cuánto pide una consola abierta y un equipo, los límites por cuenta e IP y lo que aguanta el servidor (medido).
- [estabilidad.md](estabilidad.md): la prueba de resistencia (horas con averías), lo que se midió, lo que ve la consola en cada avería y los límites.

---

## Resumen

**La idea.** Resguardo pasa a ser **un programa servidor que cualquiera instala en su propio equipo**: «Resguardo Server». Puede ir en un contenedor de Proxmox, una máquina Debian o Ubuntu, o Docker. Desde su consola web se gestionan las copias de todos los equipos de la empresa. En cada equipo (Windows, Linux o Mac) hay un **agente** que hace las copias y se conecta al servidor, siempre hacia fuera.

**La versión «en línea» es el mismo servidor**, instalado por ti en Internet para varios clientes a la vez. Si mañana lo instala otra persona, también vale. No hay dos productos: hay uno, instalado en sitios distintos.

**Los equipos no confían en el servidor; confían en la clave de administración del cliente.** El servidor es un mensajero y un archivo de datos cifrados.
- Para cambiar qué se copia, crear copias o mover el cliente de servidor hace falta la **clave de administración**.
- Para ver archivos, restaurar o borrar versiones hace falta la **contraseña del repositorio**.

El navegador las escribe en el momento, cifradas solo para el equipo que las comprueba. El servidor nunca las guarda. Un servidor comprometido, o alguien que robe su base de datos, no puede ordenar nada importante a los equipos ni leer las copias.

**Moverse es fácil y no exige reinstalar.**

| Desde | Hacia | Cómo |
|---|---|---|
| Servidor del cliente | Tu servicio en línea (o al revés) | «Cambiar de servidor»: una orden firmada con la clave de administración. Cada equipo se conecta al nuevo y le sube su configuración cifrada. |
| Cualquier servidor | Sin servidor | Los equipos siguen haciendo sus copias solos y se manejan con la línea de órdenes o la app de escritorio. |
| Sin servidor | Cualquier servidor | «Vincular»: nada se reconfigura. |
| Un servidor que muere | Uno nuevo | Los equipos siguen copiando. Se instala otro servidor y se vuelven a vincular con la clave de administración; no se pierde nada de los equipos. |
| Un servidor | Ese **y** otro a la vez | «Conectar también a otra consola»: una orden con la clave de administración y un código de conexión de la otra. Cada consola funciona sola ([consolas-multiples.md](consolas-multiples.md)). |

**Para tu carrera y la comunidad.** El servidor y la consola se publican como **código abierto** (se propone AGPL-3.0), con un modelo de seguridad público, publicaciones firmadas, compilaciones reproducibles, lista de componentes (SBOM), pruebas de fuzzing y una auditoría externa como meta. Es un proyecto que se puede contar en artículos y charlas: «copias inmunes al ransomware sin confiar en el servidor».

**Como negocio:** el software es gratis para instalarlo uno mismo. Se cobra el **servicio en línea**, la **instalación y el soporte** en las empresas y algunos **extras empresariales**.

**Plan.**
1. Un **producto mínimo para instalar en el cliente**: servidor, agentes de Windows y Linux, almacenamiento en el contenedor de Proxmox, copias, retención, verificación, restaurar con el agente y avisos. Unas 16–18 semanas.
2. Cambiar de servidor y trabajar sin servidor.
3. El servicio en línea para varios clientes.
4. Mac y extras.

Lo que hoy funciona con la web de Supabase (equipos con la app, oficinas con el agente de la fase 5 y servidores rest-server) **sigue igual** hasta que el servidor esté listo, y después se mueve sin perder nada. La web de Supabase se **retira** (ver 8).

---

## 1. Arquitectura

### 1.1 Diagrama

```
            Navegador (PC o móvil)                    App de escritorio (opcional)
       ┌──────────────────────────────┐            ┌──────────────────────────────┐
       │ Consola (la misma UI Svelte) │            │ GUI local, modo sin servidor │
       │ escribe y sella contraseñas  │            │ y verificador independiente  │
       └──────────────┬───────────────┘            └──────────────┬───────────────┘
                      │ HTTPS                                     │ local
                      ▼                                           │
   ┌──────────────────────────────────────────────────────┐       │
   │ Resguardo Server (un binario Rust)  · SIN AUTORIDAD  │       │
   │ · API HTTPS + WebSocket para agentes                 │       │
   │ · Consola web embebida                               │       │
   │ · Cuentas locales + TOTP, roles, auditoría           │       │
   │ · SQLite (WAL) por defecto · PostgreSQL opcional     │       │
   │ · TLS propio (CA autofirmada) · ACME opcional        │       │
   │ · Avisos: en la consola, web push, SMTP opcional     │       │
   │ · Multicliente: un archivo de datos por cliente      │       │
   └───────────┬─────────────────────────┬────────────────┘       │
               │ WSS saliente            │ WSS saliente           │
               ▼                         ▼                        ▼
     ┌───────────────────┐   ┌──────────────────────────┐   ┌────────────────┐
     │ Agente Windows    │   │ Agente Linux             │   │ Agente macOS   │
     │ servicio, VSS,    │   │ systemd · CLI            │   │ launchd · CLI  │
     │ CLI, app opcional │   │ «Guarda copias»:         │   └───────┬────────┘
     └─────────┬─────────┘   │ rest-server append-only, │           │
               │ LAN (TLS)   │ CA propia                │           │
               └────────────▶│                          │◀──────────┘
                             └────────────┬─────────────┘
                                          │ copia externa
                                          ▼
                          Nube S3 / Backblaze B2 con Object Lock
```

### 1.2 Piezas

| Pieza | Qué hace | Qué **no** puede hacer |
|---|---|---|
| **Resguardo Server** | Consola web, API, canal en vivo con los agentes, cuentas, roles, auditoría, avisos, historial y metadatos. Guarda las configuraciones **cifradas**. | Dar órdenes estructurales o de datos sin las contraseñas. Leer copias, contraseñas ni rutas. |
| **Agente** (Rust, por sistema) | Guarda y aplica la configuración, comprueba contraseñas y verificadores, hace copias, verificación, pruebas de restauración y retención si guarda copias. Responde a sesiones e informa. Tiene CLI. | Aceptar órdenes protegidas sin su contraseña. Borrar copias de un servidor de solo añadir. |
| **Servidor de copias** (rest-server oficial, dentro del agente) | En los equipos «Guarda copias»: un usuario por equipo, `--append-only --private-repos`, TLS con CA propia. | Leer las copias: van cifradas con la contraseña de cada repositorio. |
| **App de escritorio** | GUI local (Windows; Linux con escritorio más adelante), administración sin servidor y verificador independiente. | Ser obligatoria. |
| **Publicación** | CI que compila, genera SBOM, firma con minisign y atestaciones, y publica. | Nada en ejecución: la llave de publicación está fuera de línea. |

**El servidor puede ser también almacenamiento:** basta instalar el agente en la misma máquina y marcarla «Guarda copias». Son dos procesos separados: si el servidor cae, el almacenamiento sigue.

---

## 2. Resguardo Server

### 2.1 Instalación

| Forma | Para quién | Cómo |
|---|---|---|
| **Un comando** (Debian 12/13, Ubuntu 22.04/24.04, CT de Proxmox) | Lo recomendado | `curl -fsSL https://…/instalar-servidor.sh \| sudo sh`. El script comprueba la **firma minisign** y luego instala el binario en `/usr/local/bin`, el usuario de sistema `resguardo`, `/var/lib/resguardo-server` y la unidad systemd. Al terminar muestra la URL y un **código de primer arranque** para crear la primera cuenta. |
| **Paquete** `.deb` | Quien prefiera apt | Repositorio apt firmado (más adelante). |
| **Docker** | Quien ya usa contenedores | Imagen multiarquitectura (amd64 y arm64), un volumen `/data` y el puerto 443. `docker run` o compose de ejemplo. |
| **Binario suelto** | Cualquier Linux | Binario estático (musl) y su unidad systemd de ejemplo. |
| **Windows Server** | Más adelante | Servicio de Windows. Solo si hay demanda. |

**Requisitos mínimos:** 1 vCPU, 512 MB de RAM y 1 GB de disco para cientos de equipos. El servidor no guarda copias, solo metadatos.

### 2.2 Qué lleva dentro

- **HTTP y WebSocket:** `axum` sobre `tokio`, con `rustls`. Sin dependencias de servicios de Internet para lo esencial.
- **TLS.**
  - **Por defecto, una CA autofirmada propia**: se reutiliza la lógica de `server.rs` (CA, certificado del servidor y renovación al cambiar la IP).
  - La consola explica cómo instalar la CA en los navegadores de la oficina, o se sigue con el aviso del navegador.
  - **ACME opcional** (Let's Encrypt) si hay un nombre público: HTTP-01 o DNS-01.
  - **Detrás de un proxy** (Caddy, nginx o Cloudflare Tunnel) también funciona.
- **Identidad del servidor:** un par **Ed25519 propio**, distinto del certificado TLS. Los agentes **fijan esta identidad** al vincularse; el servidor la demuestra en cada conexión firmando un reto. Así un cambio de certificado (ACME o renovación) no rompe nada, y un servidor falso no se hace pasar por el bueno.
- **Consola web embebida.** La interfaz Svelte de la app de escritorio, adaptada:
  - Las funciones de `api.ts` dejan de llamar a `invoke` de Tauri y pasan por un **transporte**: `tauri` en la app y `http` en la consola.
  - Se comparten el sistema de diseño ([diseno.md](diseno.md)) y los componentes.
  - El paquete compilado va dentro del binario (`rust-embed`).
- **Base de datos.** Ver 2.3.
- **Cuentas y roles.**
  - Cuentas locales con contraseña (Argon2id) y **TOTP obligatorio** (RFC 6238), con códigos de recuperación.
  - Sesiones con cookie `HttpOnly` y `SameSite=Strict`, protección CSRF y límite de intentos por cuenta y por IP.
  - Roles: propietario, administrador, técnico y solo lectura. Auditoría encadenada por hash.
  - **OIDC opcional más adelante** (Entra ID, Google Workspace, Keycloak).
- **Avisos:** en la consola (siempre), **web push** (VAPID generado por el propio servidor; necesita Internet en el navegador) y **SMTP opcional** (el del cliente, Gmail o cualquiera). Sin Internet, avisos en la consola y en la app de escritorio.
- **Relé de descargas:** como el disco es del propio servidor, **restaurar descargando al navegador** está disponible desde el principio, con un límite configurable (500 MB por defecto) y borrado al terminar.

### 2.3 Base de datos

**Por defecto, SQLite en modo WAL**: un archivo, copia de seguridad trivial (`VACUUM INTO` o la propia copia de Resguardo) y suficiente para cientos de equipos. Las escrituras se serializan, pero el volumen es bajo (informes cada pocos minutos).

**Abstracción.**
- Una capa de **almacenes por dominio**: `Cuentas`, `Clientes`, `Equipos`, `Órdenes`, `Informes`, `Auditoría`, `Sesiones`… Cada uno es un *trait* de Rust con sus operaciones (no un ORM genérico).
- Dos implementaciones con `sqlx`: **SQLite** y **PostgreSQL**. El SQL se mantiene en el subconjunto común.
- Migraciones por motor (`migrations/sqlite` y `migrations/postgres`). Las pruebas de CI pasan contra los dos.
- **Contexto de cliente obligatorio:** toda operación sobre datos de un cliente recibe un `ClienteCtx`. No hay forma de consultar sin él, y es la primera defensa del aislamiento.

**Aislamiento multicliente.**

| Motor | Cómo |
|---|---|
| **SQLite** (recomendado también en línea al principio) | **Un archivo por cliente** (`clientes/<id>.db`) y un `control.db` con las cuentas, las pertenencias y la lista de clientes. Cada archivo de cliente se abre aparte: un error de consulta no puede cruzar clientes. Exportar un cliente es casi exportar su archivo (ver 3.4). |
| **PostgreSQL** (instalaciones grandes) | Un **esquema por cliente** y un esquema `control`, con roles de base de datos por conexión y RLS como defensa adicional. |

### 2.4 El canal con los agentes

- Cada agente abre un **WebSocket saliente** (WSS) al servidor y lo mantiene, con latido cada 30 s y reconexión con espera creciente. Por él llegan las órdenes al instante y salen los informes y las respuestas.
- Si un proxy corta WebSocket, el agente usa **sondeo HTTP** (cada 60 s, o cada 2 s con una sesión abierta).
- **Sesiones interactivas** (elegir carpetas, explorar versiones, progreso): mismo cifrado de sesión que en el diseño anterior (X25519 efímera, HKDF y XChaCha20-Poly1305), pero **por el WebSocket**: respuestas en ~0,2 s.
- El agente se autentica con su **secreto de dispositivo** y comprueba la **identidad fijada** del servidor (2.2).

---

## 3. Confianza: la clave de administración manda, no el servidor

### 3.1 Modelo de seguridad (se mantiene de [plataforma-web.md](plataforma-web.md) §2)

**Tres niveles.**

| Nivel | Cómo se demuestra | Para qué |
|---|---|---|
| **1. Sesión de la consola** | Cuenta + TOTP | Ver estado y avisos; peticiones **inofensivas** |
| **2. Contraseña del repositorio** | Se escribe y va sellada al equipo, que la compara con la real | Todo lo que **toca o revela datos** de un repositorio |
| **3. Clave de administración del cliente** | Se escribe y va sellada como **prueba por equipo** (`Argon2id(clave, sal_del_equipo)`); el equipo guarda solo `SHA-256(prueba)` | **Cambios de estructura** |

**Inofensivo (solo sesión).** Copiar ahora, verificar ahora, probar la restauración, subir ahora, desbloquear bloqueos antiguos, reanudar, actualizar el agente a una versión firmada y cancelar una orden con espera. Todo con límites de frecuencia. **Pausar no es inofensivo**: pide la clave de administración.

**Contraseña del repositorio:**
- explorar o buscar archivos y versiones;
- restaurar en el mismo equipo o en otro, y descargar;
- cambiar la retención, `forget` y `prune`;
- quitar el repositorio o dejar de copiar en él;
- cambiar su copia externa;
- rotar su contraseña.

**Clave de administración:**
- destinos;
- crear copias y repositorios;
- carpetas, exclusiones, horarios y ganchos;
- pausar y desactivar;
- dar de alta o de baja equipos;
- «Guarda copias»;
- la espera de lo destructivo;
- la bandeja y el modo discreto;
- **cambiar de servidor**, desvincular y exportar el cliente;
- cambiar la propia clave.

**Integridad.**
- El secreto viaja **dentro del sobre sellado** (X25519) junto con la orden, y es la autorización.
- Repeticiones: `seq` creciente, `nonce` y caducidad (1 h para lo interactivo, 24 h para lo encolado y 7 días para el cambio de clave o de servidor).
- El agente **firma sus respuestas** con Ed25519.
- Antes de sellar, el navegador comprueba la **etiqueta HMAC** de la clave pública del equipo, derivada del secreto recién escrito. Así un servidor manipulado no puede desviar contraseñas.

**Defensas en el equipo.**
- Bloqueo por intentos fallidos, con aviso.
- **Espera aplicada por el equipo** en lo destructivo: 24 h por defecto y nunca menos de 1 h. Cualquier usuario con sesión puede cancelarla.
- Las configuraciones van cifradas con `K_cfg = HKDF(Argon2id(clave, sal_cliente))`: el servidor ve nombres de copias y horarios, no rutas.

**Límite honesto.** Si alguien controla **el código que sirve el servidor**, puede capturar una contraseña cuando se escribe. Mitigaciones:
- solo añadir en el servidor de copias y Object Lock en la nube;
- esperas, avisos y bloqueos;
- **compilaciones reproducibles** con hash publicado;
- la **app de escritorio como verificador independiente**.

Aquí hay una ventaja frente a la versión con Supabase: en una instalación en el cliente, el código lo sirve **un binario firmado que el propio cliente instaló**, no un tercero.

### 3.2 Modelo de amenazas (adaptado)

| Atacante | Consigue | No consigue |
|---|---|---|
| **Servidor comprometido** (o su base de datos robada) | Metadatos. Peticiones inofensivas. Retener mensajes (denegación de servicio). Provocar bloqueos. **Servir código malicioso a la consola** (3.1). | Órdenes estructurales o de datos. Desviar contraseñas (etiquetas). Repetir órdenes. Leer copias o rutas. |
| **Operador del servicio en línea** (tú, o un tercero que lo aloje) | Lo mismo que un servidor comprometido. | Lo mismo. Por eso el cliente puede **irse con sus equipos a otro servidor** sin pedir permiso (3.3). |
| **Cuenta de la consola robada** | Lo de su rol, a nivel 1. | Nada sin las contraseñas. |
| **Equipo malicioso** | Mentir en sus informes. Ver las contraseñas de **sus** repositorios y su propia prueba de administración. Leer rutas del cliente (`K_cfg`). | Actuar sobre otros equipos (sal por equipo, `--private-repos`). |
| **Administrador local malicioso** | Parar o desinstalar el agente (se avisa). | Borrar copias ya hechas (solo añadir e inmutable). |
| **Otro cliente del mismo servidor en línea** | Nada. | Ver o tocar datos ajenos: archivo o esquema por cliente, `ClienteCtx` obligatorio, pruebas de aislamiento en CI y cifrado de extremo a extremo de lo sensible. |
| **Alguien en la red** | Nada útil. | TLS, identidad del servidor fijada y sobres cifrados. |

### 3.3 «Cambiar de servidor»

Orden estructural (clave de administración), sellada para **cada equipo** del cliente. Lleva:
- la **URL nueva**;
- la **identidad Ed25519** del servidor nuevo, que el agente fijará;
- una **ficha de alta de un solo uso** por equipo, creada antes en el servidor nuevo con «Recibir un cliente»;
- `caduca` (7 días).

Flujo en dos fases, para no quedarse a medias:
1. En el servidor nuevo: **«Recibir un cliente»** genera las fichas y su identidad. Se pegan en el antiguo o en la app.
2. En el antiguo, «Cambiar de servidor» manda la orden a cada equipo.
3. Cada agente se conecta al nuevo, comprueba su identidad, se da de alta con la ficha y **sube su configuración cifrada**, su estado y su registro local. Después confirma al antiguo y borra sus credenciales viejas.
4. Si el nuevo no responde en 24 h, el agente **sigue con el antiguo** y avisa.
5. La consola nueva muestra «12 de 14 equipos ya están aquí», con los que faltan y por qué.

**No es destructiva** (no reduce la protección, las copias siguen igual), así que no espera, pero **avisa a todos**. Redirigir equipos a un servidor ajeno solo daría a ese servidor metadatos y la posibilidad de no reenviar órdenes: no podría mandar nada, porque sigue haciendo falta la clave.

### 3.4 Qué vive en el servidor y cómo se exporta

| Dato | Dónde vive la verdad | ¿Se exporta? | Cómo |
|---|---|---|---|
| Configuración de cada equipo | **El equipo**; en el servidor, una copia cifrada con `K_cfg` | Sí | Ya va cifrada. Además, los equipos la vuelven a subir al cambiar de servidor. |
| Verificadores, `K_cfg`, llaves de los equipos | **Solo el equipo** (los públicos, también en el servidor) | Los públicos y las etiquetas | En claro: son públicos. |
| Contraseñas de repositorios y claves de la nube | **Solo el equipo** y los kits impresos | **No** | Nunca están en el servidor. |
| Historial de copias e informes (sin rutas) | Servidor | Sí | En el paquete cifrado. |
| Avisos | Servidor | Sí | En el paquete cifrado. |
| Auditoría | Servidor (y el registro propio de cada equipo) | Sí | En el paquete cifrado, **conservando la cadena de hashes**. |
| Destinos y repositorios (metadatos) | Servidor | Sí | En el paquete cifrado. |
| Cuentas, contraseñas de cuenta y TOTP | Servidor | **No** | Cada servidor tiene sus propias cuentas. Se invita de nuevo. |
| Órdenes pendientes | Servidor | **No** | Se cancelan al cambiar y se avisa. |

**Paquete de exportación** («Exportar cliente», con la clave de administración):
- Un archivo `.resguardo-cliente` con lo marcado como exportable, cifrado con `K_exp = HKDF(Argon2id(clave, sal_cliente), "export")` (XChaCha20-Poly1305 en flujo).
- **Se cifra y se descifra en el navegador**, así que ningún servidor ve el paquete en claro mientras viaja o se guarda.
- Al importar en el servidor nuevo, el navegador pide la clave, descifra y sube los datos, que son metadatos que ese servidor ya iba a tener.

### 3.5 Sin servidor, y un servidor que muere

**Desvincular** (se mantiene de [plataforma-web.md](plataforma-web.md) §5) tiene dos opciones:
- **«Dejar de copiar»**: es destructiva y espera.
- **«Seguir funcionando en local»**: el agente borra sus credenciales del servidor, conserva su configuración, verificador, `K_cfg` y llaves, y funciona solo.

**CLI** (`resguardo-agente`, como root o Administradores):

| Orden | Qué hace |
|---|---|
| `estado`, `copias` | Ver el estado y las copias |
| `copiar-ahora` | Lanzar una copia |
| `versiones`, `restaurar` | Ver versiones y restaurar |
| `retencion aplicar`, `verificar` | Retención y verificación |
| `registro` | Ver el registro |
| `kit` | Mostrar el kit de recuperación |
| `config exportar/importar` | Solo en modo local |
| `vincular CÓDIGO --servidor URL [--empezar-de-cero]` | Volver a un servidor (o empezar de cero con otro cliente) |
| `restablecer-clave` | Si se perdió la clave de administración |

Interfaz gráfica:
- **Windows:** la app de escritorio.
- **Linux con escritorio:** la app, más adelante.
- **Linux sin escritorio:** solo la CLI.

**Un servidor que muere.** Los equipos **siguen copiando**: tienen su configuración, y el almacenamiento y la nube no dependen del servidor. Para gestionarlos otra vez hay tres caminos:
1. **Servidores de respaldo** (recomendado). Con la clave de administración se puede dar a los equipos una lista de hasta 3 URL con sus identidades: el principal y uno o dos de respaldo, por ejemplo tu servicio en línea o un segundo servidor. Si el principal no responde en N días (configurable, 3 por defecto), el agente prueba los demás. En el de respaldo, «Recibir un cliente» con la clave de administración lo acepta.
2. **Restaurar el servidor** desde su copia (el archivo SQLite está en las copias de Resguardo) en otra máquina con la misma dirección. Los agentes reconocen la identidad, porque la llave del servidor va en esa copia.
3. **Volver a vincular uno a uno**: `resguardo-agente vincular` con un código del servidor nuevo, por la CLI, el instalador o la herramienta de despliegue. Con la clave de administración se demuestra que es el mismo cliente y **no se pierde ni se reconfigura nada** en los equipos. El equipo no cambia de dueño solo por el código: espera el alta con su misma clave (si no, `--empezar-de-cero`).

**Dos consolas a la vez** (lo más cómodo si ya tienes otra, p. ej. la del cliente y tu servicio en línea): el equipo está vinculado a las dos y cada una funciona sola; si una muere, la otra sigue sin hacer nada. Ver [consolas-multiples.md](consolas-multiples.md).

Guía paso a paso: «Si pierdes la consola» en [agente-gestionado.md](agente-gestionado.md).

---

## 4. Agentes

Se mantiene lo diseñado:
- **Protocolo v2:** secreto en el sobre, `seq`, `nonce` y caducidad, respuestas firmadas.
- **«Guarda copias»** con rest-server.
- **Ganchos cerrados:** SQL Server `COPY_ONLY`, Siigo y World Office (rutas por confirmar).
- **Sin servidor** (3.5) y **actualizaciones firmadas** (6).

Equivalencias por sistema (de [plataforma-web.md](plataforma-web.md) §8):

| Pieza | Windows (hecho en parte) | Linux (prioridad) | macOS |
|---|---|---|---|
| Servicio | Servicio `ResguardoAgente` (LocalSystem) | systemd (root, `CAP_DAC_READ_SEARCH`, endurecido) | LaunchDaemon |
| Secretos en reposo | DPAPI de máquina, carpeta privada | `0600` de root en `/var/lib/resguardo-agente/privado`; `systemd-creds` con TPM2 si hay | Llavero del sistema y `0600` |
| Instantáneas | VSS | LVM, btrfs o ZFS si se puede; en un CT de Proxmox, ninguna (ganchos de volcado) | Instantánea local APFS |
| Permisos y ACL | ACL NTFS | Propietario, modo, xattrs y ACL POSIX | Igual y acceso total al disco (TCC) |
| Servidor de copias | rest-server y firewall (hecho) | rest-server, systemd y nftables, firewalld o ufw | Opcional |
| Empaquetado | NSIS (hecho), `/S /CODE` | deb, rpm y tar.gz estático; una línea con minisign | `.pkg` por script (sin firmar por ahora) |
| Interfaz local | CLI y app | CLI (y app en escritorios, más adelante) | CLI |

**Cambios por el servidor propio:**
- WebSocket en vez de sondeo;
- identidad del servidor fijada;
- lista de servidores de respaldo;
- `vincular --servidor URL`.

---

## 5. La app de escritorio

- **GUI local** para Windows (y Linux con escritorio más adelante): estado, explorar versiones, restaurar rápido sin pasar por la red y el kit de recuperación.
- **Administración sin servidor:** con el agente en modo local, la app es su consola. Edita la configuración con los permisos de administrador local.
- **Verificador independiente:** muestra las órdenes que su equipo recibió y aplicó, según el registro firmado del propio agente. Puede comprobar el **hash del paquete de la consola** que sirve el servidor contra el publicado en la versión firmada. Es útil para detectar un servidor que sirve código manipulado.
- **Comparte la interfaz** con la consola del servidor: el mismo código Svelte con otro transporte. Hacer un cambio una vez sirve para las dos.

---

## 6. Actualizaciones y publicación

Se mantiene de [plataforma-web.md](plataforma-web.md) §10:
- **Manifiestos firmados con minisign** (Ed25519, llave fuera de línea), SHA-256 de cada binario, anillos (interno, temprano y general) y vuelta atrás automática si la versión nueva no informa sana en 10 min.
- **Sin Authenticode de pago por ahora.** Hay aviso de SmartScreen en instalaciones a mano, pero no en las instalaciones por script ni en las actualizaciones que descarga el propio agente. Authenticode más adelante (Azure Trusted Signing o un certificado OV).

Novedades:
- **El servidor también se actualiza** con el mismo mecanismo, y puede **servir de espejo** de actualizaciones para sus agentes. Así los equipos de una red sin Internet se actualizan desde su propio servidor.
- **Publicación en el repositorio público** del proyecto, con atestaciones de procedencia de GitHub (Sigstore, gratis) además de minisign.

---

## 7. Código abierto y comunidad

### 7.1 Licencia (decisión tuya)

| Opción | A favor | En contra |
|---|---|---|
| **AGPL-3.0 para servidor y consola, con licencia dual comercial** | Nadie puede ofrecer tu servidor como servicio cerrado sin publicar sus cambios. La licencia dual permite vender excepciones a empresas que no quieran la AGPL. Es la fórmula de varios proyectos de infraestructura. | La licencia dual exige que **seas dueño de todo el código**: los contribuyentes deben firmar un **CLA**, y eso frena contribuciones. Algunas empresas evitan la AGPL por política. |
| AGPL-3.0 sin licencia dual (con DCO) | Más fácil para contribuir. Igual de protector. | No puedes vender excepciones; el negocio va por servicio y soporte. |
| Apache-2.0 o MIT para todo | Máxima adopción. | Un tercero puede ofrecerlo cerrado como servicio y competir con tus propias mejoras. |
| **Agente con licencia permisiva** (Apache-2.0) y servidor AGPL | El agente es lo que se instala en todos los equipos: una licencia permisiva facilita que empresas e integradores (RMM) lo adopten. | El agente es la parte que más valor técnico tiene. Una permisiva permite forks cerrados. |

**Recomendación:**
- **Servidor y consola: AGPL-3.0 con CLA ligero** (para poder ofrecer licencia comercial) **desde el primer día**, porque cambiarlo después exige permiso de todos los contribuyentes.
- **Agente: AGPL-3.0 también al principio**, con la opción de relicenciarlo a Apache-2.0 si lo pide la adopción. Gracias al CLA puedes hacerlo tú solo.
- **App de escritorio:** hoy es GPL-3.0-or-later; pasarla a AGPL o mantenerla en GPL da igual en la práctica.
- **Componentes de terceros:** restic y rest-server son BSD y son compatibles.

### 7.2 Repositorios

**Recomendación: un monorepo público `resguardo`**, con espacios de trabajo de Cargo y de npm:

```
resguardo/
  crates/
    motor/        restic, planes, retención, horarios (sale de la app actual)
    protocolo/    sobres, verificadores, etiquetas, sesiones (+ vectores de prueba)
    agente/       binario del agente (Windows, Linux, macOS)
    servidor/     Resguardo Server
  ui/             componentes Svelte y sistema de diseño compartidos
  consola/        SPA del servidor (usa ui/)
  escritorio/     app Tauri (usa ui/ y motor/)
  docs/           diseño, modelo de amenazas, guías
  packaging/      NSIS, deb/rpm, Docker, scripts de instalación
```

- Ventajas: cambios del protocolo en un solo PR que toca agente, servidor y consola; una sola CI y una sola versión.
- El repositorio separado de versiones (`resguardo-releases`) deja de hacer falta: las publicaciones van en el propio repositorio público.
- La web actual de Supabase y su repositorio se archivan al retirarla (8).

### 7.3 Seguridad como proyecto abierto

- **`SECURITY.md`** con el proceso de divulgación:
  - informe privado por GitHub (*private vulnerability reporting*) o un correo dedicado;
  - acuse en 72 h, corrección y aviso público coordinados en 90 días como máximo;
  - créditos al investigador;
  - `security.txt` en el servidor.
- **Modelo de amenazas y diseño de seguridad públicos** en `docs/`: este documento y los anteriores, ordenados.
- **Compilaciones reproducibles:** `cargo build --locked`, `SOURCE_DATE_EPOCH`, compilación en contenedor fijado. Cualquiera puede comprobar que el binario publicado sale del código.
- **SBOM** (CycloneDX con `cargo-cyclonedx` para Rust y el equivalente para npm) en cada publicación.
- **Publicaciones firmadas:** minisign y atestaciones de procedencia; Authenticode más adelante.
- **CI:** pruebas, `clippy`, `cargo-deny` (licencias y avisos), `cargo-audit`, CodeQL o semgrep, y las pruebas de integración reales (rest-server, restic).
- **Fuzzing** (`cargo-fuzz`) de lo que procesa datos de otros:
  - apertura de sobres y mensajes del protocolo;
  - mensajes de sesión;
  - configuración;
  - JSON de restic;
  - paquetes de exportación.

  Más pruebas de propiedades y **vectores de prueba compartidos** entre Rust y JS.
- **Auditoría externa** como meta, tras el producto mínimo. Fuentes de financiación para proyectos abiertos de seguridad: **NLnet / NGI Zero** y **OSTIF**. Mientras tanto, revisiones entre pares por invitación.

### 7.3.1 Pendiente de seguridad

Recomendaciones de las revisiones que piden cambiar el protocolo o el diseño, y por eso no se hacen de pasada. Las pequeñas ya están (v1.12: `config` que deja sin copias activas es destructiva, correo acotado al entrar, IPv6 por /64, TOTP sin repetición, `es_local` sin usuario en la URL, espejo sin distinguir mayúsculas, resultados sin rutas, `servidor.pid` comprobado, `/PUERTO` validado y el Servidor de copias parado al actualizar o desinstalar).

| Pendiente | Riesgo hoy | Por qué no está ya |
|---|---|---|
| **Resultados firmados ligados al `nonce` y al tipo de la orden** (no solo a su id y `seq`). | Un servidor malicioso podría presentar el resultado firmado de una orden como si fuera de otra del mismo `seq` en otro contexto. Bajo: el `seq` es único por equipo y la consola comprueba la firma. | Cambia `texto_resultado` (vectores compartidos Rust/JS): hace falta una versión nueva del contrato y que consola y agentes acepten las dos durante la transición. |
| **Código de emparejamiento generado en la consola**: **hecho** (v1.4x, pendiente de numerar al unir; plan-mejoras 9a). Con un servidor que lo admite, la consola genera el código (`crypto.getRandomValues`, sin sesgo) y al servidor solo le llega su SHA-256 (el mismo que manda el equipo en `unirse`); el servidor guarda `sha256:<hash>` en vez del código y ya no puede calcular la `prueba_codigo`. El código se queda en el navegador (`localStorage`) hasta el alta, al anular o 8 días. Queda: | (1) El de 15 min se escribe a mano: 10 caracteres (≈ 49,5 bits). Un servidor malicioso tiene su SHA-256 desde que se pide y podría buscarlo por fuerza bruta (≈ 2⁴⁹ hashes: horas en varias GPU) para adelantarse con un `alta` propio antes que la consola; lo frenan el SAS (la persona lo compara), que el equipo solo acepta el primer `alta` (la consola vería «El equipo rechazó el alta») y la caducidad. Los preparados llevan 16 caracteres (≈ 79 bits): fuera de alcance. (2) Las consolas anteriores (sin `codigo_hash`) siguen con la forma de antes: el servidor genera y guarda el código. (3) En el navegador, el código está en `localStorage` hasta el alta: quien tenga ese perfil del navegador lo puede leer (un código de un solo uso, que ya solo sirve para el `alta` de ese equipo). | (1) Un código más largo para escribir a mano empeora el uso; un hash lento (Argon2) exigiría cambiar `code_hash` en los agentes. (2) Compatibilidad con consolas ya abiertas; se podrá quitar la forma de antes cuando no queden. (3) Guardarlo cifrado con la clave de administración obligaría a pedirla antes de dar el código. |
| **Instalador «listo» y línea de Linux** (v1.17): **hecho** en v1.4x con el código del navegador: el servidor da el instalador genérico (`GET …/instalador-agente`) y la consola le añade la cola (`lib/cola.ts`, los mismos bytes que en Rust) o arma la línea. El servidor ya no guarda el código en claro 24 h. | Lo del archivo que viaja sigue igual: quien tenga el instalador (correo, USB, carpeta compartida) puede unir **un** equipo al cliente antes que el bueno; lo frenan el SAS, el único uso, la caducidad de 24 h, «Anular» y el límite de códigos. La cola no lleva ninguna clave ni contraseña: solo servidor, huella de su autoridad TLS, cliente, nombre y código. Si el instalador está firmado (Authenticode), añadirle la cola invalida la firma, la añada el servidor o el navegador. | La firma: firmar en el navegador no es posible; habría que firmar la cola aparte (el agente la comprobaría) o leer el código de otro sitio. |
| **«Vincular este servidor»** (v1.19): `vincular-local.json` con el código en la carpeta de datos del servidor (30 min). | Quien pueda escribir en esa carpeta (administradores; en Linux, también el usuario del servidor) puede hacer que el agente de esa máquina se una, pero solo al servidor de la propia máquina (127.0.0.1, con la huella de su autoridad TLS): no gana nada que no tenga ya. El agente no lo lee si el archivo lo pueden cambiar otros usuarios, ni si ya estuvo vinculado alguna vez. El SAS y el alta con la clave siguen igual. | El servidor tiene que escribir el código en ese archivo: aquí lo sigue generando él (también con la consola de v1.4x). |
| **Firma en `/api/agente/recibir`** (cambiar de servidor). | La petición es anónima con límite por IP; el contenido va sellado, pero el servidor nuevo no puede comprobar que viene del agente que dice. | Necesita que el agente firme con su llave y que el servidor nuevo la conozca antes (va en la ficha). |
| **Reloj propio del agente para la espera** de las órdenes destructivas. | La espera se cuenta con el reloj del equipo: quien controle ese reloj podría acortarla. | Hace falta una referencia de tiempo firmada (del servidor o de varias fuentes) y decidir qué pasa sin red. |
| **Ancla externa de la auditoría**: **hecho** (v1.4x, pendiente de numerar al unir; plan-mejoras 9b; ver «Ancla de la auditoría» abajo). La cabeza de la cadena de cada cliente (n.º, hora y huella de la última entrada) sale en el resumen por correo y, firmada con la identidad del servidor, a los equipos del cliente; los equipos avisan a todas sus consolas si retrocede, y la consola comprueba la cadena entera contra un ancla pegada. | Quedan los límites de abajo: el equipo solo ve la cabeza (una cadena rehecha y alargada no la nota él), el correo lo manda el propio servidor y nadie comprueba las anclas si no se hace a mano. | Un registro público o un sello de tiempo de terceros (RFC 3161) darían un ancla que no depende de nadie del cliente; pide un servicio externo y decidir qué se publica. |

#### Ancla de la auditoría (plan-mejoras 9b)

**Qué protege.** La auditoría de cada cliente es una cadena de solo añadir: cada entrada lleva la huella de la anterior (`almacen/sqlite.rs`, `hash_entrada`) y «Verificar la cadena» detecta una entrada cambiada o quitada **en medio**. Pero quien controle el servidor (o su base de datos) puede rehacerla entera desde el principio con huellas nuevas que cuadran entre sí, y la verificación de siempre dice que está bien. El ancla saca la cabeza de la cadena fuera del servidor para que eso se note:

- **Resumen por correo** (diario y semanal, también Telegram, ntfy y webhook): cada cliente lleva su «ancla de la actividad», con la línea `resguardo-ancla:1:<cliente>:<n>:<creado>:<huella>`. Quien guarde esos correos puede, cuando quiera, pegarla en Actividad → **«Comprobar con un ancla»**: el navegador baja toda la actividad, la recalcula desde la primera entrada sin huecos (no se fía de lo que diga el servidor) y mira si esa entrada sigue con la misma huella. Si alguien cambió o quitó cualquier entrada anterior a esa fecha, no cuadra (SHA-256: para que cuadre habría que encontrar otra cadena con la misma huella). Actividad también enseña el **ancla de hoy** para copiarla a mano.
- **Los equipos del cliente**: la reciben firmada con la identidad del servidor (al abrir el canal, en cada sondeo y cada hora) y guardan las de cada consola. Si una consola manda la misma entrada con otra huella, o un número menor que el más alto que ya dio, el equipo lo anota en su historial (`auditoria_rehecha`), que llega a **todas** sus consolas: un servidor que rehace su cadena no puede callarlo en las demás (ni borrar el aviso del equipo en ellas). En la consola que lo recibe es un aviso crítico (correo, push).

**Qué no protege (límites).**

- **El equipo solo mira la cabeza.** Una cadena rehecha que, cuando el equipo vuelve a ver un ancla, ya es **más larga** que la última que vio no se nota en el equipo (habría que bajar las entradas de en medio, y con ellas los correos de las personas y lo que hicieron, a cada equipo). Sí se nota con «Comprobar con un ancla» y un correo anterior. Cuanto más a menudo llegan las anclas (cada hora), menor es la ventana para rehacerla y alargarla sin que se note.
- **El correo lo manda el propio servidor.** Un servidor malicioso puede dejar de mandar el resumen, o mandar anclas de la cadena falsa desde el principio: el ancla solo sirve **desde antes** del momento en que el servidor se volvió malicioso, y solo si alguien guarda los correos y los compara alguna vez (la consola no lo hace sola). Que deje de llegar el resumen ya es una señal.
- **La consola que comprueba** la sirve el mismo servidor: un servidor malicioso podría servir una consola que dijera «Cuadra» siempre. Para un caso grave, la comprobación puede repetirse fuera (la fórmula de la huella es pública: `SHA-256(prev_hash|n|creado|actor|accion|objetivo|datos)`, con la exportación CSV de la actividad).
- **Restaurar una copia de la consola** (`restaurar-respaldo`) hace volver atrás la cadena de verdad: los equipos que vieron un ancla posterior avisan («Este servidor rehízo su registro de actividad»). Es esperable y el aviso lo dice («Si nadie restauró una copia anterior…»); no hay forma de distinguirlo de un servidor que miente, a propósito.
- **Lo que sale del servidor**: solo el número de la entrada, su hora y su huella (no se puede sacar nada de la actividad a partir de la huella). El equipo no guarda la dirección de la consola en la bitácora, solo el nombre que él le dio y su identidad.
- **Sin ancla de terceros.** Un registro público o un sello de tiempo (RFC 3161) darían un ancla que no depende del correo del cliente; queda como mejora.

Revisión de lo añadido desde 0.7.2 (octubre de 2026). Ya corregido: la retención en el almacén no se fía de la hora de las versiones (un equipo comprometido podía subir versiones falsas para que el almacén borrase las buenas; ver compartir.md), `restaurar-respaldo` comprueba la identidad antes de tocar nada, deshace lo puesto si falla a medias y en Linux solo cambia el dueño de lo restaurado sin seguir enlaces (antes, un enlace dejado por el usuario del servicio le daba a ese usuario cualquier archivo del sistema), las instantáneas en claro de la copia de la consola ya no pasan por `respaldos/`, los avisos del historial tienen tope y nunca fecha futura, y al vincular con la huella de antemano la autoridad TLS tiene que ser un solo certificado (el canal confiaba en todos los del PEM). Después (v1.26, agente 0.7.10): el número de comprobación (SAS v3) incluye la huella de la autoridad TLS que fijó el equipo y la que da el servidor, así que alguien en medio al vincular sin huella de antemano ya no pasa la comparación (con un agente anterior la consola pide comprobar también la huella), y `GET …/historial` va por páginas (500 por defecto, 2000 como mucho; antes, hasta ≈ 80 MB a cualquier miembro).

Segunda ronda (0.7.11). Ya corregido:

- **Copia de la consola firmada** (formato v2): la firma la identidad Ed25519 del servidor y `restaurar-respaldo` la comprueba antes de pedir la clave, enseña la huella de la identidad y no sigue si no es la de `--confiar-en <huella>` (la línea del kit ya la trae), la del servidor que ya hay en la carpeta o la que se confirma en la terminal. Las copias v1 se siguen restaurando, con un aviso.
- **HTTP:** cookie `__Host-` con HTTPS (Secure, HttpOnly, SameSite=Strict, Path=/); sesión nueva (otra ficha) al completar la entrada con el TOTP y al cambiar la contraseña o el autenticador (antes, la ficha de la contraseña sola pasaba a ser la completa); `Sec-Fetch-Site` en la defensa CSRF; Cross-Origin-Resource-Policy, Permissions-Policy y X-Permitted-Cross-Domain-Policies en todas las respuestas; sin TLS (detrás de un proxy o en 127.0.0.1) no había límite de tiempo para las cabeceras (`axum::serve`): ahora un solo bucle con 20 s para las cabeceras, 30 s sin datos del cuerpo y 10 000 conexiones como mucho; tramas del WebSocket de los agentes de 1 MiB.
- **Procesos del agente:** restic, rclone, rest-server y sqlcmd con el entorno vaciado y una lista blanca (antes heredaban, entre otras, `SSL_CERT_FILE`/`GODEBUG`, que cambian la verificación TLS de restic, o `SQLCMDINI`, un guion que sqlcmd ejecuta como SYSTEM); `systemctl` y `stty` por su ruta.
- **Pruebas de propiedades** (proptest) de todo lo que llega de fuera (sobres, mensajes, órdenes, cifrados, paquetes, cola del instalador, copia de la consola, horarios, salida de restic, progreso e historial de los agentes, contenido de una copia al restaurarla, rutas del agente). Encontraron y quedan corregidos: sumas de tamaños de `restic ls` (los pone el repositorio) y la antigüedad de un bloqueo en los mensajes de restic que desbordaban (`panic` en depuración, valores sin sentido en la versión publicada), «mayor separación entre copias» con un «cada N días» que empieza al final del calendario, y `ruta_local` en Linux, que aceptaba rutas relativas o con `..` (en Windows ahora rechaza también nombres que Windows recorta: `...`, `a.`, `a `, y NUL).
- `cargo audit`: solo RUSTSEC-2024-0370 y RUSTSEC-2024-0429 (glib, unsound), los dos de la pila GTK de Tauri en Linux (la app de escritorio); el segundo, ignorado con su motivo en `deny.toml`. `npm audit --omit=dev` en la consola: nada.

Queda:

| Pendiente | Riesgo hoy | Por qué no está ya |
|---|---|---|
| **Huella corta de la identidad** (8 bytes) en el kit y en `--confiar-en`. | Encontrar otra identidad con los mismos 64 bits cuesta ≈ 2⁶⁴ operaciones: caro, pero menos que la firma. | El kit y la consola enseñan la corta; `--confiar-en` ya acepta la identidad entera (base64) o más cifras. Enseñar la entera en el kit es cambio de la consola. |
| **Conexiones por IP.** | El tope de 10 000 conexiones es para todo el servidor: una sola máquina que abra miles (sin pasar de las cabeceras, 20 s cada una) puede dejar fuera a las demás un rato. | Un tope por IP (y otro para las WebSocket de los agentes) pide medir cuántas tiene cada red detrás de un NAT; el proxy de delante también puede limitarlo. |
| **cargo-deny** no está en la máquina de desarrollo. | `deny.toml` (licencias, avisos, orígenes) solo se comprueba en CI. | Instalarlo o confiar en CI; `cargo audit` sí se ejecutó. |
| **`restaurar-respaldo` en Linux, con el servicio en marcha o su usuario comprometido.** | La carpeta de datos es del usuario del servicio: mientras root restaura, ese usuario podría cambiar `.restaurando-*` por un enlace y hacer que root escriba allí lo de la copia. | Exige restaurar en una carpeta solo de root y moverla después (otro sistema de archivos posible). Mientras: parar el servicio, como ya dice la guía. |
| **Retención en el almacén: revocar desde el dueño.** | El repositorio es de solo añadir, así que el equipo dueño no puede borrar la clave del almacén: solo la quita el almacén («Dejar de aplicarla»). Un almacén de otro cliente puede seguir leyendo hasta que su administrador la quite. Además, `usuario/carpeta` los toma la consola de los resúmenes que da el servidor: uno malicioso podría hacer que la regla vaya a otro repositorio que ya tenga clave del almacén (lo frenan la espera, la orden pendiente visible con su `usuario/repo` y que la clave la añade el dueño). | Revocar desde el dueño pide cambiar la contraseña del repositorio (rotarla) o una ventana sin solo añadir; atar la regla al repositorio pide que el almacén sepa qué equipo es cada usuario. |
| **Retención en el almacén: margen de 48 h.** | Versiones falsas con fecha hasta 48 h antes de su subida (o 24 h después) sí cuentan: un equipo comprometido puede desplazar las buenas de esos días. Las versiones traídas con «Traer el historial» al almacén no las poda el almacén (se quedan). | Un margen menor da falsos positivos con copias largas o relojes desajustados; podar las traídas pide una marca que el equipo no pueda falsificar. |
| **`conectar_nube` sobre una nube que usa el espejo.** | Volver a conectar con el mismo nombre cambia de cuenta de Dropbox sin espera: el espejo dejaría de subir a la buena (lo ya subido sigue allí). Pide la clave de administración. | Hacerla destructiva según el cuerpo (como `quitar_nube`) exige que la consola sepa que la nube la usa el espejo y ponga `not_before`; y «Reconectar» por un permiso caducado tendría que esperar. |
| **Resumen de «Guarda copias».** | Lleva en claro la carpeta del almacén (`carpeta`) y los nombres de los repositorios de cada usuario: el servidor ve una ruta. | La consola la usa para decir dónde dejar un repositorio antiguo; podría ir en la configuración cifrada con `K_cfg`. |

Cerrados después (octubre de 2026, pendiente de numerar al unir):

- **`PATH` de restic en el agente:** solo carpetas del sistema (`System32`, la de Windows y `System32\OpenSSH`; en Linux `/usr/sbin:/usr/bin:/sbin:/bin`), y con los repositorios `rclone:` el rclone que va junto a Resguardo (`-o rclone.program=…`). El proceso del servicio (`--service`, `--primer-plano`) usa ya solo el restic incluido, como `--agent-run` (antes, el que lanzaba dentro de sí, para explorar o restaurar, podía salir del PATH). En las pruebas y la app de escritorio, el PATH de siempre.
- **Nombres de dispositivo de Windows al restaurar** (`CON`, `PRN`, `AUX`, `NUL`, `COM0`–`COM9`, `LPT0`–`LPT9`, con ¹²³, `CONIN$`, `CONOUT$`, también con extensión): `ruta_local` y «En otra carpeta» los rechazan. Una versión con un archivo así no se puede restaurar por su ruta (Windows no deja crearlos con las herramientas normales, así que son muy raros).
- **`restaurar-respaldo` en Windows** protege la carpeta de datos (`proteger_carpeta`: solo SYSTEM y Administradores) antes de poner nada; sin permisos de administrador, no sigue.
- **Línea de Linux en la consola:** el código, la dirección y la huella que da el servidor se comprueban (`lib/emparejar.ts`, `lineaVincular`) antes de enseñarla; si algo no tiene su forma o lleva caracteres que la shell interpreta, no se enseña y se avisa.

Revisión de la ventana del agente (`ipc_local`, modo sin consola) y de varias consolas a la vez (v1.36, octubre de 2026). Ya corregido:

- **«Usar sin consola» era de quien llegase primero.** En un equipo recién instalado, cualquier usuario con sesión podía poner la clave de administración y, con ella, decidir qué copia el servicio como SYSTEM (también carpetas de otros usuarios o del sistema), tener las contraseñas de los repositorios y restaurar donde quisiera. Ahora `crear_clave` solo lo acepta de SYSTEM/root o de una cuenta del grupo Administradores (el servicio lo mira en el token del cliente de la tubería, también con UAC); en Linux, root.
- **Bloqueo por intentos común a todo el equipo.** Un usuario sin privilegios podía mandar pruebas malas y dejar al administrador sin ventana hasta una hora cada vez. Ahora el límite es de cada cuenta (SID o uid, que da el sistema), y los retos también: solo los gasta la cuenta que los pidió, y pedir muchos no echa los de las demás.
- **La ventana podía pisar órdenes de las consolas.** `ipc_local` leía el vínculo, hacía lo suyo (a veces con red: subir la configuración) y lo guardaba entero: una orden que llegase entretanto (su `seq` y su `nonce`, una clave nueva, una consola quitada) se perdía, y con ella la protección contra repetirla. Ahora lo que cambia el vínculo va bajo el cerrojo de las consolas, leyendo dentro.
- **Restaurar como SYSTEM a donde no debe.** «En otra carpeta» solo miraba el texto: `C:\Temp\..\Windows`, `\\?\C:\Windows`, `C:/Windows`, `C:\PROGRA~1`, flujos (`a:b`) o una unión hacia la carpeta de Windows pasaban. Y la carpeta «Restaurado AAAA-MM-DD HHMM» (también la de «junto al original» desde la consola) la creaba restic siguiendo lo que hubiera: un usuario que pudiera escribir junto al original podía dejar antes una unión con ese nombre (la hora se adivina) hacia `C:\Windows\System32` y hacer que SYSTEM escribiera allí lo de la copia (un archivo suyo que se copió). Ahora la carpeta de destino se valida también por su ruta real y sin enlaces en el camino, la carpeta «Restaurado …» la crea el agente y siempre es nueva, y desde la ventana nace solo para SYSTEM, Administradores y quien la pide. «En su sitio» no sigue una carpeta que ahora es un enlace.
- **Conexiones lentas.** La espera de 10 s era por lectura: un cliente que mandase un byte cada 9 s retenía uno de los 4 hilos sin fin. Ahora es un plazo total. En Linux no había tope de conexiones (un hilo por cada una): ahora 4, y la carpeta del socket tiene que ser de root.
- `anadir_consola` con `http://`: el pedido aceptaba cualquier dirección que empezase por `http://localhost` (`http://localhost.otro.com`); la conexión ya lo frenaba, ahora también el pedido (`es_local`).

Revisado sin cambios: la DACL de la tubería (los usuarios no pueden crear instancias; `FILE_FLAG_FIRST_PIPE_INSTANCE`; `PIPE_REJECT_REMOTE_CLIENTS`), la comprobación del dueño en el cliente y `SECURITY_IDENTIFICATION`, tamaños de mensaje, la prueba en tiempo constante y su reto de un uso, la ventana (protocolo propio con CSP estricta, sin navegar fuera ni abrir ventanas, sin herramientas de desarrollo en la versión publicada, no se abre elevada, solo lee dos archivos sin rutas ni secretos de una carpeta en la que los usuarios no escriben, la identidad de los avisos en `HKCU`), el token de `rclone authorize` (por la tubería con la prueba; el servicio lo guarda con DPAPI en la carpeta privada y se lo pasa a rclone por el entorno), las consolas (`seq` y bloqueos de cada vínculo, `nonce` común, sobre atado a cliente y equipo, `anadir_consola` y `quitar_consola` con la clave y solo administradores, la configuración solo cambia con órdenes con la clave) y las cifras de E/S de restic (`PROCESS_QUERY_LIMITED_INFORMATION` sobre su propio proceso hijo; solo números).

Queda (bajo o de diseño):

| Pendiente | Riesgo hoy | Por qué no está ya |
|---|---|---|
| **Ocupar el nombre de la tubería antes que el servicio.** | Un usuario que crea `\\.\pipe\ResguardoAgente` antes de que arranque el servicio deja sin ventana al equipo (el servicio reintenta cada minuto). El cliente no le manda nada: comprueba antes que la tubería es de SYSTEM o de los administradores. | Solo denegación de servicio. Un nombre aleatorio publicado en un sitio con ACL, o arrancar el servicio antes de cualquier sesión. |
| **Los 4 huecos de la tubería son de todos.** | Un usuario que reconecta sin parar puede tenerlos ocupados (10 s cada vez) y la ventana del administrador tiene que reintentar. Una restauración larga ocupa uno mientras dura. | Solo denegación de servicio; un tope por cuenta (ya se sabe quién es) lo resolvería. |
| **La prueba es un secreto fijo, no una respuesta al reto.** | Quien la consiga (la memoria de la ventana desbloqueada, un volcado del proceso) la puede usar con retos nuevos hasta que cambie la clave, solo en este equipo (la sal es de cada equipo). | El equipo guarda `SHA-256(prueba)`: comprobar un HMAC exigiría guardar la prueba. Un PAKE (OPAQUE, SPAKE2+) lo resolvería con otro contrato. |
| **Restaurar «junto al original» (desde la consola) y «en su sitio»: carrera con enlaces.** | La carpeta «Restaurado …» es nueva pero hereda los permisos de la de arriba: un usuario que puede escribir allí podría, **mientras restic escribe**, cambiar una subcarpeta por una unión. «En su sitio» escribe en carpetas que pueden ser de usuarios. Ya no vale dejarla preparada antes (se comprueba y la carpeta es nueva); desde la ventana, la carpeta nace sin permisos para otros. | Restaurar en una carpeta privada del mismo disco y moverla después, o que restic no siga enlaces que no creó; para la consola habría que saber de quién es la carpeta. |
| **La clave del modo local vale lo que un administrador del equipo.** | Con ella se elige qué copia SYSTEM (cualquier carpeta) y se restaura en cualquier carpeta que no sea del sistema (siempre en una «Restaurado …» nueva). Si un administrador la comparte con un usuario, ese usuario puede leer archivos de otros a través de las copias. | Es el modelo (la clave manda); la guía lo dice. Limitar las carpetas a las de quien pide cambiaría el producto. |
| **`quitar_consola` sin espera.** | Una consola en la que se escribió la clave puede quitar a las demás al momento y quedarse sola con el equipo (todas reciben el aviso y queda en el historial). | Es como `desvincular`, que tampoco espera. Hacer que espere cuando quita **otra** consola es un cambio del contrato (`not_before` según el cuerpo). |
| **Límites de la ventana en memoria.** | Al reiniciar el servicio se olvidan los fallos (un usuario sin privilegios no puede reiniciarlo). | Guardarlos en disco por cuenta; poco a ganar. |
| **Una sola ventana por sesión.** | El mutex `Local\ResguardoAgenteVentana` y buscar la ventana por su título: otro proceso **del mismo usuario** puede impedir que se abra o recibir el «traer al frente». | Mismo usuario: no cruza ningún límite. |
| **Linux: la primera clave, solo root.** | Los del grupo `sudo`/`wheel` tienen que usar root. | En Linux aún no hay ventana. |

### 7.4 Comunidad y tu perfil profesional

- **Documentación para contribuir:** `CONTRIBUTING.md`, guía de desarrollo, etiquetas «buen primer *issue*» y código de conducta.
- **Para mostrar el trabajo:**
  - una serie de artículos: «Copias inmunes al ransomware sin confiar en el servidor», «Por qué el servidor de Resguardo no puede borrar tus copias», «Diseñar un protocolo de órdenes con secretos que el servidor nunca ve», «Fuzzing de un protocolo en Rust»;
  - charlas en comunidades locales e hispanas (capítulos OWASP, BSides, DragonJAR, encuentros de Rust);
  - la auditoría externa y su informe publicado;
  - divulgaciones bien gestionadas.
- **Métricas visibles:** publicaciones firmadas, cobertura de fuzzing y un historial de avisos de seguridad atendidos.

---

## 8. Migración desde la web de Supabase

**Mientras se construye el servidor**, todo sigue como hoy:
- los equipos con la app de escritorio: app con agente, vinculados a la web;
- las oficinas con el agente de la fase 5: agente con su consola;
- los servidores de copias: rest-server.

**Cuando el producto mínimo esté listo:**

| Sitio | Cómo se mueve |
|---|---|
| **Un equipo con la app de escritorio** | Se instala el agente nuevo (o se actualiza la app). «Adoptar este equipo» con código y SAS desde el servidor del cliente «Propio». La app entrega su configuración actual al agente. Mismas copias y repositorios. |
| **Un servidor con un programa contable** (copia a Backblaze) | Igual. Las claves siguen en el equipo. Se revisan los ganchos del programa contable y de SQL Server y se recomienda Object Lock. |
| **Una oficina con el agente de la fase 5** | Su consola de la fase 5 envía una orden de **traspaso** firmada con su llave fijada, que lleva la URL, la identidad del servidor nuevo, la ficha de alta, el verificador y `K_cfg`. Es la misma idea que «Cambiar de servidor». |
| **Un servidor de copias rest-server** | Agente de Linux en el CT del rest-server y «Adoptar servidor de copias existente». Los equipos que copian en él no cambian de ubicación. |
| **Historial** | Opcional: una herramienta `importar-supabase` lleva los informes y la actividad de la web actual al servidor, con el mismo formato que un paquete de exportación. |

**¿Se retira la web de Supabase?** **Recomendación: sí.**
- Mantenerla como un «panel en línea» más obligaría a reimplementar el protocolo en otra pila (RPC de Postgres, RLS y Edge Functions) y a mantener dos *backends*.
- La oferta en línea será **Resguardo Server alojado por ti**.
- La web actual queda en solo lectura unas semanas tras la migración y después se archiva su repositorio y se borra el proyecto de Supabase (decisión 11).

---

## 9. Modelo de negocio

**Núcleo abierto.**
- **Gratis (AGPL):** el servidor completo, agentes, consola y app. Instalar uno mismo, sin límites de equipos. **Ninguna función de seguridad es de pago**: la credibilidad del proyecto depende de ello.
- **De pago:**
  1. **Resguardo en línea:** tu servidor multicliente. Precio por equipo y mes, y almacenamiento en la nube opcional (B2 con Object Lock revendido o gestionado).
  2. **Instalación y soporte en el cliente:** puesta en marcha (servidor en Proxmox, agentes, almacenamiento y nube), contratos de soporte con tiempos de respuesta, revisiones trimestrales y pruebas de restauración guiadas.
  3. **Extras empresariales** (licencia comercial o servicio):
     - SSO/OIDC avanzado;
     - roles avanzados con alcance por sede;
     - informes PDF para auditorías y cumplimiento;
     - integraciones (RMM/PSA, webhooks, SIEM);
     - retención larga de auditoría;
     - panel multiempresa para integradores.
  4. **Licencias comerciales** para quien no quiera la AGPL.

**Para que el servicio en línea sea seguro y multicliente hace falta:**
- **Aislamiento:** un archivo o esquema por cliente, `ClienteCtx` obligatorio y pruebas de aislamiento en CI que intentan cruzar clientes.
- **Cuotas y límites** por cliente: equipos, órdenes, relé, tamaño de la base y peticiones.
- **Operación:** copias del servidor (el propio Resguardo con Object Lock), monitorización, actualizaciones por anillos, registros sin datos sensibles y plan de incidentes.
- **Exposición:** ACME, un proxy o CDN delante (Cloudflare) para limitar ataques de denegación de servicio, y la consola protegida por TOTP. Nada administrativo expuesto.
- **Legal:**
  - términos y política de privacidad;
  - en Colombia, la **Ley 1581 de 2012** (habeas data) y su registro de bases de datos si aplica;
  - un contrato de encargo de tratamiento con los clientes.

  El diseño ayuda: el servicio no ve ni archivos ni contraseñas.
- **Facturación sencilla** al principio (facturas manuales y pagos recurrentes); automatizarla después.

---

## 10. Plan por fases

Los esfuerzos son orientativos, en semanas de un desarrollador con asistencia de agentes.

| Fase | Contenido | Esfuerzo |
|---|---|---|
| **F0. Base del monorepo** | Workspace; sacar `motor` y `protocolo` de la app (planes, retención, restic, sobres v2, verificadores, etiquetas, vectores de prueba); transporte en `api.ts` (tauri o http) | 2 sem. |
| **F1. Resguardo Server mínimo** | `axum` y `rustls`, CA propia, identidad Ed25519, SQLite con almacenes y `ClienteCtx`, cuentas con TOTP, roles, auditoría, consola embebida (estado, equipos, avisos), canal WebSocket, emparejamiento con SAS, clave de administración (alta, verificadores y `K_cfg`), instalación de un comando y Docker | 5 sem. |
| **F2. Agente Windows v2** | Del `resguardo-agente` actual: protocolo v2, configuración cifrada, órdenes inofensivas y protegidas, esperas, bloqueos, informes, «Guarda copias» | 3 sem. |
| **F3. Agente Linux y almacenamiento en el CT** | systemd, secretos, rest-server y nftables, deb, rpm y tar.gz, CLI. **Piloto: CT de Proxmox como almacenamiento** | 3 sem. |
| **F4. Consola útil** | Crear repositorios y destinos (kit), elegir carpetas en vivo con sugerencias y ganchos, retención y verificación, restaurar con el agente (mismo equipo y otro), descarga al navegador, avisos (consola, push y SMTP) | 4 sem. |
| **Producto mínimo en el cliente** | F0–F4 | **≈ 17 semanas** |
| **F5. Publicación y seguridad del proyecto** | CI completa, SBOM, minisign y atestaciones, actualizaciones automáticas con anillos y vuelta atrás, compilaciones reproducibles, `SECURITY.md`, fuzzing inicial | 2–3 sem. |
| **F6. Moverse libremente** | «Cambiar de servidor», «Recibir un cliente», exportar e importar, servidores de respaldo, desvincular en local, CLI completa, `restablecer-clave` | 3 sem. |
| **F7. Migración y retirada de Supabase** | Adoptar los equipos con la app, las oficinas con el agente de la fase 5 (traspaso) y los servidores de copias; `importar-supabase`; web actual en solo lectura y luego retirada | 1–2 sem. |
| **F8. Lanzamiento abierto** | Repositorio público, licencias y CLA, documentación, guía de contribución, primer artículo | 1–2 sem. (en paralelo a F5–F7) |
| **F9. Servicio en línea** | Modo multicliente endurecido (pruebas de aislamiento, cuotas), ACME, despliegue y copias del propio servicio, monitorización, términos, facturación básica | 3–4 sem. |
| **F10. macOS** | LaunchDaemon, llavero, APFS, TCC, `.pkg` y CLI | 3 sem. |
| **F11. Extras** | PostgreSQL, OIDC, roles avanzados, informes PDF, integraciones, servidor en Windows, app para Linux, passkeys como comodidad | 4–8 sem., según demanda |
| **F12. Auditoría externa** | Preparación, auditoría y publicación del informe | Depende de la financiación |

**Orden:** F0 → F1 → F2 → F3 → F4 (producto mínimo) → F5 → F6 → F7 → F8 → F9 → F10 → F11 → F12.

**Primer valor real:** al terminar F3 ya se puede instalar el servidor en el CT de Proxmox y gestionar un Windows y el almacenamiento local; F4 lo completa.

---

## 11. Riesgos

| Riesgo | Mitigación |
|---|---|
| **Alcance grande para una persona** | Producto mínimo acotado (F0–F4). Reutilizar el motor, el agente, `server.rs` y la interfaz. Lo demás, por demanda. |
| **Adaptar la interfaz de la app a una consola web** | El transporte en `api.ts` se hace primero (F0), y la app y la consola comparten componentes. |
| **Errores criptográficos** | Pocas primitivas (sobres sellados, Argon2id, HKDF, HMAC y XChaCha20), bibliotecas auditadas, vectores de prueba compartidos, fuzzing y auditoría externa. |
| **Mantener un proyecto abierto** (issues, seguridad, soporte) | Plantillas, límites claros de soporte gratuito, `SECURITY.md` y priorizar a los clientes de pago. |
| **Ser objetivo de ataques con el servicio en línea** | El diseño limita el daño (el servidor no tiene autoridad). Endurecimiento, proxy o CDN, monitorización y plan de incidentes. |
| **Certificados en instalaciones en el cliente** (avisos del navegador con la CA propia) | Guía para instalar la CA, ACME con DNS-01 si hay dominio, o un proxy del cliente. |
| **SQLite bajo carga en el servicio en línea** | Un archivo por cliente reparte la carga. PostgreSQL para quien lo necesite. |
| **Dos sistemas a la vez durante la transición** | Congelar funciones nuevas en la web de Supabase y migrar en cuanto haya producto mínimo. |
| **Requisitos legales del servicio en línea** | Asesoría puntual antes de cobrar. El diseño minimiza los datos personales. |
| **SmartScreen y macOS sin firma de pago** | Instalación por script y documentación. Firmar cuando haya ingresos. |

---

## 12. Decisiones abiertas para ti

1. **Licencias:** ¿AGPL-3.0 con CLA para servidor y consola? ¿Y el agente: AGPL o Apache-2.0?
2. **¿Cuándo se hace público el repositorio?** Propuesta: al terminar F1, con el diseño de seguridad ya publicado. Lo anterior se publica limpio, sin secretos en el historial (revisar el historial antes).
3. **Nombre y dominio del proyecto.** Para el servicio en línea, ACME y el correo hará falta un dominio (unos 10–15 USD al año). ¿Cuál?
4. **Dónde alojar el servicio en línea:** un VPS (5–10 USD al mes) o tu Proxmox con Cloudflare Tunnel (gratis, pero depende de tu conexión y tu energía).
5. **Retirar la web de Supabase** tras la migración (recomendado) o mantenerla un tiempo como panel de solo lectura.
6. **Servidores de respaldo por defecto:** ¿los clientes en el cliente apuntan como respaldo a tu servicio en línea (cómodo, y un posible ingreso) o a nada?
7. **Precios** del servicio en línea y del soporte.
8. **Del diseño anterior, siguen abiertas:**
   - lista de lo inofensivo (y que pausar pida la clave);
   - espera de 24 h y mínimo de 1 h;
   - clave de administración generada o elegida, y una por persona;
   - permisos de los técnicos;
   - distribuciones de Linux;
   - Mac sin cuenta de Apple;
   - nombres en la interfaz.
9. **Instalación del servidor en Windows Server:** ¿hace falta pronto (clientes sin Linux) o puede esperar a F11?
