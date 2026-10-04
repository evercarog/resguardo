# Resguardo como plataforma: todo desde la web, con agentes

> **Sustituido por [plataforma.md](plataforma.md)** (2026-10-02): Resguardo pasa a ser un servidor propio instalable («Resguardo Server»), y la versión en línea es ese mismo servidor. Este documento se conserva como antecedente: su modelo de seguridad, protocolos, amenazas, modo sin servidor, agentes por sistema y actualizaciones se reutilizan allí.

Diseño de la arquitectura futura de Resguardo. Estado: **propuesta revisada con las decisiones del usuario** (2026-10-02). La parte web la revisará el agente de la web. Este documento no incluye código.

Documentos relacionados: [agente-gestionado.md](agente-gestionado.md) (fase 5: sobres, `seq` y emparejamiento con SAS), [compartir.md](compartir.md) (fases 3 y 4: sobres cifrados y Servidor de copias), [destinos.md](destinos.md) (destinos y repositorios) y [diseno.md](diseno.md) (sistema de diseño).

---

## Resumen

**Qué cambia.** Todo se gestiona desde **Resguardo Web**, desde cualquier navegador o el móvil. En cada equipo (Windows, Linux o Mac) hay un **agente** ligero: no tiene ventana, hace las copias y solo se conecta hacia fuera, así que no hace falta abrir puertos.

**Tres llaves, como en una oficina.**
- **Entrar en la web** (correo, contraseña y código del autenticador) sirve para ver el estado y los avisos y para pedir cosas inofensivas, como «Copiar ahora».
- **La contraseña del repositorio** hace falta para todo lo que toca o enseña los datos de un repositorio: ver versiones y archivos, restaurar, borrar versiones o dejar de copiarlo.
- **La clave de administración del cliente** hace falta para los cambios de estructura: añadir equipos o destinos, crear copias, cambiar qué carpetas se copian o desactivar copias.

Las dos contraseñas **las escribes tú en el momento** y viajan cifradas **solo para el equipo** que las comprueba. **La web no las guarda nunca.** Las contraseñas de los repositorios viven solo en los equipos y en los kits de recuperación impresos.

**La web no manda.** Si alguien entra en la web o en su base de datos, puede ver el estado y pedir copias, pero **no puede** cambiar qué se copia, ver archivos, restaurar ni borrar nada. Los equipos comprueban ellos mismos cada contraseña, bloquean tras varios intentos fallidos y avisan. Las órdenes que reducen la protección **esperan** (24 h por defecto) y se pueden cancelar.

**El límite, dicho claro:** si alguien llegara a cambiar el código de la propia web, podría capturar una contraseña justo cuando la escribes. Por eso las copias son de **solo añadir**, la nube es **inmutable**, lo peligroso espera y avisa, y la app de escritorio puede servir de comprobación independiente.

**Gratis por ahora.**
- La web funciona en el **plan gratuito de Supabase**: el agente consulta cada poco en lugar de usar conexiones en tiempo real, y las restauraciones las hace siempre un equipo, no el navegador.
- Los agentes **no llevan certificado de firma de pago**: Windows avisará la primera vez.
- Las **actualizaciones sí van firmadas** con nuestra propia llave, y se publican en un repositorio público de GitHub solo para binarios.

**Si un día quieres dejar la web.** «Desvincular cliente» permite **seguir funcionando en local**: cada equipo conserva su configuración, sus horarios, su servidor de copias y su retención, y se maneja con una línea de órdenes o con la app de escritorio. Más adelante se puede **volver a vincular** sin reconfigurar nada.

**Y más adelante, «Resguardo local»:** la misma consola web instalada en el servidor del cliente, sin depender de Internet. Desde ya se evita todo lo que lo impediría.

**Plan.** Por orden:
1. Las llaves y el protocolo.
2. El agente de Windows desde la web.
3. Crear repositorios y destinos en la nube.
4. Elegir carpetas.
5. Equipos que guardan copias.
6. El agente de Linux (primero el contenedor de Proxmox).
7. Restaurar con agentes.
8. Actualizaciones firmadas.
9. Desvincular y modo local.
10. Migración de lo existente.
11. Mac.
12. Lo que necesite plan de pago.
13. Resguardo local.

Lo que ya existe (equipos con la app de escritorio, servidores con un programa contable y copia a la nube, oficinas con el agente de la fase 5 y servidores de copias rest-server) se **adopta sin perder datos ni reconfigurar**.

---

## 1. Arquitectura

### 1.1 Diagrama

```
                    Navegador del administrador (PC o móvil)
          ┌────────────────────────────────────────────────────────────┐
          │ Resguardo Web (SPA estática)                               │
          │ · Sesión: correo + contraseña + TOTP (aal2)                │
          │ · Pide la contraseña del repositorio o la clave de         │
          │   administración solo en el momento y la sella para el     │
          │   equipo (X25519); nunca la guarda                         │
          └───────────────┬────────────────────────────────────────────┘
                          │ HTTPS (RPC de Postgres)
                          ▼
     ┌──────────────────────────────────────────────────────────────────┐
     │ Supabase (plan gratuito) + Vercel   · SIN AUTORIDAD              │
     │ · Postgres + RLS: clientes, equipos (claves públicas), órdenes   │
     │   selladas, respuestas cifradas, estado, avisos, auditoría       │
     │ · Sesiones interactivas por sondeo (tabla con caducidad)         │
     │ · (Más adelante, con Pro: Realtime y relé de descargas)          │
     └──────┬─────────────────────────┬─────────────────────────┬───────┘
            │ saliente (HTTPS)        │                         │
            ▼                         ▼                         ▼
  ┌───────────────────┐   ┌─────────────────────────┐   ┌──────────────────┐
  │ Agente Windows    │   │ Agente Linux (systemd)  │   │ Agente macOS     │
  │ servicio + VSS    │   │ «Guarda copias»:        │   │ (launchd)        │
  │ CLI + app opcional│   │ rest-server append-only,│   │ CLI              │
  └────────┬──────────┘   │ TLS con CA propia, CLI  │   └────────┬─────────┘
           │ LAN (TLS)    └──────────┬──────────────┘            │
           └────────────────────────▶│◀──────────────────────────┘
                                     │ copia externa (restic copy)
                                     ▼
                     Nube S3 / Backblaze B2 con Object Lock
```

### 1.2 Componentes

