# Guía de instalación paso a paso

Para montar Resguardo en una oficina, de cero a la primera restauración de prueba. Vale para **Resguardo Server 0.7.10** y **Resguardo Agente 0.7.10**.

El esquema habitual:

- **Una máquina con la consola y el almacén.** Lleva **Resguardo Server** (la consola web, que solo coordina) y **Resguardo Agente** en modo **«Este equipo guarda copias»**: recibe las copias de los demás equipos en un disco de datos, sin que ninguno pueda borrarlas. Puede ser:
  - un **Windows** (un PC dedicado o un servidor), o
  - un **contenedor (CT) de Proxmox** con Debian o Ubuntu.
- **Los demás equipos** (Windows o Linux) con **Resguardo Agente**, que copian en el almacén.
- Fuera de la oficina: el **espejo** del almacén en otro disco y en una nube, y los **kits** impresos.

En los ejemplos, la máquina de la consola se llama `ALMACEN` (IP `192.168.1.20`), el cliente es «la oficina» y un equipo que copia es `SRV-APP`. Cambia los nombres por los tuyos.

Más detalle para Linux: [servidor-linux.md](servidor-linux.md) y [agente-linux.md](agente-linux.md).

> **Instaladores sin firmar.** Mientras no haya llave de publicación, los instaladores de Windows no están firmados (SmartScreen: «Más información → Ejecutar de todas formas») y los paquetes de Linux se instalan con `--sin-firma`. Comprueba siempre sus huellas SHA-256 con `SHA256SUMS` antes de instalar.

---

## 0. Antes de empezar

1. Ten los archivos de la versión:
   - Windows: `Resguardo-Server_0.7.10_x64-setup.exe`, `Resguardo-Agente_0.7.10_x64-setup.exe` y `SHA256SUMS`;
   - Linux: `resguardo-server-x86_64-linux-musl.tar.gz`, `resguardo-agente-x86_64-linux-musl.tar.gz` (o los `.deb`), `instalar-servidor.sh`, `instalar-agente.sh` y `SHA256SUMS`.
2. Comprueba las huellas:
   - en Windows (PowerShell, en la carpeta): `Get-FileHash .\*.exe -Algorithm SHA256 | Format-Table Hash, Path` y compara cada una con su línea de `SHA256SUMS`;
   - en Linux: `sha256sum -c --ignore-missing SHA256SUMS` (debe decir `OK` o «La suma coincide»).

   Si alguna no coincide, **no instales**.
3. Ten a mano un **móvil** con una aplicación de verificación (Google Authenticator, Microsoft Authenticator, Aegis, 1Password…), un **gestor de contraseñas**, una **impresora** (o «Guardar como PDF») y una **carpeta física** para guardar bajo llave los papeles que irás imprimiendo.
4. Dale a la máquina de la consola una **IP fija** (reserva DHCP): los equipos la buscan siempre en la misma dirección.

---

## 1. Instalar Resguardo Server

### 1.A En Windows

1. Ejecuta `Resguardo-Server_0.7.10_x64-setup.exe` como administrador.
2. En **Opciones**:
   - **Puerto de la consola (HTTPS):** `8443`, salvo que esté ocupado.
   - **«Solo redes internas (recomendado)»**: marcado. Crea una regla del cortafuegos de Windows solo para ese puerto, abierta a los equipos de la empresa (también de otras subredes o VLAN) y nunca a internet. No se abre nada en el router.
   - **«Este equipo también guarda copias (instala Resguardo Agente)»**: márcalo si esta máquina va a ser el almacén. Instala el agente sin vincular y sin icono en la bandeja; se vincula después desde la consola (paso 4).
3. La página **Listo** muestra la dirección (`https://ALMACEN:8443/`) y el **código de primer arranque**, que sirve una vez. Mientras no se use, también está en `C:\ProgramData\Resguardo Server\codigo-arranque.txt` (solo administradores).
4. En **Servicios** (`services.msc`): **Resguardo Server** (y, si lo marcaste, **Resguardo Agente**) en ejecución y con inicio automático.

