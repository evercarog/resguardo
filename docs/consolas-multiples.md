# Varias consolas a la vez

Diseño de «un equipo gestionado desde varias consolas»: el mismo agente vinculado a la vez a **varios Resguardo Server**. Cada una funciona sola: si quitas una, las demás siguen igual.

**Es simétrico: «otra consola» es cualquier Resguardo Server**, no una especial «en línea». Valen todas las combinaciones en las que los equipos alcancen a la otra consola (por su dirección `https://` y su puerto):

- la consola del cliente en su oficina **y** tu servicio en línea para varios clientes;
- la consola del cliente A **y** la de otro cliente B, unidas por una VPN;
- dos consolas de la misma red local (p. ej. una principal y otra de reserva, o la de sistemas y la de un integrador);
- una consola de pruebas temporal (una máquina virtual) mientras se decide dónde vivirá la buena.

El código de conexión, la orden y la seguridad son los mismos en todos los casos; el servidor no distingue «local» de «en línea». Estado: implementado (contrato v1.35; ver [api-servidor.md](api-servidor.md), «Cambios»).

Documentos relacionados: [plataforma.md](plataforma.md) (modelo de seguridad, «Cambiar de servidor», «Recibir un cliente», servidores de respaldo), [agente-gestionado.md](agente-gestionado.md) («Si pierdes la consola») y [api-servidor.md](api-servidor.md).

---

## Resumen

- **La autoridad no la da el servidor, la da la clave de administración del cliente**, que comprueba el propio equipo. Por eso dos consolas que tienen el mismo cliente (la misma clave) pueden gestionar el mismo equipo sin pisarse: cada una es un mensajero más.
- **El equipo guarda una lista de vínculos** («consolas»): la de siempre y las que se añadan. Cada una tiene su dirección, su autoridad TLS y su identidad fijadas, sus credenciales en ese servidor, su propio contador de órdenes (`seq`), su `K_cfg`, sus servidores de respaldo y su último contacto.
- **Cada consola tiene su canal** (WebSocket o sondeo) y recibe todo: informes, resumen, configuración cifrada, historial y progreso en vivo. Las órdenes de cualquiera se comprueban **igual que hoy** (clave de administración o contraseña del repositorio, espera de lo destructivo con las reglas del equipo, `not_before`, caducidad, `nonce`).
- **Conectar una segunda consola** es una orden con la clave de administración, `anadir_consola`, con un **código de conexión** que da la otra consola. En la consola de origen: cliente → «Conectar también a otra consola…», pegar el código, comprobar la huella y escribir la clave una vez. Se manda a todos los equipos del cliente.
- **Mover un cliente a otra consola** («Mover a otra consola…»): conectar también a la otra, esperar a que cada equipo informe allí y «Dejar esta consola» (§2.3). Ningún equipo se queda sin consola en ningún momento.
- **Quitar una consola**: la orden `quitar_consola` desde cualquiera de ellas, «Dejar de gestionar este equipo» desde la propia consola (solo se va ella), o `resguardo-agente consolas quitar` en el equipo si esa consola ya no existe.

---

## 1. Qué cambia en el equipo

### 1.1 Los vínculos

Hasta ahora el equipo tenía **un** vínculo (`servidor.bin`, `Vinculo`): el servidor principal, sus servidores de respaldo y, mientras se vuelve a vincular con un código, un vínculo pendiente del alta (`adopcion`).

Ahora `Vinculo` es una **lista de vínculos**. Para no cambiar el formato en disco ni las ~30 funciones que ya usan los campos del vínculo:

- el **primero** («la consola principal») sigue en los campos de siempre (`url`, `ca_pem`, `identidad`, `cliente_id`, `equipo_id`, `secreto`…);
- los **demás** van en `otras: [Enlace]`, con los mismos campos.

Un `servidor.bin` anterior es, sin hacer nada, una lista de uno (`otras` falta = vacía). Al añadir la primera consola se escribe ya con `otras`. Un agente anterior que leyera un archivo nuevo ignoraría `otras` y seguiría solo con la principal (no se baja de versión el agente, pero no rompe).

**Qué es de cada vínculo y qué es del equipo:**

| De cada vínculo (`Enlace`) | Del equipo (compartido) |
|---|---|
| `id` (interno), `nombre`, `url`, `ca_pem`, `identidad` | Claves del equipo (`box_secret`, `sign_seed`, `sal_equipo`) |
| `cliente_id`, `equipo_id`, `secreto` en ese servidor | Verificador de la clave de administración |
| `k_cfg` y `sal_cliente` de ese servidor | Espera mínima de lo destructivo |
| `ultimo_seq` y `config_seq` | Repositorios, destinos y configuración |
| Intentos fallidos y bloqueos (`fallos`) | Los `nonce` ya vistos (para todas) |
| Cambio de servidor en curso (`cambio`) | `modo` (gestionado o local) |
| Servidores de respaldo y sus días | Vínculo pendiente del alta (`adopcion`) |
| Último contacto, desde cuándo, configuración pendiente de subir | Último cambio de configuración (`ultimo_cambio`) |