| Pieza | Qué hace | Qué **no** puede hacer |
|---|---|---|
| **Resguardo Web** (SPA en Vercel) | Consola: estado, avisos, equipos, destinos, copias, restaurar y clientes. Sella en el navegador las contraseñas que escribes. | Guardar contraseñas. Dar órdenes estructurales o tocar datos sin la contraseña que corresponde. |
| **Supabase** (gratuito) | Cuentas con TOTP, clientes, claves públicas de los equipos, órdenes selladas, respuestas cifradas, estado, auditoría y sesiones por sondeo. | Lo mismo que la web: solo transporta texto cifrado y metadatos. |
| **Agente** (Rust, un binario por sistema) | Servicio sin ventana. Guarda y aplica la configuración, comprueba contraseñas y verificadores, ejecuta copias, verificación, pruebas de restauración y retención (si guarda copias), responde a las sesiones, informa e incluye una **CLI** local. | Aceptar una orden protegida sin su contraseña. Borrar copias de un servidor de solo añadir. |
| **Servidor de copias** (rest-server oficial, fase 4) | En los equipos «Guarda copias»: un usuario por equipo, `--append-only --private-repos` y TLS con CA propia. | Leer las copias: van cifradas con la contraseña de cada repositorio. |
| **App de escritorio** (Tauri, opcional) | Interfaz local en Windows (y en Linux con escritorio, más adelante): restaurar rápido, ver el estado, gestionar en modo local y servir de **verificador independiente**. | Ser obligatoria. |
| **Publicación** | Compila, firma con nuestra llave Ed25519 (minisign) y publica en el repositorio público de binarios. | Nada en tiempo de ejecución: la llave de publicación está fuera de línea. |

Se reutiliza:
- `managed.rs`: sobres X25519, `seq`, caducidad, SAS y código de emparejamiento.
- `endpoint.rs` y `agente.rs`: el servicio, la bandeja y el emparejamiento con el resultado escrito a un archivo.
- El instalador NSIS del agente.
- `server.rs`: rest-server, CA propia con renovación por IP, htpasswd y firewall.
- El agente de siempre: planes, reintentos, VSS, verificación, retención, prueba de restauración e informes.
- Las órdenes de la fase 5 (`backup_now`, `config`, `unpair`) y de las copias a distancia.

---

## 2. Modelo de seguridad

### 2.1 Los tres niveles

| Nivel | Cómo se demuestra | Para qué |
|---|---|---|
| **1. Sesión de la web** | Correo, contraseña y TOTP (sesión `aal2` de Supabase) | Ver estado, avisos y actividad. Peticiones **inofensivas** (2.2). |
| **2. Contraseña del repositorio** | La escribe el usuario y va sellada al equipo que tiene ese repositorio, que la compara con la real | Todo lo que **toca o revela los datos** de un repositorio (2.3). |
| **3. Clave de administración del cliente** | La escribe el usuario y va sellada a cada equipo afectado como **prueba derivada** que el equipo comprueba contra su verificador (2.4) | **Cambios de estructura** que podrían sacar datos o reducir la protección (2.3). |

### 2.2 Qué es inofensivo (solo sesión de la web)

Criterio: una petición es inofensiva si **no revela datos** ni **reduce la protección**. Como mucho gasta recursos, y eso se limita por frecuencia.

| Petición | ¿Inofensiva? | Por qué |
|---|---|---|
| **Copiar ahora** (una copia existente) | Sí | Aumenta la protección; ni revela ni borra nada. Máximo 1 por copia cada 15 min. |
| **Verificar ahora**, **probar la restauración ahora** | Sí | Solo lee y comprueba; la prueba restaura a un temporal del propio equipo y lo borra. Máximo 1 al día. |
| **Subir ahora** (copia externa ya configurada) | Sí | Aumenta la protección. Máximo 1 por hora. |
| **Desbloquear bloqueos antiguos** (`restic unlock`) | Sí | Solo quita los de procesos que ya no existen o con más de 30 min sin renovar (ya implementado). |
| **Reanudar** copias en pausa | Sí | Aumenta la protección. |
| **Actualizar el agente** a una versión publicada | Sí | El agente solo instala lo firmado con nuestra llave. |
| **Cancelar** una orden con espera pendiente | Sí | Solo impide un cambio. Lo peor es una molestia, nunca una pérdida. |
| **Pausar** copias | **No** (nivel 3) | Un atacante pausaría todo antes de cifrar los equipos. Pausar reduce la protección. |
| Mostrar u ocultar el icono de la bandeja, avisos de cada copia | **No** (nivel 3) | Es poco, pero cambia lo que ve el usuario del equipo. Se agrupa con los cambios de configuración para no tener excepciones. |

### 2.3 Qué pide cada contraseña

**Contraseña del repositorio** (para ese repositorio):
- ver nombres de archivos y versiones (explorar o buscar);
- restaurar (en el mismo equipo o en otro) y, con Pro, descargar;
- cambiar la retención y aplicar `forget` o `prune`;
- quitar el repositorio o **dejar de copiar** en él;
- cambiar su copia externa o la retención en el destino;
- rotar la contraseña del repositorio (`restic key`).

**Clave de administración del cliente:**
- añadir o quitar **destinos**;
- **crear** copias o repositorios (en un repositorio nuevo, el navegador genera además su contraseña; ver 4.5);
- cambiar **qué carpetas** se copian, las exclusiones, los horarios o los ganchos;
- **pausar** o **desactivar** copias y desactivar el agente;
- **dar de alta o de baja** equipos y **desvincular** el cliente;
- marcar o desmarcar un equipo como **«Guarda copias»**;
- cambiar la **espera** de las órdenes destructivas (con un mínimo, ver 2.6), la bandeja y el modo discreto;
- cambiar la propia clave de administración.

Algunas órdenes piden **las dos**: borrar un repositorio necesita la clave de administración (estructura) **y** su contraseña (datos).

### 2.4 Cómo comprueba el equipo la clave de administración

Objetivo: que un equipo comprometido **no** pueda aprender una clave que le sirva contra los demás equipos del cliente.

- **Al crear el cliente**, el usuario elige la clave de administración (o acepta una generada). Mínimo: 4 palabras aleatorias o 16 caracteres; la web mide su fuerza. Se imprime en el **kit del cliente**.
- **Sal por equipo.** Al emparejarse, cada equipo genera una sal aleatoria `sal_e`. El navegador calcula
  `prueba_e = Argon2id(clave, sal_e)` (64 MiB, 3 pasadas) y se la entrega sellada.
  El equipo guarda solo `verificador_e = SHA-256(prueba_e)`.
- **En cada orden protegida**, el navegador vuelve a calcular `prueba_e` para **ese** equipo y la mete dentro del sobre sellado. El equipo comprueba `SHA-256(prueba_e) == verificador_e` en tiempo constante.
- **Por qué así:**
  - Nadie en el camino (la web) puede abrir el sobre.
  - Quien robe el verificador del disco de un equipo no puede usarlo: necesitaría `prueba_e`, y SHA-256 no se invierte.
  - Un equipo malicioso sí ve `prueba_e` cuando le llega una orden, pero **solo vale para él**: la sal es distinta en cada equipo, y sacar la clave exigiría fuerza bruta contra Argon2id con una clave fuerte.
- **Varias claves por persona** (opcional, más adelante). El equipo puede guardar varios verificadores con nombre («Ever», «Técnico Norte»). Así la auditoría del propio equipo sabe quién fue, y se puede retirar la clave de una persona sin cambiar la de los demás.

### 2.5 Integridad de las órdenes (diseño elegido)

**Lo más sencillo que es sólido: el secreto viaja dentro del sobre sellado, junto con la orden.**

```
sobre = SealedBox_X25519(clave_pública_del_equipo, {
  v: 2, cliente, equipo, seq, nonce, emitida, caduca, not_before?,
  tipo, cuerpo,
  autorizacion: { prueba_admin?: prueba_e, clave_repo?: { repo_id, contraseña } }
})
```

