# Plan de la versión 0.8.1

Propuesta del 2026-10-19. **Visto bueno del responsable el 2026-10-19:** se hace todo, en **una sola versión llamada 0.8.1** («vamos muy rápido con las versiones»), y se añaden los detalles menores de la revisión (bloque G). Recoge el bloque 15 de [plan-mejoras.md](plan-mejoras.md) (15a–15rr, apuntado durante la primera semana de uso real de la 0.8.0) y lo que quedó abierto de los bloques 11–14. El detalle de cada punto está en `plan-mejoras.md`; aquí, el orden, el alcance y las decisiones. Marca aquí el estado de cada bloque (`[ ]` pendiente, `[~]` en curso, `[x]` hecho, con la rama). Lo que cambie mensajes entre agente y consola va en `docs/api-servidor.md` → «Cambios», compatible hacia atrás.

**Objetivo: pulir.** Menos pantallas y más claras, ver de un vistazo dónde está cada cosa, que nada falle en silencio y que lo de cada día (renombrar, excluir, restaurar, desplegar) sea fácil. Pocas funciones nuevas grandes.

**Principio (2026-10-16):** Resguardo es para cualquiera, no para un cliente concreto. Cada punto se diseña para cualquier entorno (equipo físico, Hyper-V, VMware, Proxmox, NAS, nube); lo propio de un proveedor, como extra opcional.

## Bloque R — Arreglos que estorban el despliegue (primero)

Solo fallos, para desplegar muchos equipos sin rodeos. Con su prueba cada uno. Van primero dentro de la versión.

- [x] **R1. Instalación silenciosa que no vincula (15ff, causa real de 14ee):** el instalador corta `/SERVIDOR=https://…` en la primera `/`. Leerlo bien, la línea de la consola con comillas, y **prueba real en el CI**: compilar el instalador, instalarlo en silencio en el Windows del CI y comprobar que vincula. (Rama `ia/081-r`: `LeerServidor` en `agente.nsi`, con o sin comillas; la línea de PowerShell con `/SERVIDOR="…"`, que `${GetOptions}` de los instaladores 0.8.0 también entiende; el CI de Windows compila el instalador sin firmar, lo ejecuta en silencio contra `https://127.0.0.1:9`, con y sin comillas, y comprueba en `instalador.log` que el motivo ya no es la dirección cortada. Sin servidor de pruebas: comprueba que **intenta** vincular, no que vincula.)
- [x] **R2. Quitar la derivada «de cada día» no la quita de la consola (15p):** limpiar `externa` al quitarla y, al arrancar, las que quedaron así. (Rama `ia/081-r`; también el destino creado solo para ella y las derivadas con id; «la orden viene del futuro» con la diferencia de hora y cómo corregirla.)
- [x] **R3. El instalador de «Instalar muchos equipos» se queda viejo (15x):** cambiarlo solo al bajar una versión más nueva; la página dice qué versión reparte; «Cambiarlo» también si se puso a mano. (Rama `ia/081-r`: uno puesto a mano se reconoce por su SHA-256 entre las publicaciones que tiene el servidor; si no es de ninguna, no se pisa solo pero se ofrece «Cambiarlo por el de la X»; un equipo recién vinculado —sin ninguna copia aún— no espera los días de su anillo.)
- [x] **R4. «Ordenar nombres de carpeta» rompe las derivadas hacia el almacén (15i).** (Rama `ia/081-r`, con prueba contra el rest-server real.)
- [x] **R5. Copia cortada al apagar sin rastro ni reintento (15jj):** anotarla «en curso» al empezar, «interrumpida» al arrancar, y reintentarla. (Rama `ia/081-r`; queda de 15jj lo de la consola cuando un equipo deja de responder a mitad de una copia y el estado del equipo junto a la copia en «Copias».)

### Bloque A — Que nada falle en silencio

