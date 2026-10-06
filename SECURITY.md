# Política de seguridad

<!--
Nota para mantenedores: el correo de contacto de seguridad es, por ahora, el
personal del responsable del proyecto. Cuando exista un alias dedicado
(p. ej. seguridad@<dominio>), cámbialo aquí, en security.txt del servidor y
en CONTRIBUTING.md.
-->

Resguardo protege copias de seguridad, así que nos tomamos en serio cualquier fallo de seguridad. Gracias por avisar de forma responsable.

## Cómo informar de una vulnerabilidad

**No abras un issue público.**

1. **Preferido:** el botón **«Report a vulnerability»** de la pestaña *Security* del repositorio en GitHub (avisos privados de seguridad, *Private vulnerability reporting*).
2. **Alternativa:** un correo a **evercarog@gmail.com** con el asunto «[Resguardo] Seguridad». Si quieres cifrarlo, pídenos antes una clave.

Incluye, si puedes: versión y sistema, componente (app, agente, servidor de copias, web), pasos para reproducirlo, impacto que esperas y si ya lo has comunicado a alguien más.

**Lo que puedes esperar:**

| Paso | Plazo orientativo |
|---|---|
| Acuse de recibo | 72 horas |
| Primera valoración (si se confirma y su gravedad) | 7 días |
| Corrección publicada y aviso de seguridad | Lo antes posible; como máximo 90 días desde el aviso, salvo que acordemos otra cosa |

Coordinamos contigo la publicación del aviso (GitHub Security Advisory y notas de la versión) y, si quieres, te damos el crédito.

**Puerto seguro.** No emprenderemos acciones contra investigaciones de buena fe que respeten esta política: sin acceder a datos de terceros más allá de lo imprescindible para demostrar el fallo, sin degradar servicios ajenos y dándonos un plazo razonable antes de publicarlo.

## Versiones con soporte

Resguardo está en **desarrollo activo**. Solo la última versión publicada recibe correcciones de seguridad.

| Versión | Soporte |
|---|---|
| La última publicada (0.7.x del agente y del servidor; la app de escritorio, su última versión) | ✅ |
| Anteriores | ❌ (actualiza a la última) |

## Alcance

**Dentro:**
- la app de escritorio y su agente (`resguardo.exe`, `resguardo-agente.exe`) y sus instaladores;
- el protocolo entre la consola y los agentes (`crates/protocolo`) y el motor de copias (`crates/motor`);
- el Servidor de copias que configura Resguardo (argumentos, certificados, usuarios, firewall);
- Resguardo Server y su consola, cuando existan (`crates/servidor`);
- los scripts de instalación y de compilación de este repositorio.