- **Confidencialidad:** solo el equipo abre el sobre.
- **Autenticidad:** solo quien conoce el secreto puede crear un sobre que el equipo acepte. El secreto **es** la autorización; no hace falta una firma aparte.
- **Integridad:** el sobre sellado es un AEAD. Cambiar un byte lo invalida, y nadie puede rehacerlo con el mismo secreto sin conocerlo.
- **Repetición:**
  - `seq` mayor que el último aceptado por ese equipo;
  - `nonce` único, que el equipo recuerda durante la vida de la orden;
  - caducidad: 1 h para órdenes interactivas, 24 h para las encoladas y 7 días para el cambio de clave de administración, que debe llegar a equipos apagados.

**Por qué no Ed25519 en el navegador.** Habría que guardar una llave privada en algún sitio. Antes era una bóveda desbloqueada por passkey con PRF, y eso es lo que el usuario descartó. Sin bóveda, firmar no añade seguridad sobre «conocer el secreto». Ed25519 se mantiene donde sí aporta:
- **El equipo firma sus respuestas**: listados de carpetas y versiones, resultados de órdenes. Así el navegador sabe que vienen de ese equipo.
- **Las actualizaciones** van firmadas con la llave de publicación.

**Autenticidad de la clave pública del equipo.** El navegador cifra para la clave X25519 que le da la web. Si la web estuviera comprometida, podría poner la suya y capturar la contraseña. Contramedida:
- Al emparejar (con SAS), el navegador calcula `etiqueta = HMAC(K_cfg, equipo | box_pub | sign_pub)` y la guarda en la web. `K_cfg` se deriva de la clave de administración (2.7).
- Al crear un repositorio guarda además `etiqueta_repo = HMAC(Argon2id(contraseña_repo, sal_repo), equipo | box_pub)`.
- Antes de sellar una contraseña, el navegador **comprueba la etiqueta** derivada de la contraseña que el usuario acaba de escribir. Si no cuadra, no envía nada y avisa: «La identidad del equipo no coincide».

Así, una base de datos manipulada no puede desviar contraseñas. Lo único que no cubre es el JavaScript malicioso (2.8).

### 2.6 Intentos fallidos y órdenes con espera

- **Bloqueo en el equipo.**
  - 5 intentos fallidos de una misma contraseña en 15 min bloquean ese tipo de orden 15 min, y el bloqueo se duplica en cada racha, hasta 24 h.
  - Cada fallo y cada bloqueo se informa como **aviso** («3 intentos fallidos con la clave de administración en RECEPCION»).
  - La web no puede adivinar contraseñas por fuerza bruta: no tiene verificadores. Lo más que puede hacer es enviar basura y provocar bloqueos (denegación de servicio), que se ve en los avisos.
- **Órdenes destructivas.**
  - Son destructivas: acortar la retención, `prune` fuera de política, quitar un repositorio, dejar de copiar, pausar más de 24 h, desactivar el agente o la copia externa, dar de baja un equipo, quitar «Guarda copias» y desvincular con «Dejar de copiar».
  - Llevan `not_before = ahora + espera`. **El equipo aplica la espera**: guarda la espera mínima del cliente, que solo se cambia con la clave de administración y nunca baja de 1 h. Rechaza una orden destructiva que llegue con menos.
  - Mientras espera, todos los usuarios del cliente reciben un aviso, y **cualquiera con sesión puede cancelarla** (2.2).

### 2.7 La configuración, cifrada

La web muestra nombres de copias, horarios y estado, pero **no** las rutas.
- La configuración completa de cada equipo (carpetas, exclusiones y ganchos) se guarda cifrada con `K_cfg = HKDF(Argon2id(clave_admin, sal_cliente), "config")`.
- El navegador la descifra cuando el usuario escribe la clave de administración para editarla.
- El equipo también conoce `K_cfg`: se le entrega al emparejar. Así puede subir su configuración cifrada (por ejemplo, al revincularse) y **la fuente de verdad es el equipo**.
- Un equipo comprometido que conozca `K_cfg` puede leer las rutas configuradas de otros equipos del cliente. Es información de configuración, no de archivos ni de autoridad, y se acepta.

### 2.8 Límite honesto

**Si alguien controla el código que sirve la web** (Vercel, el repositorio de la web o la cadena de compilación), puede capturar una contraseña **cuando el usuario la escribe**. Ningún diseño de cifrado en el navegador lo evita del todo.

Mitigaciones:
- **Solo añadir** en los servidores de copias: ni con la contraseña del repositorio se borra nada allí; solo la retención local del equipo de almacenamiento, que espera y avisa.
- **Object Lock** en la nube: inmutable durante el periodo de retención.
- **Esperas** en todo lo destructivo, **avisos** a todos los usuarios y **cancelación** fácil.
- **Bloqueo** por intentos y avisos de fallos.
- **La app de escritorio como verificador independiente.** Puede mostrar las órdenes pendientes de su equipo y comprobar en local el hash del paquete de la web publicado en el repositorio de versiones. Es opcional.
- **Compilaciones reproducibles** de la web con su hash publicado, SRI y CSP estrictas.

### 2.9 Simplificaciones respecto a la versión anterior

- Ya no hay passkeys ni PRF obligatorios, ni bóveda de secretos en la web.
- Ya no corre prisa un dominio propio: las passkeys dependían del dominio; las contraseñas, no.
- Más adelante, una passkey podrá **recordar** la clave de administración en un navegador concreto, como comodidad opcional, sin cambiar el modelo.

---

## 3. Modelo de amenazas

| Atacante | Qué consigue | Qué **no** consigue, y por qué |
|---|---|---|
| **Web o base de datos comprometidas** (sin cambiar el código servido) | Ver metadatos (nombres, fechas, tamaños, estado). Pedir lo inofensivo: copiar, verificar, cancelar esperas. Borrar o retener mensajes (denegación de servicio). Provocar bloqueos con basura. | Órdenes estructurales o de datos: no tiene las contraseñas ni los verificadores. Desviar contraseñas cambiando claves públicas: el navegador comprueba las etiquetas (2.5). Repetir órdenes: `seq`, `nonce` y caducidad. |
| **Cuenta de la web robada** (contraseña y TOTP) | Lo mismo que el caso anterior, con su rol. | Lo mismo: sin las contraseñas no hay órdenes protegidas. |
| **Código de la web malicioso** (o un navegador con malware) | Capturar la contraseña que se escriba mientras dure el ataque y usarla. | Borrar copias inmutables: solo añadir y Object Lock. Hacerlo sin que se note: las esperas, los avisos y la auditoría del propio equipo lo delatan. |
| **Equipo malicioso** (agente o binario manipulado) | Mentir en sus informes. Ver la contraseña de **sus** repositorios y la `prueba_e` de **su** verificador. Leer las rutas configuradas del cliente (`K_cfg`). | Actuar sobre otros equipos: la sal es por equipo y los repositorios están separados (`--private-repos`). Leer copias ajenas. |
| **Administrador local malicioso en un equipo** | Parar el servicio (se avisa: «detenido por un administrador»), desinstalarlo o leer los datos del propio equipo. | Borrar las copias ya hechas: solo añadir e inmutable. Cambiar la clave del cliente para los demás equipos. |
| **Alguien en la red** | Nada útil. | TLS en todo, CA propia fijada para el servidor de copias y sobres cifrados. |

