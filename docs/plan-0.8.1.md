# Plan de la versión 0.8.1

Propuesta del 2026-10-19. **Visto bueno del responsable el 2026-10-19:** se hace todo, en **una sola versión llamada 0.8.1** («vamos muy rápido con las versiones»), y se añaden los detalles menores de la revisión (bloque G). Recoge el bloque 15 de [plan-mejoras.md](plan-mejoras.md) (15a–15rr, apuntado durante la primera semana de uso real de la 0.8.0) y lo que quedó abierto de los bloques 11–14. El detalle de cada punto está en `plan-mejoras.md`; aquí, el orden, el alcance y las decisiones. Marca aquí el estado de cada bloque (`[ ]` pendiente, `[~]` en curso, `[x]` hecho, con la rama). Lo que cambie mensajes entre agente y consola va en `docs/api-servidor.md` → «Cambios», compatible hacia atrás.

**Objetivo: pulir.** Menos pantallas y más claras, ver de un vistazo dónde está cada cosa, que nada falle en silencio y que lo de cada día (renombrar, excluir, restaurar, desplegar) sea fácil. Pocas funciones nuevas grandes.

**Principio (2026-10-16):** Resguardo es para cualquiera, no para un cliente concreto. Cada punto se diseña para cualquier entorno (equipo físico, Hyper-V, VMware, Proxmox, NAS, nube); lo propio de un proveedor, como extra opcional.

## Bloque R — Arreglos que estorban el despliegue (primero)

Solo fallos, para desplegar muchos equipos sin rodeos. Con su prueba cada uno. Van primero dentro de la versión.

- [ ] **R1. Instalación silenciosa que no vincula (15ff, causa real de 14ee):** el instalador corta `/SERVIDOR=https://…` en la primera `/`. Leerlo bien, la línea de la consola con comillas, y **prueba real en el CI**: compilar el instalador, instalarlo en silencio en el Windows del CI y comprobar que vincula.
- [ ] **R2. Quitar la derivada «de cada día» no la quita de la consola (15p):** limpiar `externa` al quitarla y, al arrancar, las que quedaron así.
- [ ] **R3. El instalador de «Instalar muchos equipos» se queda viejo (15x):** cambiarlo solo al bajar una versión más nueva; la página dice qué versión reparte; «Cambiarlo» también si se puso a mano.
- [ ] **R4. «Ordenar nombres de carpeta» rompe las derivadas hacia el almacén (15i).**
- [ ] **R5. Copia cortada al apagar sin rastro ni reintento (15jj):** anotarla «en curso» al empezar, «interrumpida» al arrancar, y reintentarla.

### Bloque A — Que nada falle en silencio

- [ ] **A1.** Un repositorio que sale de un espejo avisa, y lo que queda en el destino sin actualizar se ve (15m).
- [ ] **A2.** La página del repositorio conoce sus derivadas, y cada derivada tiene su página con «Retención en el almacén…» (15g).
- [ ] **A3.** Volcados de SQL: el permiso que falta, dicho claro y como aviso, no como fallo (15f); «todas las bases» y aviso de bases nuevas (15u).
- [ ] **A4.** Los errores de las copias en palabras, con qué hacer, y el detalle técnico plegado (15oo); el historial corta las líneas largas (15j).
- [ ] **A5.** Parar y pausar una copia en marcha, con la clave de administración; restic muere con el servicio (15hh).
- [ ] **A6.** Que el equipo no se suspenda mientras copia (15ll).
- [ ] **A7.** Quitar un repositorio vacío solo con la clave; `kit` y `copias` ven todos los repositorios del equipo (15q). Lo que queda de 14nn.

### Bloque B — Menos pantallas, más orden

- [ ] **B1. Menú izquierdo de ~8 entradas con pestañas** (15bb): Estado · Equipos · Copias · Destinos · Recuperar · Avisos · Actividad · Informes, y Ajustes aparte. Probarlo con alguien que no conozca la consola.
- [ ] **B2. «Organización»** (hoy «Sedes y categorías»): sedes, categorías, **etiquetas** (renombrar, borrar, unir) y **grupos de exclusiones** (15a, 15y).
- [ ] **B3. Un almacén, una sola ficha** (15dd) y su tarjeta por secciones: espejos, retención y nubes, con página propia de cada nube (15s); «Ordenar nombres de carpeta» como aviso suave (15h).
- [ ] **B4. Clientes:** borrar o archivar uno, aviso de nombre repetido y un camino claro para traer equipos de otra consola (15nn).
- [ ] **B5. Lo que describe los datos, igual en todas las consolas:** nombres y observaciones de equipos, copias, repositorios y destinos (15pp).
- [ ] **B6.** Las derivadas con su nombre en todas las listas (15k).

### Bloque C — Ver dónde está cada cosa

- [ ] **C1. «Dónde está»** en cada repositorio y copia, con la ruta real en cada sitio, su tamaño y si está al día (15n).
- [ ] **C2. El mapa:** grupos que se despliegan y agrupación por sede (15rr), derivadas y espejos animados (15cc), vista «Mapa» en Equipos (15b) y capas (15l, empezando por Sedes, Copias y Estado).
- [ ] **C3. Tipo de equipo** (físico, VM, contenedor), detectado y corregible (15c).
- [ ] **C4. Infraestructura, fase 1, para cualquier entorno** (15d): lo que el agente detecta solo, el resto anotado con ayuda, una línea por destino. Sin conectar ningún hipervisor (15e aparcada).