- [x] (rama `ia/081-a`; el punto 5 de 15m, el mapa, lo lleva otra línea) **A1.** Un repositorio que sale de un espejo avisa, y lo que queda en el destino sin actualizar se ve (15m).
- [x] (rama `ia/081-a`; sin probar contra un almacén real) **A2.** La página del repositorio conoce sus derivadas, y cada derivada tiene su página con «Retención en el almacén…» (15g).
- [x] (rama `ia/081-mensajes`; sin probar contra un SQL Server real) **A3.** Volcados de SQL: el permiso que falta, dicho claro y como aviso, no como fallo (15f); «todas las bases» y aviso de bases nuevas (15u).
- [x] (rama `ia/081-mensajes`) **A4.** Los errores de las copias en palabras, con qué hacer, y el detalle técnico plegado (15oo); el historial corta las líneas largas (15j).
- [x] (rama `ia/081-a`: parar y pausar —consola, ventana y `resguardo-agente parar`—, el trabajo de Windows, los procesos sueltos y la velocidad según la hora en las copias. Comprobado con restic 0.19: guarda su índice cada ~10 min, así que de una subida cortada se pierden como mucho esos minutos; no hace falta más. **Sin hacer:** parar una restauración en un .zip; sin probar en Linux ni con el servicio instalado) **A5.** Parar y pausar una copia en marcha, con la clave de administración; restic muere con el servicio (15hh).
- [~] (rama `ia/081-a`: hecho en Windows y Linux, la opción por copia y el aviso de la tarjeta de red en Windows. **Falta:** el Wi‑Fi en ahorro máximo y la tarjeta de red en Linux, que el agente no lee sin privilegios raros; sin probar en un portátil de verdad) **A6.** Que el equipo no se suspenda mientras copia (15ll).
- [x] (rama `ia/081-a`; la salida en `cmd` se arregla con una pulsación de Intro al terminar y la clave se pide en una ventana propia: la forma limpia, un `resguardo-agente-cli.exe` de consola, toca el instalador) **A7.** Quitar un repositorio vacío solo con la clave; `kit` y `copias` ven todos los repositorios del equipo (15q). Lo que queda de 14nn.

### Bloque B — Menos pantallas, más orden

- [x] **B1. Menú izquierdo de ~8 entradas con pestañas** (15bb; rama `ia/081-b`: `lib/secciones.ts`, `ui/componentes/Pestanas.svelte`, direcciones nuevas con las de antes redirigiendo; **falta** probarlo con alguien que no conozca la consola): Estado · Equipos · Copias · Destinos · Recuperar · Avisos · Actividad · Informes, y Ajustes aparte. Probarlo con alguien que no conozca la consola.
- [x] **B2. «Organización»** (rama `ia/081-b`; los grupos de exclusiones, solo el dato y su gestión: su uso en el editor de copias es D2) (hoy «Sedes y categorías»): sedes, categorías, **etiquetas** (renombrar, borrar, unir) y **grupos de exclusiones** (15a, 15y).
- [~] **B3. Un almacén, una sola ficha** (rama `ia/081-b`: hecho todo salvo «Pasar al almacén gestionado» los adoptados, 15dd punto 2, que necesita una orden coordinada nueva; «Probar» una nube lanza uno de sus espejos) (15dd) y su tarjeta por secciones: espejos, retención y nubes, con página propia de cada nube (15s); «Ordenar nombres de carpeta» como aviso suave (15h).
- [x] **B4. Clientes:** (rama `ia/081-b`) borrar o archivar uno, aviso de nombre repetido y un camino claro para traer equipos de otra consola (15nn).
- [x] **B5. Lo que describe los datos, igual en todas las consolas:** (rama `ia/081-b`; los comentarios de antes de la 0.8.1 no se reparten) nombres y observaciones de equipos, copias, repositorios y destinos (15pp).
- [x] **B6.** Las derivadas con su nombre en todas las listas (15k). (Rama `ia/081-b`; sin el nombre legible de la carpeta de las derivadas en «Ordenar nombres de carpeta».)

### Bloque C — Ver dónde está cada cosa