---

## 4. Protocolos

### 4.1 Alta de un equipo (emparejamiento, fase 5)

1. En la web, **Añadir equipo** muestra un código de un solo uso (10 caracteres). Caduca a los 15 min. La web guarda solo su hash.
2. Se instala el agente:
   - en Windows, el instalador tiene la página del código (ya hecha) o `/S /CODE=`;
   - en Linux, `resguardo-agente vincular CÓDIGO` o el instalador de una línea;
   - en Mac, el script de instalación.
3. El agente se une con `pairing_join` (anónima) y envía su X25519, su Ed25519 y su `sal_e`. Recibe su identidad de dispositivo.
4. Los dos muestran el **SAS** de 6 cifras. El usuario lo compara y **escribe la clave de administración**. El navegador calcula `prueba_e`, `K_cfg` y la etiqueta (2.5) y sella para el equipo el verificador, `K_cfg` y la espera mínima. El equipo queda gestionado.
5. **Sin pantalla:** el número queda en un archivo solo para administradores en el equipo (ya implementado en Windows). La web muestra el nombre y el sistema para confirmarlo.

### 4.2 Órdenes y respuestas

- El navegador construye la orden, añade la autorización que pida su tipo y la sella (2.5).
- El equipo responde:
  - un estado corto sin rutas (*recibida*, *en marcha*, *hecha*, *fallida*);
  - si hay detalle (listas de archivos o errores con rutas), cifrado para la clave efímera X25519 que el navegador incluyó en la orden;
  - todo firmado con su Ed25519.
- **Configuración declarativa:** el documento completo del equipo cifrado con `K_cfg` y sellado para él. El equipo lo aplica entero y el último gana.

### 4.3 Sondeo (plan gratuito) y sesiones interactivas

**Sondeo base.**
- El agente llama a `take` cada **60 s**: una respuesta vacía de pocos cientos de bytes.
- La web marca `atencion_hasta` en el equipo cuando alguien abre su ficha. En la siguiente consulta el agente lo ve y pasa a **cada 2 s** mientras dure (máximo 10 min, renovable mientras la página siga abierta).

**Sesión interactiva** («Elegir carpetas», explorar versiones, progreso de una restauración).
1. El navegador crea una sesión con su X25519 efímera, sellada como orden. Si toca datos, va con la contraseña del repositorio.
2. El equipo responde con su efímera firmada. Ambos derivan una clave de sesión: HKDF del secreto compartido y XChaCha20-Poly1305 con contador.
3. Los mensajes van por la tabla `session_messages`, solo texto cifrado, con caducidad de 10 min y borrado por `pg_cron`. El navegador consulta cada 1–2 s; el equipo, cada 2 s mientras la sesión esté abierta.
4. **Latencia esperada.** El primer contacto tarda lo que falte para la siguiente consulta del equipo (hasta 60 s; la interfaz muestra «Conectando con el equipo…»). Después, de 2 a 4 s por página. Es aceptable para elegir carpetas y explorar versiones.
5. Los listados se paginan (unas 500 entradas por página, por carpeta según se expande el árbol) para no gastar tráfico de salida del plan gratuito.

**Más adelante, con Pro:** Supabase Realtime en canales privados con el mismo cifrado de sesión, para respuestas inmediatas, manteniendo el sondeo como respaldo.

**Sugerencias detectadas** al elegir carpetas: carpetas de usuario de todos los usuarios, Siigo, SQL Server (con un gancho `BACKUP DATABASE … WITH COPY_ONLY`), World Office y otros. Igual que en la versión anterior; siguen pendientes de confirmar las rutas y motores de Siigo y World Office. Los ganchos son **plantillas cerradas** con parámetros validados, nunca órdenes de shell libres.

### 4.4 Restaurar (siempre lo hace un equipo)

- **En el mismo equipo.** Se explora con una sesión: versiones y archivos, usando la contraseña del repositorio. La orden `restore` restaura junto al original o en una carpeta elegida, y el progreso llega por la sesión. **Reemplazar** archivos existentes es una opción explícita.
- **En otro equipo.** El usuario escribe la contraseña del repositorio de origen y el navegador la sella **para el equipo destino**, junto con la ubicación y las credenciales de acceso al destino. Esas credenciales las obtiene así:
  - **Nube:** el usuario también escribe o pega la clave de la nube (de su kit). La web no la guarda.
  - **Servidor de copias:** el equipo de almacenamiento, con una orden que lleva la clave de administración, crea un **usuario temporal de lectura**. Caduca a las 24 h. Hará falta un modo de solo lectura o un proxy; a evaluar.

  El equipo destino borra el acceso al terminar.
- **Descargar al navegador:** se aplaza a Pro (relé por Storage con trozos cifrados). Ver 9.

### 4.5 Repositorios desde la web

1. El usuario escribe la **clave de administración**: crear es estructural.
2. El navegador **genera la contraseña del repositorio** (32 bytes aleatorios) y muestra el **kit de recuperación** para imprimir o guardar en PDF. Hay que confirmarlo antes de seguir.
3. Sella la contraseña y las credenciales del destino para el equipo (y para el equipo de almacenamiento si hace falta crear un usuario), junto con la orden `init_repo`.
4. El equipo ejecuta `restic init` y devuelve el `id` del repositorio. El navegador guarda la etiqueta `etiqueta_repo` (2.5) y **olvida la contraseña**.

### 4.6 «Este equipo guarda copias»

Es la fase 4 llevada al agente:
- rest-server oficial con hash fijado, `--append-only --private-repos`, TLS con CA propia y renovación por cambio de IP, firewall (Windows Firewall, nftables, firewalld o ufw);
- un usuario por equipo cliente;
- la retención y la verificación las aplica el equipo de almacenamiento sobre su carpeta local;
- en la misma sede no hacen falta puertos; para otra sede, un puerto reenviado, un túnel saliente o la nube.

**Estándar recomendado por cliente:** almacenamiento local y copia externa a la nube con Object Lock.

---

## 5. Desvincular y seguir funcionando en local

### 5.1 Opciones

**«Desvincular cliente»** (con la clave de administración) y **«Desvincular equipo»** ofrecen dos opciones:

| Opción | Qué hace |
|---|---|
| **a) Dejar de copiar** | Lo de hoy: el equipo deja de copiar y olvida la configuración gestionada. Es destructiva, así que **espera**. Las copias hechas se conservan. |
| **b) Seguir funcionando en local** | Cada equipo conserva su última configuración, horarios, servidor de copias, retención, verificación y pruebas de restauración, y **funciona solo, sin la web**. No es destructiva: no espera. |

### 5.2 Qué pasa con la confianza en la opción b

- El equipo **borra sus credenciales de la web**: el secreto del dispositivo y su vínculo. Deja de consultar. La web marca el equipo como «Desvinculado (local)» y deja de contarlo.
- **Conserva** su verificador de la clave de administración, `K_cfg`, su sal y su par de llaves. Los necesitará para volver.
- A partir de ese momento, **el administrador local** (root, o Administradores en Windows) es quien manda, a través de la CLI o la app. Es coherente: ya podía parar el servicio.
- Las órdenes que estuvieran esperando se cancelan al desvincular.