Los datos de la consola están en `C:\ProgramData\Resguardo Server` y su registro en `servidor.log`. Ahí queda también, como mucho una vez por minuto, cada límite de peticiones que salta («Límite de intentos superado desde IP: cuenta…», ver [capacidad](capacidad.md)) y, al arrancar, si el puerto aún estaba ocupado por el proceso anterior (se espera hasta 10 s a que lo suelte). `Restart-Service ResguardoServer` para el servicio ordenadamente (deja de escuchar y termina) antes de volver a arrancarlo.

### 1.B En un CT de Proxmox (Debian o Ubuntu)

Resumen; la guía completa es [servidor-linux.md](servidor-linux.md).

1. CT con **systemd** y **Anidamiento** (nesting) activado. Para el almacén, un **punto de montaje** con el disco de datos (por ejemplo `/mnt/datos`), nunca el disco raíz del CT.
2. Copia al CT (`scp` o `pct push`) los paquetes del servidor **y** del agente, los dos scripts y `SHA256SUMS`, y como root:

   ```sh
   cd /root
   sha256sum -c --ignore-missing SHA256SUMS
   sh instalar-servidor.sh --paquete ./resguardo-server-x86_64-linux-musl.tar.gz --sin-firma --con-agente ./resguardo-agente-x86_64-linux-musl.tar.gz
   ```

   `--con-agente` (con `instalar-agente.sh` en la misma carpeta) instala también el agente, **sin vincular** (es el equivalente de «Este equipo también guarda copias»). Para otro puerto, `--puerto 9443`.