Por qué así:

- **`seq` por vínculo**: cada servidor numera sus órdenes desde 1. Un `seq` común obligaría a que los servidores se pusieran de acuerdo, que es justo lo que no pueden hacer.
- **`nonce` común**: una orden que un servidor reenviase a otro vínculo se rechaza por repetida aunque su `seq` cupiera allí. Además, el sobre va atado al cliente y al equipo de ese servidor.
- **`K_cfg` por vínculo**: `K_cfg = HKDF(Argon2id(clave, sal_cliente))` depende de la sal del cliente **en ese servidor**. Si el cliente se creó con otra sal en la otra consola (lo normal al conectarla con un código de conexión), la clave es la misma pero `K_cfg` no. El vínculo pendiente de un alta (`adopcion`) ya hacía esto.
- **Bloqueos por vínculo**: un servidor malicioso que mande pruebas falsas solo bloquea las órdenes que llegan por él, no las de la otra consola. Probar claves por varias consolas a la vez no ayuda a adivinar: la prueba de administración es una salida de Argon2id de 32 bytes, y las contraseñas de los repositorios siguen con 5 intentos cada 15 minutos y bloqueos que se duplican **por consola** (como mucho 5 consolas).

**Implementación.** Para trabajar con el vínculo `X`, el agente lo intercambia con el principal (los campos del principal van a `otras` y los de `X` a los de siempre), hace lo que toque con el código de siempre y lo vuelve a poner en su sitio. Todo bajo un cerrojo del proceso, leyendo el vínculo del disco justo antes y guardándolo justo después: dos consolas que mandan a la vez no se pisan.

### 1.2 Un canal por consola

El servicio arranca **un hilo por vínculo**, cada uno con su WebSocket (o su sondeo si no pasa), su reintento y sus registros. Si una consola cae, las demás no lo notan. Un hilo vigila la lista: al añadir una consola arranca su canal, y al quitarla el canal se cierra solo.

Por cada canal va **todo**: el informe, el resumen y la configuración cifrada con la `K_cfg` de esa consola, el historial que le falte (`POST /api/agente/historial`, con su propio punto de subida) y el progreso en vivo.

### 1.3 Órdenes de cualquiera, en el orden en que llegan

Cada orden se comprueba como hoy, con el contador de **su** vínculo. Los cambios de configuración (`config`, `crear_repositorio`, `cambiar_retencion`, `pausar`…) se aplican en el orden en que llegan, una cada vez (cerrojo). Después:

1. se sube al momento la configuración nueva a la consola que la mandó;
2. las demás quedan marcadas con «configuración pendiente» y su canal la sube enseguida (cifrada con su `K_cfg`). Así una consola apagada no frena a la que manda;
3. el resumen lleva `cambio_config: { tipo, cuando, consola: { nombre, url, identidad } }`: la otra consola enseña **«Cambiado desde otra consola (consola.ejemplo.com) · hace 3 min»**.

Si las dos consolas cambian la configuración a la vez, gana la última en llegar al equipo (como dos personas en la misma consola). La consola ya relee la configuración del equipo antes de editarla.

### 1.4 Servidores de respaldo y cambiar de servidor

Siguen siendo **de cada vínculo**: `servidores_respaldo` desde la consola en línea solo cambia el respaldo del vínculo con la consola en línea. `cambiar_servidor` desde una consola mueve **ese** vínculo; los demás no se tocan. No se puede cambiar a (ni añadir) un servidor que ya es uno de los vínculos: el agente lo rechaza.

### 1.5 Desvincular, dar de baja y «Dejar de gestionar»

- **`desvincular` «Seguir funcionando en local»** desde una consola **solo quita esa consola**. Si era la única, el equipo pasa a modo local como hasta ahora. En la consola es **«Dejar de gestionar este equipo»** cuando hay otras.
- **`desvincular` «Dejar de copiar»** y **`baja_equipo`** (destructivas, esperan) dejan de copiar en el equipo, que es uno solo: afecta a todas las consolas, y además quitan la que las manda. Las demás lo ven en el resumen («sin copias»).
- Si la consola que se quita es la principal, la siguiente pasa a serlo. Nada más cambia.

### 1.6 Sin servidor y «Si pierdes la consola»

Igual que hoy. Lo nuevo es que **tener dos consolas ya es el mejor respaldo**: si una muere, la otra sigue gestionando sin hacer nada. Los servidores de respaldo siguen siendo útiles para una consola que ha de seguir en marcha con otra dirección.

`resguardo-agente vincular <código>` (volver a vincular con un código) sustituye a la **principal**, como antes; las demás se conservan.

---

## 2. Conectar una segunda consola

### 2.1 Lo que ve la persona

**En la otra consola (la nueva)**, quien la administra pide un **código de conexión**:

- si el cliente aún no existe ahí (lo normal en tu consola en línea): **Clientes → Recibir un cliente → «Gestionarlo también desde aquí»**, escribe el nombre y pulsa «Crear el código». Crea el cliente con una sal nueva;
- si ya existe: en el cliente, **Servidor → «Dar un código de conexión»**.