### 5.3 Gestión local

**CLI en todos los equipos** (`resguardo-agente`, como root o administrador). Las órdenes, en español:

| Orden | Qué hace |
|---|---|
| `estado` | Equipo, modo (gestionado o local), copias y su última ejecución, próximas, almacenamiento |
| `copias` | Lista de copias y repositorios |
| `copiar-ahora [copia]` | Lanza una copia |
| `versiones <repo>` | Lista de versiones |
| `restaurar <repo> <versión> <ruta> [--destino …]` | Restaura |
| `retencion aplicar <repo>` | Aplica la retención (en los equipos que guardan copias) |
| `verificar <repo>` | Verificación |
| `registro [--seguir]` | El registro del agente |
| `kit <repo>` | Muestra el kit de recuperación (contraseña incluida, solo como root o administrador) |
| `config exportar` / `config importar` | Ver o cambiar la configuración en un archivo legible (solo en modo local) |
| `vincular CÓDIGO` | Volver a la web (5.4) |

**Lo que sigue funcionando solo:**
- en Windows, el servicio, o la tarea programada si no hay servicio;
- en Linux, la unidad de systemd, con *timers* de systemd para la verificación y la retención si se prefieren;
- en Mac, launchd.

**Interfaz gráfica:**
- **Windows:** la app de escritorio (ya existe), en modo local.
- **Linux con escritorio:** la app de escritorio cuando exista la versión Linux (Tauri lo permite; es una fase propia de bajo coste).
- **Linux sin escritorio** (servidores, contenedores): **solo la CLI**, documentada.
- **macOS:** la CLI y, más adelante, la app si se compila para Mac.

### 5.4 Volver a vincular sin reconfigurar

- En la web, **«Volver a vincular un cliente»** crea el cliente (o lo reutiliza) y da un código por equipo. En cada equipo: `resguardo-agente vincular CÓDIGO`, el instalador o la app.
- Como el equipo conserva su verificador, el navegador pide la **clave de administración** del cliente y manda una prueba sellada. Así se demuestra que es **el mismo cliente**, sin depender de nada guardado en la web.
- El equipo sube su configuración cifrada con `K_cfg`: carpetas, horarios, repositorios (sin contraseñas) y retención. La web la muestra **tal cual**: no se reconfigura nada.
- **Si se perdió la clave de administración:** el administrador local de cada equipo puede ejecutar `resguardo-agente restablecer-clave`, que borra el verificador y `K_cfg` y pide un emparejamiento nuevo con una clave nueva. La configuración local se conserva.

---

## 6. Clientes, roles y auditoría

- **Clientes:** cada uno agrupa sus equipos, destinos, avisos y auditoría, con su clave de administración. Una cuenta puede estar en varios. Hay un selector arriba y una vista «Todos los clientes». RLS por `tenant_id`, y el equipo comprueba `cliente` en cada orden.
- **Roles en la web:**

  | Rol | Puede |
  |---|---|
  | Propietario | Todo, incluidas las invitaciones |
  | Administrador | Todo menos invitar |
  | Técnico | Lo inofensivo, y enviar órdenes si conoce las contraseñas, salvo dar de baja equipos y desvincular |
  | Solo lectura | Ver estado, avisos y actividad |

- **Lo que hay que saber de los roles.** La autoridad real son **las contraseñas que conoce cada persona**: el equipo no distingue a las personas, solo comprueba secretos. Los roles de la web deciden quién puede enviar qué (RLS). Para separar a las personas de verdad están las **claves por persona** (2.4).
- **Auditoría:**
  - Tabla de solo añadir, encadenada por hash por cliente: quién (cuenta), qué tipo, sobre qué, cuándo y el hash del sobre.
  - El **equipo lleva su propio registro** de órdenes aceptadas, rechazadas y bloqueos, y lo informa. Si no coincide con el de la web, se ve.

---

## 7. Modelo de datos en Supabase

Nombres orientativos; los ajusta el agente de la web. Las tablas actuales se mantienen mientras dura la migración.

| Tabla | Campos principales | Notas |
|---|---|---|
| `tenants` | `id`, `name`, `client_salt`, `destructive_delay_min`, `state` (`gestionado` o `desvinculado`) | Sin clave ni verificador. |
| `memberships` | `tenant_id`, `user_id`, `role` | Cuenta ↔ cliente. |
| `devices` (ampliada) | + `tenant_id`, `os`, `agent_version`, `box_pub`, `sign_pub`, `device_salt`, `admin_tag` (HMAC con `K_cfg`), `role` (`agente` o `almacenamiento`), `mode` (`gestionado` o `local`), `atencion_hasta`, `last_seen`, `service_state` | La sal y la etiqueta son públicas; el verificador **solo** está en el equipo. |
| `device_configs` | `device_id`, `seq`, `ciphertext` (con `K_cfg`), `summary` (nombres de copias y horarios en claro) | La fuente de verdad es el equipo. |
| `destinations` | `id`, `tenant_id`, `kind`, `display` (sin secretos), `storage_device_id?`, `object_lock` | |
| `repositories` | `id`, `tenant_id`, `destination_id`, `device_id`, `name`, `restic_id`, `repo_salt`, `repo_tag` | **Sin contraseña.** |
| `agent_commands` | `id`, `tenant_id`, `device_id`, `seq`, `kind` (en claro), `sealed`, `not_before`, `expires_at`, `issued_by`, `cancelled_at`, `taken_at` | |
| `command_results` | `command_id`, `state`, `message_short`, `detail_ciphertext`, `agent_sig`, `at` | |
| `sessions`, `session_messages` | Sesión, mensajes cifrados (TTL de 10 min) | Sondeo; Realtime más adelante. |
| `alerts` | `tenant_id`, `device_id`, `kind` (intentos fallidos, espera pendiente, retraso, error…), `at`, `ack_by` | Origen de los avisos y las notificaciones. |
| `reports` | El `device_report` de hoy, ampliado (sin rutas) | Agregar el histórico a los 90 días, por el límite de 500 MB de la base gratuita. |
| `audit_log` | `tenant_id`, `n`, `prev_hash`, `hash`, `actor`, `action`, `target`, `envelope_hash`, `at` | Solo añadir. |
| `push_subscriptions` | `user_id`, `endpoint`, `keys` | Web push. |

**Desaparecen respecto a la versión anterior:** las llaves de administrador, los envoltorios de passkey, las revocaciones, las llaves de bóveda y las entradas de bóveda.

**RPC.**
- Anónima: `pairing_join`, con límites por IP.
- De dispositivo: `take`, `result`, `report`, `config_put`, `session_poll`.
- De usuario: `pairing_open`, `send_command`, `cancel_command`, `session_*`, `alerts_*`, `invite_*`.

**Todo en RPC de Postgres, sin Edge Functions** (ver 12). `pg_cron` se usa para limpiar sesiones caducadas.

---

## 8. Agentes por sistema

Igual que en la versión anterior, con estos cambios: **CLI** en todos y **sin firma de pago** (ver 9).