**Fuera** (avísales a ellos directamente): fallos de [restic](https://github.com/restic/restic/security) o de [rest-server](https://github.com/restic/rest-server), de Tauri, de Windows o de los proveedores de nube. Si un fallo suyo afecta a Resguardo de forma especial, avísanos también.

## Modelo de amenazas y diseño de seguridad

- [docs/plataforma.md](docs/plataforma.md): arquitectura, niveles de autorización (sesión, contraseña del repositorio, clave de administración), protocolo y modelo de amenazas (§3).
- [docs/agente-gestionado.md](docs/agente-gestionado.md): mensajes firmados y sellados, emparejamiento con código de comprobación.
- [docs/compartir.md](docs/compartir.md): sobres cifrados y Servidor de copias (solo añadir, TLS con autoridad propia).
- [docs/actualizaciones.md](docs/actualizaciones.md): actualización automática de los agentes (manifiestos firmados con minisign y llave fuera de línea, consolas como espejo que no firman, sin bajar de versión, vuelta atrás) y su modelo de amenazas (§7).
- Vectores de prueba del protocolo: `crates/protocolo/vectors/`.

## Diseño de seguridad de la app y el agente (Windows)

- Las contraseñas de los repositorios y de los servidores REST se guardan únicamente en el almacén de credenciales del sistema operativo. `repos.json` solo contiene datos no secretos (nombre, ubicación, usuario del servidor, rutas).
- No se aceptan ubicaciones con la contraseña incrustada (`rest:https://usuario:clave@host/`), porque acabaría en texto plano en la configuración.
- restic se ejecuta sin shell y con los argumentos como lista. La contraseña, la ubicación y las credenciales del servidor REST (`RESTIC_REST_USERNAME` / `RESTIC_REST_PASSWORD`) se pasan por variables de entorno del proceso hijo, nunca como argumentos de línea de comandos.
- Las acciones destructivas o que cambian la configuración de un repositorio (quitarlo de la app, cambiar qué carpetas se copian o las exclusiones, restaurar reemplazando archivos existentes, anotar el kit de recuperación como guardado…) exigen escribir su contraseña. La comprobación la hace el backend (comparación en tiempo constante con la guardada), no la interfaz. Hacer una copia no la pide: solo añade un snapshot nuevo, y las copias programadas deben poder ejecutarse sin intervención.
- Se ignoran las variables `RESTIC_*` heredadas del entorno para que no puedan redirigir la contraseña ni el repositorio.
- Al restaurar, por defecto nunca se sobrescribe nada (`--overwrite never`). Los ids de snapshot se validan como hexadecimal y las rutas y nombres se validan en el backend (sin `..` ni separadores), para que nada se interprete como una opción de restic ni escape de la carpeta elegida.
- La interfaz tiene una política CSP estricta y solo los permisos básicos de Tauri (`core:default`, más abrir la carpeta de una restauración en el Explorador).
- La app no abre puertos de red ni envía telemetría.
- Las DLL solo se cargan de `System32` y de la carpeta del programa (`SetDefaultDllDirectories`), nunca de la carpeta actual ni del `PATH`, tanto en la app como en el agente.
- Bloqueo opcional con Windows Hello: con la app bloqueada, el backend rechaza todos los comandos salvo el desbloqueo.

### Limitaciones conocidas

- Si un usuario sin permisos de administrador creó `C:\ProgramData\Resguardo` y dejó archivos dentro **antes** de que se programara la primera copia automática, Resguardo corrige los permisos, pero no puede saber si esos archivos los puso Resguardo. En equipos compartidos, comprueba que la carpeta no exista antes de la primera configuración.

- Pedir la contraseña protege frente a errores y frente a alguien que use tu sesión abierta, pero no frente a un programa que ya se ejecute con tu usuario: ese programa podría leer el almacén de credenciales directamente.
- Con un servidor REST por `http://` (sin TLS), el usuario y la contraseña del servidor viajan sin cifrar. Los datos de las copias siguen cifrados por restic. La app lo advierte al añadir el repositorio.

- Las variables de entorno de un proceso pueden ser leídas por otros procesos **del mismo usuario** del sistema. Es el mismo nivel de confianza que el almacén de credenciales.
- Resguardo usa el `restic` incluido en el instalador (descargado de la publicación oficial y verificado con su SHA-256; la compilación para publicar comprueba además la huella del ejecutable). Solo en desarrollo, si no está, usa el del `PATH`.

## Copias automáticas (modo agente, Windows)

Las copias programadas las hace una tarea del Programador de tareas que ejecuta `resguardo.exe --agent-run` cada 5 minutos como **SYSTEM**. Como SYSTEM tiene todos los permisos del equipo, el agente se diseñó para que un usuario normal no pueda aprovecharlo:

- **Carpeta `C:\ProgramData\Resguardo`**: pertenece a Administradores y sus permisos se aplican de una vez (dueño y lista protegida, sin heredar de `ProgramData`). Los usuarios solo pueden leer la programación y el estado. Si la carpeta tuviera otro dueño o permisos distintos (por ejemplo, creada antes por otra cuenta), Resguardo toma su propiedad y reemplaza sus permisos, quitando cualquier otro acceso.
- **Secretos** (contraseñas de los repositorios y el secreto del equipo para la web): en `privado\secrets.bin`, cifrados con DPAPI del equipo, dentro de una subcarpeta sin ningún acceso para los usuarios desde el momento en que se crea el archivo (siempre un archivo nuevo, nunca uno que ya existiera).
- **Riesgo aceptado — DPAPI a nivel de equipo:** ese cifrado solo impide usar el archivo en **otro** equipo. En este equipo, cualquier cuenta (no solo un administrador) puede descifrarlo si consigue leerlo; la confidencialidad depende por completo de los permisos de `privado\` (solo SYSTEM y Administradores). Cualquier administrador, o quien ejecute código como SYSTEM, puede leer las contraseñas del agente. No se usa el ámbito de usuario de SYSTEM porque la app abierta como administrador debe poder leer y escribir esos secretos al configurar las copias. Si un administrador del equipo no es de confianza, no uses las copias automáticas: usa solo copias a mano.
- **Sin enlaces:** si `C:\ProgramData\Resguardo`, `privado` o `solicitudes` fueran una unión o un enlace simbólico (cualquier usuario puede crear la carpeta antes de instalar), Resguardo quita el enlace —nunca lo que hay en su destino— y crea una carpeta normal; mientras tanto, el agente no lee nada ni escribe el registro.
- **Lo que leen todos los usuarios** (`agent.json`, `state.json`, `historial.jsonl`, `agent.log`): la app sin permisos de administrador los necesita. Los mensajes que se guardan ahí no llevan rutas ni nombres de archivos (`[ruta]` en su lugar) ni contraseñas en direcciones; sí los nombres de los destinos y de las copias, que ya ven los usuarios del equipo. `agent.json` contiene las carpetas que copia cada copia programada. Para diagnosticar, el agente guarda además un registro detallado con las rutas completas (nunca contraseñas) en `privado\agente-detalle.log` (solo SYSTEM y Administradores, rota a unos 5 MB); la app solo lo muestra abierta como administrador («Ver detalle», «Ver qué archivos»).
- **Certificados de CA propios** (servidores REST con certificado propio): se copian a `privado\` al programar, para que SYSTEM nunca use un archivo que un usuario pueda cambiar.
- **Solicitudes de copia** (`solicitudes\`): cualquier usuario puede pedir una copia de un repositorio ya programado (por ejemplo, al cerrar la app con una copia en marcha). Los usuarios solo pueden crear y escribir archivos ahí; no pueden borrar ni cambiar la carpeta. El agente nunca borra ni escribe en ella: solo mira la fecha de cada archivo.
- **Qué ejecuta SYSTEM**: la tarea solo se registra si Resguardo está instalado en *Archivos de programa* (donde los usuarios no pueden cambiar archivos), y el agente solo usa el `restic` que va junto a `resguardo.exe`, nunca uno del `PATH`.
- **Qué copia SYSTEM**: las carpetas elegidas en la app, excluyendo siempre el registro de Windows (`System32\config`), que contiene las credenciales del equipo.
- **Cambios**: programar o cambiar una copia automática exige abrir la app como administrador y escribir la contraseña del repositorio.
- **Un solo agente a la vez**, con un bloqueo que Windows libera solo si el proceso termina de forma inesperada.
- **Desinstalar:** siempre quita la tarea programada. Pregunta si también se borran los datos del agente (`C:\ProgramData\Resguardo`: programación, historial y secretos cifrados); por defecto se conservan (y al actualizar o en una desinstalación silenciosa, nunca se borran). Si se conservan, las contraseñas siguen ahí, cifradas y solo para administradores.
- **Prueba de restauración:** los archivos se restauran en `privado\` (solo SYSTEM y Administradores) y se borran siempre al terminar, también los de solo lectura.

### Verificación y copia externa

- La verificación (`restic check`) y la copia externa (`restic copy` a otro repositorio, p. ej. S3) las hace el agente en un proceso aparte, también como SYSTEM, con su propio bloqueo.
- Las credenciales de la nube (ID de clave y clave secreta) y la contraseña del destino se guardan con los demás secretos del agente, en la carpeta privada y cifradas con DPAPI. restic las recibe por variables de entorno del proceso, nunca como argumentos; las variables de nube heredadas del sistema (`AWS_*`, `B2_*`, `AZURE_*`) se ignoran.
- Configurar o cambiar estas tareas exige administrador y la contraseña del repositorio.
- **Recomendación:** la clave de la nube que usa el agente no debería poder borrar versiones antiguas (versionado del bucket con una clave sin permiso de borrado definitivo, u *Object Lock*). Así, aunque alguien tome el equipo, lo que ya se subió no se puede eliminar. La retención en el destino solo «oculta» copias y el proveedor las borra según su regla de ciclo de vida.

## Resguardo Web (opcional)

Si vinculas el equipo con Resguardo Web, el agente envía por HTTPS (certificado verificado con el almacén de Windows) un informe con **metadatos**: nombre y tipo de cada repositorio, servidor sin credenciales, horario, fechas, duraciones, tamaños, número de archivos y el resultado de la última copia. Los mensajes de error se envían sin rutas locales. **Nunca** se envían contraseñas, nombres de archivos ni el contenido de las copias. El equipo se identifica con un secreto propio del que la web solo guarda la huella SHA-256; desvincularlo desde la web lo revoca.