- [x] (rama `ia/081-c`; la ruta de una derivada en su destino no se sabe del todo: se dice el destino) **C1. «Dónde está»** en cada repositorio y copia, con la ruta real en cada sitio, su tamaño y si está al día (15n).
- [x] (rama `ia/081-c`; la capa «Infraestructura» es una línea en cada tarjeta, no columnas propias servidor → hipervisor → VM) **C2. El mapa:** grupos que se despliegan y agrupación por sede (15rr), derivadas y espejos animados (15cc), vista «Mapa» en Equipos (15b) y capas (15l, empezando por Sedes, Copias y Estado).
- [x] (rama `ia/081-c`) **C3. Tipo de equipo** (físico, VM, contenedor), detectado y corregible (15c).
- [~] (rama `ia/081-c`; [infraestructura.md](infraestructura.md). **Falta:** que sea común a todas las consolas del cliente —de momento es de cada consola—; ya es la pestaña «Infraestructura» de «Destinos» (al unir con el bloque B); y probar la detección de discos en un Windows con Espacios de almacenamiento o controladora RAID y en un Linux con mdadm/ZFS reales) **C4. Infraestructura, fase 1, para cualquier entorno** (15d): lo que el agente detecta solo, el resto anotado con ayuda, una línea por destino. Sin conectar ningún hipervisor (15e aparcada).

### Bloque D — Las copias de cada día, más fáciles

- [x] (rama `ia/081-d`; orden `nombre_copia`, sin espera) **D1. Renombrar** una copia, un espejo o una derivada con un lápiz y la clave; el nombre arriba del asistente (15t).
- [x] (rama `ia/081-d`; el límite de tamaño con excepciones lo aplica el agente recorriendo antes las carpetas; plantillas de copias con sus grupos; «Aplicarles el cambio» en Organización; sin «Varios a la vez» ni plantilla por etiqueta) **D2. Grupos de exclusiones** de un toque, gestionables por cliente, «no copiar más de N MB» y vista previa de lo que se deja fuera (15y).
- [x] (rama `ia/081-d`; en el editor común de retención: copias, almacén y derivadas; la de los espejos, que solo tiene días, no lo necesita) **D3. Retención «Personalizada»** desde cero o desde la elegida (15z).
- [x] (rama `ia/081-d`) **D4. Etiquetas de las versiones** por copia y filtro en el historial (15v).
- [x] (rama `ia/081-d`; un repositorio aún sin crear deja el espejo pendiente y abre «Añadir un espejo» al guardar) **D5. Espejo desde una copia** con su repositorio marcado, o añadirlo a uno que ya existe (15r).
- [x] (rama `ia/081-d`; el almacén los mueve con un `rename` dentro de su zona, sin seguir enlaces; sin probar contra un almacén real) **D6. Traer repositorios viejos al almacén** sin terminal: repositorios sueltos del disco del almacén, «Dar a un equipo» y adoptar (15gg).
- [~] (rama `ia/081-d`: hecho «al volver a conectarse», margen y aviso «lleva N días», batería y conexión medida, «Sale de la oficina» en la consola; **falta** reanudar una copia cortada por perder la red aprovechando lo subido, que va con 15hh/15jj) **D7. Equipos que entran y salen:** «además, al volver a conectarse», avisos de «lleva N días fuera» y no empezar sin batería (15ii).
- [x] (rama `ia/081-d`; solo probado con pruebas unitarias y el modo de prueba: no se apagó ningún equipo de verdad) **D8. Apagar el equipo al terminar** una copia, con la clave y aviso al usuario (15kk).

### Bloque E — Restaurar

- [ ] **E1.** El árbol de archivos marca lo de dentro y deja quitar lo que no (15w).
- [ ] **E2.** El motivo **antes** de restaurar, obligatorio por defecto, común a todas las consolas (15ee).
- [ ] **E3.** Las pruebas de restauración automáticas y a mano en «Restauraciones», con su ficha (15aa).
- [ ] **E4.** Lo que quedó de la 0.8.0: restaurar desde los espejos del propio equipo (14m) y la prueba desde un disco del almacén (14j).

### Bloque F — El agente en Windows

- [x] **F1.** `resguardo-agente` en el PATH, la bandeja tras instalar, y que diga por qué no abre la ventana (elevada o sin WebView2) (15o). (Rama `ia/081-r`.)
- [x] **F2.** Instalar sin rodeos con ESET, Intune o GPO: guía con las órdenes en forma `cmd` (15x) y, con R1, una sola tarea. (Rama `ia/081-r`: `guia-instalacion.md` §5.6, la guía «Instalar con ESET, Intune o GPO» en la ayuda de la consola y las órdenes de `cmd` en «Para una herramienta de despliegue».)

## Bloque G — Detalles menores (de la revisión del 2026-10-19)