| Pieza | Windows (hecho en parte) | Linux (prioridad) | macOS |
|---|---|---|---|
| Servicio | Servicio `ResguardoAgente` (LocalSystem) | systemd, root con `CAP_DAC_READ_SEARCH` y endurecimiento | LaunchDaemon (root) |
| Secretos en reposo | DPAPI de máquina en `ProgramData\Resguardo\privado` | Archivos `0600` de root en `/var/lib/resguardo-agente/privado`; `systemd-creds` con TPM2 si existe | Llavero del sistema y archivos `0600` |
| Instantáneas | VSS | LVM, btrfs o ZFS si se puede; en un CT de Proxmox, ninguna (ganchos de volcado para las bases de datos) | Instantánea local APFS |
| Permisos y ACL | restic guarda las ACL NTFS | Propietario, modo y xattrs (ACL POSIX) | Igual que Linux; necesita acceso total al disco (TCC) |
| Servidor de copias | rest-server, servicio y firewall (hecho) | rest-server, unidad systemd y nftables, firewalld o ufw | Opcional y no prioritario |
| Empaquetado | NSIS (hecho), `/S /CODE` | `.deb`, `.rpm` y `tar.gz` estático (musl) con `install.sh`. Instalación de una línea que **comprueba la firma minisign** antes de instalar | `.pkg` **sin firmar** instalado por script (`curl` + `sudo installer`; sin cuarentena, Gatekeeper no lo bloquea). Firmado y notarizado cuando haya cuenta de Apple |
| Interfaz local | CLI y app de escritorio | CLI; la app en escritorios Linux, más adelante | CLI |

---

## 9. Plan gratuito: qué cabe y qué espera a Pro

**Límites del plan gratuito de Supabase** (a la fecha; a verificar):
- base de datos de 500 MB;
- 1 GB de Storage;
- unos 5 GB de tráfico de salida al mes;
- 200 conexiones simultáneas de Realtime;
- 500 000 invocaciones de Edge Functions;
- **el proyecto se pausa tras 7 días sin actividad**: el sondeo de los agentes lo mantiene activo, pero hay que confirmarlo;
- copias de seguridad de la base limitadas.

**Cómo se ajusta el diseño:**
- Sondeo cada 60 s con respuestas mínimas. Con 50 equipos, ~72 000 consultas al día de pocos cientos de bytes: en torno a 1 GB al mes, dentro del límite. El intervalo es ajustable por cliente (por ejemplo, 120 s en clientes grandes) y solo baja a 2 s con la ficha abierta.
- Listados paginados.
- Sin relé de descargas.
- Histórico de informes agregado.
- Sin Edge Functions.

**Qué ganaría con Pro** (unos 25 USD al mes):

| Función | Por qué Pro |
|---|---|
| **Respuestas inmediatas** (Realtime para avisos y sesiones) | Menos sondeo, latencia de ~0,2 s y más de 200 conexiones |
| **Descargar al navegador** (relé por Storage, hasta 500 MB) | Tráfico de salida y almacenamiento |
| **Sin pausas del proyecto**, copias de la base con PITR | Tranquilidad operativa |
| **Más histórico** (informes y auditoría largos) | 8 GB de base de datos |
| Más clientes y equipos | Tráfico y base de datos |

**Cuándo merece la pena pagar:**
- a partir de unos **100–150 equipos** (tráfico de sondeo y tamaño de la base);
- cuando un cliente necesite **descargar al navegador** o respuestas inmediatas;
- o cuando se cobre el servicio: 25 USD entre 100 equipos son unos 0,25 USD por equipo y mes.

**Correo y avisos (recomendación).**
1. **Avisos dentro de la web** y **notificaciones push web** (VAPID; gratis y sin dominio. En iPhone hace falta instalar la web como app).
2. **Invitaciones por enlace:** la web genera el enlace de invitación y el propietario lo envía por donde quiera (WhatsApp, correo). No hace falta ningún servicio de correo.
3. **Correo opcional:**
   - con **SMTP de Gmail** (contraseña de aplicación) configurado en Supabase, para un resumen diario de avisos (gratis, con límite diario);
   - **Resend** (gratis hasta 3 000 al mes) cuando haya un dominio propio, porque exige verificar uno para enviar a cualquier destinatario.

   **Recomendado ahora: push y enlaces; el correo, opcional con Gmail.**

**Firma de código: ninguna de pago por ahora.**
- **Windows:** el instalador del agente y la app sin Authenticode. SmartScreen mostrará «Windows protegió su PC», que se salta con «Más información → Ejecutar de todas formas». Algunos antivirus pueden desconfiar más.
  - En **instalaciones por script o por herramientas de despliegue**, el archivo no lleva la marca de descarga y SmartScreen no aparece.
  - La guía lo explica y publica el SHA-256 de cada instalador.
  - Se puede enviar cada versión a Microsoft para análisis y reducir los falsos positivos.
- **Autenticidad:** las **actualizaciones automáticas** y la instalación de una línea en Linux y Mac comprueban la firma **minisign (Ed25519)** con nuestra llave pública incluida en el agente y en el script. Lo descargado por el propio agente tampoco lleva la marca de descarga, así que SmartScreen no interviene en las actualizaciones.
- Cuando haya presupuesto: Azure Trusted Signing (~10 USD al mes) o un certificado OV; y la cuenta de Apple (99 USD al año) para Mac.

---

## 10. Actualizaciones y repositorio público de versiones

**Dónde:** un repositorio **público** solo de binarios, por ejemplo `<usuario>/resguardo-releases`. El código sigue en el repositorio privado.

**Qué pasa a ser público:**
- los **instaladores y binarios** de cada versión (agentes y, si se quiere, la app);
- los **manifiestos** firmados con la versión, la URL, el SHA-256, el anillo y la versión mínima, y sus **firmas minisign**;
- la **llave pública** de publicación;
- las **notas de la versión**;
- el **hash del paquete de la web** de cada despliegue (para el verificador independiente, 2.8);
- las licencias de restic y rest-server.

**No** se publican el código ni nada de clientes.

**Licencia.** Resguardo es GPL-3.0-or-later. Distribuir binarios públicamente obliga a dar acceso al **código fuente correspondiente**: publicando en ese mismo repositorio un archivo con el código de cada versión, o con una oferta escrita de entregarlo a quien lo pida. **Decisión abierta** (14). restic y rest-server son BSD, con sus licencias incluidas.

**Cómo actualiza el agente.**
1. Cada 6 h consulta el manifiesto en GitHub (sin autenticación; el límite de peticiones anónimas sobra con un intervalo así).
2. Comprueba la **firma minisign** del manifiesto y el SHA-256 del binario.
3. Se reemplaza:
   - en Windows, un actualizador para el servicio, cambia el binario y lo arranca;
   - en Linux, un reemplazo atómico, o el gestor de paquetes si viene de un repositorio;
   - en Mac, `installer`.
4. Si no informa sano en 10 min, **vuelve atrás**.

**Anillos:** interno, temprano y general. Desde la web se puede fijar una versión por cliente o pedir «actualizar ahora» (inofensivo, 2.2).