3. El script termina con la **dirección de la consola**, la **huella de la autoridad TLS** y el **código de primer arranque**. Para volver a verlos: `sudo resguardo-server codigo-inicial`.
4. **El cortafuegos:** el instalador de Linux no lo toca. Abre el 8443 (y después el puerto del almacén) solo a la red de la oficina, con el cortafuegos de Proxmox (*CT → Cortafuegos*) o como explica [servidor-linux.md](servidor-linux.md#cortafuegos).

> **Si algo falla**
> - Windows: «el puerto está ocupado» o «No se pudo instalar el servicio»: vuelve atrás y elige otro puerto.
> - Linux: el servicio falla con `226/NAMESPACE`: activa el anidamiento del CT y reinícialo. `No se pudo escuchar en 0.0.0.0:8443`: otro programa usa el puerto (`ss -ltnp | grep 8443`).
> - Si el agente no se instaló con la consola, instálalo aparte (Windows: `Resguardo-Agente_0.7.10_x64-setup.exe`, dejando el código en blanco; Linux: `sh instalar-agente.sh --paquete … --sin-firma`) y vincúlalo con «Vincular este servidor» (paso 4).

---

## 2. Abrir la consola y crear la cuenta de propietario

1. Desde el navegador de otro equipo de la red: `https://192.168.1.20:8443/`.
2. El navegador avisa de que la conexión no es privada: el certificado lo firma una autoridad propia del servidor. **Comprueba la huella antes de aceptar**: en el navegador, el aviso → certificado → **Resguardo Server** (la de arriba) → **SHA-256**; en la máquina, `codigo-inicial` (Windows, en una consola de administrador: `"C:\Program Files\Resguardo Server\resguardo-server.exe" codigo-inicial`; Linux: `sudo resguardo-server codigo-inicial`). Si coincide, continúa. Si no, para.
3. **«Bienvenido a Resguardo Server»**: código de primer arranque, tu nombre, correo y una contraseña de al menos 12 caracteres → **Crear la cuenta**.
4. **«Activa la verificación en dos pasos»** (obligatoria): escanea el QR, escribe el código de 6 cifras → **Activar y entrar**.
5. **«Códigos de recuperación»**: 10 códigos que **no se vuelven a mostrar**. **Imprimir** o **Descargar**, guárdalos bajo llave, marca la casilla → **Seguir**.

Para no ver el aviso del certificado en ese PC, importa la autoridad (`tls\ca.crt` en la carpeta de datos, o `https://<servidor>:8443/api/servidor/ca`) como «Entidad de certificación raíz de confianza».

> **Si algo falla**
> - El código de primer arranque no vale: cambia en cada arranque del servicio mientras no haya cuenta; vale el último (`codigo-inicial`).
> - El código de 6 cifras no entra: revisa la hora del móvil y la del servidor.
> - Alguien de tu equipo perdió el móvil **y** sus códigos: **Personas y ajustes** → su menú → **Restablecer verificación en dos pasos**. Confirmas con un código de tu aplicación y te da un **código de un solo uso** (24 h) para que vincule su móvil nuevo al entrar. Pásaselo en persona o por teléfono. La de un propietario solo la restablece el propietario del servidor.

---

## 3. Crear el cliente

Tras los códigos se abre **Nuevo cliente** (o **Clientes → Nuevo cliente**).

1. **Nombre:** la empresa u oficina.
2. **Espera antes de borrar:** **24 horas (recomendado)**. Es lo que esperan las órdenes que pueden borrar o dejar sin copias (acortar la retención, quitar un repositorio, pausar…). Mientras esperan, se pueden cancelar en **Órdenes**.
3. **Crear cliente**. La consola te lleva a **Añadir equipo**.

La **clave de administración** del cliente se elige al dar de alta el **primer equipo** (paso 4). En **Estado**, **Primeros pasos** se va marcando solo.

---

## 4. El almacén: vincular la máquina de la consola

### 4.1 Vincular este servidor

1. En **Añadir equipo**, la tarjeta **«Este servidor también puede guardar copias»** → **Vincular este servidor**. (Solo sale si el agente está instalado en la misma máquina.)
2. El agente se une solo. La consola enseña un **número de comprobación** de 6 cifras.
3. En la máquina, como administrador, `resguardo-agente registro 5` (Windows: `"C:\Program Files\Resguardo Agente\resguardo-agente.exe" registro 5`; Linux: `sudo resguardo-agente registro 5`) muestra el número con el que se vinculó. Compáralo cifra a cifra → **Coincide** (o **No coincide**, y no sigas).

### 4.2 La clave de administración

Como es el primer equipo, la consola pide **«Elige la clave de administración»**:

- autoriza todo lo que cambia la protección (qué se copia, dónde y cuándo, altas, pausas, mover el cliente);
- **el servidor nunca la ve** y no se puede recuperar.

1. **Generar una clave segura** (o una propia de al menos 16 caracteres), repítela.
2. Guárdala en el **gestor de contraseñas y en papel bajo llave**. Marca «La he guardado en un sitio seguro».
3. **Dar de alta** → «Alta hecha».

Si se pierde, los equipos siguen copiando, pero para cambiarlos habrá que desvincular cada uno y volver a emparejarlo con una clave nueva.

### 4.3 «Este equipo guarda copias»

1. En **Listo**, **Usarlo como almacén** (o, en la ficha del equipo, tarjeta «Este equipo puede guardar copias» → **Este equipo guarda copias**).
2. **Carpeta o disco donde guardar:** **Explorar…** y elige una carpeta del disco de datos (Windows: `D:\Resguardo`; Linux: `/mnt/datos/resguardo`). No vale la del sistema ni la de los programas.
3. **Puerto:** la consola propone uno libre (8000 si lo está; si no, otro como 8002). No puede ser el de la consola.
4. **Solo redes internas:** marcado. En un CT sin privilegios donde nftables no esté permitido, desmárcalo y limita el puerto con el cortafuegos de Proxmox.
5. **Clave de administración** → **Confirmar con la clave de administración**.

El agente arranca el rest-server oficial de restic en modo **solo añadir**, con un usuario y una carpeta por equipo, certificado propio y una regla del cortafuegos solo para ese puerto. La ficha muestra la tarjeta **Guarda copias**.

> **Si algo falla**
> - «El puerto ya está en uso»: elige otro.
> - «Falta nftables» / «No se pudieron aplicar las reglas de nftables» (Linux): `apt install nftables`, o desmarca «Solo redes internas» y usa el cortafuegos de Proxmox.
> - El almacén puede estar en una máquina distinta de la consola: instala allí solo el agente, añádelo como un equipo más (paso 5) y pulsa **Este equipo guarda copias** en su ficha.

---

## 5. Añadir los demás equipos

En **Equipos → Añadir equipo**, elige el sistema.

### 5.1 Windows: instalador listo (recomendado)

1. **Descargar instalador listo**. **Nombre del equipo** (así saldrá en la consola) y **«Los equipos llegarán a este servidor en»**: la dirección con la que **ese equipo** llega a la consola, con `https://` y el puerto (por ejemplo `https://192.168.1.20:8443`). Si la que sale no vale, **Cambiar**.
2. **Descargar instalador listo**. El `.exe` lleva dentro la dirección, la huella del certificado del servidor y un código de **un solo uso que caduca en 24 h**: trátalo como una llave temporal. (Por eso su huella no coincide con `SHA256SUMS`.) Si no lo usas, **Anular** en **Preparados**.
3. En el equipo, ábrelo **como administrador**. Se vincula solo y termina en **«Comprueba el número»**.
4. En la consola, **Preparados** → «Se ha unido» → **Comprobar y dar de alta**. Compara el número cifra a cifra → **Coincide** → **clave de administración** (la misma del paso 4) → **Dar de alta**.

Este botón solo sale con Resguardo Server para Windows (es el que trae el instalador del agente). En un servidor Linux, usa el código (5.2).

### 5.2 Windows: instalador normal y un código

1. **Instalador normal y un código** → **Generar el código** (vale 15 min).
2. En el equipo, `Resguardo-Agente_0.7.10_x64-setup.exe` como administrador. En **«Emparejar con la consola»**: el código y la **dirección de Resguardo Server** (`https://192.168.1.20:8443`). **No la dejes vacía**: vacía, intenta emparejarse con la app de escritorio antigua.
3. El instalador enseña el número. En la consola, compáralo → **Coincide** → clave de administración.

Para muchos equipos, sin ventanas: `Resguardo-Agente-setup.exe /S /CODE=ABCD-EFGH-JK /SERVIDOR=https://192.168.1.20:8443 [/TRAY=0]` (el número queda en `emparejamiento.txt`, junto al programa).

### 5.3 Linux (Debian, Ubuntu o un CT)

1. Escribe el **nombre** → **Preparar la línea**. Te da una línea con un código (24 h) y la **huella del certificado** del servidor.
2. Instala el agente ([agente-linux.md](agente-linux.md)) y pega la línea como root:

   ```sh
   sudo resguardo-agente vincular <código> --servidor https://192.168.1.20:8443 --huella-ca <huella>
   ```

   Con `--huella-ca`, el agente comprueba el certificado antes de enviar nada.
3. En la consola, **Preparados → Comprobar y dar de alta**, compara el número y confirma con la clave.

### 5.4 El número de comprobación y la huella

El número lo calculan por separado la consola y el equipo. Con agentes 0.7.7 o posteriores incluye la **huella del certificado del servidor**: si coincide, el equipo habla con tu consola y nadie se ha puesto en medio. Con un agente antiguo, la consola avisa y enseña la huella para compararla también a mano. Si algo no coincide, **No coincide** y revisa la red antes de seguir.

> **Si algo falla**
> - «No se pudo vincular con los datos de este instalador»: el equipo no llega al servidor (abre la dirección en su navegador), o el código caducó o ya se usó. Escribe un código nuevo en la misma pantalla del instalador.
> - «El código no es válido o ha caducado» (Linux): pide uno nuevo.
> - «Esa no es la clave de administración de este cliente»: usa la del paso 4.2.

---

## 6. Repositorios: dónde se guardan las copias

Un **repositorio** es el sitio cifrado donde se guardan las versiones. Su **contraseña** la genera la consola, viaja sellada solo para el equipo y sale en su **kit de recuperación** para imprimir. Sin ella nadie puede leer las copias.

1. **Repositorios y destinos → Nuevo repositorio → Crear uno nuevo.**
2. **Equipo** y **Nombre** (por ejemplo «Contabilidad» o «Archivos»).
3. **Destino:** **Almacén ALMACEN (recomendado)**. No hay que escribir dirección ni contraseñas: el equipo tendrá allí su usuario y no podrá borrar lo ya copiado. **Seguir**.
4. **Clave de administración** → **Confirmar**. El almacén da acceso al equipo (sellado: el servidor no lo ve).
5. **Kit de recuperación:** **Imprimir o guardar en PDF**, guárdalo bajo llave y la contraseña en el gestor. Marca la casilla → **Crear el repositorio en …**.

Para un segundo repositorio del mismo equipo, el almacén sale como **«… · almacén de la oficina»** en **Destino**; después, el kit y la clave.

Otros destinos (**Un destino nuevo…**): un disco o carpeta del equipo (con **Explorar…**), otro servidor de copias, S3 o Backblaze B2. **Usar uno que ya existe** añade un repositorio que ya tenía el equipo (por ejemplo, el de la app de escritorio) con su contraseña.

> **Si algo falla**
> - En **Destino** no sale el almacén: el paso 4.3 no terminó.
> - «… no ha respondido todavía»: el almacén y el equipo tienen que estar encendidos y conectados.

---

## 7. Las copias: carpetas, horario y «Antes de copiar»

En la ficha del equipo → **Añadir o cambiar copias** → **clave de administración** → **Abrir las copias**. Las carpetas viajan y se guardan cifradas: solo se ven con la clave.

1. **Añadir una copia** y ponle nombre.
2. **Carpetas:** **Elegir en el equipo** (navega por sus carpetas) o escríbelas, una por línea.
3. **No copiar:** una regla por línea. Ya trae `*.tmp`, `~$*` y `Thumbs.db`.
4. **Cuándo:** reglas que se suman.
   - **A estas horas**: días (con atajos **Lun–Vie**, **Lun–Sáb**, **Todos los días**) y horas (**+ Hora**).
   - **Cada cierto tiempo**: cada 5, 10, 15, 20 o 30 minutos, o cada 1, 2, 3, 4, 6, 8 o 12 horas, en una franja (**de … a …**).
   - **Cada N días** (desde un día) y **Un día de cada mes** (o el último).
   - **Añadir otra regla** para combinarlas. Debajo, el resumen en frase y cuántas copias salen al día.
5. **Solo guardar si hay cambios:** encendido, una copia sin cambios no guarda versión nueva. Imprescindible con horarios de cada pocos minutos u horas.
6. **Repositorio:** el del paso 6.
7. **Antes de copiar (opcional):**
   - **Volcar bases de datos de SQL Server antes de copiar**: bases (una por línea), instancia (`.` es la predeterminada) y carpeta para los volcados. Cada base se vuelca con `COPY_ONLY`, entra en la copia y se borra. La cuenta del equipo (`NT AUTHORITY\SYSTEM`) necesita el rol **db_backupoperator** en cada base.
   - **Avisar si la carpeta de copias de una aplicación lleva más de N horas sin archivos nuevos**: para programas que hacen sus propias copias. Solo avisa.
8. **Enviar al equipo** (no debe salir «Antes de enviar: …»).

Las reglas de minutos, «cada N días» y «un día de cada mes» piden el agente 0.7.9 o posterior; «Solo guardar si hay cambios» (apagable), el 0.7.7; «Antes de copiar», el 0.7.2. Con uno anterior, la consola lo dice y pide actualizarlo.

En el menú de cada copia, **Guardar como plantilla…** guarda la configuración (cifrada) para usarla en otros equipos del cliente (**Desde una plantilla**).

> **Si algo falla**
> - El volcado falla por «permiso» o «inicio de sesión»: falta el rol db_backupoperator. Por «acceso denegado» al escribir: el servicio de SQL Server no puede escribir en la carpeta (mejor deja que la cree el agente).
> - «Las llaves de un equipo han cambiado»: no escribas contraseñas para ese equipo. Si sabes que se reinstaló, **Detalles → Volver a comprobar** con la clave; si no, revisa la **Actividad**.

---

## 8. «Copiar ahora» y el progreso en directo

1. En la ficha del equipo, el botón ▶ de una copia (o **Copiar ahora** arriba). No pide claves: no borra nada.
2. El diálogo sigue la orden («Enviada», «El equipo está en ello…») y después **Copiando**, con porcentaje, fase («Antes de copiar», copiando…), archivos, bytes, velocidad y tiempo restante.
3. Puedes cerrar: sigue en el equipo, la barra lateral muestra «1 copia en marcha» y la consola avisa al terminar con el resultado: **Correcta**, **Con avisos** o **Fallida** (con el motivo).

> **Si algo falla**
> - «No se pudo llegar al destino»: el almacén está apagado, sin red o con el puerto cerrado.
> - «Algunos archivos no se pudieron leer»: estaban abiertos; entran en la próxima copia.
> - «Repositorio bloqueado»: **Detalles → Quitar bloqueos antiguos**.

---

## 9. La retención la aplica el almacén

El almacén es de **solo añadir**: los equipos no pueden borrar allí, ni siquiera sus versiones antiguas. Por eso la retención la aplica **el propio almacén**, en local y a su hora.

**El precio:** para podar, el almacén recibe una **clave propia de restic** de cada repositorio (la añade el equipo dueño). Con esa clave, **el almacén también puede leer ese repositorio**. Sin retención, el almacén solo guarda datos que no puede abrir, pero el repositorio crece sin fin. Decide con el cliente y protege bien la máquina del almacén. **Dejar de aplicarla** borra esa clave del repositorio.

1. Página del repositorio → **Comprobaciones → Retención en el almacén…** (o, en la ficha del equipo, **Más… → Retención en ALMACEN…**).
2. **Qué versiones guarda:** una regla de un clic o **A medida** (por ejemplo 30 · 26 · 0 · 0, o 7 · 4 · 12 · 2 diarias, semanales, mensuales y anuales). Debajo, la regla en palabras y cuántas versiones quedan más o menos.
3. **Cuándo la aplica:** días y hora del almacén, fuera de las horas de copia (mientras poda, las copias de ese repositorio esperan).
4. **Comprobar el repositorio después**: marcado (verifica tras cada poda).
5. **Contraseña del repositorio** + **clave de administración** → **Confirmar con las dos claves**. El equipo dueño tiene que estar encendido.
6. Borra versiones: **espera** el plazo del cliente (cancelable en **Órdenes**) y después se aplica sola. La tarjeta **Guarda copias** del almacén muestra cada retención con su resultado; en la página del repositorio, **Aplicar ahora** la adelanta.

En la 0.7.10 la regla solo cuenta diarias, semanales, mensuales y anuales: no hay «cada hora durante N días», y las versiones de varias veces al día duran hasta la siguiente poda. Con un agente posterior (en el almacén y en el equipo dueño) hay plazos y «siempre»: **Programas contables: horarias 15 días, diarias 1 año, mensuales siempre** (lo recomendado para los datos de un programa contable), **Diarias 30 días, semanales 6 meses**, o a medida. Con un agente anterior, la consola lo dice («Actualiza el agente»).

---

## 10. El espejo: otro disco y una nube

Cada noche, el almacén copia **todo lo que guarda** (ya cifrado) a otros destinos. **Solo añade**: nunca borra allí (lo que la retención poda sigue en el espejo, que por eso crece más que el almacén) y no necesita contraseñas.

### 10.1 Otro disco

1. Ficha del almacén → **Guarda copias → Espejo de lo que guarda** → **Otra carpeta**.
2. **Explorar…** → una carpeta **en otro disco** (Windows: `E:\Resguardo-espejo`; Linux: `/mnt/disco2/espejo`). No puede estar dentro de la del almacén.
3. **Cada noche a las** `02:00` → clave de administración → **Confirmar**.

### 10.2 Dropbox

1. **Guarda copias → Más acciones → Conectar Dropbox…** → nombre (por ejemplo «Dropbox Oficina») → **Abrir Dropbox**.
2. En Dropbox: la cuenta de la oficina → **Permitir** → copia el **código**.
3. En la consola: pega el código, clave de administración → **Conectar**. El permiso (solo para **Aplicaciones/Resguardo**) se guarda sellado **solo en el almacén**.
4. **Añadir destino del espejo** → **Nube** → la nube conectada, **Carpeta dentro de la nube** (relativa a Aplicaciones/Resguardo, por ejemplo `oficina`) y, si quieres, **Límite de subida** en KiB/s → **Confirmar**.

Dropbox **no es inmutable**: quien tenga la cuenta o el permiso puede borrar. Activa su verificación en dos pasos y, si se pierde el almacén, quita **Resguardo** en **Aplicaciones conectadas** de la web de Dropbox. Cuando se pueda, mejor un destino con bloqueo de objetos (Backblaze B2 con Object Lock).

> **Si algo falla**
> - «La app «Resguardo» de Dropbox aún no está configurada en este servidor»: falta su clave en Resguardo Server.
> - Un destino sale **Falló**: **Qué hacer** lo explica. Un fallo no borra nada; la noche siguiente lo vuelve a intentar.
> - Permiso caducado o revocado: vuelve a conectar la nube con el **mismo nombre**.

---

## 11. Verificación, prueba de restauración y copia externa

- **Verificar** (en cada repositorio): comprueba ahora una parte de lo guardado. No pide claves.
- **Más… → Probar la restauración**: restaura unos archivos al azar a una carpeta temporal y los compara.
- **Salud de la protección** (página del repositorio): siete comprobaciones (copias automáticas, protección contra borrado, copia externa, verificación, prueba de restauración, kit y retención) con lo que falta.
- **Verificación automática** (agente posterior a la 0.7.10; **Cambiar las copias → Verificación automática**, o **Verificar automáticamente…** en la página del repositorio): cada N días, un porcentaje de los datos que va rotando (con 10 %, todo en 10 comprobaciones); la primera, a las 03:00 siguientes. En un almacén con retención, el almacén además comprueba la estructura tras cada poda; la página del repositorio enseña las dos.
- **Copia externa** (**Más… → Copia externa…**): cada día, el equipo dueño copia las versiones del repositorio a otro destino (otro disco, servidor de copias, SFTP, S3 o B2), con su retención y, si quieres, otra contraseña (apúntala en el kit). Pide la contraseña del repositorio. Si el repositorio está en un almacén con espejo, el espejo suele bastar.

---

## 12. Copia de la consola

Cada noche, el servidor guarda una copia cifrada de sí mismo (cuentas, clientes, equipos, historial, identidad y certificado). Con ella se restaura en otra máquina **sin volver a vincular** los equipos.

1. **Servidor → Copia de la consola** (solo el propietario del servidor; también en **Primeros pasos**) → **Poner la clave de respaldo…**: una frase larga, **distinta de la clave de administración**. Hora y cuántas copias se guardan → **Seguir: ver el kit**.
2. **Kit de la copia de la consola**: **Imprimir o guardar en PDF**, guárdalo fuera de la máquina. No se vuelve a mostrar. → **Listo**.
3. **Hacer ahora** para la primera.
4. Para que salgan de la máquina: añade la carpeta `<datos>/respaldos` a una copia del agente de esa misma máquina (**Cambiar las copias → Añadir una copia**):
   - Windows: `C:\ProgramData\Resguardo Server\respaldos`;
   - Linux: `/var/lib/resguardo-server/respaldos`.

   Con la 0.7.10, el almacén no puede copiar en sí mismo: usa un repositorio en otro disco (**Un destino nuevo… → Disco o carpeta del equipo**) o en la nube. Con un agente posterior (el que admite «Copiar en este mismo almacén»), en la ficha del almacén, **Copiar en este mismo almacén**: el repositorio queda en su propio almacén (por localhost, solo añadir) y así entra en su espejo y en Dropbox.

**Restaurar** (en una máquina con la misma dirección, con el servicio parado): `resguardo-server restaurar-respaldo <archivo> --reemplazar` como administrador (Linux: con `sudo`) y la clave del kit. Al arrancar, los equipos vuelven solos. Detalles en [servidor-linux.md](servidor-linux.md#copia-de-la-consola).

---

## 13. Probar una restauración

1. **Restaurar** → equipo → repositorio → **contraseña del repositorio**.
2. **¿De qué momento?** → una versión. **Archivos** → marca uno pequeño → **Seguir**.
3. **¿Dónde los quieres?**:
   - **En el equipo, junto al original (Recomendado):** crea al lado una carpeta `Restaurado AAAA-MM-DD HHMM`. Nunca sobrescribe.
   - **En su sitio:** vuelve a donde estaba; lo que ya exista se deja como está salvo que marques **Reemplazar** (y entonces se pierde lo que haya).
   - **Descargar en este navegador:** hasta 500 MB; llega cifrado por el servidor, que no puede leerlo.
4. **Restaurar** y comprueba que el archivo se abre. Borra la carpeta «Restaurado…».

**Restaurar en otro equipo** (arriba en **Restaurar**): con el equipo original vivo, comparte su acceso; si ya no existe, escribe los datos de su kit.

---

## 14. Servidores de respaldo (opcional)

Hasta tres servidores a los que los equipos se van solos si este deja de responder unos días:

1. Aquí, **Servidor → Mover…** solo para copiar el **bloque del cliente**, y cancela.
2. En el otro servidor: **Clientes → Recibir un cliente** con ese bloque; después, en ese cliente, **Servidor → Dar una ficha…** con **365 días (para respaldo)** y copia su bloque.
3. Aquí: **Servidor → Servidores de respaldo → Configurar…**, pega el bloque, **Irse tras** N días y la clave de administración.

---

## 15. Qué guardar fuera de la máquina

| Qué | Dónde |
| --- | --- |
| Contraseña de tu cuenta | Gestor de contraseñas |
| Códigos de recuperación | Impresos, bajo llave |
| **Clave de administración** de cada cliente | Gestor **y** papel bajo llave |
| **Kit de recuperación** de cada repositorio | Impreso, bajo llave (contraseña también en el gestor) |
| **Kit de la copia de la consola** (clave de respaldo) | Impreso, bajo llave, y gestor |
| Cuenta de la nube del espejo (con dos pasos) | La guarda el cliente |
| Dirección de la consola y huella de su certificado | Gestor |

Mejor si una copia de esos papeles está **fuera de la oficina**: con el espejo en la nube y los kits se recupera todo aunque se pierda la oficina.

---

## 16. Actualizar más adelante

Se actualiza **instalando encima**; se conservan datos, cuentas, vinculaciones, copias y el servidor de copias. Primero la consola, después los agentes.

- **Windows:**
  1. `Resguardo-Server_<nueva>_x64-setup.exe` como administrador (mismo puerto; «Este equipo también guarda copias» puede quedar sin marcar si el agente ya está). Dirá «Este servidor ya tenía cuentas (actualización)».
  2. `Resguardo-Agente_<nueva>_x64-setup.exe` como administrador en cada equipo (también en el del almacén). No pide código.
- **Linux:** copia los paquetes nuevos y repite la instalación: `sh instalar-servidor.sh --paquete … --sin-firma` y `sh instalar-agente.sh --paquete … --sin-firma` (con `.deb`: `apt install ./…_<nueva>_amd64.deb`).

Con la copia de la consola puesta, el servidor hace una antes de abrir su base de datos con la versión nueva. «Actualizar el agente» desde la consola aún no está disponible: se hace en cada equipo. Después, comprueba en cada ficha la versión del agente y que sigue **Conectado**.