### Bloque D — Las copias de cada día, más fáciles

- [ ] **D1. Renombrar** una copia, un espejo o una derivada con un lápiz y la clave; el nombre arriba del asistente (15t).
- [ ] **D2. Grupos de exclusiones** de un toque, gestionables por cliente, «no copiar más de N MB» y vista previa de lo que se deja fuera (15y).
- [ ] **D3. Retención «Personalizada»** desde cero o desde la elegida (15z).
- [ ] **D4. Etiquetas de las versiones** por copia y filtro en el historial (15v).
- [ ] **D5. Espejo desde una copia** con su repositorio marcado, o añadirlo a uno que ya existe (15r).
- [ ] **D6. Traer repositorios viejos al almacén** sin terminal: repositorios sueltos del disco del almacén, «Dar a un equipo» y adoptar (15gg).
- [ ] **D7. Equipos que entran y salen:** «además, al volver a conectarse», avisos de «lleva N días fuera» y no empezar sin batería (15ii).
- [ ] **D8. Apagar el equipo al terminar** una copia, con la clave y aviso al usuario (15kk).

### Bloque E — Restaurar

- [ ] **E1.** El árbol de archivos marca lo de dentro y deja quitar lo que no (15w).
- [ ] **E2.** El motivo **antes** de restaurar, obligatorio por defecto, común a todas las consolas (15ee).
- [ ] **E3.** Las pruebas de restauración automáticas y a mano en «Restauraciones», con su ficha (15aa).
- [ ] **E4.** Lo que quedó de la 0.8.0: restaurar desde los espejos del propio equipo (14m) y la prueba desde un disco del almacén (14j).

### Bloque F — El agente en Windows

- [ ] **F1.** `resguardo-agente` en el PATH, la bandeja tras instalar, y que diga por qué no abre la ventana (elevada o sin WebView2) (15o).
- [ ] **F2.** Instalar sin rodeos con ESET, Intune o GPO: guía con las órdenes en forma `cmd` (15x) y, con R1, una sola tarea.

## Bloque G — Detalles menores (de la revisión del 2026-10-19)

Cuatro revisiones del código (textos, estados de las pantallas, accesibilidad y móvil, y lo que se ve fuera de la consola); unos 100 hallazgos, ordenados en [revision-detalles-0.8.1.md](revision-detalles-0.8.1.md).

- [ ] **G1. Fallos de verdad (con el bloque R):** variables de CSS que no existen (los fallos de los espejos no salen en rojo), Escape que cierra diálogos que no deben cerrarse (se pierde lo escrito o un código de un solo uso), cargas que no terminan nunca si falla una petición, un informe para el cliente con datos en blanco sin avisar, botones que mandan dos veces con un doble clic, errores inesperados en inglés, un enlace roto, la prueba de restauración contada como verificación en los avisos de Windows. Con una comprobación estática de variables de CSS en `test:vectores`.
- [ ] **G2. Confirmar y no perder trabajo:** avisar al salir con cambios sin enviar, confirmar lo que borra, `BotonFalta` en los botones apagados que no dicen qué falta, estados vacíos con su botón.
- [ ] **G3. Un solo vocabulario en la consola:** tipos de destino, credenciales, botones («Seguir», «Listo»…), «copia derivada», «quitar», unidades y fechas, erratas y frases largas.
- [ ] **G4. Fuera de la consola:** avisos y correos con nombres legibles y qué hacer; «número de comprobación», «vincular», «almacén» y «copia derivada» también en el agente, la bandeja, la ventana y el instalador; textos del instalador que ya no son ciertos; ayuda de `resguardo-agente`; errores en inglés.
- [ ] **G5. Accesibilidad:** estados con icono y texto, sin `title` nativo, tablas y cambios legibles con lector de pantalla, teclado en las pestañas.
- [ ] **G6. Pantallas estrechas:** tablas y fichas que desbordan a 375 px, direcciones largas, campos que hacen ampliar la página en el iPhone.
- [ ] **G7. Sistema de diseño:** tamaños con sus tokens y piezas repetidas a `ui/` (kit imprimible, buscador, barra de fuerza de la clave).

## Bloque H — Lo que se decidió meter también (2026-10-19)

- [ ] **H1.** Que la persona del equipo recupere sus propios archivos desde el menú contextual, con sus salvaguardas (15mm).
- [ ] **H2.** Paquete de evidencia para auditoría y campos opcionales de responsable, criticidad y datos personales (15qq).
- [ ] **H3.** Passkeys (WebAuthn) para entrar en la consola, además del TOTP (12c).

## Decisiones (2026-10-19)

1. **Una sola versión, 0.8.1**, con el bloque R primero. Mientras sale, en los despliegues por ESET se sigue con dos tareas (instalar y luego vincular).
2. **Todo dentro:** también C4 (infraestructura fase 1), D7 (portátiles), D8 (apagar al terminar), 15mm, 15qq y las passkeys (bloque H).
3. **14v** (derivadas filtradas hechas por el almacén) sigue en «no». **12a, 12b, 12d y 12e** (resto de la seguridad de la consola) quedan para una versión propia.

## Fuera de este plan

- 11f: probar la actualización real en máquinas virtuales. 11g: que Resguardo Server se actualice solo.
- 15e: conectar Proxmox u otro hipervisor (aparcada, opcional).
- Mac, instaladores con firma comercial y volcados de otras bases de datos (decisión anterior).