**La llave de publicación** vive fuera de línea (minisign con contraseña) en manos del dueño del proyecto, con una copia de seguridad impresa o cifrada. Si se pierde, una versión firmada con la antigua introduce la nueva.

---

## 11. Migración de lo existente

Principio: **adoptar, no reinstalar**. Mismas copias, mismos repositorios y mismas contraseñas. Las contraseñas **se quedan en los equipos** (como hoy), lo que simplifica la migración: nada se mueve a la web.

| Equipo | Hoy | Paso a paso |
|---|---|---|
| **Un equipo con la app de escritorio** | App completa con agente, vinculada | Cliente «Propio». «Adoptar este equipo» desde la app (como administrador): código y SAS. Se fija la clave de administración y la app entrega al agente el verificador y su configuración actual. La web recibe el resumen y la configuración cifrada. Nada cambia en las copias. |
| **Un servidor con un programa contable** (copia a Backblaze) | App y agente con copia a Backblaze | Igual. La clave de Backblaze y la contraseña del repositorio **siguen en el equipo**. Se revisa la sugerencia «Siigo» y el gancho de SQL Server si aplica. Recomendado: Object Lock en el bucket. |
| **Una oficina con el agente de la fase 5** | Agente de la fase 5 con su consola | La consola, que ya está fijada, le manda una orden de **traspaso** con el verificador y `K_cfg` del cliente nuevo, sellada y firmada por ella. El agente la acepta y desde entonces sigue el modelo nuevo. Las contraseñas del repositorio y del servidor que tenía la consola siguen en el equipo (y en la consola como respaldo); no van a la web. |
| **Un servidor de copias rest-server** | rest-server (CT o servidor; ver [rest-server-remoto.md](rest-server-remoto.md)) | Agente de Linux y «Adoptar servidor de copias existente». Si es el nuestro, el agente lo gestiona con sus usuarios, CA o túnel; si es otro, lo conserva tal cual y añade la retención y la verificación locales. Los equipos que copian en él no cambian de ubicación. |

Las copias a distancia y lo compartido (fases 2 y 3) pasan a ser órdenes del modelo nuevo; las tablas antiguas se retiran después de dos versiones.

---

## 12. Más adelante: «Resguardo local» (instalado en el cliente)

**Qué es:** la misma consola web, **instalada en un servidor del cliente**:
- Supabase autoalojado (Docker Compose: Postgres, PostgREST, GoTrue, Realtime y Storage si hacen falta);
- la SPA estática servida en local (Caddy o nginx con TLS de la CA propia o la del cliente);
- los agentes del cliente apuntando a esa dirección.

Sin dependencia de Internet, salvo para la copia externa a la nube si se usa.

**Restricciones de diseño desde ya**, para no quedar atados a la nube:

| Elemento | Regla |
|---|---|
| **Edge Functions** | No depender de ellas: toda la lógica en **RPC de Postgres**. Si alguna fuera imprescindible, que funcione en el *edge runtime* autoalojado. |
| **pg_cron** | Permitido: existe en Postgres autoalojado. Documentar cómo activar la extensión. |
| **Web push** | Necesita Internet (servicios push de los navegadores). En modo local, **avisos dentro de la web** y correo por el **SMTP del cliente**. Push, opcional. |
| **SMTP** | Configurable: ninguno, Gmail, Resend o el del cliente. Las invitaciones por enlace no lo necesitan. |
| **URLs y configuración** | Nada fijo a `supabase.co` ni a Vercel. La SPA lee su configuración (URL de la API y clave pública) de un `config.json` servido junto a ella. Los agentes reciben la URL al vincularse (`--web-url`, ya existe) y la cambian con una orden. |
| **Actualizaciones** | Fuente configurable: GitHub o un **espejo local** con los mismos manifiestos firmados. |
| **TLS** | Admitir certificados de una CA propia: el agente puede fijarla, como el servidor de copias. |
| **Auth** | Solo funciones de GoTrue disponibles autoalojadas: correo, contraseña y TOTP. Sin proveedores OAuth obligatorios. |
| **Realtime y Storage** | Opcionales. El sistema funciona con sondeo y sin relé. |

**Esfuerzo:** 2–3 semanas (empaquetado Docker, `config.json`, guía de instalación y actualización, prueba en un Proxmox), después de tener la web estable.

---

## 13. Cómo se ve en la web

Sigue [diseno.md](diseno.md): sobrio, mucho aire, el color para el estado y voz tranquila de tú.

**Navegación:**
- selector de cliente;
- **Estado** (resumen grande y lo urgente);
- **Equipos**;
- **Destinos**;
- **Restaurar**;
- **Actividad**;
- **Avisos** (con su contador);
- **Ajustes:** cliente, usuarios, auditoría y desvincular.

**Confirmaciones, sin tecnicismos.**
- **«Confirmar con la contraseña del repositorio»:** un modal con el nombre del repositorio, un campo de contraseña y, debajo, «Se comprueba en el equipo; la web no la guarda».
- **«Confirmar con la clave de administración»:** igual, con «Clave de administración de PC-01».
- Tras un fallo, el texto viene del equipo: «Contraseña incorrecta. Quedan 3 intentos antes de bloquear 15 min».
- **Comodidad:** la clave de administración se puede **recordar en esta pestaña** durante 10 min (solo en memoria; se olvida al cerrarla), con un chip «Clave recordada · 8 min · Olvidar». La contraseña de un repositorio se puede recordar en la pestaña mientras se explora y se restaura.
- **Órdenes con espera:** una tarjeta «Pendiente: quitar el repositorio “Fotos” de RECEPCION · se aplica en 23 h · Cancelar», visible para todos los usuarios del cliente.

**Flujos.**
1. **Añadir equipo:**
   - elegir el sistema y descargar el instalador (con su SHA-256 y la nota de SmartScreen) o copiar la línea de Linux;
   - el código grande;
   - «Conectando con el equipo… (hasta 1 min)»;
   - el número de comprobación;
   - «Confirmar con la clave de administración».
2. **Nueva copia** (asistente):
   - **Equipo**;
   - **Qué copiar:** el árbol en vivo, con «cargando…» por carpeta, y las sugerencias como chips;
   - **Dónde:** un destino existente o uno nuevo, con el kit para imprimir;
   - **Cuándo**;
   - **Revisar:** un resumen en frases y «Confirmar con la clave de administración».
3. **Restaurar:**
   - qué equipo;
   - «Confirmar con la contraseña del repositorio»;
   - qué versión y qué archivos;
   - dónde: junto al original, una carpeta u otro equipo;
   - el progreso.
4. **Desvincular:** dos tarjetas grandes, «Seguir funcionando en local» (recomendada) y «Dejar de copiar», cada una con lo que pasará en una frase y la guía de la CLI enlazada.
5. **Avisos:** lista con intentos fallidos, esperas, retrasos y equipos parados, cada uno con su acción.

**Móvil:** estado, avisos, «Copiar ahora», cancelar esperas y restaurar en una columna.

---

## 14. Plan por fases

Los esfuerzos son orientativos, en semanas de un desarrollador con asistencia de agentes. **Ninguna fase necesita pagar**, salvo F12.