El código es una línea (`RGC1.…`) que vale **7 días** (o lo que se elija) y lleva:

| Campo | Para qué |
|---|---|
| `url` | Dirección de la otra consola **para los agentes**: su `url_agentes` (v1.34; en una consola en internet, `https://agentes.<dominio>`, que sirve su autoridad TLS propia) o, si no la da, su dirección |
| `identidad` | Identidad Ed25519 que el equipo fijará y que el servidor demuestra en cada conexión |
| `huella_ca` | Huella SHA-256 de su autoridad TLS **propia** (la de `GET /api/servidor`, no la de Let's Encrypt): el equipo la descarga de `url` y la compara |
| `ficha` | Ficha de «Recibir un cliente» (un solo uso por equipo, con un tope de equipos y caducidad) |
| `sal_cliente` | La sal del cliente en esa consola (para calcular su `K_cfg`) |
| `nombre` | Cómo se llama esa consola, para enseñarlo |
| `caduca` | Hasta cuándo vale la ficha |

**En la consola de siempre**: cliente → **«Conectar también a otra consola…»**:

1. Pega el código. La consola enseña la dirección, la **huella corta de la identidad** y la de la **autoridad TLS** y pide comprobarlas de palabra con quien administra la otra («He comprobado la huella»).
2. Elige los equipos (todos, por defecto) y escribe la **clave de administración una sola vez**.
3. La consola calcula la `K_cfg` de la otra consola (con su sal) y manda `anadir_consola` a cada equipo, con una lista de progreso («Enviada», «Conectando…», «Hecho», o el motivo si falla). Los equipos desconectados la reciben al volver (la orden dura 7 días).
4. El cliente aparece en las dos. En cada equipo: **«También lo gestiona: Consola en línea (consola.ejemplo.com) · último contacto hace 2 min · Quitar»**.

### 2.2 La orden `anadir_consola`

(Desde cualquier consola hacia cualquier otra: el equipo solo necesita llegar a su dirección.)


Clave de administración, **no destructiva** (no reduce la protección) pero **sensible**: solo administradores y propietarios, el diálogo enseña la dirección y la huella, queda en la auditoría y avisa a todas las consolas al hacerse.

```json
{ "url": "https://consola.ejemplo.com", "identidad": "<Ed25519 b64>", "huella_ca": "AB:CD:…",
  "ficha": "…", "sal_cliente": "<b64>", "k_cfg": "<b64, 32 bytes>", "nombre": "Consola en línea",
  "sal_origen": "<sal del cliente en la consola que la manda>" }
```

El equipo (contesta `en_marcha` y sigue aparte):

1. Comprueba los campos, que no es ya una de sus consolas (por identidad) y que no pasa de **5 consolas**.
2. Descarga la autoridad TLS de esa dirección (sin comprobarla esa única vez) y exige que sea **un solo certificado con esa huella**. Desde ahí, la fija.
3. Se da de alta con `POST /api/agente/recibir`: la ficha, su mismo id, sus llaves públicas y su **etiqueta calculada con la `K_cfg` de la orden** (la prueba de que quien la mandó tiene la clave de administración: la otra consola la comprueba con la clave y su sal), `motivo: "anadir_consola"`.
4. Exige que la respuesta traiga **la identidad de la orden** y que el servidor la demuestre firmando un reto (`/api/agente/tomar`).
5. Guarda el vínculo nuevo; su canal arranca y sube la configuración (cifrada con esa `K_cfg`), el informe y **todo el historial**.
6. Avisa a **todas** sus consolas (`cambio_inusual`, también en su historial): «Este equipo se conectó también a otra consola: consola.ejemplo.com (identidad AB:CD:…)». Y contesta `hecha`.

Si algo falla, `fallida` con el motivo («ficha no válida o caducada», «no tiene la identidad que dice la orden»…), y no queda nada a medias en el equipo.

### 2.3 Mover un cliente de una consola a otra («Mover a otra consola…»)

Para irse del todo a otra consola (por ejemplo, de la máquina virtual de pruebas a la consola definitiva, o de una oficina a otra) sin que ningún equipo se quede nunca sin consola. En la consola de siempre: cliente → **Servidor → «Mover a otra consola…»**:

1. **Conectar también a la otra**: se pega su código de conexión, se comprueban las huellas y se escribe la clave de administración. Es `anadir_consola` a cada equipo que aún no la tiene (los que ya la tienen se saltan).
2. **Esperar a que cada equipo informe allí**: la lista enseña, equipo a equipo, «Conectando…», «Conectado: esperando su primer informe allí…» y «Ya informa en …» (su resumen lista la otra consola con su último contacto). Se puede cerrar y volver: no hay prisa.
3. **«Dejar esta consola»**: con la clave otra vez, `quitar_consola` **de esta misma** en cada equipo que ya está allí. El equipo contesta `hecha` con `detalle: {"deja_esta_consola": true, "siguen": […]}` (firmado) y esta consola lo marca como `modo: "local"` (ya no se gestiona desde aquí; su historial se queda). El agente nunca quita la única consola que tiene: si la otra no está, la orden falla y el equipo sigue aquí.
4. **Comprobación final**: «Listo: N equipos ya solo están en …», o cuáles siguen aquí y por qué (agente anterior, orden fallida); se puede repetir con esos.

Los equipos con un agente anterior (sin `admite: consolas_multiples`) no pueden hacerlo así: se actualizan antes o se usa «Mover…» (`cambiar_servidor`, que va en una sola orden).

**Si es la propia máquina la que cambia** (la misma consola, en otro hardware, con la misma dirección), no hace falta nada de esto: se restaura la **«Copia de la consola»** en la máquina nueva (`resguardo-server restaurar-respaldo …`, [api-servidor.md](api-servidor.md) §2 y «Si pierdes la consola» en [agente-gestionado.md](agente-gestionado.md)): tiene la misma identidad y los equipos vuelven solos. «Mover a otra consola» es para cambiar a **otro** servidor (otra identidad, otra dirección).

### 2.4 Quitar una consola

- **`quitar_consola { identidad }`** (clave de administración, solo administradores): desde cualquier consola, quita **otra** (o la propia, si quedan más: es el paso 3 de «Mover a otra consola»). La que se quita recibe un último aviso («Este equipo dejó de conectarse a esta consola») y las demás otro. Quitar la única no se puede: es «Desvincular».
- **«Dejar de gestionar este equipo»** en la propia consola: `desvincular` en «seguir en local», que con otras consolas solo la quita a ella (1.5).
- **En el equipo**, como administrador, si esa consola ya no existe:

  ```text
  resguardo-agente consolas                  # la lista, con su huella y último contacto
  resguardo-agente consolas quitar <n.º | dirección | huella>
  ```

### 2.5 Equipos que no están en todas las consolas

Un equipo que se añade en una consola **solo llega a esa**: las demás no lo conocen hasta que se conecta también a ellas (es lo seguro: hace falta la clave de administración y un código de conexión de la otra consola). Para que no se quede uno fuera sin darse cuenta (`docs/plan-mejoras.md`, tarea 2), cada consola lo deduce de lo que ya sabe:

- **Qué otras consolas tiene el cliente**: la unión de las `consolas` del resumen de sus equipos (nombre, dirección e identidad; nada secreto), agrupadas **por identidad**. No se sugieren las abandonadas: sin contacto de ningún equipo en 30 días, o añadidas hace más de 7 y nunca contactadas. Código: `consola/src/lib/consolasCliente.ts`.
- **Qué equipos faltan en cada una**: los gestionados desde aquí y confirmados que no la tienen en su lista. Un equipo sin resumen todavía no cuenta (no se sabe), salvo el que se acaba de dar de alta. Uno con un agente anterior (sin `consolas_multiples`) cuenta, con «necesita actualizar el agente».
- **Dónde se ve**:
  - al terminar de dar de alta un equipo y en su ficha: «Este equipo solo está en esta consola; los demás también están en «X»» → **Conectar también…**;
  - en Estado: «N equipos no están en todas tus consolas», por consola y con los equipos que faltan → **Conectar también…** con todos ellos a la vez.
- **El botón abre el flujo de siempre** (§2.1) con los equipos ya elegidos y la consola esperada: al pegar el código, dice si es de esa consola (misma identidad que ya tienen fijada los demás equipos) o avisa si es de otra. La comprobación de las huellas de palabra no se quita.
- **Repetirlo no hace daño**: si un equipo ya tiene esa consola (el resumen iba atrasado), el agente contesta `fallida` con «Esa consola ya gestiona este equipo.» antes de tocar nada (ni la ficha ni el vínculo), y la consola lo enseña como «Ya estaba conectado».

Sin cambios en el contrato: todo sale del resumen que ya mandan los agentes (v1.36).

---

## 3. El servidor

**Nada en el servidor supone ser la única consola.** El equipo es, para cada servidor, un equipo más del cliente:

- `POST /api/agente/recibir` acepta el equipo aunque esté gestionado en otro sitio (lo hacía ya para `cambiar_servidor`). Con `motivo: "anadir_consola"`, el aviso dice «se conectó también a esta consola». Un servidor anterior ignora el campo.
- Cada servidor guarda solo lo suyo (su cliente, su equipo, sus órdenes); el resumen que sube el equipo lleva la lista `consolas` (dirección, nombre, identidad, sal y último contacto de cada una), que el servidor guarda tal cual.
- `POST /api/agente/config` acepta además `etiqueta` (la que el equipo calcula con la `K_cfg` de ese servidor) y `espera_min_horas`. Así, tras **cambiar la clave de administración** o **la espera** desde una consola, la otra queda al día sin que nadie haga nada. Un servidor anterior ignora los dos campos.
- Las órdenes nuevas (`anadir_consola`, `quitar_consola`) solo las pueden mandar administradores y propietarios, como `cambiar_servidor`.
- **Multicliente**: el equipo entra en el cliente de la ficha y en ningún otro (`ClienteCtx`). Si ese id ya está en otro cliente del mismo servidor, 409, como ya pasaba.

---

## 4. Seguridad

### 4.1 Qué puede y qué no puede una segunda consola maliciosa

Una consola (un servidor) que el cliente conectó y que resulta ser maliciosa, o que comprometen después, es exactamente **un servidor comprometido** (plataforma.md, 3.2):

| Puede | No puede |
|---|---|
| Ver metadatos del equipo: nombre, sistema, resumen (nombres de copias y horarios, destinos sin rutas), informes, historial y **la lista de las otras consolas** (dirección, nombre, identidad y sal) | Leer la configuración (cifrada con `K_cfg`), las copias, las contraseñas ni las rutas |
| Mandar órdenes **inofensivas** (copiar ahora, verificar…), con sus límites | Mandar nada estructural o de datos sin la clave de administración o la contraseña del repositorio |
| No reenviar las órdenes de su propia consola (denegación de servicio **de esa consola**) | Retener o retrasar las órdenes de **otra** consola (van por otro canal) |
| Bloquear, con pruebas falsas, las órdenes protegidas **que llegan por ella** | Bloquear las de las demás consolas (bloqueos por vínculo) |
| Repetir un sobre suyo | Repetirlo en otra consola (`nonce` común, sobre atado a su cliente y su equipo) |
| Servir código malicioso **a quien entre en esa consola** y capturar la clave si se escribe allí (el límite honesto de 3.1) | Capturarla si solo se escribe en la otra consola |
| Saber que el cliente tiene otra consola y dónde | Quitar la otra consola o añadir una tercera: son órdenes con la clave de administración |

**Lo que añade tener dos consolas** (y por eso se pide la clave para conectarlas y se avisa a todas):

- Cada consola es **otro sitio donde se puede escribir la clave**. Una consola en línea que no es tuya ve los metadatos de los equipos del cliente y puede servir código a quien la usa: conecta solo consolas en las que confíes tanto como en la tuya.
- **Ninguna consola puede ocultar a las demás que existe**: el equipo lista todas en su resumen, avisa a todas al añadir o quitar una y lo anota en su historial, que también reciben todas.

### 4.2 La clave de administración cambia en una consola

El verificador es **del equipo** (uno), así que tras `cambiar_clave_admin` desde cualquier consola vale la clave nueva para todas. Lo que hay que llevar a las demás:

- **Las personas**: quien use la otra consola necesita la clave nueva. Es un secreto: el sistema no la reparte (ni puede).
- **`K_cfg` de cada consola**: la consola que cambia la clave conoce la nueva y la sal de cada consola (va en el resumen, `consolas[].sal_cliente`): manda, además, `k_cfg_consolas: { "<identidad>": "<K_cfg>" }`. Si no lo manda (una consola anterior), el equipo pone la nueva `K_cfg` en las consolas que tenían **la misma** que la que manda (misma sal, mismo resultado) y deja las otras como estaban, con un aviso en el registro: esas siguen cifrando la configuración con la clave anterior hasta que se mande otra vez.
- **Etiquetas**: el equipo sube con su configuración la etiqueta calculada con la `K_cfg` de cada consola (3): la otra consola, con la clave nueva, comprueba la etiqueta como siempre.
- Se avisa en todas (`cambio_clave`: en la que lo manda, con el resultado firmado; en las demás, cuando el equipo les sube la etiqueta nueva con `cambio_config` de `cambiar_clave_admin` desde otra consola).
- En la consola: cliente → Personas y ajustes → «Cambiar la clave de administración». Comprueba la actual con las etiquetas, manda la orden a cada equipo (con `k_cfg_consolas`) y dice en qué va cada uno; los que no están conectados la aplican al conectar y, mientras, en ellos sigue valiendo la anterior («Cambio a medias»).

### 4.3 Revocar

| Situación | Qué hacer |
|---|---|
| Ya no quieres la consola en línea | Desde la local, en el equipo (o en el cliente, para todos): «Quitar» → `quitar_consola`. O desde la en línea: «Dejar de gestionar este equipo». |
| La consola en línea está comprometida | Quítala desde la local (`quitar_consola`, no hace falta su colaboración). Si se escribió la clave en ella, **cambia la clave de administración** desde la local. |
| La consola ya no existe o no responde | `quitar_consola` desde otra, o `resguardo-agente consolas quitar` en el equipo. Mientras tanto, nada se rompe: su canal reintenta sin molestar a las demás. |
| La ficha del código de conexión se filtró | Caduca sola (7 días) y tiene un tope de equipos. Sin la clave de administración nadie puede usarla para conectar un equipo: la orden la exige. Lo peor es que alguien dé de alta, con ella, un equipo falso en esa consola (lo mismo que con «Recibir un cliente» hoy). |
| Una consola de más que no reconoces en la lista | Alguien con la clave de administración la añadió: quítala y cambia la clave. |

### 4.4 Alternativas descartadas

- **Una consola «maestra» que reenvía a la otra.** Más simple para el agente, pero la segunda dependería de la primera (si la local cae, la en línea tampoco funciona) y una tendría que confiar en la otra. Va contra «cada una funciona sola».
- **Federación entre servidores** (que se sincronicen entre ellos). Exige que los servidores se autentiquen entre sí y repartan secretos, y deja a un servidor con poder sobre el otro. El equipo ya es el punto de encuentro natural: tiene la verdad (su configuración) y comprueba la clave.
- **Un solo `seq` para todas.** Obliga a coordinar servidores; con `seq` por vínculo y `nonce` común se consigue lo mismo sin coordinación.
- **Bloqueo por intentos común.** Una consola maliciosa podría bloquear las órdenes de la buena. Por vínculo, cada consola solo se bloquea a sí misma.

### 4.5 Límites conocidos

- La lista de consolas (direcciones y nombres) la ven **todas**: un servidor sabe dónde más se gestiona el cliente. Es necesario para enseñarla y avisar; no da ningún poder.
- La consola que no manda un cambio lo ve **después** (en segundos si está conectada; si estaba apagada, al volver).
- Dos cambios de configuración a la vez desde dos consolas: gana el último en llegar al equipo (1.3).
- Un servidor de respaldo de una consola que ya es otra de las consolas del equipo no se usa (el equipo ya está allí).

---

## 5. Órdenes en espera a la vista de todas las consolas (v1.49)

Tareas 1 y 9c de [plan-mejoras.md](plan-mejoras.md). Contrato «v1.49, pendiente de numerar al unir» en [api-servidor.md](api-servidor.md), «Cambios».

**Problema.** Hasta ahora la consola que mandaba una orden con espera (las destructivas, con `not_before`) la guardaba ella y solo la entregaba al equipo al llegar su hora. El equipo no sabía que existía: **las demás consolas no la veían ni podían cancelarla**, y tampoco veían las órdenes que habían mandado las otras. Alguien con la clave de administración en una consola podía mandar algo destructivo sin que quien vigila desde la otra se enterase hasta que se aplicaba.

### 5.1 Resumen

1. **El equipo recibe al momento las órdenes con espera** (si su agente lo admite) y las guarda «en espera». Las aplica cuando llega su `not_before`, contado con **su reloj y el de la consola que la mandó** (5.4).
2. **El resumen** del equipo lleva `en_espera`, la lista de esas órdenes, y **todas las consolas** la enseñan en «Órdenes esperando su turno», diciendo desde qué consola vino.
3. **Cualquier consola puede cancelarlas** con la orden `cancelar_espera { id }`, inofensiva (sin clave: cancelar solo aumenta la protección).
4. **Aviso** en las demás consolas (en la consola, por correo y push, según sus reglas) cuando entra una orden en espera.
5. **Historial común:** el equipo anota en su historial (el que reciben todas) cada orden que recibe, aplica, cancela o caduca: tipo, consola, quién y resultado. «Órdenes» enseña también las de las otras consolas.

### 5.2 Convivencia con agentes y servidores anteriores

- El agente nuevo anuncia `admite: ["ordenes_en_espera", …]` en su resumen.
- El servidor nuevo solo entrega antes de su hora las órdenes **que piden autorización** (no las inofensivas) a los equipos que lo anuncian. A los demás, como siempre: en su `not_before`. **Sigue comprobando la espera al crearla** (la del equipo o la del cliente) para todos.
- Un servidor anterior nunca entrega antes de tiempo: el agente nuevo recibe la orden a su hora y la aplica al momento, como antes.
- Un agente anterior no recibe nada antes de tiempo (no lo anuncia).
- La consola nueva con un servidor o un agente anterior: sin `en_espera` en el resumen, enseña lo de siempre (las de su propio servidor). «Cancelar» de las de otra consola solo aparece si el equipo lo admite.

### 5.3 En el equipo: recibir una orden con espera

Cuando llega una orden cuyo `not_before` está más allá de la holgura de relojes (5 min) del reloj del equipo:

1. Se abre y se comprueba **todo como hoy** (destinatario, versión, `seq` creciente de ese vínculo, `nonce` nuevo, metadatos, `emitida` no futura, caducidad de 7 días como mucho, destructiva con `not_before`, espera mínima del equipo desde `emitida`), salvo que la hora aún no ha llegado. Además, `not_before` tiene que ser anterior a `caduca`.
2. El `seq` y el `nonce` se anotan **al recibirla** (como hoy): la protección contra repeticiones es la misma, y el servidor no la vuelve a entregar.
3. Solo se guardan las órdenes que piden autorización (clave de administración o contraseña del repositorio), y **la autorización se comprueba ya al recibirla**, con los intentos fallidos y bloqueos de siempre. Una inofensiva con espera, o una con la clave mal, se rechaza al momento: un servidor malicioso no puede llenar la lista (ni avisar a las demás consolas) con órdenes que nunca se aplicarían.
4. Se guarda **el sobre sellado tal cual** (cifrado para la llave del equipo) dentro del vínculo (`servidor.bin`, protegido con DPAPI en Windows y solo de root en Linux), con lo justo para enseñarla: id de la orden en su servidor, tipo, una descripción sin rutas ni secretos, la consola (vínculo e identidad), quién la mandó (si la consola lo dice), `emitida`, `not_before` y `caduca`. La contraseña o la prueba de la clave no se guardan nunca en claro: al aplicarla se vuelve a abrir el sobre. Sobrevive a reinicios del agente y del equipo.
5. Como mucho **20 en espera por consola**: la siguiente se rechaza («demasiadas órdenes en espera»).
6. No contesta nada al servidor (la orden sigue «entregada» allí, que es lo que el servidor ya sabía cancelar), anota la entrada `orden` en el historial (`resultado: "en_espera"`), avisa a las **demás** consolas (5.6) y sube el resumen nuevo a todas enseguida.

### 5.4 Aplicarla a su hora (el reloj, tarea 9c)

La espera ya no la cuenta solo el servidor. Una orden en espera se aplica cuando se cumplen **las dos cosas**:

- el **reloj del equipo** llegó a `not_before` (con la misma holgura de 5 min que ya aceptaba el agente), y
- el **reloj de la consola que la mandó** también: el servidor nuevo dice su hora (`ahora`) en el saludo del canal, en cada latido (`ping`) y en `tomar`; el equipo le suma lo que pasa en su reloj monotónico (que no se adelanta cambiando la hora del sistema).

Así, **adelantar el reloj del equipo** (alguien con acceso a él, o un servidor de hora falso en la red) **no acorta la espera**, y **un servidor con la hora adelantada tampoco**: hacen falta los dos. Es lo mismo que pasaba hasta ahora (el servidor entregaba a su hora y el agente comprobaba la suya), ahora con el equipo como guardián.

Además, solo se aplica **dentro del canal con la consola que la mandó**, justo después de hablar con ella (con el canal abierto o tras una consulta de sondeo correcta, que es por donde llegan sus cancelaciones). Si esa consola no está (apagada, sin red), la orden **espera** y, si caduca antes de volver a hablar con ella, se descarta: igual que antes, cuando una orden que el servidor no entregaba nunca se aplicaba. Una orden destructiva nunca se aplica sin que su consola haya podido cancelarla.

Al aplicarla se vuelve a abrir el sobre y se comprueba otra vez: que no ha caducado, que esa consola sigue siendo la misma (misma identidad: si se quitó o cambió de servidor, se cancela), la espera mínima y la autorización **de ese momento** (si alguien cambió la clave de administración mientras tanto, se rechaza: «cancélala y cambia la clave» sigue funcionando). Después se ejecuta con el código de siempre y el resultado firmado va a esa consola.

### 5.5 `resumen.en_espera`

```json
"en_espera": [{ "id": "<id de la orden en su servidor>", "tipo": "quitar_repositorio",
  "descripcion": "Quitar el repositorio «Documentos»" | null,
  "consola": { "nombre": "Oficina" | null, "identidad": "<Ed25519 b64>", "esta": false },
  "por": "Ana" | null, "emitida": "<RFC 3339>", "aplica": "<RFC 3339>", "caduca": "<RFC 3339>" }]
```

- `consola.nombre` es el que esa consola tiene en el equipo, **nunca su dirección** (como el progreso de «Mover a otro sitio»); si no tiene, `null`, y la consola la busca por `identidad` en `resumen.consolas`. `esta: true` si la mandó la consola que recibe el resumen.
- `por`: el nombre de quien la mandó, si la consola lo pone en la orden sellada (campo opcional `por` del sobre, v1.49). Es lo que dice esa consola: informativo.
- Las caducadas no salen.

### 5.6 Avisos

Al guardar una orden en espera, el equipo avisa a **las demás** consolas con `POST /api/agente/aviso` de un tipo nuevo, `orden_en_espera` («Orden en espera desde la consola «Oficina»: Quitar el repositorio «Documentos» (pedida por Ana). Se aplicará el 07/10 10:30 si nadie la cancela.»). Es crítico, como «Orden destructiva pendiente»: sale por correo y push con las reglas de cada consola. Un servidor anterior no conoce el tipo (422) y el equipo lo manda entonces como `cambio_inusual`. La consola que la mandó ya tiene su «Orden destructiva pendiente». Una consola apagada en ese momento no recibe el aviso, pero ve la orden en «Órdenes esperando su turno» (resumen) y en el historial al volver.

### 5.7 `cancelar_espera { id }`

Orden **inofensiva** (basta la sesión: técnicos, administradores y propietarios), sin espera. El equipo:

1. busca `id` entre sus órdenes en espera (de cualquier consola); si no está, `fallida` («ya no está esperando: se aplicó, se canceló o caducó»);
2. la quita, la anota en el historial (`resultado: "cancelada"`, desde qué consola y quién) y contesta `hecha`;
3. le dice a la consola que la mandó que no se aplicará: un resultado firmado `rechazada` con el mensaje «Cancelada desde otra consola («En línea»)…» y `detalle: {"cancelada": true, "consola": "<nombre>"}`. El servidor la guarda tal cual (no la cambia a `cancelada`: la firma del equipo es sobre `rechazada` y la consola la comprueba). Si esa consola no responde, el resultado se guarda y se manda cuando vuelva (como el de las órdenes largas).

La consola que la mandó la sigue cancelando como siempre (`POST …/ordenes/{o}/cancelar`): su servidor la marca `cancelada` y avisa al equipo (`{ "t": "cancelada" }` en el canal o `canceladas` en `tomar`, que el agente ya recibía y ahora usa). Una cancelación de un servidor solo vale para las órdenes de **ese** vínculo.

**Si una cancelación no llega a tiempo** (el servidor la marca `cancelada` justo cuando el equipo la aplica), el resultado firmado del equipo **manda**: el servidor nuevo la pasa a su estado real y crea un aviso `cambio_inusual` («se aplicó una orden que se había cancelado»). Nunca se queda enseñando «cancelada» algo que se aplicó.

### 5.8 Historial común (`orden`)

Tipo nuevo de entrada del historial del equipo (§6 de api-servidor.md), que reciben todas sus consolas:

```json
{ "id", "hora", "tipo": "orden", "orden": "<tipo>", "orden_id": "<id en su servidor>", "descripcion"?,
  "consola": "<nombre>"?, "identidad": "<de la consola que la mandó>", "por"?,
  "resultado": "en_espera" | "hecha" | "en_marcha" | "fallida" | "rechazada" | "cancelada" | "caducada",
  "mensaje"?, "aplica"?, "cancelada_desde"?: "<nombre de la consola que la canceló>" }
```

Se anotan todas las órdenes que el equipo recibe salvo las que abren una sesión interactiva (explorar, elegir carpetas) y `cancelar_espera` (ya cuenta en la orden cancelada). Sin rutas ni secretos (el mensaje pasa por el mismo filtro que los resultados). El servidor solo las da si se piden (`tipo=orden`): una consola anterior no las conoce. En «Órdenes», la consola enseña «Desde otras consolas» con las de los equipos que tienen más de una consola, sin las suyas (por `identidad`).

### 5.9 Seguridad

| Riesgo | Qué pasa |
|---|---|
| **Una consola maliciosa (o comprometida) cancela lo legítimo de otra** | Puede: cancelar es inofensivo a propósito (que cualquiera pueda frenar algo destructivo vale más). Es **reversible** (se vuelve a mandar), queda en el historial de todas («Cancelada desde la consola X») y la que la mandó lo ve en su orden. Si se repite, se quita esa consola (`quitar_consola`, con la clave de administración) y se cambia la clave si se escribió allí. Es una denegación de servicio de lo destructivo, nunca una pérdida de datos. |
| **Un servidor malicioso llena la lista o hace saltar avisos** | Solo se guardan órdenes cuya autorización vale al recibirlas (las demás se rechazan y cuentan como intentos fallidos, con sus bloqueos por vínculo), como mucho 20 por consola. Las inofensivas con espera se rechazan. |
| **Repetir una orden en espera** | `seq` y `nonce` se anotan al recibirla, como siempre: ni la misma consola ni otra pueden repetirla. La orden sale de la lista antes de ejecutarse (bajo el cerrojo del vínculo): se aplica una sola vez. |
| **Reinicios** | Las órdenes en espera están en `servidor.bin` (cifrado como el resto del vínculo), con el sobre sellado. Al volver, siguen esperando. Una que caducó mientras tanto se descarta y se anota. |
| **Cambiar el reloj del equipo** | No acorta la espera: hace falta también la hora de la consola que la mandó (5.4). Retrasarlo solo retrasa la orden (como antes). |
| **Un servidor con la hora adelantada** | No basta: hace falta también el reloj del equipo. Un servidor que diga una hora absurda solo retrasa (o impide) sus propias órdenes. |
| **Cambiar la clave de administración mientras espera** | La autorización se vuelve a comprobar al aplicarla: con la clave nueva, la orden pendiente se rechaza. |
| **Quitar la consola que la mandó** | Sus órdenes en espera se cancelan (no se aplicarán) y se anota. |
| **Privacidad entre consolas** | El resumen y el historial dicen el nombre y la identidad de la consola (que ya estaban en `resumen.consolas`), nunca su dirección. La descripción no lleva rutas ni secretos. |

### 5.10 Límites conocidos

- Si la consola que la mandó deja de responder para siempre, su orden no se aplica (caduca): es lo seguro, igual que antes.
- Las consolas apagadas cuando entra la orden no reciben el aviso por correo o push: la ven al volver (resumen e historial).
- Una cancelación desde otra consola llega al equipo por el canal de esa otra consola: si el equipo no habla con ella, no se puede cancelar desde allí (desde la que la mandó, sí).
- `por` lo pone la consola que manda la orden: una consola maliciosa puede poner cualquier nombre.
- La ventana del propio equipo todavía no enseña ni cancela las órdenes en espera (lo hace cualquier consola).