Cuatro revisiones del código (textos, estados de las pantallas, accesibilidad y móvil, y lo que se ve fuera de la consola); unos 100 hallazgos, ordenados en [revision-detalles-0.8.1.md](revision-detalles-0.8.1.md).

- [x] **G1. Fallos de verdad (con el bloque R):** (rama `ia/081-consola-g`: hecha la parte de la consola y `ui/`; la prueba de restauración contada como verificación en los avisos de Windows, en la rama `ia/081-mensajes`) variables de CSS que no existen (los fallos de los espejos no salen en rojo), Escape que cierra diálogos que no deben cerrarse (se pierde lo escrito o un código de un solo uso), cargas que no terminan nunca si falla una petición, un informe para el cliente con datos en blanco sin avisar, botones que mandan dos veces con un doble clic, errores inesperados en inglés, un enlace roto, la prueba de restauración contada como verificación en los avisos de Windows. Con una comprobación estática de variables de CSS en `test:vectores`.
- [x] **G2. Confirmar y no perder trabajo:** (rama `ia/081-consola-g`) avisar al salir con cambios sin enviar, confirmar lo que borra, `BotonFalta` en los botones apagados que no dicen qué falta, estados vacíos con su botón.
- [x] **G3. Un solo vocabulario en la consola:** (rama `ia/081-consola-g`; lo del agente, la ventana y el servidor va con G4) tipos de destino, credenciales, botones («Seguir», «Listo»…), «copia derivada», «quitar», unidades y fechas, erratas y frases largas.
- [x] (rama `ia/081-mensajes`; el instalador y la ventana sin WebView2 o elevada, en la rama `ia/081-r`: avisos y correos, agente, bandeja, ventana, ayuda de `resguardo-agente` y errores de la API; **falta** lo del instalador, que hace otra línea, y los dos textos de la ventana que tocan el aviso de WebView2 y la ventana como administrador, que van con F1) **G4. Fuera de la consola:** avisos y correos con nombres legibles y qué hacer; «número de comprobación», «vincular», «almacén» y «copia derivada» también en el agente, la bandeja, la ventana y el instalador; textos del instalador que ya no son ciertos; ayuda de `resguardo-agente`; errores en inglés.
- [x] **G5. Accesibilidad:** (rama `ia/081-consola-g`) estados con icono y texto, sin `title` nativo, tablas y cambios legibles con lector de pantalla, teclado en las pestañas.
- [x] **G6. Pantallas estrechas:** (rama `ia/081-consola-g`) tablas y fichas que desbordan a 375 px, direcciones largas, campos que hacen ampliar la página en el iPhone.
- [x] **G7. Sistema de diseño:** (rama `ia/081-consola-g`) tamaños con sus tokens y piezas repetidas a `ui/` (kit imprimible, buscador, barra de fuerza de la clave).

## Bloque H — Lo que se decidió meter también (2026-10-19)

- [ ] **H1.** Que la persona del equipo recupere sus propios archivos desde el menú contextual, con sus salvaguardas (15mm).
- [ ] **H2.** Paquete de evidencia para auditoría y campos opcionales de responsable, criticidad y datos personales (15qq).
- [ ] **H3.** Passkeys (WebAuthn) para entrar en la consola, además del TOTP (12c).
- [ ] **H4.** Las pruebas funcionales (registros y política) iguales en todas las consolas del cliente (15ss, pedido del 2026-10-20).

## Decisiones (2026-10-19)

1. **Una sola versión, 0.8.1**, con el bloque R primero. Mientras sale, en los despliegues por ESET se sigue con dos tareas (instalar y luego vincular).
2. **Todo dentro:** también C4 (infraestructura fase 1), D7 (portátiles), D8 (apagar al terminar), 15mm, 15qq y las passkeys (bloque H).
3. **14v** (derivadas filtradas hechas por el almacén) sigue en «no». **12a, 12b, 12d y 12e** (resto de la seguridad de la consola) quedan para una versión propia.

## Fuera de este plan

- 11f: probar la actualización real en máquinas virtuales. 11g: que Resguardo Server se actualice solo.
- 15e: conectar Proxmox u otro hipervisor (aparcada, opcional).
- Mac, instaladores con firma comercial y volcados de otras bases de datos (decisión anterior).