| Fase | Contenido | Esfuerzo | Depende de |
|---|---|---|---|
| **F0. Llaves y protocolo** | Clientes y pertenencias, clave de administración (verificador por equipo, `K_cfg`, etiquetas), protocolo v2 (secreto en el sobre, `seq`, `nonce`, caducidad), bloqueos y avisos, auditoría, roles básicos, sondeo base | 2–3 sem. | — |
| **F1. Agente Windows desde la web** | Órdenes inofensivas y protegidas, editor de copias (rutas escritas), configuración cifrada, esperas aplicadas por el equipo, emparejamiento con SAS en la web | 3 sem. | F0 |
| **F2. Repositorios y nube** | `init_repo` con kit, destinos S3 y B2, Object Lock, copia externa | 1–2 sem. | F1 |
| **F3. Elegir carpetas** | Sesiones por sondeo, árbol en vivo paginado, sugerencias y ganchos (usuarios, Siigo, SQL Server, World Office) | 2 sem. | F1 |
| **F4. «Guarda copias»** | Usuarios por orden, retención y verificación en el servidor, CA y renovación por IP. Windows | 2 sem. | F2 |
| **F5. Agente Linux** | systemd, secretos, instantáneas, rest-server y nftables, deb, rpm y tar.gz, instalación de una línea con minisign, CLI. **Piloto: el CT de Proxmox como almacenamiento** | 3 sem. | F4 |
| **F6. Restaurar con agentes** | Explorar versiones con sesión, restaurar en el mismo equipo y en otro, usuario temporal de lectura en el servidor | 2–3 sem. | F3 |
| **F7. Actualizaciones firmadas** | Repositorio público de versiones, manifiestos minisign, anillos, reemplazo y vuelta atrás | 1–2 sem. | F1 (conviene antes de tener muchos equipos fuera) |
| **F8. Desvincular y modo local** | Opciones a y b, CLI completa, `restablecer-clave`, volver a vincular sin reconfigurar | 2 sem. | F1, F5 |
| **F9. Migración** | Adoptar los equipos con la app, traspaso de las oficinas con el agente de la fase 5, adoptar los servidores de copias existentes | 1–2 sem. | F1, F4, F5 |
| **F10. macOS** | LaunchDaemon, llavero, APFS, TCC, `.pkg` por script (sin firmar) y CLI | 3 sem. | F5 |
| **F11. App de escritorio para Linux** | Compilación Tauri para Linux con escritorio, en modo local | 1 sem. | F8 |
| **F12. Funciones Pro** (cuando se pague) | Realtime para avisos y sesiones, relé de descargas al navegador | 2–3 sem. | Plan Pro |
| **F13. «Resguardo local»** | Supabase autoalojado, SPA con `config.json`, espejo de actualizaciones, guía | 2–3 sem. | Web estable |
| **F14. Comodidades** | Claves por persona, passkey para recordar la clave de administración, verificador independiente en la app | 1–2 sem. | F0 |

**Total aproximado:** 27–35 semanas.

**Orden recomendado:** F0 → F1 → F2 → F7 → F3 → F4 → F5 → F9 → F6 → F8 → F10. El resto según la demanda.

Con F0–F2 ya se gestionan Windows y la nube desde la web. Con F4 y F5, el almacenamiento local en el CT.

---

## 15. Riesgos

| Riesgo | Mitigación |
|---|---|
| **Olvidar la clave de administración** | Kit del cliente impreso al crearlo. `restablecer-clave` en cada equipo como último recurso (5.4); las copias no se pierden. |
| **Olvidar la contraseña de un repositorio** | Está en el equipo y en su kit. `resguardo-agente kit` la muestra en local como root o administrador. |
| **JavaScript malicioso servido por la web** | 2.8: solo añadir, Object Lock, esperas, avisos, bloqueos, compilaciones reproducibles y verificador independiente. |
| **Claves de administración débiles** | Mínimo de fuerza, generador de palabras, Argon2id con sal por equipo y bloqueo por intentos. |
| **Límites del plan gratuito** (tráfico, base de datos, pausa por inactividad) | Sondeo mínimo y ajustable, agregación del histórico, paginación. Vigilar el uso desde la web y pasar a Pro cuando toque (9). |
| **Latencia del sondeo** (hasta 60 s el primer contacto) | `atencion_hasta`, mensajes claros («Conectando…») y Realtime cuando haya Pro. |
| **SmartScreen y antivirus** sin firma de pago | Guía, SHA-256 publicado, despliegue por script y envío a Microsoft. Firma de pago cuando haya presupuesto. |
| **macOS sin cuenta de Apple** | Instalación por script de técnicos. Se pospone la distribución a usuarios finales. |
| **GPL y binarios públicos** | Publicar el código de cada versión o la oferta escrita (decisión 14). |
| **CT de Proxmox sin instantáneas** | Ganchos de volcado y vzdump del anfitrión como complemento. |
| **Complejidad criptográfica** | Pocas piezas (sobres sellados, Argon2id, HKDF y HMAC), bibliotecas auditadas, vectores de prueba compartidos entre Rust y JS, y revisión externa antes de abrirlo a clientes. |

---

## 16. Decisiones

### Ya tomadas

- Gestión desde la web, con agentes y sin consola autoalojada por ahora («Resguardo local», más adelante).
- Seguridad con **sesión, contraseña del repositorio y clave de administración**. Las contraseñas no se guardan en la web y las passkeys quedan como comodidad futura.
- **Plan gratuito** de Supabase. Sondeo primero; Realtime y descargas al navegador, con Pro.
- **Sin firma de código de pago** por ahora. Actualizaciones firmadas con **minisign**.
- Versiones en **GitHub Releases**, en un repositorio público solo de binarios.
- **Desvincular con «Seguir funcionando en local»** y volver a vincular sin reconfigurar.

### Abiertas (para ti)

1. **Lo inofensivo:** ¿estás de acuerdo con la lista de 2.2? En especial, que **pausar** pida la clave de administración y que **cancelar** una espera baste con la sesión.
2. **Espera de las órdenes destructivas:** 24 h por defecto y mínimo de 1 h. ¿Te parece?
3. **Clave de administración:** ¿la eliges tú o la genera la web (4 palabras)? ¿Una por cliente o, desde el principio, una por persona?
4. **Técnicos:** ¿pueden dar de baja equipos o desvincular si conocen la clave, o se reserva a propietarios y administradores?
5. **Código fuente y GPL:** ¿publicar el código de cada versión en el repositorio de versiones, o una oferta escrita?
6. **Correo:** ¿te vale push y enlaces de invitación ahora, con Gmail SMTP opcional para el resumen diario?
7. **Cuándo pasar a Pro:** ¿a partir de cuántos equipos, o cuando un cliente lo pague?
8. **Distribuciones de Linux** al principio. Propuesta: Debian 12 y 13, Ubuntu 22.04 y 24.04, y Proxmox CT con Debian.
9. **Mac:** ¿se hace en F10 sin firmar (solo para técnicos), o se espera a tener la cuenta de Apple?
10. **Nombres en la interfaz:** «Cliente» u «Organización»; «clave de administración» o «clave maestra».
11. **Verificador independiente** (app de escritorio que comprueba el hash de la web): ¿se prioriza?
